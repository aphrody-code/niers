import { cn } from "@/lib/utils";

/**
 * Mapping rareté → couleurs d'affichage
 *
 * In-game rarity progression:
 *   Normal → Expérimenté → Héros → BASARA
 *
 * BASARA      = rose/rouge
 * HERO        = violet/indigo (Héros)
 * EXPERIMENTE = cyan (Expérimenté)
 * NORMAL      = vert
 */
export const RARITY_STYLES: Record<string, { text: string; bg: string; ring?: string }> = {
	BASARA: { bg: "bg-pink-600/30", ring: "ring-1 ring-pink-400/50", text: "text-white" },
	EXPERIMENTE: { bg: "bg-cyan-400/15", text: "text-cyan-300" },
	HERO: { bg: "bg-violet-500/20", ring: "ring-1 ring-violet-400/40", text: "text-violet-300" },
	NORMAL: { bg: "bg-green-400/15", text: "text-green-400" },
};

/**
 * Labels affichés — noms officiels du jeu FR
 */
const RARITY_LABEL: Record<string, string> = {
	BASARA: "BASARA",
	EXPERIMENTE: "Expérimenté",
	HERO: "Héros",
	NORMAL: "Normal",
};

/**
 * Normalise une valeur de rareté vers une clé standard.
 *
 * Accepts: text labels (from DB rarity_label), numeric codes (as string),
 * or internal keys (NORMAL/EXPERIMENTE/HERO/BASARA).
 */
function normalizeRarity(rarity: string): string {
	const r = rarity.trim();
	// Text labels (from DB rarity_label / enrichment)
	if (r === "Héros") {
		return "HERO";
	}
	if (r === "Expérimenté") {
		return "EXPERIMENTE";
	}
	if (r === "Normal" || r.toLowerCase() === "normal") {
		return "NORMAL";
	}
	if (r === "BASARA") {
		return "BASARA";
	}
	// Legacy labels — map to Normal
	if (r === "Légendaire" || r === "Émérite" || r === "En progression") {
		return "NORMAL";
	}
	// Numeric codes (as string)
	if (r === "0" || r === "1" || r === "3" || r === "4" || r === "5" || r === "6" || r === "7") {
		return "NORMAL";
	}
	if (r === "2") {
		return "EXPERIMENTE";
	}
	if (r === "10") {
		return "HERO";
	}
	if (r === "20") {
		return "BASARA";
	}
	return r;
}

interface RarityBadgeProps {
	rarity: string;
	size?: "xs" | "sm" | "md";
	showIcon?: boolean;
	showLabel?: boolean;
	className?: string;
}

export function RarityBadge({
	rarity,
	size = "sm",
	showLabel = true,
	className,
}: RarityBadgeProps) {
	const key = normalizeRarity(rarity);
	const style = RARITY_STYLES[key] || RARITY_STYLES.NORMAL;
	const label = RARITY_LABEL[key] || RARITY_LABEL[rarity] || rarity;

	const textSize = size === "xs" ? "text-[9px]" : size === "sm" ? "text-[10px]" : "text-xs";
	const padding = size === "xs" ? "px-1.5 py-0.5" : size === "sm" ? "px-2 py-0.5" : "px-2.5 py-1";

	return (
		<span
			className={cn(
				"inline-flex items-center gap-1 rounded-full font-black uppercase tracking-wide",
				style.bg,
				style.ring,
				textSize,
				padding,
				className
			)}
		>
			{showLabel && <span className={cn("leading-none", style.text)}>{label}</span>}
		</span>
	);
}

/** Normalised rarity key (for external use, e.g. canvas cards) */
export { normalizeRarity };
