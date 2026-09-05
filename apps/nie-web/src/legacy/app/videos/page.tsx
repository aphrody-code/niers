/**
 * Galerie vidéo — les cinématiques USM d'Inazuma Eleven: Victory Road, décodées **à la volée**
 * par nie-model-serve (route CDN `/video/<vfs-path>` : démux USM Sofdec2 → MP4 H.264 ou WebM VP9,
 * seek par Range).
 *
 * La page ne devine plus rien : elle lit le **catalogue** publié par `/video/catalog.json`, bâti
 * par la crate `nie_explore::cinema` — la MÊME fiche que `niers video` et que l'explorateur
 * desktop. Elle en tire ce qu'aucune heuristique ne pouvait donner : durée, définition, cadence,
 * codec, rubrique (15, pas 4), langue, bande-son et jointure `movie_playing_config`.
 *
 * Trois faits mesurés qui changent l'interface :
 *
 * 1. **20 des 97 films sont en MPEG-2** (18 écrans-titres, 2 logos). Aucun navigateur ne les
 *    décode : leur monter une balise `<video>`, comme le faisait cette page, donnait un lecteur
 *    noir. Ils sont désormais annoncés comme tels et proposés au téléchargement.
 * 2. **95 films sur 97 sont muets dans leur conteneur** : leur son vit dans `anime_stream` et se
 *    sert à part (`?track=audio`). 30 films en ont un, et le lecteur le monte en `<audio>`
 *    synchronisé ; les 65 autres le disent au lieu de laisser croire à une panne.
 * 3. `dx11/movie` n'est PAS un doublon de `common/movie` : les deux dossiers portent les 96 mêmes
 *    noms, mais 16,1 Gio contre 3,7 Gio — c'est la variante PC à haut débit, et c'est elle qu'on
 *    sert quand elle existe.
 *
 * Server Component (`force-dynamic`). Le catalogue est un artefact produit hors ligne
 * (`niers video catalogue --out <cache>/video-catalog.json`) : le serveur ne le construit pas
 * lui-même — il vit collé à son plafond mémoire, et une passe sur les 3,7 Gio de films le fait
 * redémarrer par son watchdog. Tant qu'il manque, le CDN répond 503 et la page retombe sur le
 * listing VFS live : moins riche, mais elle rend.
 *
 * L'état de la page — filtres, recherche, film ouvert — vit dans l'URL (`etat-url.ts`). C'est ce
 * qui rend une cinématique partageable : `/videos?film=ev01_00050` ouvre son lecteur, et le
 * canonique comme le titre de l'onglet en découlent.
 */
import type { Metadata } from "next";
import { VideoGallery } from "@/app/videos/VideoGallery";
import { CLES_CANONIQUES, lireEtat } from "@/app/videos/etat-url";
import type { FilmVue } from "@/app/videos/film-vue";
import { MediaEmpty, MediaHeader } from "@/components/wiki/MediaShell";
import { videoLabel } from "@/lib/cpk/media-names";
import { buildCanonical } from "@/lib/seo";
import { lsOrNull } from "@rosegriffon/azalee/cpk/live";
import {
	aDuSon,
	type FilmDto,
	fetchVideoCatalogueOrNull,
	type LangueDto,
} from "@rosegriffon/azalee/cpk/video";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

/** Les paramètres d'URL, tels que Next les livre à cette page. */
type Recherche = Promise<Record<string, string | string[] | undefined>>;

/**
 * Le titre et le canonique suivent les paramètres.
 *
 * Une lightbox ouverte sur `?film=ev01_00050` est un document à part entière : elle mérite son
 * titre — le vrai libellé du film, résolu par `videoLabel` comme dans la page elle-même, pas le
 * code de fichier — et son canonique. `q` est la seule clé écartée du canonique : c'est une
 * recherche interne, sur un espace non borné, et chaque frappe y deviendrait une page de plus
 * (même raisonnement que `LIST_CANONICAL_KEYS` dans `lib/seo.ts`).
 */
export async function generateMetadata({
	searchParams,
}: {
	searchParams: Recherche;
}): Promise<Metadata> {
	const params = await searchParams;
	const etat = lireEtat(params);

	let titre = "Vidéos & cinématiques";
	if (etat.film) {
		const label = await videoLabel(etat.film);
		titre = `${label.title} (${etat.film})`;
	} else if (etat.rubrique !== "toutes") {
		titre = `Vidéos — ${etat.rubrique}`;
	}

	return {
		alternates: { canonical: buildCanonical("/videos", params, CLES_CANONIQUES) },
		description:
			"Cinématiques et vidéos d'Inazuma Eleven: Victory Road (écrans-titre, événements), décodées à la volée depuis les fichiers du jeu (USM → MP4).",
		openGraph: {
			description:
				"Toutes les cinématiques d'Inazuma Eleven: Victory Road, décodées live depuis les CPK.",
			locale: "fr_FR",
			siteName: "Azalée - Inazuma Eleven Victory Road",
			title: `${titre} | Azalée`,
			type: "website",
			url: buildCanonical("/videos", params, CLES_CANONIQUES),
		},
		title: `${titre} — Inazuma Eleven Victory Road - Azalée`,
	};
}

