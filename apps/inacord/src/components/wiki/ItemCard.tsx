"use client";

import { Store } from "lucide-react";
import { Image } from "@/components/ui/image";
import { Link } from "@/components/ui/link";
import { useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { getItemIconUrl, PLACEHOLDERS, resolveAssetUrl } from "@/lib/wikiImages";
import { cn } from "@/lib/utils";

export interface ItemCardProps {
	id: string;
	name: string;
	category: string;
	categoryLabel?: string;
	rarity?: number;
	internalCode?: string;
	icon?: string;
	price?: number | null;
	location?: string | null;
	stats?: Record<string, any> | null;
	/** Bonus de stats RÉELS décodés (kick +6, …) — prioritaires sur `stats` brut. */
	bonuses?: Record<string, number> | null;
	className?: string;
}

const CATEGORY_ICONS: Record<string, string> = {
	accessory: "diamond",
	animal: "pets",
	consume: "local_pharmacy",
	costume: "apparel",
	craft_obj: "construction",
	emblem: "shield",
	fashion: "styler",
	formation: "sports",
	important: "priority_high",
	kizuna_link: "link",
	misanga: "wrist_band",
	name_plate: "id_card",
	performance: "music_note",
	shoes: "steps",
	special: "star",
	special_skill: "auto_awesome",
	special_tactics: "strategy",
	super_tactics: "military_tech",
	title: "badge",
};

export function ItemCard({
	id,
	name,
	category,
	categoryLabel,
	rarity: _rarity,
	internalCode,
	icon,
	price: _price,
	location,
	stats,
	bonuses,
	className,
}: ItemCardProps) {
	const resolvedSrc = resolveAssetUrl(icon) || (internalCode ? getItemIconUrl(internalCode) : null);
	const [imgError, setImgError] = useState(false);
	const catIcon = CATEGORY_ICONS[category] || "backpack";
	// Une URL = placeholder (`/ievr.webp`) ou erreur runtime → on n'affiche PAS de <img>
	// cassé : on rend l'icône de catégorie (toujours propre, jamais 404). Les items
	// « important » `gd120002..091` n'ont aucune icône réelle (vérité terrain CPK), résolus
	// en amont vers le placeholder ; le `onError` ci-dessous couvre tout 404 résiduel.
	const hasRealImage = !!resolvedSrc && resolvedSrc !== PLACEHOLDERS.item && !imgError;

	// Bonus de stats RÉELS prioritaires (kick/control/…). Les `stats` bruts
	// (stat1/stat2 = codes non interprétables) ne sont PLUS affichés tels quels :
	// on ne garde que les clés de stats reconnues.
	const bonusEntries = bonuses
		? Object.entries(bonuses).filter(([k, v]) => k in STAT_SHORT && typeof v === "number" && v !== 0)
		: [];
	const fallbackStatEntries =
		bonusEntries.length === 0 && stats
			? Object.entries(stats).filter(([k, v]) => k in STAT_SHORT && typeof v === "number" && v > 0)
			: [];
	const topEntries: [string, number][] = (
		bonusEntries.length > 0 ? bonusEntries : fallbackStatEntries
	) as [string, number][];
	// `toSorted` demande la lib ES2023 ; la cible de l'app est plus basse. Une copie explicite
	// donne la même sémantique (pas de tri en place sur la source).
	const topStats = [...topEntries]
		.sort(([, a], [, b]) => Math.abs(b) - Math.abs(a))
		.slice(0, 2);

	// Tactic effects
	const isTactic = category === "special_tactics";
	const effects =
		isTactic && stats ? [stats.effect1, stats.effect2, stats.effect3].filter(Boolean) : [];

	return (
		<Link
			href={isTactic ? `/tactic/${id}` : `/item/${id}`}
			className={cn(
				"block h-full transition-all duration-200 relative overflow-hidden group",
				"hover:shadow-lg hover:-translate-y-0.5",
				"border border-outline-variant/30 bg-surface-container-low hover:bg-surface-container",
				"rounded-2xl",
				className
			)}
		>
			<div className="p-4 flex gap-4">
				{/* Icon */}
				<div className="size-14 shrink-0 rounded-xl bg-surface-container-high flex items-center justify-center border border-outline-variant/20 overflow-hidden">
					{hasRealImage ? (
						<Image
							src={resolvedSrc as string}
							alt={name}
							width={48}
							height={48}
							className="object-contain size-11"
							unoptimized
							onError={() => setImgError(true)}
						/>
					) : (
						<Icon name={catIcon} size={24} className="text-on-surface-variant/30" />
					)}
				</div>

				{/* Content */}
				<div className="flex-1 min-w-0">
					<h3 className="font-bold text-sm text-on-surface leading-tight group-hover:text-primary transition-colors line-clamp-2">
						{name}
					</h3>

					{/* Meta chips */}
					<div className="flex flex-wrap items-center gap-1.5 mt-1.5">
						<span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-surface-container-highest/50 text-[10px] font-bold text-on-surface-variant uppercase tracking-wider">
							<Icon name={catIcon} size={12} />
							{categoryLabel || category}
						</span>

						{topStats.map(([key, value]) => (
							<span
								key={key}
								className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-primary/10 text-[10px] font-bold text-primary"
							>
								{STAT_SHORT[key] || key} {value > 0 ? "+" : ""}
								{value}
							</span>
						))}

						{/* Tactic Duration/Cooldown */}
						{isTactic && stats && (
							<>
								{stats.duration && (
									<span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-tertiary/10 text-[10px] font-bold text-tertiary">
										⏱ {stats.duration}s
									</span>
								)}
								{stats.cooldown && (
									<span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-error/10 text-[10px] font-bold text-error">
										⏳ {stats.cooldown}s
									</span>
								)}
							</>
						)}
					</div>

					{/* Tactic Effects List */}
					{effects.length > 0 && (
						<div className="mt-2 space-y-1">
							{effects.slice(0, 2).map((eff, i) => (
								<p
									key={i}
									className="text-[10px] text-on-surface-variant leading-tight truncate"
									title={eff}
								>
									• {eff}
								</p>
							))}
						</div>
					)}

					{/* Bottom row: price + location (Hide for tactics if they have effects to save space, or show if space permits) */}
					{!isTactic && location && (
						<div className="flex items-center gap-3 mt-2 text-xs text-on-surface-variant">
							<span className="inline-flex items-center gap-0.5 truncate">
								<Store size={14} aria-hidden="true" />
								{LOCATION_FR[location] || location}
							</span>
						</div>
					)}

					{isTactic && location && (
						<div className="flex items-center gap-3 mt-2 text-xs text-on-surface-variant">
							<span className="inline-flex items-center gap-0.5 truncate">
								<Store size={14} aria-hidden="true" />
								{LOCATION_FR[location] || location}
							</span>
						</div>
					)}
				</div>
			</div>
		</Link>
	);
}

const STAT_SHORT: Record<string, string> = {
	agility: "AGI",
	control: "CTR",
	intelligence: "INT",
	kick: "TIR",
	physical: "PHY",
	pressure: "PRE",
	technique: "TEC",
};

const LOCATION_FR: Record<string, string> = {
	Chronicle: "Chronique",
	"G-Mart (Arcade Branch)": "G-Mart (Arcade)",
	"G-Mart (Odaiba)": "G-Mart (Odaiba)",
	"Kool Kit (Odaiba Branch)": "Kool Kit (Odaiba)",
	"Special Training Booth": "Entraînement",
	Spirit: "Esprits",
	VS: "VS",
};
