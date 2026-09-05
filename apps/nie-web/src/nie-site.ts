/**
 * Client des routes REELLES de `crates/tools/nie-site` : chaque champ ci-dessous
 * est copie des structures Rust (`routes/api_v1.rs`, `routes/mod.rs`,
 * `index_vfs.rs`, `etat.rs`), aucune n'est devinee. Un chemin VFS cite de
 * memoire est presque toujours faux — c'est le serveur qui les enumere.
 *
 * Deux espaces, un seul principe (amendement A3) : le chemin VFS voyage EN
 * SEGMENT, verbatim, extension du jeu conservee. Jamais en query. Les vues
 * nommees ne designent pas des fichiers : ce sont des filtres enregistres.
 */

/** Une ressource du jeu, par son chemin VFS exact. */
export const urlFichier = (cheminVfs: string) => `/f/${cheminVfs}`;

/** Le contenu d'un prefixe du VFS. */
export const urlDossier = (prefixe = "") => (prefixe ? `/b/${prefixe}` : "/b");

/** Les quatre filtres enregistres servis par `/api/v1/<vue>`. */
export type VueCatalogue = "textures" | "modeles" | "sons" | "videos";

/** Une page, telle que `Page<T>` la serialise. */
export interface Page<T> {
	elements: T[];
	page: number;
	per_page: number;
	total: number;
	pages: number;
}

/** Une entree du VFS (`index_vfs::Fichier`). */
export interface Fichier {
	/** Chemin VFS verbatim — c'est aussi l'URL, sous `/f/`. */
	chemin: string;
	/** Nom de la feuille, extension du jeu conservee. */
	nom: string;
	taille: number;
}

/** Ce que le serveur sait faire a l'instant de la mesure (`etat::Capacites`). */
export interface Capacites {
	/** `en_cours`, `pret` ou `absent` — l'index du VFS se monte en tache de fond. */
	vfs: "en_cours" | "pret" | "absent";
	vfs_entrees: number;
	vfs_dump: boolean;
	vfs_contenu: boolean;
	gisement: boolean;
	bundle: boolean;
}

/** Corps de `/api/v1/health`. */
export interface SanteApi {
	service: string;
	api: string;
	version: string;
	capacites: Capacites;
	/** Un resume par filtre ; `total` reste `null` tant que le VFS n'est pas pret. */
	vues: { nom: string; extensions: string[]; total: number | null }[];
}

async function lire<T>(url: string, signal?: AbortSignal): Promise<T> {
	const r = await fetch(url, { signal, headers: { accept: "application/json" } });
	if (!r.ok) throw new Error(`${url} a repondu ${r.status}`);
	return (await r.json()) as T;
}

/** Une page d'un catalogue. `per_page` est borne a 200 par le serveur. */
export function catalogue(
	vue: VueCatalogue,
	{ page = 1, parPage = 60, signal }: { page?: number; parPage?: number; signal?: AbortSignal } = {},
): Promise<Page<Fichier>> {
	return lire(`/api/v1/${vue}?page=${page}&per_page=${parPage}`, signal);
}

/** L'etat du serveur. */
export const sante = (signal?: AbortSignal) => lire<SanteApi>("/api/v1/health", signal);
