/**
 * Client du VFS **live** d'Inazuma Eleven: Victory Road.
 *
 * L'index CPK de `cpk/index.ts` est une photo : un NDJSON gzippé matérialisé en SQLite, à
 * régénérer à la main, muet sur le rôle des dossiers et sur ce qu'un fichier sait produire.
 * Ce module interroge à la place le pont `/vfs/*` de `nie-model-serve` (crate Rust
 * `nie_explore::listing`) — exactement la vue qu'affiche l'explorateur desktop Tauri, sur le
 * VFS réellement monté (250 805 entrées, 921 CPK).
 *
 * ⚠ Module **client-safe** : `fetch` seul, ni pilote SQLite ni accès disque. Importable depuis un
 * composant `"use client"` comme depuis une page serveur — contrairement à `cpk/index.ts`.
 *
 * L'index figé reste en place et reste le repli : si le CDN ne répond pas, une page qui sait
 * dégrader (cf. `lsOrNull`) continue de rendre au lieu de casser.
 */

import {
	baseJeu,
	cheminCatalogueTextures,
	cheminListe,
	cheminRecherche,
	cheminStatsVfs,
	cheminFiche,
	urlExport,
	urlImage,
} from "@niers/catalog/jeu";

import type { CpkDir, CpkFile } from "./shared";

/**
 * Un fichier du VFS live.
 *
 * Le pont rend la **taille décompressée**, que l'index figé ne portait pas : c'est ce qui permet
 * à une galerie de trier par poids et d'annoncer le volume d'un dossier sans le télécharger.
 */
export interface VfsFile extends CpkFile {
	/** Taille décompressée, en octets. */
	size: number;
}

/** Rôle catalogué d'un dossier du VFS (table `nie_explore::folder_roles`). */
export interface VfsRole {
	/** Ce à quoi le dossier sert, en clair. */
	role: string;
	/** État de l'exploitation de ce dossier par le moteur Rust. */
	status: string;
}

/** Vue « dossier » paginée du VFS live. */
export interface VfsListing {
	/** Répertoire listé, normalisé (sans slash final). */
	dir: string;
	/** Sous-dossiers directs, triés par nom. Jamais paginés. */
	dirs: CpkDir[];
	/** Fichiers directs, tranche `[fileOffset, fileOffset + limit)`. */
	files: VfsFile[];
	/** Nombre total de fichiers directs, AVANT pagination. */
	fileTotal: number;
	/** Décalage appliqué. */
	fileOffset: number;
	/** Rôle catalogué du dossier, `null` s'il n'en a pas — jamais un rôle deviné. */
	role: VfsRole | null;
}

/** Résultat d'une recherche dans le VFS live. */
export interface VfsSearch {
	/** La requête telle qu'elle a été comprise. */
	query: string;
	/** Taille de CETTE page. */
	count: number;
	/** Nombre total de correspondances dans tout le VFS. */
	total: number;
	/** Décalage appliqué. */
	offset: number;
	/** Les correspondances de la page, triées par chemin. */
	files: VfsFile[];
}

/** Compteurs globaux du VFS monté. */
export interface VfsStats {
	/** Nombre total de fichiers indexés. */
	total: number;
	/** Nombre d'archives CPK montées. */
	cpkCount: number;
	/** Fichiers issus des paquets additionnels. */
	extraCount: number;
	/** Fichiers présents en clair sur le disque, hors CPK. */
	looseCount: number;
}

/** Un fichier du VFS, enrichi de ce que le moteur sait en faire. */
export interface VfsFileMeta extends VfsFile {
	/** Formats d'export que les crates savent produire pour ce fichier. */
	formats: {
		/** Identifiant du format (`raw`, `wav`, `glb`…). */
		id: string;
		/** Extension produite. */
		ext: string;
		/** Libellé humain. */
		label: string;
		/** Le format rend les octets d'origine, sans conversion. */
		brut: boolean;
		/** La conversion ne perd aucune information. */
		sansPerte: boolean;
	}[];
	/** Lignes de description issues de l'inspection réelle des octets, `null` si non reconnu. */
	describe: string[] | null;
}

/** Erreur de transport du pont VFS, avec le code HTTP quand il y en a un. */
export class VfsError extends Error {
	constructor(
		message: string,
		/** Code HTTP de la réponse, ou `0` si la requête n'a pas abouti. */
		readonly status: number,
	) {
		super(message);
		this.name = "VfsError";
	}
}

async function getJson<T>(chemin: string): Promise<T> {
	let res: Response;
	try {
		res = await fetch(baseJeu() + chemin);
	} catch {
		// Statut 0 = la requête n'a pas abouti (DNS, TLS, réseau) : distinct d'un 404 du pont.
		throw new VfsError(`pont VFS injoignable (${chemin})`, 0);
	}
	if (!res.ok) throw new VfsError(`pont VFS ${res.status} sur ${chemin}`, res.status);
	return (await res.json()) as T;
}

/**
 * Liste le contenu direct d'un répertoire du VFS live.
 *
 * `limit` à 0 ne renvoie que les sous-dossiers et le total — ce que veut un arbre qui n'affiche
 * que la structure, sans payer le transfert des fichiers.
 */
