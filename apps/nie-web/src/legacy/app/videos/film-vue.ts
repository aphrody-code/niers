/**
 * La fiche d'un film telle que la page la manipule : celle du catalogue, augmentée de ce que le
 * Server Component a résolu (libellé humain, variante servie).
 *
 * Ce type vivait dans `VideoGallery.tsx`, que le lecteur en lightbox doit importer — et qui
 * importe le lecteur. Un cycle d'imports que TypeScript tolère en `import type` mais que le
 * bundler paie ; il est ici, où les trois modules le lisent sans se citer l'un l'autre.
 */

import type { FilmDto } from "@rosegriffon/azalee/cpk/video";

/** Un film du catalogue, augmenté de ce que la page a résolu côté serveur. */
export interface FilmVue extends FilmDto {
	/** Titre affichable — un vrai nom quand les données du jeu en portent un, sinon le code. */
	titre: string;
	/** Contexte attesté (épisode, langue), `null` s'il n'y en a pas. */
	contexte: string | null;
	/** Chemin réellement servi : la variante haut débit quand elle existe. */
	cheminServi: string;
	/** Taille de la variante servie, en octets. */
	octetsServis: number;
	/** Variante servie : `haute` = dx11 (débit PC), `standard` = common. */
	variante: "haute" | "standard";
}
