"use client";

/**
 * Le lecteur unique de la page vidéo : une lightbox plein écran, montée en portail.
 *
 * La grille montait un `<video>` par carte. Avec 97 films dont **chaque flux déclenche un démux
 * USM→MP4 live côté CDN**, c'est le mauvais modèle : une grille de lecteurs, c'est autant de
 * décodages concurrents sur un service qui vit collé à son plafond mémoire, pour des lecteurs
 * dont 96 sont hors de l'écran. Il n'y a plus qu'un lecteur, celui du film ouvert.
 *
 * Ce que la lightbox apporte et qu'une carte ne pouvait pas donner :
 *
 * * **le film est adressable** — l'état de la page porte le radical ouvert, donc une cinématique
 *   se partage par lien (`/videos?film=ev01_00050`) ;
 * * **on parcourt la sélection** — ←/→ et boutons, dans la liste FILTRÉE, avec son rang ;
 * * **le détail technique tient enfin quelque part** : les trente champs que le catalogue mesure
 *   n'avaient aucune place sur une vignette (`FilmDetails`).
 *
 * Trois faits du corpus que ce lecteur doit respecter :
 *
 * 1. **20 films sur 97 sont en MPEG-2** : aucun navigateur ne les décode. On ne monte **aucun**
 *    élément `<video>` pour eux — on annonce le codec et on propose le téléchargement. La
 *    lecture continue les saute.
 * 2. **95 films sur 97 sont muets dans leur conteneur** : leur bande-son vit dans `anime_stream`
 *    et se sert à part (`?track=audio`). Le `<video>` et l'`<audio>` sont deux flux distincts que
 *    ce lecteur tient ensemble.
 * 3. **Le remux est vidéo seule** : aucune piste sonore n'entre dans le MP4 produit. Le volume et
 *    la vitesse du `<video>` ne pilotent donc rien — ils sont répercutés sur l'`<audio>`.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { FilmDetails } from "@/app/videos/FilmDetails";
import type { FilmVue } from "@/app/videos/film-vue";
import { Icon } from "@/components/ui/Icon";
import { cpkExplorerHref } from "@rosegriffon/azalee/cpk/live";
import {
	aDuSon,
	formatDefinition,
	formatDuree,
	formatOctets,
	formatSortie,
	videoAudioUrl,
	videoDownloadUrl,
	videoUrl,
} from "@rosegriffon/azalee/cpk/video";

/** Écart au-delà duquel la piste sonore est recalée sur l'image, en secondes. */
const DERIVE_MAX = 0.25;

/**
 * Poids au-delà duquel on ne passe plus par un blob.
 *
 * Le téléchargement par `fetch` → blob existe parce que l'attribut `download` est **ignoré
 * cross-origin** : un lien direct vers le CDN ne peut pas imposer de nom de fichier. Mais un blob
 * tient le fichier ENTIER en mémoire, et le plus gros film du corpus pèse 325 Mio. Au-delà de ce
 * seuil on repasse par le navigateur : la route `/export` pose elle-même un `Content-Disposition`,
 * donc le nom reste correct — c'est seulement l'onglet qui clignote.
 */
const SEUIL_BLOB = 128 * 1024 * 1024;

/** Ce que le navigateur dit quand la lecture échoue, traduit en une phrase utile. */
function messageErreur(erreur: MediaError | null): string {
	switch (erreur?.code) {
		case MediaError.MEDIA_ERR_ABORTED:
			return "Lecture interrompue.";
		case MediaError.MEDIA_ERR_NETWORK:
			return "Le flux s'est interrompu. Le démux USM→MP4 se fait à la volée : le serveur a pu abandonner en cours de route.";
		case MediaError.MEDIA_ERR_DECODE:
			return "Le navigateur n'a pas su décoder ce flux.";
		case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
			return "Aucune source lisible : le remux n'a rien renvoyé pour ce film.";
		default:
			return "Lecture impossible.";
	}
}

/**
 * Le lecteur d'un film : l'image d'un côté, le son de l'autre, tenus ensemble.
 *
 * Monté avec une `key` sur le chemin du film : changer de film remonte l'élément, ce qui remet à
 * zéro les erreurs, la position et l'état de blocage sans un seul effet de nettoyage.
 */
