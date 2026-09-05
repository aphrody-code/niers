"use client";

/**
 * Grille des cinématiques (îlot client), rangée par rubrique.
 *
 * La grille montait autrefois un `<video>` par carte. Elle n'ouvre plus qu'**un** lecteur, en
 * lightbox (`VideoLightbox`) : avec 97 films dont chaque flux déclenche un démux USM→MP4 live
 * côté CDN, une grille de lecteurs fait travailler le serveur pour 96 vidéos que personne ne
 * regarde. Les cartes ne chargent donc plus un seul octet de flux — elles décrivent, elles
 * n'invoquent pas.
 *
 * Trois choses que cet îlot fait et qu'une grille filtrée en `useState` pur ne faisait pas :
 *
 * * **l'état est dans l'URL** — rubrique, langue, son, recherche et surtout le film ouvert. Une
 *   cinématique se partage par lien ; auparavant elle ne s'atteignait que par une suite de clics.
 * * **les chips comptent** — combien de films chaque rubrique laisserait passer, compte tenu des
 *   AUTRES filtres. Une rubrique à 0 se voit avant d'être cliquée.
 * * **le vide se répare** — un filtrage qui ne rend rien propose sa propre annulation, au lieu
 *   d'une ligne « Aucune vidéo » sans issue.
 *
 * Le filtrage reste client : il est instantané. L'URL, elle, n'est réécrite qu'après 400 ms de
 * stabilité — la page est `force-dynamic`, chaque `router.replace` refait un rendu serveur
 * (catalogue + 97 libellés). Sans ce délai, tenir la flèche droite enfoncée dans la lightbox
 * lancerait un rendu par image.
 */

import { useCallback, useEffect, useMemo, useRef, useState, useTransition } from "react";
import { useRouter } from "next/navigation";
import { VideoLightbox } from "@/app/videos/VideoLightbox";
import {
	aDesFiltres,
	type EtatVideos,
	ETAT_VIDE,
	serialiserEtat,
} from "@/app/videos/etat-url";
import type { FilmVue } from "@/app/videos/film-vue";
import { Icon } from "@/components/ui/Icon";
import { MediaCount, MediaEmpty, MediaTitle } from "@/components/wiki/MediaShell";
import { cpkExplorerHref } from "@rosegriffon/azalee/cpk/live";
import {
	aDuSon,
	type LangueDto,
	formatDefinition,
	formatDuree,
	formatOctets,
	formatSortie,
	ordreRubrique,
	videoDownloadUrl,
} from "@rosegriffon/azalee/cpk/video";

export type { FilmVue } from "@/app/videos/film-vue";

/** Délai de stabilité avant de réécrire l'URL, en millisecondes. */
const DELAI_URL = 400;

/** Les repères d'un film, dans l'ordre où on les cherche : durée, définition, poids, cadence. */
function Reperes({ film }: { film: FilmVue }) {
	const duree = formatDuree(film.duree);
	const definition = formatDefinition(film);
	return (
		<span className="text-[11px] text-on-surface-variant/70">
			{duree && <>{duree} · </>}
			{definition && <>{definition} · </>}
			{formatOctets(film.octetsServis)}
			{film.variante === "haute" && <span title="Variante PC, débit supérieur"> · HD</span>}
			{film.cadence != null && film.cadence > 0 && <> · {film.cadence.toFixed(0)} i/s</>}
		</span>
	);
}

