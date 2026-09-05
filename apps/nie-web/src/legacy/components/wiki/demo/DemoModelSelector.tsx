/**
 * Sélecteur MONO-SÉLECTION des modèles chargeables dans la scène héro (formes + cut-ins 3D).
 *
 * Présentationnel pur (aucun import lourd) : remonte juste `onSelect(id)` au parent
 * `DemoExperience`. La contrainte d'une seule option active à la fois vient de
 * `CutinScene.fitObject` (chaque GLB normalisé individuellement → pas de composition spatiale).
 */
import type { DemoModelOption } from "./types";

export function DemoModelSelector({
	options,
	selectedId,
	onSelect,
}: {
	options: DemoModelOption[];
	selectedId: string;
	onSelect: (id: string) => void;
}) {
	return (
		<div className="grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-6">
			{options.map((o) => {
				const active = o.id === selectedId;
				return (
					<button
						key={o.id}
						type="button"
						onClick={() => onSelect(o.id)}
						title={o.label}
						className={`group relative aspect-square overflow-hidden rounded-xl border-2 transition-colors ${
							active
								? "border-primary"
								: "border-transparent bg-surface-container-high hover:border-outline-variant"
						}`}
					>
						{o.iconUrl ? (
							// eslint-disable-next-line @next/next/no-img-element
							<img
								src={o.iconUrl}
								alt={o.label}
								loading="lazy"
								className="absolute inset-0 size-full object-cover [image-rendering:pixelated]"
							/>
						) : (
							<span className="absolute inset-0 flex items-center justify-center text-xs text-on-surface-variant">
								{o.kind === "waza" ? "Cut-in" : "Forme"}
							</span>
						)}
						<span
							className={`absolute inset-x-0 bottom-0 truncate px-1.5 py-0.5 text-[10px] font-medium ${
								active
									? "bg-primary text-on-primary"
									: "bg-surface/70 text-on-surface-variant backdrop-blur"
							}`}
						>
							{o.label}
						</span>
					</button>
				);
			})}
		</div>
	);
}
