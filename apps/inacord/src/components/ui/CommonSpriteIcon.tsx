import type React from "react";
import { SPRITE_SHEET_COMMON_SRC, SPRITES_COMMON } from "@/config/sprites-common";
import type { SpriteCommonKey } from "@/config/sprites-common";
import { cn } from "@/lib/utils";
import styles from "./SpriteIcon.module.css";

// Native spritesheet dimensions
const SHEET_W = 1140;
const SHEET_H = 1152;

interface CommonSpriteIconProps extends React.HTMLAttributes<HTMLDivElement> {
	name: SpriteCommonKey;
	variant?: "default" | "glow" | "shadow";
	scale?: number;
}

export const CommonSpriteIcon = ({
	name,
	variant = "default",
	scale = 1,
	className,
	style,
	...props
}: CommonSpriteIconProps) => {
	const sprite = SPRITES_COMMON[name];

	if (!sprite) {
		console.warn(`Sprite "${name}" not found in common configuration.`);
		return null;
	}

	const { x, y, w, h } = sprite;
	const displayW = w * scale;
	const displayH = h * scale;

	return (
		<div
			className={cn("relative inline-block override-icon-size", className)}
			style={{
				height: displayH,
				width: displayW,
				...style,
			}}
			role="img"
			aria-label={name}
			{...props}
		>
			<div
				className={cn(styles.sprite, styles[variant])}
				style={{
					backgroundImage: `url(${SPRITE_SHEET_COMMON_SRC})`,
					backgroundPosition: `-${x * scale}px -${y * scale}px`,
					backgroundSize: `${SHEET_W * scale}px ${SHEET_H * scale}px`,
					height: `${displayH}px`,
					width: `${displayW}px`,
				}}
			/>
		</div>
	);
};
