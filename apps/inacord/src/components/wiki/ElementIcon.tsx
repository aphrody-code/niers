"use client";

import { Image } from "@niers/inacord-ui/components/ui/image";
import { getSkillIconUrl } from "@niers/inacord-ui/lib/wikiImages";
import { cn } from "@niers/inacord-ui/lib/utils";

interface ElementIconProps {
	element: string;
	size?: "sm" | "md" | "lg";
	className?: string;
}

const SIZES = {
	lg: 32,
	md: 24,
	sm: 16,
};

export function ElementIcon({ element, size = "md", className }: ElementIconProps) {
	const px = SIZES[size];
	const iconUrl = getSkillIconUrl(element);

	if (!iconUrl) {
		return null;
	}

	return (
		<div
			className={cn("relative inline-flex items-center justify-center", className)}
			style={{ height: px, width: px }}
		>
			<Image src={iconUrl} alt={element} width={px} height={px} className="object-contain" />
		</div>
	);
}
