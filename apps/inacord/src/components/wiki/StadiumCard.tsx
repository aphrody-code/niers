"use client";

import { LandPlot } from "lucide-react";
import { Image } from "@/components/ui/image";
import { Link } from "@/components/ui/link";
import { useState } from "react";
import { cn } from "@/lib/utils";

export interface StadiumCardProps {
	id: string;
	code: string;
	title: string;
	index?: number | null;
	/** Vignette WebP w=400 (illustration in-game du terrain). */
	thumb?: string | null;
	className?: string;
}

/**
 * Carte de stade : la VRAIE illustration in-game du terrain (ratio paysage 16/9)
 * au premier plan, sous un libellé générique honnête (« Terrain N ») et le code
 * interne en sous-titre technique. Fallback propre (icône `LandPlot`) si l'image
 * est absente ou 404. AUCUN nom de stade ni nom d'équipe n'est affiché : la table
 * `inagle_stadiums` n'en contient pas, on n'invente donc rien.
 */
export function StadiumCard({ id, code, title, index, thumb, className }: StadiumCardProps) {
	const [imgError, setImgError] = useState(false);
	const hasImage = !!thumb && !imgError;

	return (
		<Link
			href={`/stade/${id}`}
			className={cn(
				"group block h-full overflow-hidden rounded-2xl",
				"border border-outline-variant/30 bg-surface-container-low",
				"transition-all duration-200 hover:-translate-y-0.5 hover:shadow-lg hover:bg-surface-container",
				className
			)}
		>
			<div className="relative flex aspect-video w-full items-center justify-center overflow-hidden bg-surface-container-high">
				{hasImage ? (
					<Image
						src={thumb as string}
						alt={title}
						fill
						sizes="(max-width: 640px) 100vw, (max-width: 1024px) 50vw, 33vw"
						className="object-cover transition-transform duration-300 group-hover:scale-105"
						unoptimized
						onError={() => setImgError(true)}
					/>
				) : (
					<LandPlot size={32} className="text-on-surface-variant/25" aria-hidden="true" />
				)}
			</div>
			<div className="p-3">
				<div className="flex items-center justify-between gap-2">
					<span className="text-[10px] font-bold uppercase tracking-wider text-on-surface-variant/60">
						Stade
					</span>
					{typeof index === "number" && (
						<span className="text-[10px] font-mono text-on-surface-variant/50">#{index}</span>
					)}
				</div>
				<h3 className="line-clamp-1 text-sm font-bold leading-tight text-on-surface transition-colors group-hover:text-primary">
					{title}
				</h3>
				<p className="mt-0.5 line-clamp-1 font-mono text-[10px] text-on-surface-variant/50">
					{code}
				</p>
			</div>
		</Link>
	);
}
