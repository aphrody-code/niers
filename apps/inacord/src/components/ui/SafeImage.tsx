"use client";

import { Image } from "@/components/ui/image";
import type { ImageProps } from "@/components/ui/image";
import { useEffect, useState } from "react";

interface SafeImageProps extends ImageProps {
	zukanHash?: string;
	/** URL de l'image placeholder quand toutes les tentatives échouent. Par défaut : /placeholder-chara.webp */
	fallbackSrc?: string;
}

/** Placeholder par défaut pour les personnages sans image */
const DEFAULT_PLACEHOLDER = "/ievr.webp";

export function SafeImage({
	src,
	zukanHash,
	alt,
	fallbackSrc,
	unoptimized,
	...props
}: SafeImageProps) {
	const [imgSrc, setImgSrc] = useState(src);
	const [fallbackStage, setFallbackStage] = useState(0);
	const [isFailed, setIsFailed] = useState(false);

	useEffect(() => {
		setImgSrc(src);
		setFallbackStage(0);
		setIsFailed(false);
	}, [src]);

	const handleError = () => {
		const nextStage = fallbackStage + 1;
		setFallbackStage(nextStage);

		// URL zukan propre
		const zukanUrl = zukanHash
			? `https://dxi4wb638ujep.cloudfront.net/1/${zukanHash.startsWith("/") ? zukanHash.slice(1) : zukanHash}.png`
			: null;

		if (nextStage === 1 && zukanUrl && imgSrc !== zukanUrl) {
			setImgSrc(zukanUrl);
			return;
		}

		if (nextStage <= 2) {
			// Retirer proprement tous les suffixes _5000/_5100 du face URL
			const srcStr = typeof imgSrc === "string" ? imgSrc : "";
			if (srcStr.includes("/face/")) {
				const cleanUrl = srcStr.replaceAll(/_\d{4}/g, "");
				if (cleanUrl !== srcStr) {
					setImgSrc(cleanUrl);
					return;
				}
			}
		}

		// Toutes les tentatives ont échoué → placeholder
		setIsFailed(true);
		setImgSrc(fallbackSrc || DEFAULT_PLACEHOLDER);
	};

	return (
		<Image
			{...props}
			src={imgSrc}
			alt={alt}
			onError={isFailed ? undefined : handleError}
			// On respecte le `unoptimized` du caller (images CDN tierces déjà en webp →
			// éviter le quota d'optim Next/402) et on force `unoptimized` au fallback final.
			unoptimized={unoptimized || isFailed}
		/>
	);
}