function LecteurFilm({
	film,
	auto,
	onFini,
}: {
	film: FilmVue;
	/** Vrai quand la lecture doit démarrer seule (ouverture, ou enchaînement). */
	auto: boolean;
	/** Appelé à la fin du film — c'est le parent qui décide d'enchaîner ou non. */
	onFini: () => void;
}) {
	const videoRef = useRef<HTMLVideoElement>(null);
	const audioRef = useRef<HTMLAudioElement>(null);
	const avecSon = aDuSon(film);
	const [erreurVideo, setErreurVideo] = useState<string | null>(null);
	const [erreurSon, setErreurSon] = useState(false);
	const [bloque, setBloque] = useState(false);

	const recaler = useCallback(() => {
		const v = videoRef.current;
		const a = audioRef.current;
		if (!v || !a) return;
		if (Math.abs(a.currentTime - v.currentTime) > DERIVE_MAX) a.currentTime = v.currentTime;
	}, []);

	// Le son suit l'image : lecture, pause, saut, vitesse, volume.
	useEffect(() => {
		const v = videoRef.current;
		const a = audioRef.current;
		if (!v || !a) return;
		const jouer = () => {
			a.currentTime = v.currentTime;
			void a.play().catch(() => setErreurSon(true));
		};
		const pause = () => a.pause();
		const vitesse = () => {
			a.playbackRate = v.playbackRate;
		};
		const volume = () => {
			a.volume = v.volume;
			a.muted = v.muted;
		};
		v.addEventListener("play", jouer);
		v.addEventListener("pause", pause);
		v.addEventListener("seeked", recaler);
		v.addEventListener("timeupdate", recaler);
		v.addEventListener("ratechange", vitesse);
		v.addEventListener("volumechange", volume);
		volume();
		return () => {
			v.removeEventListener("play", jouer);
			v.removeEventListener("pause", pause);
			v.removeEventListener("seeked", recaler);
			v.removeEventListener("timeupdate", recaler);
			v.removeEventListener("ratechange", vitesse);
			v.removeEventListener("volumechange", volume);
		};
	}, [recaler]);

	// Démarrage explicite plutôt que l'attribut `autoPlay` : les 32 films sonores montent un
	// `<video>` NON muet (son volume pilote celui de l'`<audio>`), et le navigateur peut refuser
	// de les lancer seul. `autoPlay` échouerait alors en silence, sur un lecteur immobile
	// indiscernable d'une panne ; ici le refus se voit et se rattrape d'un clic.
	useEffect(() => {
		const v = videoRef.current;
		if (!v || !auto) return;
		void v.play().catch(() => setBloque(true));
	}, [auto]);

	return (
		<div className="relative aspect-video w-full overflow-hidden rounded-xl bg-black">
			{/* eslint-disable-next-line jsx-a11y/media-has-caption -- les sous-titres du jeu ne sont pas encore extraits en WebVTT */}
			<video
				ref={videoRef}
				controls
				muted={!avecSon}
				preload="metadata"
				playsInline
				className="size-full"
				src={videoUrl(film.cheminServi)}
				onPlay={() => setBloque(false)}
				onEnded={onFini}
				onError={(e) => setErreurVideo(messageErreur(e.currentTarget.error))}
			/>
			{avecSon && (
				// eslint-disable-next-line jsx-a11y/media-has-caption -- piste sonore seule, sans dialogue transcrit
				<audio
					ref={audioRef}
					preload="auto"
					src={videoAudioUrl(film.chemin)}
					onError={() => setErreurSon(true)}
				/>
			)}

			{erreurVideo && (
				<div className="absolute inset-x-0 bottom-14 mx-auto flex w-fit max-w-[90%] items-center gap-2 rounded-full bg-error px-3 py-1.5 text-xs text-on-error">
					<Icon name="error" size={14} className="shrink-0" />
					<span>{erreurVideo}</span>
				</div>
			)}
			{!erreurVideo && bloque && (
				<div className="absolute inset-x-0 bottom-14 mx-auto flex w-fit max-w-[90%] items-center gap-2 rounded-full bg-surface-container-high px-3 py-1.5 text-xs text-on-surface">
					<Icon name="play_arrow" size={14} className="shrink-0" />
					<span>Le navigateur a refusé de lancer la lecture seul — appuyez sur Lecture.</span>
				</div>
			)}
			{avecSon && erreurSon && (
				<div className="absolute left-2 top-2 flex items-center gap-1.5 rounded-full bg-error px-2.5 py-1 text-[11px] text-on-error">
					<Icon name="error" size={12} className="shrink-0" />
					<span>Bande-son indisponible — l'image joue sans le son.</span>
				</div>
			)}
		</div>
	);
}

