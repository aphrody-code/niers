"use client";

import { Store } from "lucide-react";
import { Image } from "@/components/ui/image";
import { Link } from "@/components/ui/link";
import { useState } from "react";
import { CommonSpriteIcon } from "@/components/ui/CommonSpriteIcon";
import type { SpriteCommonKey } from "@/config/sprites-common";
import { getSkillImageUrl } from "@/lib/wikiImages";
import { cn } from "@/lib/utils";

const ELEMENT_SPRITE: Record<string, SpriteCommonKey> = {
	Feu: "fire",
	Fire: "fire",
	Forest: "forest",
	Forêt: "forest",
	Montagne: "mountain",
	Mountain: "mountain",
	Vent: "wind",
	Wind: "wind",
};

const ELEMENT_ACCENT: Record<string, string> = {
	Feu: "border-red-500/40",
	Fire: "border-red-500/40",
	Forest: "border-green-500/40",
	Forêt: "border-green-500/40",
	Montagne: "border-amber-500/40",
	Mountain: "border-amber-500/40",
	Néant: "border-purple-500/40",
	Vent: "border-teal-500/40",
	Void: "border-purple-500/40",
	Wind: "border-teal-500/40",
};

const CAT_LABEL: Record<string, string> = {
	Arrêt: "GAR",
	Block: "DEF",
	Catch: "GAR",
	Dribble: "DRI",
	Défense: "DEF",
	Shoot: "TIR",
	Tir: "TIR",
};

const CAT_COLOR: Record<string, string> = {
	Arrêt: "bg-amber-500",
	Block: "bg-blue-600",
	Catch: "bg-amber-500",
	Dribble: "bg-emerald-600",
	Défense: "bg-blue-600",
	Shoot: "bg-red-600",
	Tir: "bg-red-600",
};

export interface MoveCardProps {
	id: string;
	name: string;
	powerMin?: number;
	powerMax?: number;
	tensionCost?: number;
	element?: string;
	category: string;
	className?: string;
	videoUrl?: string;
	posterUrl?: string;
	/** Vignette webp zukan (~6 Ko) — préférée au poster jpg (~70 Ko) dans la grille. */
	thumbnailUrl?: string;
	shop?: string;
}

export function MoveCard({
	id,
	name,
	powerMin: _powerMin,
	powerMax,
	tensionCost,
	element,
	category,
	className,
	videoUrl,
	posterUrl,
	thumbnailUrl,
	shop,
}: MoveCardProps) {
	const [imgError, setImgError] = useState(false);

	const telopUrl = getSkillImageUrl(id);
	// Vignette webp d'abord : ~6 Ko contre ~70 Ko pour le poster jpg, soit une
	// grille de 60 techniques à ~0,4 Mo au lieu de ~4 Mo.
	const displayImage = thumbnailUrl || posterUrl || telopUrl;
	const elSprite = element ? ELEMENT_SPRITE[element] : undefined;
	const borderAccent = element
		? ELEMENT_ACCENT[element] || "border-outline-variant/20"
		: "border-outline-variant/20";
	const catLabel = CAT_LABEL[category] || category?.slice(0, 3).toUpperCase();
	const catColor = CAT_COLOR[category] || "bg-slate-600";

	return (
		<Link
			href={`/skill/${id}`}
			className={cn(
				"group relative flex flex-col rounded-xl overflow-hidden border transition-all duration-200",
				"hover:shadow-lg hover:-translate-y-0.5 active:scale-[0.98]",
				"bg-surface-container-highest",
				borderAccent,
				className
			)}
		>
			{/* Thumbnail image (poster > telop fallback) */}
			<div className="relative w-full aspect-[16/9] overflow-hidden bg-surface-container-high">
				{!imgError ? (
					<Image
						src={displayImage}
						alt={name}
						fill
						className={cn(
							"object-cover transition-transform duration-300",
							"group-hover:scale-105"
						)}
						sizes="(max-width: 640px) 50vw, (max-width: 1024px) 33vw, 25vw"
						onError={() => setImgError(true)}
						unoptimized
					/>
				) : (
					<div className="absolute inset-0 flex items-center justify-center">
						<span className="text-on-surface/20 text-3xl font-black">{catLabel}</span>
					</div>
				)}

				{/* Play button centered (only if video exists) */}
				{videoUrl && (
					<div className="absolute inset-0 flex items-center justify-center pointer-events-none">
						<div
							className={cn(
								"flex items-center justify-center size-10 sm:w-12 sm:h-12 rounded-full",
								"bg-black/50 backdrop-blur-sm border border-white/20",
								"transition-all duration-200",
								"group-hover:bg-black/70 group-hover:scale-110"
							)}
						>
							<svg viewBox="0 0 24 24" fill="white" className="size-5 sm:w-6 sm:h-6 ml-0.5">
								<path d="M8 5v14l11-7z" />
							</svg>
						</div>
					</div>
				)}

				{/* Top-left: category pill */}
				<div className="absolute top-1 left-1 flex items-center gap-1">
					<span
						className={cn(
							"px-1.5 py-0.5 rounded-sm text-[8px] font-black text-on-surface leading-none tracking-wide",
							catColor
						)}
					>
						{catLabel}
					</span>
				</div>

				{/* Top-right: element sprite */}
				{elSprite && (
					<div className="absolute top-1 right-1 drop-shadow-lg">
						<CommonSpriteIcon name={elSprite} scale={0.35} />
					</div>
				)}

				{/* Bottom gradient */}
				<div className="absolute inset-x-0 bottom-0 h-10 bg-linear-to-t from-neutral-950 to-transparent" />
			</div>

			{/* Name + stats bar */}
			<div className="px-2 py-1.5 flex flex-col gap-0.5 min-w-0 bg-surface-container-highest">
				<div className="flex items-center justify-between gap-1 min-w-0">
					<p className="text-[11px] sm:text-xs font-bold text-on-surface truncate leading-tight flex-1 min-w-0">
						{name}
					</p>
					<div className="flex items-center gap-1.5 shrink-0">
						{tensionCost != null && tensionCost > 0 && (
							<span className="text-[9px] font-bold text-primary/80 tabular-nums">
								{tensionCost}TP
							</span>
						)}
						{powerMax != null && powerMax > 0 && (
							<span className="text-[9px] font-mono font-bold text-on-surface/60 tabular-nums">
								{powerMax}
							</span>
						)}
					</div>
				</div>
				{shop && (
					<div className="flex items-center gap-1 min-w-0">
						<Store size={10} className="text-on-surface/30" aria-hidden="true" />
						<span className="text-[9px] text-on-surface/40 truncate leading-tight">{shop}</span>
					</div>
				)}
			</div>
		</Link>
	);
}
