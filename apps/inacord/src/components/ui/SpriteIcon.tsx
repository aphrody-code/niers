import type React from "react";
import { SPRITE_SHEET_SRC, SPRITES } from "@/config/sprites";
import type { SpriteKey } from "@/config/sprites";
import { cn } from "@/lib/utils";
import styles from "./SpriteIcon.module.css";

// Native spritesheet dimensions
const SHEET_W = 516;
const SHEET_H = 568;

interface SpriteIconProps extends React.HTMLAttributes<HTMLDivElement> {
	name: SpriteKey;
	variant?: "default" | "glow" | "shadow";
	scale?: number;
}

export const SpriteIcon = ({
	name,
	variant = "default",
	scale = 1,
	className,
	style,
	...props
}: SpriteIconProps) => {
	const sprite = SPRITES[name];

	if (!sprite) {
		console.warn(`Sprite "${name}" not found in configuration.`);
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
					backgroundImage: `url(${SPRITE_SHEET_SRC})`,
					backgroundPosition: `-${x * scale}px -${y * scale}px`,
					backgroundSize: `${SHEET_W * scale}px ${SHEET_H * scale}px`,
					height: `${displayH}px`,
					width: `${displayW}px`,
				}}
			/>
		</div>
	);
};
