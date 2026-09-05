/**
 * Catalogue des cinématiques du jeu — la couche qui rend un film **regardable et décrit**.
 *
 * Un `.usm` n'est pas une vidéo : c'est un conteneur Sofdec2 où le codec vidéo et la bande-son
 * Criware sont entrelacés par blocs, qu'aucun navigateur n'ouvre. `nie-model-serve` en publie
 * l'inventaire complet sur `/video/catalog.json` (crate `nie_explore::cinema`, la MÊME fiche que
 * `niers video` et que l'explorateur desktop) et sert chaque film remuxé sur `/video/<chemin>`.
 *
 * Trois faits mesurés sur les 97 films, que toute interface doit prendre au sérieux :
 *
 * 1. **20 films sont en MPEG-2** (18 écrans-titres, 2 logos) : `lisibleNavigateur` est faux, et
 *    leur monter une balise `<video>` donne un lecteur noir. On propose le téléchargement.
 * 2. **95 films sur 97 sont muets dans leur conteneur.** Leur son vit dans `anime_stream`, que le
 *    serveur résout par le nom du film et sert à part en WAV (`?track=audio`) — d'où un `<audio>`
 *    séparé, à synchroniser avec la vidéo. 30 films en ont un, 65 n'en ont aucun.
 * 3. **La rubrique et la langue ne se devinent pas** : elles viennent des conventions de nommage
 *    du jeu (`nie_formats::usm`), pas d'une regex maison — il y a 15 rubriques, pas 4.
 *
 * ⚠ Module **client-safe** : `fetch` seul, ni `bun:sqlite` ni `node:fs`. Le libellé humain d'un
 * film exige le miroir SQLite et vit donc côté serveur (`apps/azalee/lib/cpk/media-names.ts`).
 *
 * Les quatre URL, les fiches (`FilmDto` & co.) et les formateurs viennent de
 * `@niers/catalog/jeu` : c'est le serveur qui décide de la forme des unes et des autres, et les
 * réécrire ici en ferait une deuxième vérité, qui dérive en silence.
 */

import {
	formatDefinition as definition,
	formatSortie as formatDeSortie,
	urlBandeSon,
	urlCatalogueFilms,
	urlFicheFilm,
	urlFilm,
} from "@niers/catalog/jeu";
import type { CatalogueVideo, FilmDto } from "@niers/catalog/jeu";

import { exportUrl } from "@rosegriffon/azalee/cpk/live";

export type {
	CatalogueVideo,
	FilmBandeSon,
	FilmDto,
	FilmGamedata,
	FilmPisteInterne,
	LangueDto,
} from "@niers/catalog/jeu";

export { formatDuree, formatOctets, ordreRubrique } from "@niers/catalog/jeu";

/** URL du catalogue complet. */
export function videoCatalogUrl(): string {
	return urlCatalogueFilms();
}

/** URL du flux vidéo remuxé d'un film (MP4 pour H.264, WebM pour VP9). */
export function videoUrl(path: string): string {
	return urlFilm(path);
}

/**
 * URL de la bande-son d'un film, en WAV.
 *
 * Vaut pour les deux provenances : la piste du conteneur quand il y en a une, la cue
 * d'`anime_stream` sinon. Répond 404 quand le film n'a aucune bande-son identifiable — ce qui
 * est le cas de 65 films, et se dit plutôt que de se combler par le son d'un autre.
 */
export function videoAudioUrl(path: string): string {
	return urlBandeSon(path);
}

/** URL de la fiche détaillée d'un film (remux mesuré compris). */
export function videoInfoUrl(path: string): string {
	return urlFicheFilm(path);
}

/**
 * URL de téléchargement d'un film, nommée par le serveur.
 *
 * Passe par `/export`, qui pose un `Content-Disposition` : un `<a download>` vers une origine
 * tierce ne peut PAS imposer le nom du fichier — l'attribut est ignoré cross-origin, et le
 * téléchargement arrivait sous le nom de l'URL, sans extension utile.
 */
export function videoDownloadUrl(path: string, format: string): string {
	return exportUrl(path, format);
}

/**
 * Le format de téléchargement qui correspond au codec du film.
 *
 * H.264 → MP4, VP9 → WebM, MPEG-2 → flux élémentaire `.m2v` (VLC et mpv le lisent ; aucun
 * navigateur ne le décode, et l'emballer en MP4 serait un mensonge).
 */
export function formatSortie(film: FilmDto): { id: string; ext: string; libelle: string } {
	return formatDeSortie(film.codec);
}

/** Vrai si le film a une bande-son, d'où qu'elle vienne. */
export function aDuSon(film: FilmDto): boolean {
	return film.audio.length > 0 || film.bandeSon != null;
}

/** Récupère le catalogue. Lève si le pont ne répond pas ou ne l'a pas encore construit. */
export async function fetchVideoCatalogue(): Promise<CatalogueVideo> {
	const res = await fetch(videoCatalogUrl(), { cache: "no-store" });
	if (!res.ok) throw new Error(`catalogue vidéo ${res.status}`);
	return (await res.json()) as CatalogueVideo;
}

/**
 * Variante tolérante : `null` au lieu d'une exception.
 *
 * Le serveur répond 503 tant que le catalogue n'est pas construit (il parcourt 3,7 Gio au
 * premier démarrage) : une page qui sait dégrader continue de rendre au lieu de casser.
 */
export async function fetchVideoCatalogueOrNull(): Promise<CatalogueVideo | null> {
	try {
		return await fetchVideoCatalogue();
	} catch {
		return null;
	}
}

/** Définition `1920×1080`, ou `null` si le conteneur ne la déclare pas. */
export function formatDefinition(film: FilmDto): string | null {
	return definition(film.largeur, film.hauteur);
}
