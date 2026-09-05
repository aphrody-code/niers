"use client";

import { Image } from "@/components/ui/image";
import { Link } from "@/components/ui/link";
import { useState } from "react";
import { Icon } from "@/components/ui/Icon";
import type { DropEntry } from "@/lib/wikiTypes";
import { dropCategoryLabel, dropSourceLabel } from "@/lib/wikiTypes";
import { getItemIconUrl, PLACEHOLDERS, resolveAssetUrl } from "@/lib/wikiImages";
import { cn } from "@/lib/utils";

export interface DropsCardProps {
	drop: DropEntry;
	className?: string;
}

const SOURCE_ICON: Record<DropEntry["source"], string> = {
	win_treasure: "stars",
	item_emission: "auto_awesome",
};

/** Pastille de probabilité teintée selon la rareté du taux (plus rare = plus chaud). */
function chanceTone(chance: number): string {
	if (chance >= 20) {
		return "bg-primary/10 text-primary";
	}
	if (chance >= 5) {
		return "bg-tertiary/10 text-tertiary";
	}
	return "bg-error/10 text-error";
}

/** Carte d'un drop concret : objet nommé + source + taux réel (poids normalisé). */
export function DropsCard({ drop, className }: DropsCardProps) {
	const [imgError, setImgError] = useState(false);
	const resolvedSrc =
		resolveAssetUrl(drop.itemImageUrl) ||
		(drop.itemInternalCode ? getItemIconUrl(drop.itemInternalCode) : null);
	const hasRealImage = !!resolvedSrc && resolvedSrc !== PLACEHOLDERS.item && !imgError;

	// Les Hyper Tactiques sont des objets `super_tactics`/`special_tactics` → fiche /tactic,
	// tout le reste → fiche /item (les deux acceptent l'id hash de l'objet).
	const isTactic =
		drop.itemCategory === "special_tactics" || drop.itemCategory === "super_tactics";
	const href = isTactic ? `/tactic/${drop.itemId}` : `/item/${drop.itemId}`;

	return (
		<Link
			href={href}
			className={cn(
				"block h-full transition-all duration-200 relative overflow-hidden group",
				"hover:shadow-lg hover:-translate-y-0.5",
				"border border-outline-variant/30 bg-surface-container-low hover:bg-surface-container",
				"rounded-2xl",
				className
			)}
		>
			<div className="p-4 flex gap-4">
				{/* Icône objet */}
				<div className="size-14 shrink-0 rounded-xl bg-surface-container-high flex items-center justify-center border border-outline-variant/20 overflow-hidden">
					{hasRealImage ? (
						<Image
							src={resolvedSrc as string}
							alt={drop.itemName}
							width={48}
							height={48}
							className="object-contain size-11"
							unoptimized
							onError={() => setImgError(true)}
						/>
					) : (
						<Icon name="backpack" size={24} className="text-on-surface-variant/30" />
					)}
				</div>

				{/* Contenu */}
				<div className="flex-1 min-w-0">
					<div className="flex items-start justify-between gap-2">
						<h3 className="font-bold text-sm text-on-surface leading-tight group-hover:text-primary transition-colors line-clamp-2">
							{drop.itemName}
						</h3>
						<span
							className={cn(
								"shrink-0 inline-flex items-center px-2 py-0.5 rounded-full text-xs font-extrabold tabular-nums",
								chanceTone(drop.chance)
							)}
							title={`Poids ${drop.weight} dans le groupe`}
						>
							{drop.chance.toLocaleString("fr-FR", { maximumFractionDigits: 1 })}%
						</span>
					</div>

					{/* Méta : source + catégorie */}
					<div className="flex flex-wrap items-center gap-1.5 mt-1.5">
						<span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-surface-container-highest/50 text-[10px] font-bold text-on-surface-variant uppercase tracking-wider">
							<Icon name={SOURCE_ICON[drop.source]} size={12} />
							{dropSourceLabel(drop.source)}
						</span>
						{drop.itemCategory && (
							<span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-secondary/10 text-[10px] font-bold text-secondary">
								{dropCategoryLabel(drop.itemCategory)}
							</span>
						)}
					</div>

					{/* Groupe source (hash) */}
					<p
						className="mt-2 text-[10px] text-on-surface-variant/70 font-mono truncate"
						title={drop.sourceId}
					>
						Groupe {drop.sourceId}
					</p>
				</div>
			</div>
		</Link>
	);
}
