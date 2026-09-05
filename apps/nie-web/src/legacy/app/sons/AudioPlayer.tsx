"use client";

/**
 * Lecteur audio de la galerie sonore.
 *
 * L'ancien rendu se contentait de repointer un `<audio>` caché : aucune progression, aucun
 * moyen de se déplacer dans la piste, rien qui dise ce qui joue. Un fichier de 63 secondes se
 * jouait donc en aveugle.
 *
 * Ce lecteur est une barre fixe en bas de page : lecture/pause, curseur de position, temps
 * écoulé et durée, passage au cue précédent/suivant DANS la banque ouverte, et téléchargement
 * de la piste en cours. Il ne s'affiche que lorsqu'une piste est chargée.
 *
 * Le décodage HCA/ADX se fait côté serveur (`/audio?id=`) : le navigateur ne reçoit qu'un WAV,
 * donc le `<audio>` natif suffit et le seek est immédiat.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { cpkAudioCueUrl, formatDuration } from "@rosegriffon/azalee/cpk/audio";
import { exportUrl } from "@rosegriffon/azalee/cpk/live";

/** La piste en cours de lecture. */
export interface Piste {
	/** Chemin VFS de la banque. */
	path: string;
	/** Identifiant AWB du cue — c'est lui qu'on passe à la route, pas le rang. */
	awbId: number | null;
	/** Nom du cue, ou son rang à défaut. */
	label: string;
	/** Nom de la banque, pour le contexte. */
	banque: string;
	/** Durée annoncée par la banque, en secondes. */
	duree: number;
}

/**
 * @param piste ce qui doit jouer, `null` pour masquer le lecteur.
 * @param onNavigate demande le cue précédent (`-1`) ou suivant (`+1`) dans la banque ouverte.
 */
export function AudioPlayer({
	piste,
	onNavigate,
	onClose,
}: {
	piste: Piste | null;
	onNavigate?: (delta: number) => void;
	onClose?: () => void;
}) {
	const ref = useRef<HTMLAudioElement | null>(null);
	const [joue, setJoue] = useState(false);
	const [position, setPosition] = useState(0);
	const [duree, setDuree] = useState(0);
	const [chargement, setChargement] = useState(false);

	// Changement de piste : on recharge et on lance. `autoPlay` ne suffit pas — la source
	// change sans que l'élément soit remonté.
	useEffect(() => {
		const el = ref.current;
		if (!el || !piste) return;
		setPosition(0);
		setDuree(piste.duree || 0);
		setChargement(true);
		el.src = cpkAudioCueUrl(piste.path, piste.awbId);
		el.load();
		void el
			.play()
			.then(() => setJoue(true))
			.catch(() => setJoue(false));
	}, [piste]);

	const basculer = useCallback(() => {
		const el = ref.current;
		if (!el) return;
		if (el.paused) {
			void el.play().then(() => setJoue(true)).catch(() => setJoue(false));
		} else {
			el.pause();
			setJoue(false);
		}
	}, []);

	const chercher = useCallback((valeur: number) => {
		const el = ref.current;
		if (!el || !Number.isFinite(el.duration)) return;
		el.currentTime = valeur;
		setPosition(valeur);
	}, []);

	if (!piste) return null;

	// `duration` du WAV décodé fait autorité ; `piste.duree` (annoncée par la banque) sert de
	// repli tant que les métadonnées ne sont pas chargées.
	const total = duree > 0 ? duree : piste.duree;

	return (
		<div className="sticky bottom-0 z-30 -mx-1 mt-4 rounded-t-2xl border border-outline-variant bg-surface-container-high/95 px-4 py-3 backdrop-blur">
			{/* eslint-disable-next-line jsx-a11y/media-has-caption -- flux audio du jeu, sans dialogue transcrit */}
			<audio
				ref={ref}
				preload="metadata"
				onLoadedMetadata={(e) => {
					const d = e.currentTarget.duration;
					if (Number.isFinite(d) && d > 0) setDuree(d);
					setChargement(false);
				}}
				onTimeUpdate={(e) => setPosition(e.currentTarget.currentTime)}
				onEnded={() => setJoue(false)}
				onError={() => {
					setChargement(false);
					setJoue(false);
				}}
			/>

			<div className="flex items-center gap-3">
				<button
					type="button"
					onClick={basculer}
					aria-label={joue ? "Pause" : "Lecture"}
					className="flex size-10 shrink-0 items-center justify-center rounded-full bg-primary text-on-primary transition hover:opacity-90"
				>
					<Icon name={joue ? "pause" : "play_arrow"} size={22} />
				</button>

				{onNavigate && (
					<span className="hidden shrink-0 gap-1 sm:flex">
						<button
							type="button"
							onClick={() => onNavigate(-1)}
							aria-label="Cue précédent"
							className="flex size-8 items-center justify-center rounded-full text-on-surface-variant transition hover:bg-surface-container hover:text-on-surface"
						>
							<Icon name="skip_previous" size={18} />
						</button>
						<button
							type="button"
							onClick={() => onNavigate(1)}
							aria-label="Cue suivant"
							className="flex size-8 items-center justify-center rounded-full text-on-surface-variant transition hover:bg-surface-container hover:text-on-surface"
						>
							<Icon name="skip_next" size={18} />
						</button>
					</span>
				)}

				<span className="min-w-0 flex-1">
					<span className="block truncate text-sm font-medium text-on-surface">
						{piste.label}
					</span>
					<span className="block truncate text-[11px] text-on-surface-variant">
						{piste.banque}
						{chargement && " · décodage…"}
					</span>
					<input
						type="range"
						min={0}
						max={total > 0 ? total : 1}
						step={0.01}
						value={Math.min(position, total || 0)}
						onChange={(e) => chercher(Number(e.target.value))}
						aria-label="Position dans la piste"
						className="mt-1 h-1 w-full cursor-pointer appearance-none rounded-full bg-outline-variant accent-primary"
					/>
				</span>

				<span className="shrink-0 tabular-nums text-xs text-on-surface-variant">
					{formatDuration(position)} / {formatDuration(total)}
				</span>

				<a
					href={exportUrl(piste.path, "wav", { awbId: piste.awbId })}
					download
					aria-label="Télécharger cette piste en WAV"
					className="flex size-9 shrink-0 items-center justify-center rounded-full text-on-surface-variant transition hover:bg-surface-container hover:text-on-surface"
				>
					<Icon name="download" size={18} />
				</a>

				{onClose && (
					<button
						type="button"
						onClick={onClose}
						aria-label="Fermer le lecteur"
						className="flex size-9 shrink-0 items-center justify-center rounded-full text-on-surface-variant transition hover:bg-surface-container hover:text-on-surface"
					>
						<Icon name="close" size={18} />
					</button>
				)}
			</div>
		</div>
	);
}
