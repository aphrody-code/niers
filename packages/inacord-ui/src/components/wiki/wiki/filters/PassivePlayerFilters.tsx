"use client";

import { usePathname, useRouter, useSearchParams } from "../../../../compat/next";
import { useCallback } from "react";
import { cn } from "../../../../lib/utils";

/* ── Options ───────────────────────────────────────── */

const ELEMENT_OPTIONS: ReadonlyArray<{ value: string; label: string; dot: string }> = [
	{ dot: "bg-sky-500", label: "Vent", value: "wind" },
	{ dot: "bg-green-500", label: "Forêt", value: "forest" },
	{ dot: "bg-red-500", label: "Feu", value: "fire" },
	{ dot: "bg-stone-500", label: "Montagne", value: "mountain" },
	{ dot: "bg-slate-400", label: "Neutre", value: "neutral" },
];

const CATEGORY_OPTIONS: ReadonlyArray<{ value: string; label: string }> = [
	{ label: "Joueur", value: "player" },
	{ label: "Miximax", value: "miximax" },
	{ label: "Personnalisé", value: "custom" },
	{ label: "Héros", value: "hero" },
	{ label: "Soul", value: "soul" },
	{ label: "Swap", value: "swap" },
];

/* ── Composant ─────────────────────────────────────── */

interface PassivePlayerFiltersProps {
	elementCounts?: Record<string, number>;
	categoryCounts?: Record<string, number>;
	rarityCounts?: Record<string, number>;
}

export function PassivePlayerFilters({
	elementCounts,
	categoryCounts,
	rarityCounts,
}: PassivePlayerFiltersProps) {
	const router = useRouter();
	const pathname = usePathname();
	const searchParams = useSearchParams();

	const activeElement = searchParams.get("element") || "";
	const activeCategory = searchParams.get("pcat") || "";
	const activeRarity = searchParams.get("rarity") || "";

	const toggle = useCallback(
		(key: string, value: string) => {
			const params = new URLSearchParams(searchParams.toString());
			if (params.get(key) === value) {
				params.delete(key);
			} else {
				params.set(key, value);
			}
			params.delete("page");
			const qs = params.toString();
			router.replace(`${pathname}${qs ? `?${qs}` : ""}`);
		},
		[router, pathname, searchParams]
	);

	const rarityOptions = Object.keys(rarityCounts ?? {}).sort((a, b) => Number(a) - Number(b));

	return (
		<div className="space-y-2" role="search" aria-label="Filtres des passifs joueur">
			{/* Catégorie (préfixe string_id) */}
			<div
				className="flex flex-wrap gap-1.5"
				role="group"
				aria-label="Filtrer par catégorie de passif"
			>
				{CATEGORY_OPTIONS.map((opt) => {
					const isActive = activeCategory === opt.value;
					const count = categoryCounts?.[opt.value];
					if (count === 0) return null;
					return (
						<button
							key={opt.value}
							type="button"
							onClick={() => toggle("pcat", opt.value)}
							aria-pressed={isActive}
							className={cn(
								"inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-bold tracking-wider transition-all border min-h-11 sm:min-h-0",
								isActive
									? "bg-primary/15 text-primary border-primary/30"
									: "bg-surface-container-low text-on-surface-variant border-transparent hover:bg-surface-container-highest hover:border-outline-variant/30"
							)}
						>
							{opt.label}
							{count !== undefined && (
								<span className="text-[9px] opacity-60 ml-0.5">({count})</span>
							)}
						</button>
					);
				})}
			</div>

			{/* Élément */}
			<div className="flex flex-wrap gap-1.5" role="group" aria-label="Filtrer par élément">
				{ELEMENT_OPTIONS.map((opt) => {
					const isActive = activeElement === opt.value;
					const count = elementCounts?.[opt.value];
					if (count === 0) return null;
					return (
						<button
							key={opt.value}
							type="button"
							onClick={() => toggle("element", opt.value)}
							aria-pressed={isActive}
							className={cn(
								"inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-bold tracking-wider transition-all border min-h-11 sm:min-h-0",
								isActive
									? "bg-primary/15 text-primary border-primary/30"
									: "bg-surface-container-low text-on-surface-variant border-transparent hover:bg-surface-container-highest hover:border-outline-variant/30"
							)}
						>
							<span className={cn("size-2 rounded-full", opt.dot)} aria-hidden="true" />
							{opt.label}
							{count !== undefined && (
								<span className="text-[9px] opacity-60 ml-0.5">({count})</span>
							)}
						</button>
					);
				})}
			</div>

			{/* Rareté */}
			{rarityOptions.length > 1 && (
				<div className="flex flex-wrap gap-1.5" role="group" aria-label="Filtrer par rareté">
					{rarityOptions.map((r) => {
						const isActive = activeRarity === r;
						const count = rarityCounts?.[r];
						return (
							<button
								key={r}
								type="button"
								onClick={() => toggle("rarity", r)}
								aria-pressed={isActive}
								className={cn(
									"inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-bold tracking-wider transition-all border min-h-11 sm:min-h-0",
									isActive
										? "bg-amber-500/20 text-amber-600 border-amber-300/40"
										: "bg-surface-container-low text-on-surface-variant border-transparent hover:bg-surface-container-highest hover:border-outline-variant/30"
								)}
							>
								Rareté {r}
								{count !== undefined && (
									<span className="text-[9px] opacity-60 ml-0.5">({count})</span>
								)}
							</button>
						);
					})}
				</div>
			)}
		</div>
	);
}