/** Une carte : ce que le film EST, et de quoi l'ouvrir. Aucun flux n'est chargé ici. */
function CarteFilm({ film, onOuvrir }: { film: FilmVue; onOuvrir: () => void }) {
	const sortie = formatSortie(film);
	const avecSon = aDuSon(film);
	const lisible = film.lisibleNavigateur;

	return (
		<div className="group overflow-hidden rounded-2xl border border-outline-variant/20 bg-surface-container transition-colors hover:border-primary/40">
			<div className="relative aspect-video bg-surface-container-high">
				{/* Les 20 MPEG-2 s'ouvrent aussi : la lightbox y montre leur fiche et leur
				    téléchargement, jamais un lecteur — aucun navigateur ne décode ce flux. */}
				<button
					type="button"
					onClick={onOuvrir}
					className="absolute inset-0 flex flex-col items-center justify-center gap-2 text-on-surface-variant transition-colors hover:text-primary"
					aria-label={
						lisible ? `Lire ${film.titre}` : `${film.titre} — fiche (codec non lisible ici)`
					}
				>
					<Icon name={lisible ? "play_circle" : "movie_off"} size={56} />
					<span className="px-3 text-center text-xs">
						{lisible ? "Lire" : `${film.codec.toUpperCase()} — aucun navigateur ne le décode`}
					</span>
				</button>
				{film.langue && (
					<span className="pointer-events-none absolute left-2 top-2 rounded bg-black/60 px-1.5 py-0.5 text-[10px] font-medium uppercase text-white">
						{film.langue}
					</span>
				)}
				{avecSon && (
					<span
						className="pointer-events-none absolute right-2 top-2 rounded bg-black/60 px-1.5 py-0.5 text-[10px] text-white"
						title={
							film.bandeSon
								? `Bande-son ${film.bandeSon.cue} (anime_stream)`
								: "Bande-son dans le conteneur"
						}
					>
						<Icon name="music_note" size={11} />
					</span>
				)}
			</div>

			<div className="min-w-0 p-2.5">
				<MediaTitle title={film.titre} code={film.nom} context={film.contexte ?? undefined} />
				<div className="mt-1 flex items-center justify-between gap-2">
					<Reperes film={film} />
					<span className="flex items-center gap-2">
						<a
							href={videoDownloadUrl(film.cheminServi, sortie.id)}
							className="inline-flex items-center gap-1 text-[11px] text-on-surface-variant transition-colors hover:text-primary"
							title={`Télécharger le ${sortie.libelle} (remux sans réencodage)`}
						>
							<Icon name="download" size={12} /> {sortie.libelle}
						</a>
						<a
							href={cpkExplorerHref(film.cheminServi)}
							className="text-[11px] text-on-surface-variant/70 transition-colors hover:text-primary"
							title="Voir le fichier dans l'explorateur CPK"
						>
							USM
						</a>
					</span>
				</div>
			</div>
		</div>
	);
}