/** Le pavé qui remplace le lecteur pour les 20 films qu'aucun navigateur ne décode. */
function PaveIllisible({ film }: { film: FilmVue }) {
	return (
		<div className="flex aspect-video w-full flex-col items-center justify-center gap-3 rounded-xl border border-outline-variant/30 bg-surface-container-low px-6 text-center">
			<Icon name="movie_off" size={56} className="text-on-surface-variant/60" />
			<p className="text-sm font-medium text-on-surface">
				{film.codec.toUpperCase()} — aucun navigateur ne décode ce flux
			</p>
			<p className="max-w-md text-xs text-on-surface-variant">
				{film.remuxImpossible ??
					"Ce codec n'entre dans aucun conteneur web. Le flux élémentaire reste téléchargeable : VLC et mpv le lisent."}
			</p>
		</div>
	);
}

/** Le rang du prochain film réellement lisible, ou `null` s'il n'y en a plus. */
function indexLisibleSuivant(films: FilmVue[], depuis: number): number | null {
	for (let i = depuis + 1; i < films.length; i++) {
		const f = films[i];
		if (f?.lisibleNavigateur) return i;
	}
	return null;
}

/**
 * La lightbox.
 *
 * @param films la liste **filtrée** — c'est dans elle qu'on navigue, et c'est son rang qu'on
 *   affiche. Un lien qui ouvre un film exclu par ses propres filtres reçoit une liste d'un seul
 *   élément : on l'ouvre quand même, sans prétendre naviguer dans une sélection qui l'exclut.
 * @param index rang du film ouvert, `null` quand la lightbox est fermée.
 */