export function vfsLs(dir: string, limit = 1000, offset = 0): Promise<VfsListing> {
	return getJson<VfsListing>(cheminListe(dir, limit, offset));
}

/**
 * Cherche par sous-chaîne dans les chemins internes, filtrable par extension.
 *
 * `total` est le dénominateur réel : une galerie peut donc dire si elle est complète, ce que
 * l'ancien `count` tronqué à la limite ne permettait pas.
 */
export function vfsFind(
	query: string,
	options: { ext?: string; limit?: number; offset?: number } = {},
): Promise<VfsSearch> {
	return getJson<VfsSearch>(
		cheminRecherche(query, {
			limite: options.limit ?? 200,
			decalage: options.offset ?? 0,
			ext: options.ext,
		}),
	);
}

/** Métadonnées d'un fichier précis, avec ses formats d'export et sa description. */
export function vfsStat(path: string): Promise<VfsFileMeta> {
	return getJson<VfsFileMeta>(cheminFiche(path));
}

/** Compteurs globaux du VFS monté. */
export function vfsStats(): Promise<VfsStats> {
	return getJson<VfsStats>(cheminStatsVfs());
}

/**
 * Variante tolérante de [`vfsLs`] : `null` au lieu d'une exception.
 *
 * Une galerie qui rend côté serveur ne doit pas répondre 500 parce que le CDN a hoqueté ; elle
 * doit rendre son cadre et dire que la source est indisponible.
 */
export async function lsOrNull(dir: string, limit = 1000, offset = 0): Promise<VfsListing | null> {
	try {
		return await vfsLs(dir, limit, offset);
	} catch {
		return null;
	}
}

/**
 * URL de téléchargement d'un fichier du jeu **converti au format voulu**.
 *
 * Même table de formats et même règle de nommage que l'app desktop
 * (`nie_explore::export`) : une texture sort en png/webp/gif/jpg/bmp/tga/tiff/qoi/json, un
 * audio en wav, un film en mp4, un modèle en glb. Le serveur pose le `Content-Disposition`,
 * donc le fichier arrive avec son vrai nom au lieu de celui de l'URL.
 *
 * @param path chemin VFS complet, préfixe `data/` compris.
 * @param format identifiant de format (`raw` rend les octets d'origine).
 * @param options `awbId` exporte un cue précis d'une banque audio.
 */
export function exportUrl(
	path: string,
	format: string,
	options: { awbId?: number | null } = {},
): string {
	return urlExport(path, format, options.awbId ?? undefined);
}

/**
 * Emplacement du fichier dans l'explorateur CPK du site.
 *
 * Une page média montre un fichier décodé ; l'explorateur montre où il VIT — son dossier, son
 * archive CPK, sa taille, ses formats. Le lien manquait dans les deux sens.
 */
export function cpkExplorerHref(path: string): string {
	return `/cpk/${path.replace(/^data\//, "")}`;
}

/** Une texture nommée à l'intérieur d'un conteneur G4TX. */
export interface TexEntry {
	/** Identifiant dans la table d'ids du conteneur. */
	id: number;
	/** Nom résolu, ex. `performance_type_01`. */
	name: string;
	/** Largeur en pixels. */
	width: number;
	/** Hauteur en pixels. */
	height: number;
	/** Le payload est un DDS. */
	dds: boolean;
	/** Taille du payload, en octets. */
	size: number;
	/** Chemin CDN de CETTE texture (forme nommée de `/tex`), à préfixer de l'origine. */
	path: string;
	/** Nombre de régions d'atlas rattachées. */
	regions: number;
}

/** Catalogue d'un conteneur G4TX. */
export interface TexContainer {
	/** Chemin VFS du conteneur. */
	path: string;
	/** Nombre de textures. */
	count: number;
	/** Les textures, dans l'ordre du fichier. */
	textures: TexEntry[];
}

/**
 * Catalogue des textures d'un conteneur G4TX.
 *
 * Un `.g4tx` n'est pas une image mais un conteneur : les icônes d'objets vivent à des dizaines
 * par fichier (mesuré : 82,8 en moyenne sur `200_icon/02_icon_item`, jusqu'à 203). Sans cet
 * index, une galerie ne peut afficher qu'une image par fichier et laisse tout le reste invisible.
 *
 * @param rel chemin SANS le préfixe `data/` ni l'extension, ex. `dx11/menu/…/icon_item01`.
 */
export async function texContainer(rel: string): Promise<TexContainer | null> {
	try {
		const res = await fetch(baseJeu() + cheminCatalogueTextures(rel));
		if (!res.ok) return null;
		return (await res.json()) as TexContainer;
	} catch {
		return null;
	}
}

/** URL absolue d'une texture du catalogue (le champ `path` est relatif au CDN). */
export function texUrl(path: string, width?: number): string {
	// `width` absent OU nul : l'URL reste la pleine résolution, sans query. Un `?w=0` demanderait
	// une image de largeur zéro à `cdn-variants` — d'où le test de véracité, et non de présence.
	return urlImage(path, width ? { largeur: width } : {});
}

/** Variante tolérante de [`vfsFind`]. */
export async function findOrNull(
	query: string,
	options: { ext?: string; limit?: number; offset?: number } = {},
): Promise<VfsSearch | null> {
	try {
		return await vfsFind(query, options);
	} catch {
		return null;
	}
}