/**
 * Fiche minimale bâtie sur le seul listing VFS, quand le catalogue n'est pas disponible.
 *
 * Tout ce qui exige d'ouvrir le conteneur reste vide — on n'invente ni durée, ni codec, ni
 * rubrique. `lisibleNavigateur` est laissé à `true` : sans mesure, refuser la lecture à un film
 * qui se lit serait pire que de laisser le navigateur essayer.
 */
function ficheDeRepli(path: string, nom: string, octets: number): FilmDto {
	return {
		audio: [],
		cadence: null,
		chemin: path,
		codec: "inconnu",
		duree: null,
		hauteur: 0,
		images: 0,
		langue: null,
		largeur: 0,
		lisibleNavigateur: true,
		nom,
		octets,
		octetsVideo: 0,
		rubrique: "Sans rubrique",
		totalImagesDeclare: 0,
	};
}

export default async function VideosPage({ searchParams }: { searchParams: Recherche }) {
	// Le catalogue porte les métadonnées ; le listing dx11 dit lesquels existent en haut débit,
	// avec leur poids réel. Les deux en parallèle : ni l'un ni l'autre ne bloque l'autre.
	const [params, catalogue, hd] = await Promise.all([
		searchParams,
		fetchVideoCatalogueOrNull(),
		lsOrNull("data/dx11/movie", 500),
	]);
	const etatUrl = lireEtat(params);

	// Repli : le catalogue n'est pas encore construit. On liste au moins les films.
	let films: FilmDto[] = catalogue?.films ?? [];
	let langues: LangueDto[] = catalogue?.langues ?? [];
	let degrade = false;
	if (films.length === 0) {
		degrade = true;
		const sd = await lsOrNull("data/common/movie", 500);
		const source = (sd?.files ?? []).length > 0 ? sd : hd;
		films = (source?.files ?? [])
			.filter((f) => f.ext === "usm")
			.map((f) => ficheDeRepli(f.path, f.name.replace(/\.usm$/i, ""), f.size));
		langues = [];
	}

	// Poids de la variante haut débit, par radical — c'est elle qu'on sert quand elle existe.
	const poidsHd = new Map<string, number>();
	for (const f of hd?.files ?? []) {
		if (f.ext === "usm") poidsHd.set(f.name.replace(/\.usm$/i, ""), f.size);
	}

	// Le code de fichier n'est pas un titre : on résout un vrai libellé quand les données du jeu
	// en portent un (épisode via `inagle_events`, langue des écrans-titre), et le code passe en
	// sous-titre. Résolu ici, côté serveur, parce que la table vit dans le miroir SQLite.
	// Les 97 libellés d'un coup : `videoLabel` partage un index d'épisodes mémoïsé, et une
	// boucle séquentielle ne ferait qu'ajouter 96 allers-retours au miroir.
	const labels = await Promise.all(films.map((f) => videoLabel(f.nom)));
	const vues: FilmVue[] = films
		.map((film, i) => {
			const label = labels[i];
			const octetsHd = poidsHd.get(film.nom);
			return Object.assign({}, film, {
				cheminServi:
					octetsHd == null ? film.chemin : film.chemin.replace("/common/movie/", "/dx11/movie/"),
				contexte: label?.context ?? null,
				octetsServis: octetsHd ?? film.octets,
				titre: label?.title ?? film.nom,
				variante: octetsHd == null ? ("standard" as const) : ("haute" as const),
			});
		})
		.toSorted((a, b) => a.nom.localeCompare(b.nom, "fr", { numeric: true }));

	const hautes = vues.filter((v) => v.variante === "haute").length;
	const sonores = vues.filter((v) => aDuSon(v)).length;
	const duree = vues.reduce((t, v) => t + (v.duree ?? 0), 0);
	const heures = Math.floor(duree / 3600);
	const minutes = Math.round((duree % 3600) / 60);

	return (
		<div className="w-full space-y-6">
			<MediaHeader
				title="Vidéos & cinématiques"
				active="/videos"
				description={
					`${vues.length.toLocaleString("fr")} cinématiques d'Inazuma Eleven: Victory Road, ` +
					`décodées à la volée depuis les fichiers du jeu (USM → MP4 H.264 ou WebM VP9). ` +
					(degrade
						? "Le catalogue détaillé n'est pas publié en ce moment : durées, codecs et " +
							"bandes-son manquent à cette liste."
						: `${heures > 0 ? `${heures} h ${minutes} min` : `${minutes} min`} au total, ` +
							`${sonores.toLocaleString("fr")} avec bande-son` +
							(hautes > 0
								? `, ${hautes.toLocaleString("fr")} servies dans leur variante PC haut débit.`
								: "."))
				}
			/>

			{vues.length === 0 ? (
				<MediaEmpty>
					Le VFS du jeu est momentanément injoignable — la liste des cinématiques ne peut pas être
					établie.
				</MediaEmpty>
			) : (
				<VideoGallery films={vues} langues={langues} degrade={degrade} etatUrl={etatUrl} />
			)}
		</div>
	);
}