export function VideoGallery({
	films,
	langues,
	degrade,
	etatUrl,
}: {
	films: FilmVue[];
	langues: LangueDto[];
	degrade: boolean;
	/** L'état lu dans l'URL par le Server Component — l'état initial, puis l'autorité en cas de retour arrière. */
	etatUrl: EtatVideos;
}) {
	const router = useRouter();
	const [, demarrerTransition] = useTransition();
	const [etat, setEtat] = useState<EtatVideos>(etatUrl);
	const [lectureContinue, setLectureContinue] = useState(false);
	// La dernière chaîne qu'on a poussée dans l'URL. Ce qui arrive d'ailleurs (retour arrière,
	// lien collé) en diffère, et c'est à cela qu'on le reconnaît.
	const dernierPousse = useRef(serialiserEtat(etatUrl));

	// L'URL fait autorité quand elle change SANS nous : navigation arrière, lien ouvert.
	useEffect(() => {
		const venu = serialiserEtat(etatUrl);
		if (venu === dernierPousse.current) return;
		dernierPousse.current = venu;
		setEtat(etatUrl);
	}, [etatUrl]);

	// …et l'état est réécrit dans l'URL quand il se stabilise. `replace` et non `push` : chaque
	// frappe de recherche n'a pas à laisser une entrée dans l'historique.
	useEffect(() => {
		const cible = serialiserEtat(etat);
		if (cible === dernierPousse.current) return;
		const minuteur = setTimeout(() => {
			dernierPousse.current = cible;
			demarrerTransition(() => {
				router.replace(cible ? `/videos?${cible}` : "/videos", { scroll: false });
			});
		}, DELAI_URL);
		return () => clearTimeout(minuteur);
	}, [etat, router]);

	/** Modifie une facette de l'état sans toucher aux autres. */
	const majEtat = useCallback((changement: Partial<EtatVideos>) => {
		setEtat((e) => ({ ...e, ...changement }));
	}, []);

	// Les rubriques viennent des films eux-mêmes, dans l'ordre du récit — pas de l'alphabet, qui
	// intercalerait « Chronicle » entre deux chapitres.
	const rubriques = useMemo(() => {
		const vues = [...new Set(films.map((f) => f.rubrique))];
		return vues.toSorted((a, b) => ordreRubrique(a) - ordreRubrique(b) || a.localeCompare(b, "fr"));
	}, [films]);

	// Les langues réellement portées par le corpus, nommées par la table du jeu.
	const languesPresentes = useMemo(() => {
		const codes = new Set(films.map((f) => f.langue).filter((l): l is string => l != null));
		const nom = new Map(langues.map((l) => [l.code, l.nom]));
		return [...codes]
			.toSorted((a, b) => a.localeCompare(b, "fr"))
			.map((code) => ({ code, nom: nom.get(code) ?? code }));
	}, [films, langues]);

	/**
	 * Le prédicat de filtrage, facette par facette.
	 *
	 * Isolé parce qu'il sert deux fois : pour la liste affichée, et pour compter ce que chaque
	 * rubrique laisserait passer une fois les AUTRES filtres appliqués. Compter sur le total du
	 * corpus donnerait des chips qui promettent des résultats que le clic ne rend pas.
	 */
	const passe = useCallback(
		(f: FilmVue, e: EtatVideos, saufRubrique = false): boolean => {
			if (!saufRubrique && e.rubrique !== "toutes" && f.rubrique !== e.rubrique) return false;
			if (e.langue !== "toutes" && f.langue !== e.langue) return false;
			if (e.son === "avec" && !aDuSon(f)) return false;
			if (e.son === "sans" && aDuSon(f)) return false;
			const q = e.requete.trim().toLowerCase();
			if (q === "") return true;
			// La recherche porte sur le libellé, le code et le contexte : « épisode 3 » doit se
			// trouver par son nom, et un code connu doit continuer de fonctionner.
			return (
				f.nom.toLowerCase().includes(q) ||
				f.titre.toLowerCase().includes(q) ||
				f.rubrique.toLowerCase().includes(q) ||
				(f.contexte?.toLowerCase().includes(q) ?? false)
			);
		},
		[],
	);

	const visibles = useMemo(() => films.filter((f) => passe(f, etat)), [films, etat, passe]);

	/** Ce que chaque rubrique laisserait passer, les autres filtres restant en place. */
	const comptesRubrique = useMemo(() => {
		const c = new Map<string, number>();
		let total = 0;
		for (const f of films) {
			if (!passe(f, etat, true)) continue;
			total += 1;
			c.set(f.rubrique, (c.get(f.rubrique) ?? 0) + 1);
		}
		return { parRubrique: c, total };
	}, [films, etat, passe]);

	// Regroupement par rubrique : c'est ainsi que les films se cherchent — par moment du récit,
	// pas par ordre alphabétique de nom de fichier.
	const sections = useMemo(() => {
		const par = new Map<string, FilmVue[]>();
		for (const f of visibles) {
			const liste = par.get(f.rubrique);
			if (liste) liste.push(f);
			else par.set(f.rubrique, [f]);
		}
		return [...par.entries()].toSorted(
			([a], [b]) => ordreRubrique(a) - ordreRubrique(b) || a.localeCompare(b, "fr"),
		);
	}, [visibles]);

	/**
	 * La liste dans laquelle la lightbox navigue.
	 *
	 * C'est la sélection filtrée — sauf quand un lien désigne un film que ces mêmes filtres
	 * excluent : plutôt que de refuser d'ouvrir (un lien partagé qui ne montre rien), on l'ouvre
	 * seul. Le compteur dit alors « 1 / 1 », ce qui est la vérité : il n'y a pas de sélection
	 * autour de lui.
	 */
	const listeLecteur = useMemo(() => {
		if (!etat.film) return visibles;
		if (visibles.some((f) => f.nom === etat.film)) return visibles;
		const isole = films.find((f) => f.nom === etat.film);
		return isole ? [isole] : visibles;
	}, [visibles, films, etat.film]);

	const rangOuvert = etat.film ? listeLecteur.findIndex((f) => f.nom === etat.film) : -1;

	const classeChip = (actif: boolean) =>
		`h-9 shrink-0 rounded-full px-3 text-sm transition-colors ${
			actif
				? "bg-primary text-on-primary"
				: "bg-surface-container text-on-surface-variant hover:bg-surface-container-high"
		}`;

	return (
		<div className="space-y-4">
			<div className="flex flex-wrap items-center gap-2">
				<div className="flex h-10 min-w-[12rem] flex-1 items-center gap-2 rounded-full bg-surface-container-highest px-3">
					<Icon name="search" size={18} className="shrink-0 text-on-surface-variant" />
					<input
						value={etat.requete}
						onChange={(e) => majEtat({ requete: e.target.value })}
						placeholder="Rechercher une vidéo…"
						aria-label="Rechercher une vidéo"
						className="min-w-0 flex-1 bg-transparent text-sm text-on-surface outline-none placeholder:text-on-surface-variant/60"
					/>
					{etat.requete !== "" && (
						<button
							type="button"
							onClick={() => majEtat({ requete: "" })}
							aria-label="Effacer la recherche"
							className="shrink-0 text-on-surface-variant transition-colors hover:text-on-surface"
						>
							<Icon name="close" size={16} />
						</button>
					)}
				</div>

				{languesPresentes.length > 0 && (
					<select
						value={etat.langue}
						onChange={(e) => majEtat({ langue: e.target.value })}
						className="h-9 rounded-full bg-surface-container px-3 text-sm text-on-surface-variant"
						aria-label="Filtrer par langue"
					>
						<option value="toutes">Toutes les langues</option>
						{languesPresentes.map((l) => (
							<option key={l.code} value={l.code}>
								{l.nom}
							</option>
						))}
					</select>
				)}

				{/* Le mode dégradé n'a pas de catalogue : sans bande-son mesurée, ce filtre
				    trierait sur une donnée absente. Il disparaît plutôt que de mentir. */}
				{!degrade && (
					<div className="flex gap-1.5" role="group" aria-label="Filtrer par bande-son">
						{(
							[
								["tous", "Toutes"],
								["avec", "Avec son"],
								["sans", "Muettes"],
							] as const
						).map(([valeur, libelle]) => (
							<button
								key={valeur}
								type="button"
								aria-pressed={etat.son === valeur}
								onClick={() => majEtat({ son: valeur })}
								className={classeChip(etat.son === valeur)}
							>
								{libelle}
							</button>
						))}
					</div>
				)}
			</div>

			<div className="flex flex-wrap gap-1.5" role="group" aria-label="Filtrer par rubrique">
				<button
					type="button"
					aria-pressed={etat.rubrique === "toutes"}
					onClick={() => majEtat({ rubrique: "toutes" })}
					className={classeChip(etat.rubrique === "toutes")}
				>
					Toutes les rubriques{" "}
					<span className="opacity-70">{comptesRubrique.total.toLocaleString("fr")}</span>
				</button>
				{rubriques.map((r) => {
					const compte = comptesRubrique.parRubrique.get(r) ?? 0;
					return (
						<button
							key={r}
							type="button"
							aria-pressed={etat.rubrique === r}
							onClick={() => majEtat({ rubrique: r })}
							className={`${classeChip(etat.rubrique === r)}${compte === 0 && etat.rubrique !== r ? " opacity-50" : ""}`}
						>
							{r} <span className="opacity-70">{compte.toLocaleString("fr")}</span>
						</button>
					);
				})}
			</div>

			<MediaCount
				left={`${visibles.length.toLocaleString("fr")} vidéo${visibles.length > 1 ? "s" : ""}`}
				right={
					visibles.length === films.length
						? undefined
						: `sur ${films.length.toLocaleString("fr")}`
				}
			/>

			{visibles.length === 0 ? (
				<MediaEmpty>
					Aucune cinématique ne correspond à ces filtres.
					{aDesFiltres(etat) && (
						<button
							type="button"
							onClick={() => setEtat({ ...ETAT_VIDE })}
							className="mt-4 inline-flex items-center gap-2 rounded-full bg-primary px-4 py-2 text-sm font-medium not-italic text-on-primary transition-colors hover:bg-primary/90"
						>
							<Icon name="filter_alt_off" size={16} />
							Réinitialiser les filtres
						</button>
					)}
				</MediaEmpty>
			) : (
				sections.map(([titre, liste]) => (
					<section key={titre} className="space-y-2">
						{sections.length > 1 && (
							<h2 className="px-1 text-sm font-semibold text-on-surface">
								{titre}
								<span className="ml-2 text-xs font-normal text-on-surface-variant">
									{liste.length}
								</span>
							</h2>
						)}
						<div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
							{liste.map((f) => (
								<CarteFilm key={f.chemin} film={f} onOuvrir={() => majEtat({ film: f.nom })} />
							))}
						</div>
					</section>
				))
			)}

			<VideoLightbox
				films={listeLecteur}
				index={rangOuvert >= 0 ? rangOuvert : null}
				onClose={() => majEtat({ film: null })}
				onNavigate={(rang) => majEtat({ film: listeLecteur[rang]?.nom ?? null })}
				lectureContinue={lectureContinue}
				onLectureContinue={setLectureContinue}
			/>
		</div>
	);
}
