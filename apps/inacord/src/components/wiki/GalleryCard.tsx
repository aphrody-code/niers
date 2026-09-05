"use client";

import { Image } from "@/components/ui/image";
import { useRef, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

export interface GalleryCardProps {
	id: string;
	title: string;
	/** Vignette WebP légère (w=400) affichée dans la carte. */
	thumb: string | null;
	/** Variante grande (w=1600) préchargée au survol pour un lightbox instantané. */
	full?: string | null;
	categoryLabel?: string;
	categoryIcon?: string;
	className?: string;
	/** Charge l'image en priorité (1re rangée de la grille) — pas de lazy. */
	priority?: boolean;
	/** Ouvre l'illustration en plein écran (lightbox). */
	onOpen?: () => void;
}

/**
 * Carte d'illustration de galerie in-game. Vignette WebP légère (illustration 16:9 du
 * dump dx11, ~4–60 Ko au lieu du PNG ~12 Mo), titre dérivé du `img_path`, chip de
 * catégorie. Cliquable → ouvre le lightbox plein écran via `onOpen`. Au survol,
 * précharge la grande variante (w=1600) pour une ouverture instantanée du lightbox.
 * Fallback propre (icône de catégorie) si l'image est absente ou renvoie 404.
 */
export function GalleryCard({
	thumb,
	full,
	title,
	categoryLabel,
	categoryIcon = "image",
	className,
	priority = false,
	onOpen,
}: GalleryCardProps) {
	const [imgError, setImgError] = useState(false);
	const prefetchedRef = useRef(false);
	const hasImage = !!thumb && !imgError;

	// Préchargement au survol : crée une <img> détachée pour amorcer le cache navigateur
	// avec la GRANDE variante (w=1600) que le lightbox affichera — ouverture instantanée.
	const prefetch = () => {
		const target = full ?? thumb;
		if (prefetchedRef.current || !target) {
			return;
		}
		prefetchedRef.current = true;
		const img = new window.Image();
		img.src = target;
	};

	return (
		<button
			type="button"
			onClick={onOpen}
			onMouseEnter={prefetch}
			onFocus={prefetch}
			aria-label={`Ouvrir l'illustration : ${title}`}
			className={cn(
				"group block h-full w-full cursor-zoom-in overflow-hidden rounded-2xl text-left",
				"border border-outline-variant/30 bg-surface-container-low",
				"transition-all duration-200 hover:-translate-y-0.5 hover:bg-surface-container hover:shadow-lg",
				"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary",
				className
			)}
		>
			<div className="relative aspect-video w-full overflow-hidden bg-surface-container-high">
				{hasImage ? (
					<Image
						src={thumb as string}
						alt={title}
						fill
						sizes="(max-width: 640px) 100vw, (max-width: 1024px) 50vw, 33vw"
						className="object-cover transition-transform duration-300 group-hover:scale-105"
						unoptimized
						priority={priority}
						loading={priority ? "eager" : "lazy"}
						onError={() => setImgError(true)}
					/>
				) : (
					<div className="flex size-full items-center justify-center">
						<Icon name={categoryIcon} size={40} className="text-on-surface-variant/25" />
					</div>
				)}
				{categoryLabel && (
					<span className="absolute left-2 top-2 inline-flex items-center gap-1 rounded-full bg-surface/80 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-on-surface backdrop-blur-sm">
						<Icon name={categoryIcon} size={12} />
						{categoryLabel}
					</span>
				)}
			</div>
			<div className="p-3">
				<h3 className="line-clamp-1 text-xs font-bold leading-tight text-on-surface transition-colors group-hover:text-primary">
					{title}
				</h3>
			</div>
		</button>
	);
}
