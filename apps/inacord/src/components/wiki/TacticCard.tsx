"use client";

import { Map } from "lucide-react";
import { Image } from "@/components/ui/image";
import { Link } from "@/components/ui/link";
import { useState } from "react";
import { cn } from "@/lib/utils";

export interface TacticCardProps {
	id: string;
	name: string;
	/** Bannière telop in-game (`220_img/telop_waza/...`, ratio ~4.9:1). */
	image?: string | null;
	categoryLabel?: string;
	className?: string;
}

/**
 * Carte de tactique : l'« icône » d'une tactique est en réalité une bannière telop
 * large (1728×352, ~4.9:1) — la rendre dans un slot carré l'écrase jusqu'à la rendre
 * invisible. On l'affiche donc dans une zone bannière (`aspect-[24/5]`), pas un carré.
 * Fallback propre (icône `Map`) si l'image est absente ou 404.
 */
export function TacticCard({
	id,
	name,
	image,
	categoryLabel = "Tactique",
	className,
}: TacticCardProps) {
	const [imgError, setImgError] = useState(false);
	const hasImage = !!image && !imgError;

	return (
		<Link
			href={`/tactic/${id}`}
			className={cn(
				"group block h-full overflow-hidden rounded-2xl",
				"border border-outline-variant/30 bg-surface-container-low",
				"transition-all duration-200 hover:-translate-y-0.5 hover:shadow-lg hover:bg-surface-container",
				className
			)}
		>
			<div className="relative flex aspect-[24/5] w-full items-center justify-center overflow-hidden bg-surface-container-high">
				{hasImage ? (
					<Image
						src={image as string}
						alt={name}
						fill
						sizes="(max-width: 640px) 100vw, (max-width: 1024px) 50vw, 25vw"
						className="object-contain p-2 transition-transform duration-300 group-hover:scale-105"
						unoptimized
						onError={() => setImgError(true)}
					/>
				) : (
					<Map size={32} className="text-on-surface-variant/25" aria-hidden="true" />
				)}
			</div>
			<div className="p-3">
				<span className="text-[10px] font-bold uppercase tracking-wider text-on-surface-variant/60">
					{categoryLabel}
				</span>
				<h3 className="line-clamp-1 text-sm font-bold leading-tight text-on-surface transition-colors group-hover:text-primary">
					{name}
				</h3>
			</div>
		</Link>
	);
}
