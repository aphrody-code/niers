/**
 * L'implémentation d'`AssetSource` pour Inacord.
 *
 * Elle enveloppe `api` — les liaisons `tauri-specta` générées depuis le Rust — et la présente
 * sous la forme que l'interface partagée attend. Les composants de `@niers/inacord-ui` ne
 * voient donc jamais Tauri, alors qu'ils tournent dans une fenêtre Tauri.
 *
 * ## Pourquoi cette enveloppe vit ici et non dans `@niers/asset-source`
 *
 * Le plan prévoyait de la placer dans le paquet, aux côtés du contrat. Elle y aurait fait
 * dépendre de Tauri un paquet que le navigateur doit consommer — exactement ce que la gate
 * « zéro `@tauri-apps` dans l'interface partagée » cherche à empêcher. Elle dépend en outre de
 * `bindings.ts`, régénéré à chaque build par `export-bindings` : un paquet ne peut pas
 * s'appuyer sur un fichier qui appartient au cycle de construction d'une application.
 *
 * Le contrat vit donc dans le paquet, les implémentations chez leurs hôtes.
 */
import {
	type AssetSource,
	AUCUNE_CAPACITE,
	type CapacitesSource,
	type ContenuDossier,
	type EntreeVfs,
	type OptionsParcours,
	type SanteApi,
	type VueCatalogue,
} from "@niers/asset-source";

import { api } from "./api";

/** Extensions retenues par chaque filtre enregistré — les mêmes que `nie-site`, côté Rust. */
const FILTRES: Record<VueCatalogue, string[]> = {
	textures: ["g4tx", "dds", "png"],
	modeles: ["g4md", "g4mg", "g4sk", "g4mt", "g4pk", "g4pkm"],
	sons: ["acb", "awb", "hca", "adx", "wav"],
	videos: ["usm", "mp4", "webm"],
};

/**
 * Construit la source desktop.
 *
 * @param racineJeu racine du jeu choisie par l'utilisateur, transmise aux commandes qui en ont
 * besoin. Les composants ne la connaissent pas : c'est un détail de cet hôte.
 */
export function creerDesktopSource(racineJeu?: string): AssetSource {
	return {
		hote: "inacord",

		async capacites(): Promise<CapacitesSource> {
			// Le desktop sait tout faire — à condition que la racine du jeu soit valide. On la
			// MESURE au lieu de l'affirmer : sans jeu, le VFS ne rend rien et l'interface doit
			// le savoir avant d'afficher des vues vides.
			let jeuPresent = false;
			try {
				// `checkGameDir` exige une racine : sans elle, il n'y a rien a verifier, et
				// l'hote doit le dire plutot que d'appeler avec `undefined`.
				jeuPresent = racineJeu ? Boolean(await api.checkGameDir(racineJeu)) : false;
			} catch {
				jeuPresent = false;
			}
			return {
				...AUCUNE_CAPACITE,
				vfs: jeuPresent,
				texture: jeuPresent,
				modele: jeuPresent,
				avatar: jeuPresent,
				audio: jeuPresent,
				video: jeuPresent,
				// Ces trois-là ne dépendent pas du jeu : ce sont les pouvoirs de l'hôte natif,
				// et ils sont ce qui distingue Inacord d'Aphrody.
				wiki: true,
				ecriture: true,
				disque: true,
				outils: true,
			};
		},

		async sante(): Promise<SanteApi> {
			const stats = await api.stats(racineJeu);
			const entrees = stats?.total ?? 0;
			// La forme est celle de `nie-site`, pour que l'interface n'ait qu'un seul modèle à
			// lire. L'hôte natif n'a ni bundle ni version d'API : il le dit plutôt que d'inventer.
			return {
				service: "inacord",
				api: "tauri",
				version: "",
				capacites: {
					vfs: entrees > 0 ? "pret" : "absent",
					vfs_entrees: entrees,
					vfs_dump: false,
					vfs_contenu: entrees > 0,
					gisement: true,
					bundle: false,
				},
				vues: (Object.keys(FILTRES) as VueCatalogue[]).map((nom) => ({
					nom,
					extensions: FILTRES[nom],
					total: null,
				})),
			};
		},

		async parcourir(prefixe: string, options: OptionsParcours = {}): Promise<ContenuDossier> {
			const ls = await api.ls(prefixe, racineJeu);
			const tous = (ls?.files ?? []) as unknown as EntreeVfs[];
			// Le filtre est appliqué ICI, pas par l'IPC : `ls` rend le dossier entier de toute
			// façon, et un dossier du VFS tient en quelques milliers d'entrées. Filtrer en
			// mémoire donne donc le même résultat que le filtre serveur d'Aphrody, pour le même
			// coût de transport — l'asymétrie serait fausse sur une VUE (six extensions,
			// 255 308 entrées), pas sur un dossier.
			const q = options.q?.trim().toLowerCase();
			const ext = options.ext?.trim().replace(/^\./, "").toLowerCase();
			const { tailleMin, tailleMax } = options;
			const fichiers = tous.filter(
				(f) =>
					(!q || f.chemin.toLowerCase().includes(q)) &&
					(!ext || f.nom.toLowerCase().endsWith(`.${ext}`)) &&
					// `0` est une borne legitime : il existe des fichiers de zero octet.
					// `Number.isFinite` plutot qu'une verite, sinon `tailleMax: 0` disparait.
					(!Number.isFinite(tailleMin) || f.taille >= (tailleMin as number)) &&
					(!Number.isFinite(tailleMax) || f.taille <= (tailleMax as number)),
			);
			// Le tri est applique APRES le filtre, sur la meme liste : l'ordre du serveur porte
			// sur le chemin entier et sur la taille, celui-ci fait de meme.
			if (options.tri === "taille") {
				fichiers.sort((a, b) => a.taille - b.taille);
			} else {
				fichiers.sort((a, b) => a.chemin.localeCompare(b.chemin));
			}
			if (options.ordre === "desc") fichiers.reverse();
			return {
				prefixe,
				dossiers: (ls?.dirs ?? []).map((d) => d.name),
				fichiers,
				total: fichiers.length,
				totalSansFiltre: tous.length,
				filtres: {
					q: q || undefined,
					ext: ext || undefined,
					// Meme information que cote serveur, mesuree sur le meme jeu de fichiers :
					// l'extension demandee n'apparait sur aucune entree de ce dossier.
					extInconnue: ext
						? !tous.some((f) => f.nom.toLowerCase().endsWith(`.${ext}`))
						: undefined,
				},
			};
		},

		// Sur le desktop, un chemin VFS n'a pas d'URL : les octets passent par l'IPC. On rend
		// donc le chemin tel quel — les composants qui en font un `href` de téléchargement
		// s'appuient sur `capacites().disque`, pas sur cette valeur.
		urlFichier: (cheminVfs: string) => cheminVfs,

		async vignette(cheminVfs: string, cote: number, racine?: string): Promise<string | null> {
			// Pas de serveur local : la texture est décodée en natif et rendue en URL de données.
			// C'est la raison d'être du caractère asynchrone de `vignette` dans le contrat.
			const b64 = await api.textureThumbB64(cheminVfs, cote, racine ?? racineJeu);
			return b64 ? `data:image/png;base64,${b64}` : null;
		},
	};
}
