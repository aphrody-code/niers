"use client";

import { CircleDot, Sparkles } from "lucide-react";
import { Image } from "@/components/ui/image";
import { Link } from "@/components/ui/link";
import { useState } from "react";
// PAS `@rosegriffon/ui` : cette bibliothèque est WEB, son `lib/adsense.ts` lit `process.env`, qui
// n'existe pas dans une WebView. L'importer levait un `ReferenceError: process is not defined` au
// montage et rendait TOUTE l'application blanche. Les composants locaux ont la même API.
import { Badge } from "@/components/ui/badge";
import { CommonSpriteIcon } from "@/components/ui/CommonSpriteIcon";
import { ElementIcon } from "@/components/wiki/ElementIcon";
import type { SpriteCommonKey } from "@/config/sprites-common";
import { getAuraImageUrl, resolveAssetUrl } from "@/lib/wikiImages";
import { cn } from "@/lib/utils";

export interface AuraCardProps {
	id: string;
	name: string;
	description?: string;
	element?: { en?: string; ja?: string; fr?: string };
	image?: string;
	assetCode?: string;
	subType?: string;
	category: string;
	passiveEffect?: string;
	hissatsuName?: string;
	className?: string;
}

const SUBTYPE_COLORS: Record<string, string> = {
	Aura: "bg-blue-600/80 text-white",
	Awakening: "bg-secondary/80 text-on-secondary",
	Keshin: "bg-primary/80 text-on-primary",
	Miximax: "bg-primary/80 text-on-primary",
	ModeChange: "bg-error/80 text-white",
	Soul: "bg-tertiary/80 text-on-tertiary",
};

const SUBTYPE_LABELS: Record<string, string> = {
	Aura: "Aura",
	Awakening: "Éveil",
	Keshin: "Esprit Guerrier",
	Miximax: "Miximax",
	ModeChange: "Mode",
	Soul: "Totem",
};

const SUBTYPE_SPRITES: Record<string, SpriteCommonKey> = {
	Awakening: "eveil",
	Keshin: "keshin",
	Miximax: "miximax",
	ModeChange: "mode_change",
	Soul: "soul",
};

export function AuraCard({
	id,
	name,
	element,
	image,
	assetCode,
	subType = "Aura",
	category,
	passiveEffect,
	hissatsuName,
	className,
}: AuraCardProps) {
	const colorClass = SUBTYPE_COLORS[subType] || SUBTYPE_COLORS.Aura;
	const label = SUBTYPE_LABELS[subType] || subType;
	const [imgError, setImgError] = useState(false);

	const imageUrl = resolveAssetUrl(image) || getAuraImageUrl(assetCode, subType);

	return (
		<Link href={`/aura/${category}/${id}`} className={cn("block h-full", className)}>
			<div
				className={cn(
					"group relative flex flex-col rounded-xl overflow-hidden border transition-all duration-200",
					"hover:shadow-lg hover:-translate-y-0.5 active:scale-[0.98]",
					"bg-neutral-950 border-white/10 h-full"
				)}
			>
				{/* Image area */}
				<div className="relative w-full aspect-[16/9] overflow-hidden bg-neutral-900">
					{imageUrl && !imgError ? (
						<Image
							src={imageUrl}
							alt={name}
							fill
							className="object-contain group-hover:scale-105 transition-transform duration-300"
							sizes="(max-width: 640px) 50vw, (max-width: 1024px) 33vw, 25vw"
							onError={() => setImgError(true)}
							unoptimized
						/>
					) : (
						<div className="absolute inset-0 flex items-center justify-center opacity-15">
							{SUBTYPE_SPRITES[subType] ? (
								<CommonSpriteIcon name={SUBTYPE_SPRITES[subType]} scale={0.7} />
							) : (
								<Sparkles size={48} className="text-white" aria-hidden="true" />
							)}
						</div>
					)}

					{/* Top-left: type badge */}
					<div className="absolute top-1.5 left-1.5">
						<Badge
							className={cn(
								colorClass,
								"border-0 text-[8px] font-black uppercase tracking-wide px-1.5 py-0.5 rounded-sm leading-none"
							)}
						>
							{label}
						</Badge>
					</div>

					{/* Top-right: element */}
					{element?.en && (
						<div className="absolute top-1.5 right-1.5 drop-shadow-lg">
							<ElementIcon element={element.en} size="sm" />
						</div>
					)}

					{/* Bottom gradient */}
					<div className="absolute inset-x-0 bottom-0 h-12 bg-linear-to-t from-neutral-950 to-transparent" />
				</div>

				{/* Info area */}
				<div className="px-3 py-2 flex flex-col gap-1 bg-neutral-950 grow">
					<h3 className="text-xs sm:text-sm font-bold text-white leading-tight line-clamp-2">
						{name}
					</h3>

					{passiveEffect && (
						<p className="text-[10px] text-tertiary/80 font-medium line-clamp-1">
							<Sparkles size={10} className="inline align-middle mr-0.5" aria-hidden="true" />
							{passiveEffect}
						</p>
					)}
					{hissatsuName && (
						<p className="text-[10px] text-primary/80 font-medium line-clamp-1">
							<CircleDot size={10} className="inline align-middle mr-0.5" aria-hidden="true" />
							{hissatsuName}
						</p>
					)}
				</div>
			</div>
		</Link>
	);
}
