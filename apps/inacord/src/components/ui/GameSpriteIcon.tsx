import type React from "react";
import { GAME_ICON_ATLASES, GAME_ICONS } from "@/config/game-icons";
import type { GameIconKey } from "@/config/game-icons";
import { cn } from "@/lib/utils";

interface GameSpriteIconProps extends React.HTMLAttributes<HTMLSpanElement> {
	/** Cle de l'icone dans GAME_ICONS. */
	name: GameIconKey;
	/**
	 * Hauteur cible en px (prioritaire). L'icone est mise a l'echelle pour
	 * occuper cette hauteur quelle que soit la cellule native -> toutes les
	 * icones rendent a la meme taille meme si les atlas ont des cellules de
	 * dimensions differentes (256x200 vs 192x120).
	 */
	size?: number;
	/** Echelle directe de la cellule native (ignore si `size` est fourni). */
	scale?: number;
}

/**
 * Rend une VRAIE icone de menu du jeu IEVR via un MASQUE CSS teinte.
 *
 * L'atlas webp (silhouette blanche, alpha = forme) sert de mask-image : la
 * couleur visible vient de `background-color: currentColor`, donc l'icone prend
 * la teinte du theme courant (clair/sombre) exactement comme une icone lucide.
 *
 * Calcul identique a CommonSpriteIcon (background-position/-size) mais en mask :
 *   mask-size     = sheetW x sheetH a l'echelle
 *   mask-position = -x,-y a l'echelle (decalage vers la cellule)
 *   width/height  = w x h a l'echelle (fenetre = cellule)
 */
export const GameSpriteIcon = ({
	name,
	size,
	scale = 1,
	className,
	style,
	...props
}: GameSpriteIconProps) => {
	const icon = GAME_ICONS[name];

	if (!icon) {
		return null;
	}

	const atlas = GAME_ICON_ATLASES[icon.atlas];
	// `size` (hauteur cible) prime : echelle = size / hauteur de cellule.
	const effScale = size != null ? size / icon.h : scale;
	const displayW = icon.w * effScale;
	const displayH = icon.h * effScale;
	const maskImage = `url(${atlas.src})`;
	const maskPosition = `-${icon.x * effScale}px -${icon.y * effScale}px`;
	const maskSize = `${atlas.sheetW * effScale}px ${atlas.sheetH * effScale}px`;

	return (
		<span
			className={cn("inline-block shrink-0 bg-current", className)}
			role="img"
			aria-label={icon.nameFr}
			style={{
				height: `${displayH}px`,
				maskImage,
				maskPosition,
				maskRepeat: "no-repeat",
				maskSize,
				WebkitMaskImage: maskImage,
				WebkitMaskPosition: maskPosition,
				WebkitMaskRepeat: "no-repeat",
				WebkitMaskSize: maskSize,
				width: `${displayW}px`,
				...style,
			}}
			{...props}
		/>
	);
};
