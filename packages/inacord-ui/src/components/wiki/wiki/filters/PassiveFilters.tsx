"use client";

import { usePathname, useRouter, useSearchParams } from "../../../../compat/next";
import { useCallback } from "react";
import { CommonSpriteIcon } from "../../../../components/wiki/ui/CommonSpriteIcon";
import { Icon } from "../../../../components/wiki/ui/Icon";
import type { SpriteCommonKey } from "../../../../config/sprites-common";
import { cn } from "../../../../lib/utils";

/* ── Filter options ────────────────────────────────── */

const TYPE_OPTIONS = [
	{ color: "text-blue-600", icon: "person", label: "Joueur", value: "player" },
	{ color: "text-purple-600", icon: "auto_fix_high", label: "Personnalisé", value: "custom" },
	{ color: "text-amber-600", icon: "groups", label: "Coord. / Manager", value: "coordinator" },
] as const;

const PLAYSTYLE_OPTIONS: ReadonlyArray<{
	value: string;
	label: string;
	sprite?: SpriteCommonKey;
	icon?: string;
	color: string;
}> = [
	{ color: "text-on-surface-variant", icon: "tune", label: "Généraux", value: "" },
	{ color: "text-red-500", label: "Brèche", sprite: "breche", value: "Breach" },
	{ color: "text-amber-500", label: "Tension", sprite: "tension", value: "Tension" },
	{ color: "text-blue-500", label: "Contre-attaque", sprite: "contre", value: "Counter" },
	{ color: "text-pink-500", label: "Lien", sprite: "lien", value: "Bond" },
	{ color: "text-orange-500", label: "Jeu brutal", sprite: "jeu_violent", value: "Rough Play" },
	{ color: "text-emerald-500", label: "Justice", sprite: "justice", value: "Justice" },
];

const STAT_CATEGORY_OPTIONS = [
	{ icon: "sports_soccer", label: "Tir", value: "shot" },
	{ icon: "flash_on", label: "Affrontement", value: "focus" },
	{ icon: "swap_horiz", label: "Mêlée", value: "scramble" },
	{ icon: "shield", label: "Mur de déf.", value: "castle" },
	{ icon: "groups", label: "Équipe", value: "team" },
	{ icon: "person", label: "Personnel", value: "own" },
] as const;

/* ── Component ─────────────────────────────────────── */

interface PassiveFiltersProps {
	typeCounts?: Record<string, number>;
	playstyleCounts?: Record<string, number>;
}

export function PassiveFilters({ typeCounts, playstyleCounts }: PassiveFiltersProps) {
	const router = useRouter();
	const pathname = usePathname();
	const searchParams = useSearchParams();

	const activeType = searchParams.get("type") || "";
	const activePlaystyle = searchParams.get("playstyle");
	const activeStatCat = searchParams.get("category") || "";

	const toggleParam = useCallback(
		(key: string, value: string) => {
			const params = new URLSearchParams(searchParams.toString());
			if (params.get(key) === value) {
				params.delete(key);
			} else {
				if (value) {
					params.set(key, value);
				} else {
					// For empty string values like "general" playstyle
					params.set(key, value);
				}
			}
			// Reset to page 1 when filtering
			params.delete("page");
			const qs = params.toString();
			router.replace(`${pathname}${qs ? `?${qs}` : ""}`);
		},
		[router, pathname, searchParams]
	);

	const setPlaystyle = useCallback(
		(value: string | null) => {
			const params = new URLSearchParams(searchParams.toString());
			if (value === null || params.get("playstyle") === value) {
				params.delete("playstyle");
			} else {
				params.set("playstyle", value);
			}
			params.delete("page");
			const qs = params.toString();
			router.replace(`${pathname}${qs ? `?${qs}` : ""}`);
		},
		[router, pathname, searchParams]
	);

	return (
		<div className="space-y-3" role="search" aria-label="Filtres des passifs">
			{/* Type filter */}
			<div className="flex flex-wrap gap-1.5" role="group" aria-label="Filtrer par type de passif">
				{TYPE_OPTIONS.map((opt) => {
					const isActive = activeType === opt.value;
					const count = typeCounts?.[opt.value];
					return (
						<button
							key={opt.value}
							onClick={() => toggleParam("type", opt.value)}
							aria-pressed={isActive}
							className={cn(
								"inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-[11px] font-bold uppercase tracking-wider transition-all border min-h-11 sm:min-h-0",
								isActive
									? "bg-primary/15 text-primary border-primary/30"
									: "bg-surface-container-low text-on-surface-variant border-transparent hover:bg-surface-container-highest hover:border-primary/20"
							)}
						>
							<Icon
								name={opt.icon}
								size={14}
								className={cn(isActive ? "text-primary" : opt.color)}
							/>
							{opt.label}
							{count !== undefined && (
								<span className="text-[9px] opacity-60 ml-0.5">({count})</span>
							)}
						</button>
					);
				})}
			</div>

			{/* Playstyle filter */}
			<div className="flex flex-wrap gap-1.5" role="group" aria-label="Filtrer par style de jeu">
				{PLAYSTYLE_OPTIONS.map((opt) => {
					const isActive = activePlaystyle === opt.value;
					const count = playstyleCounts?.[opt.value];
					return (
						<button
							key={opt.value || "_general"}
							onClick={() => setPlaystyle(opt.value)}
							aria-pressed={isActive}
							className={cn(
								"inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-bold tracking-wider transition-all border min-h-11 sm:min-h-0",
								isActive
									? "bg-primary/15 text-primary border-primary/30"
									: "bg-surface-container-low text-on-surface-variant border-transparent hover:bg-surface-container-highest hover:border-outline-variant/30"
							)}
						>
							{opt.sprite ? (
								<CommonSpriteIcon name={opt.sprite} scale={0.22} />
							) : opt.icon ? (
								<Icon
									name={opt.icon}
									size={14}
									className={cn(isActive ? "text-primary" : opt.color)}
								/>
							) : null}
							{opt.label}
							{count !== undefined && (
								<span className="text-[9px] opacity-60 ml-0.5">({count})</span>
							)}
						</button>
					);
				})}
			</div>

			{/* Stat category filter */}
			<div
				className="flex flex-wrap gap-1.5"
				role="group"
				aria-label="Filtrer par catégorie de stat"
			>
				{STAT_CATEGORY_OPTIONS.map((opt) => {
					const isActive = activeStatCat === opt.value;
					return (
						<button
							key={opt.value}
							onClick={() => toggleParam("category", opt.value)}
							aria-pressed={isActive}
							className={cn(
								"inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-bold tracking-wider transition-all border min-h-11 sm:min-h-0",
								isActive
									? "bg-secondary-container text-on-secondary-container border-secondary/30"
									: "bg-surface-container-low text-on-surface-variant border-transparent hover:bg-surface-container-highest hover:border-outline-variant/30"
							)}
						>
							<Icon name={opt.icon} size={14} />
							{opt.label}
						</button>
					);
				})}
			</div>
		</div>
	);
}
