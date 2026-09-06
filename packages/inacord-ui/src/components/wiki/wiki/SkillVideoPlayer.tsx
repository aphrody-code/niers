"use client";

import { Play } from "lucide-react";
import { useId, useState } from "react";
import { cn } from "../../../lib/utils";

/**
 * Une variante vidéo d'une technique, telle que publiée par zukan.inazuma.jp.
 *
 * Îlot client : ne reçoit que des données sérialisables (aucun module serveur
 * importé ici). Les libellés viennent de la source — « Shot », « Success »,
 * « Failure », « Defence », « Offence », « Shot Block » — et ne sont traduits
 * qu'à l'affichage.
 */
export interface SkillVideoVariant {
	label: string;
	position: number;
	videoUrl: string;
	posterUrl: string | null;
}

/** Les 6 libellés possibles côté zukan ; tout autre libellé est affiché tel quel. */
const VARIANT_FR: Record<string, string> = {
	Defence: "Défense",
	Failure: "Échec",
	Movie: "Vidéo",
	Offence: "Attaque",
	"Shot Block": "Contre-tir",
	Shot: "Tir",
	Success: "Réussite",
};

interface SkillVideoPlayerProps {
	videos: SkillVideoVariant[];
	skillName: string;
	/** Image de repli quand la variante n'a pas de poster (icône de la technique). */
	fallbackPoster?: string;
	className?: string;
	/**
	 * Variante affichée, quand le parent doit la connaître (le bouton
	 * « Télécharger » de la fiche vit hors du lecteur et doit suivre l'onglet
	 * choisi). Sans ces deux props, le lecteur gère son onglet tout seul.
	 */
	activeIndex?: number;
	onActiveIndexChange?: (index: number) => void;
}

export function SkillVideoPlayer({
	videos,
	skillName,
	fallbackPoster,
	className,
	activeIndex: controlledIndex,
	onActiveIndexChange,
}: SkillVideoPlayerProps) {
	const [internalIndex, setInternalIndex] = useState(0);
	const [failed, setFailed] = useState<Record<number, boolean>>({});
	// Préfixe stable et unique, côté serveur comme côté client : deux lecteurs sur
	// la même page ne doivent pas partager les identifiants de leurs onglets.
	const idBase = useId();

	const activeIndex = controlledIndex ?? internalIndex;
	const setActiveIndex = (index: number) => {
		setInternalIndex(index);
		onActiveIndexChange?.(index);
	};

	const active = videos[activeIndex];
	if (!active) {
		return null;
	}

	const poster = active.posterUrl || fallbackPoster || undefined;
	const variantFr = VARIANT_FR[active.label] || active.label;

	const multiple = videos.length > 1;
	const idOnglet = (i: number) => `${idBase}-onglet-${i}`;
	const idPanneau = `${idBase}-panneau`;

	return (
		<div className={cn("space-y-2", className)}>
			{/* Le cadre vidéo EST le panneau des onglets : sans `role="tabpanel"` ni
			    `aria-controls`, un `role="tablist"` promet une relation qu'aucun
			    lecteur d'écran ne peut suivre — l'utilisateur active un onglet sans
			    savoir ce qui a changé, ni où aller le lire. */}
			<div
				id={multiple ? idPanneau : undefined}
				role={multiple ? "tabpanel" : undefined}
				aria-labelledby={multiple ? idOnglet(activeIndex) : undefined}
				tabIndex={multiple ? 0 : undefined}
				className="w-full aspect-video relative rounded-2xl overflow-hidden shadow-lg border border-outline-variant/20 bg-surface-container-highest"
			>
				{failed[activeIndex] ? (
					<div className="absolute inset-0 flex flex-col items-center justify-center gap-2 text-on-surface-variant">
						<Play size={40} aria-hidden="true" />
						<p className="text-sm">Vidéo indisponible</p>
					</div>
				) : (
					<video
						// `key` force le remontage au changement de variante : sans lui le
						// navigateur garde la source précédente et ignore le nouveau poster.
						key={active.videoUrl}
						src={active.videoUrl}
						poster={poster}
						controls
						playsInline
						// Rien n'est téléchargé tant que le visiteur n'a pas cliqué : une
						// technique pèse ~1,5 Mo, et la fiche s'ouvre le plus souvent pour
						// ses statistiques, pas pour sa vidéo.
						preload="none"
						className="size-full object-contain"
						aria-label={
							videos.length > 1
								? `Vidéo de la technique ${skillName} — ${variantFr}`
								: `Vidéo de la technique ${skillName}`
						}
						onError={() => setFailed((f) => ({ ...f, [activeIndex]: true }))}
					>
						Votre navigateur ne prend pas en charge la lecture de vidéos.
					</video>
				)}
			</div>

			{/* Sélecteur de variante — 236 techniques sur 895 en ont deux (réussite/échec,
			    défense/contre-tir). L'onglet n'apparaît que s'il y a un choix à faire. */}
			{multiple && (
				<div className="flex flex-wrap gap-2" role="tablist" aria-label={`Variantes de ${skillName}`}>
					{videos.map((v, i) => (
						<button
							key={v.videoUrl}
							id={idOnglet(i)}
							type="button"
							role="tab"
							aria-selected={i === activeIndex}
							aria-controls={idPanneau}
							// Un `tablist` ne compte qu'un seul arrêt de tabulation : la flèche
							// change d'onglet, la tabulation en sort vers le panneau.
							tabIndex={i === activeIndex ? 0 : -1}
							onKeyDown={(e) => {
								if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") {
									return;
								}
								e.preventDefault();
								const pas = e.key === "ArrowRight" ? 1 : -1;
								const suivant = (activeIndex + pas + videos.length) % videos.length;
								setActiveIndex(suivant);
								document.getElementById(idOnglet(suivant))?.focus();
							}}
							onClick={() => setActiveIndex(i)}
							className={cn(
								"inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm font-medium transition-colors min-h-11 sm:min-h-9",
								i === activeIndex
									? "bg-primary text-on-primary"
									: "bg-surface-container-highest text-on-surface-variant hover:bg-surface-container-high"
							)}
						>
							<Play size={16} aria-hidden="true" />
							{VARIANT_FR[v.label] || v.label}
						</button>
					))}
				</div>
			)}
		</div>
	);
}