export function VideoLightbox({
	films,
	index,
	onClose,
	onNavigate,
	lectureContinue,
	onLectureContinue,
}: {
	films: FilmVue[];
	index: number | null;
	onClose: () => void;
	onNavigate: (rang: number) => void;
	lectureContinue: boolean;
	onLectureContinue: (valeur: boolean) => void;
}) {
	const [monte, setMonte] = useState(false);
	const [telechargement, setTelechargement] = useState<string | null>(null);
	// Une lecture qui démarre seule n'est légitime qu'après un geste : à l'ouverture (le clic sur
	// la carte) ou à l'enchaînement (la fin du film précédent). Pas quand on feuillette.
	const [auto, setAuto] = useState(true);
	const fermerRef = useRef<HTMLButtonElement>(null);
	const dialogueRef = useRef<HTMLDivElement>(null);

	const ouvert = index !== null;
	const film = ouvert ? films[index] : undefined;
	const avantDisponible = ouvert && index > 0;
	const apresDisponible = ouvert && index < films.length - 1;

	useEffect(() => {
		setMonte(true);
	}, []);

	// Clavier : navigation, fermeture, et piège à focus (Tab cyclé dans la modale).
	useEffect(() => {
		if (!ouvert) return;
		const auClavier = (e: KeyboardEvent) => {
			// Les flèches servent aussi à se déplacer DANS la vidéo quand ses contrôles ont le
			// focus : on ne les détourne pas dans ce cas.
			const cible = document.activeElement;
			const dansLeLecteur = cible instanceof HTMLMediaElement;
			if (e.key === "Escape") {
				onClose();
			} else if (e.key === "ArrowLeft" && !dansLeLecteur && index > 0) {
				setAuto(false);
				onNavigate(index - 1);
			} else if (e.key === "ArrowRight" && !dansLeLecteur && index < films.length - 1) {
				setAuto(false);
				onNavigate(index + 1);
			} else if (e.key === "Tab") {
				const focusables = dialogueRef.current?.querySelectorAll<HTMLElement>(
					'button:not([disabled]), a[href], input, summary, video, audio, [tabindex]:not([tabindex="-1"])',
				);
				if (!focusables || focusables.length === 0) return;
				const premier = focusables[0];
				const dernier = focusables[focusables.length - 1];
				if (!premier || !dernier) return;
				if (e.shiftKey && document.activeElement === premier) {
					e.preventDefault();
					dernier.focus();
				} else if (!e.shiftKey && document.activeElement === dernier) {
					e.preventDefault();
					premier.focus();
				}
			}
		};
		document.addEventListener("keydown", auClavier);
		return () => document.removeEventListener("keydown", auClavier);
	}, [ouvert, index, films.length, onClose, onNavigate]);

	// Bloquer le défilement de la page + donner le focus au bouton de fermeture à l'ouverture.
	useEffect(() => {
		if (!ouvert) return;
		document.body.style.overflow = "hidden";
		fermerRef.current?.focus();
		return () => {
			document.body.style.overflow = "";
		};
	}, [ouvert]);

	// Une réouverture repart en lecture automatique — c'est le clic sur la carte qui l'autorise.
	useEffect(() => {
		if (ouvert) setAuto(true);
	}, [ouvert]);

	/** Fin du film : enchaîner sur le prochain LISIBLE, quand l'enchaînement est demandé. */
	const auFini = useCallback(() => {
		if (!lectureContinue || index === null) return;
		const suivant = indexLisibleSuivant(films, index);
		if (suivant === null) return;
		setAuto(true);
		onNavigate(suivant);
	}, [lectureContinue, index, films, onNavigate]);

	const naviguer = useCallback(
		(rang: number) => {
			setAuto(false);
			onNavigate(rang);
		},
		[onNavigate],
	);

	/**
	 * Télécharge par blob pour imposer le nom du fichier, avec repli sur le navigateur.
	 *
	 * L'attribut `download` d'un lien cross-origin est ignoré : le MP4 arrivait sous le nom de
	 * l'URL. Le blob rend la main sur le nom ; s'il échoue (CORS, mémoire, réseau) on repasse par
	 * `window.open`, où le `Content-Disposition` de `/export` prend le relais.
	 */
	const telecharger = useCallback(
		async (url: string, nomFichier: string, poids: number) => {
			if (telechargement) return;
			if (poids > SEUIL_BLOB) {
				window.open(url, "_blank", "noopener");
				return;
			}
			setTelechargement(nomFichier);
			try {
				const res = await fetch(url, { mode: "cors" });
				if (!res.ok) throw new Error(`HTTP ${res.status}`);
				const blob = await res.blob();
				const objet = URL.createObjectURL(blob);
				const a = document.createElement("a");
				a.href = objet;
				a.download = nomFichier;
				document.body.append(a);
				a.click();
				a.remove();
				URL.revokeObjectURL(objet);
			} catch {
				window.open(url, "_blank", "noopener");
			} finally {
				setTelechargement(null);
			}
		},
		[telechargement],
	);

	if (!monte || !ouvert || !film) return null;

	const sortie = formatSortie(film);
	const avecSon = aDuSon(film);
	const duree = formatDuree(film.duree);
	const definition = formatDefinition(film);
	const nomVideo = `${film.nom}.${sortie.ext}`;
	const nomAudio = `${film.nom}.wav`;

	return createPortal(
		<div
			ref={dialogueRef}
			role="dialog"
			aria-modal="true"
			aria-label={`Cinématique : ${film.titre}`}
			className="fixed inset-0 z-[60] flex flex-col bg-surface/95 backdrop-blur-md"
		>
			{/* Barre d'outils : ce qu'on regarde à gauche, ce qu'on peut en faire à droite. */}
			<div className="flex flex-wrap items-center justify-between gap-3 border-b border-outline-variant/30 px-4 py-3">
				<div className="flex min-w-0 flex-1 items-center gap-2">
					<span className="inline-flex shrink-0 items-center gap-1 rounded-full bg-surface-container-high px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-on-surface-variant">
						<Icon name="movie" size={12} />
						{film.rubrique}
					</span>
					<span className="min-w-0">
						<h2 className="truncate text-sm font-bold text-on-surface" title={film.titre}>
							{film.titre}
						</h2>
						<span className="block truncate text-[11px] text-on-surface-variant">
							{film.contexte && `${film.contexte} · `}
							<span className="font-mono">{film.nom}</span>
							{duree && ` · ${duree}`}
							{definition && ` · ${definition}`}
							{` · ${formatOctets(film.octetsServis)}`}
							{film.variante === "haute" && " · HD"}
							{film.cadence != null && film.cadence > 0 && ` · ${film.cadence.toFixed(0)} i/s`}
						</span>
					</span>
				</div>

				<div className="flex shrink-0 items-center gap-2">
					<label
						className="flex cursor-pointer items-center gap-1.5 rounded-full bg-surface-container px-3 py-2 text-xs text-on-surface-variant transition-colors hover:bg-surface-container-high"
						title="À la fin d'un film, passer au suivant de la sélection. Les films qu'aucun navigateur ne décode sont sautés."
					>
						<input
							type="checkbox"
							checked={lectureContinue}
							onChange={(e) => onLectureContinue(e.target.checked)}
							className="size-3.5 accent-primary"
						/>
						<Icon name="skip_next" size={14} className="shrink-0" />
						Lecture continue
					</label>

					<button
						type="button"
						onClick={() =>
							void telecharger(
								videoDownloadUrl(film.cheminServi, sortie.id),
								nomVideo,
								film.octetsServis,
							)
						}
						disabled={telechargement === nomVideo}
						title={`Télécharger le ${sortie.libelle} (remux sans réencodage)`}
						className="inline-flex min-h-11 items-center gap-2 rounded-full bg-primary px-4 py-2 text-sm font-medium text-on-primary transition-colors hover:bg-primary/90 disabled:opacity-50 sm:min-h-0"
					>
						<Icon
							name={telechargement === nomVideo ? "hourglass_top" : "download"}
							size={16}
							className="shrink-0"
						/>
						<span className="hidden sm:inline">{sortie.libelle}</span>
					</button>

					{avecSon && (
						<button
							type="button"
							onClick={() =>
								void telecharger(videoDownloadUrl(film.chemin, "wav"), nomAudio, 0)
							}
							disabled={telechargement === nomAudio}
							title="Télécharger la bande-son décodée (WAV)"
							className="inline-flex size-10 items-center justify-center rounded-full text-on-surface-variant transition-colors hover:bg-surface-container-high disabled:opacity-50"
						>
							<Icon name="music_note" size={18} />
						</button>
					)}

					<a
						href={cpkExplorerHref(film.cheminServi)}
						title="Voir le fichier dans l'explorateur CPK"
						className="inline-flex size-10 items-center justify-center rounded-full text-on-surface-variant transition-colors hover:bg-surface-container-high"
					>
						<Icon name="open_in_new" size={18} />
					</a>

					<button
						ref={fermerRef}
						type="button"
						onClick={onClose}
						aria-label="Fermer le lecteur"
						className="flex size-11 items-center justify-center rounded-full text-on-surface-variant transition-colors hover:bg-surface-container-high sm:size-10"
					>
						<Icon name="close" size={20} />
					</button>
				</div>
			</div>

			{/* Le lecteur et son détail. Un clic dans la marge ferme ; un clic sur le lecteur non. */}
			{/* biome-ignore lint/a11y/noStaticElementInteractions: fermeture au clic sur le fond, doublée par Échap et par le bouton ✕ */}
			<div
				className="relative flex-1 overflow-y-auto p-4 sm:p-6"
				onClick={(e) => {
					if (e.target === e.currentTarget) onClose();
				}}
			>
				<div className="mx-auto flex w-full max-w-5xl flex-col gap-3">
					{film.lisibleNavigateur ? (
						<LecteurFilm key={film.chemin} film={film} auto={auto} onFini={auFini} />
					) : (
						<PaveIllisible film={film} />
					)}

					{avecSon && film.bandeSon && (
						<p className="px-1 text-[11px] text-on-surface-variant">
							<Icon name="music_note" size={11} className="mr-1 inline-block align-[-1px]" />
							Bande-son servie à part depuis <span className="font-mono">anime_stream</span> (cue{" "}
							<span className="font-mono">{film.bandeSon.cue}</span>) : le conteneur du film est
							muet, l'image et le son sont deux flux recalés l'un sur l'autre.
						</p>
					)}
					{!avecSon && (
						<p className="px-1 text-[11px] text-on-surface-variant">
							Aucune bande-son identifiée pour ce film — ni dans son conteneur, ni dans{" "}
							<span className="font-mono">anime_stream</span>.
						</p>
					)}

					<FilmDetails film={film} />
				</div>

				{/* Flèche précédent. */}
				{avantDisponible && (
					<button
						type="button"
						aria-label="Cinématique précédente"
						onClick={() => naviguer(index - 1)}
						className="absolute left-1 top-1/2 flex size-11 -translate-y-1/2 items-center justify-center rounded-full bg-surface-container/80 text-on-surface backdrop-blur-sm transition-colors hover:bg-surface-container-high sm:left-2"
					>
						<Icon name="chevron_left" size={24} />
					</button>
				)}
				{/* Flèche suivant. */}
				{apresDisponible && (
					<button
						type="button"
						aria-label="Cinématique suivante"
						onClick={() => naviguer(index + 1)}
						className="absolute right-1 top-1/2 flex size-11 -translate-y-1/2 items-center justify-center rounded-full bg-surface-container/80 text-on-surface backdrop-blur-sm transition-colors hover:bg-surface-container-high sm:right-2"
					>
						<Icon name="chevron_right" size={24} />
					</button>
				)}
			</div>

			{/* Rang dans la sélection — pas dans le corpus : c'est la liste filtrée qu'on parcourt. */}
			<div className="border-t border-outline-variant/30 px-4 py-2 text-center text-xs font-medium text-on-surface-variant">
				{index + 1} / {films.length}
			</div>
		</div>,
		document.body,
	);
}
