import { CommonSpriteIcon } from "../../../components/wiki/ui/CommonSpriteIcon";
import type { SpriteCommonKey } from "../../../config/sprites-common";
import { CircleHelp } from "lucide-react";
import { iconMap } from "../../ui/Icon";
import { cn } from "../../../lib/utils";

const SPRITE_FOR_ICON: Record<string, SpriteCommonKey> = {
	air: "wind",
	bolt: "keshin",
	change_circle: "mode_change",
	cyclone: "wind",
	electric_bolt: "keshin",
	flash_on: "keshin",
	forest: "forest",
	group_add: "miximax",
	landscape: "mountain",
	local_fire_department: "fire",
	pets: "soul",
	wb_twilight: "eveil",
};

export interface IconProps {
	name: string;
	fill?: boolean;
	size?: number;
	className?: string;
	preferSprite?: boolean;
}

export const Icon = ({
	name,
	fill = false,
	size = 24,
	className,
	preferSprite = true,
}: IconProps) => {
	if (preferSprite) {
		const spriteKey = SPRITE_FOR_ICON[name];
		if (spriteKey) {
			return <CommonSpriteIcon name={spriteKey} scale={size / 64} className={cn(className)} />;
		}
	}

	const LucideIcon = iconMap[name];
	if (!LucideIcon) {
		// Un nom inconnu rendait `null` en silence : l'icône disparaissait mais son
		// conteneur gardait sa taille, laissant un carré vide impossible à repérer.
		// En développement on le rend visible ; en production on n'affiche rien.
		if (process.env.NODE_ENV !== "production") {
			console.warn(`Icon inconnue : « ${name} » (absente de iconMap)`);
			return <CircleHelp size={size} className={cn("opacity-40", className)} aria-hidden="true" />;
		}
		return null;
	}
	return (
		<LucideIcon
			size={size}
			className={cn(className)}
			fill={fill ? "currentColor" : "none"}
			aria-hidden="true"
		/>
	);
};
