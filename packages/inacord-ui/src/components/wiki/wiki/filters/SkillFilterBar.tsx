"use client";

import { PlayCircle, Wifi, X, Zap } from "lucide-react";
import { Image } from "../../../../compat/next";
import { useRouter, useSearchParams } from "../../../../compat/next";
import { useCallback, useEffect, useState } from "react";
import { CommonSpriteIcon } from "../../../../components/wiki/ui/CommonSpriteIcon";
import { Icon } from "../../../../components/wiki/ui/Icon";
import { SpriteIcon } from "../../../../components/wiki/ui/SpriteIcon";
import { Slider } from "../../../../components/ui/slider";
import type { SpriteKey } from "../../../../config/sprites";
import type { SpriteCommonKey } from "../../../../config/sprites-common";
import { cn } from "../../../../lib/utils";

export interface CategoryFilterOption {
	value: string;
	label: string;
	count: number;
	imageIcon?: string;
	sprite?: SpriteKey;
}

export interface ElementFilterOption {
	value: string;
	label: string;
	count: number;
	imageIcon?: string;
	icon?: string;
	commonSprite?: SpriteCommonKey;
}

/**
 * Domaine du curseur de puissance — les bornes réelles de la colonne, pas une valeur ronde.
 *
 * Le maximum était figé à 640 alors que `MAX(power_max)` vaut **880** sur les
 * techniques que la liste affiche (`wh*`/`rh*`, hors variantes `_or`, hors hyper) :
 * **141 techniques étaient hors d'atteinte du curseur**, donc invisibles à qui
 * filtrait par puissance. Mesuré sur le miroir :
 *
 * ```sql
 * SELECT MAX(power_max) FROM inagle_skills
 *  WHERE (internal_code LIKE 'wh%' OR internal_code LIKE 'rh%')
 *    AND id NOT LIKE '%\_or' ESCAPE '\' AND is_hyper = 0;   -- 880
 * ```
 */
const POWER_MIN = 0;
const POWER_MAX = 880;

interface SkillFilterBarProps {
	currentCategory: string;
	currentElement: string;
	currentHasVideo?: string;
	currentShowAura?: string;
	currentOverdrive?: string;
	currentSort?: string;
	currentPowerMin?: string;
	currentPowerMax?: string;
	categories: CategoryFilterOption[];
	elements: ElementFilterOption[];
}

export function SkillFilterBar({
	currentCategory,
	currentElement,
	currentHasVideo,
	currentShowAura,
	currentOverdrive,
	currentSort,
	currentPowerMin,
	currentPowerMax,
	categories,
	elements,
}: SkillFilterBarProps) {
	const router = useRouter();
	const searchParams = useSearchParams();

	// Local slider state for smooth dragging
	const [powerRange, setPowerRange] = useState<[number, number]>([
		currentPowerMin ? Number.parseInt(currentPowerMin, 10) : POWER_MIN,
		currentPowerMax ? Number.parseInt(currentPowerMax, 10) : POWER_MAX,
	]);

	// Sync from URL when params change externally
	useEffect(() => {
		setPowerRange([
			currentPowerMin ? Number.parseInt(currentPowerMin, 10) : POWER_MIN,
			currentPowerMax ? Number.parseInt(currentPowerMax, 10) : POWER_MAX,
		]);
	}, [currentPowerMin, currentPowerMax]);

	const updateFilters = useCallback(
		(key: "type" | "element" | "has_video" | "show_aura" | "overdrive" | "sort", value: string) => {
			const params = new URLSearchParams(searchParams.toString());
			const current = params.get(key) || "";

			if (current === value) {
				params.delete(key);
			} else {
				params.set(key, value);
			}

			// Reset page on filter change
			params.delete("page");

			router.push(`/skill?${params.toString()}`);
		},
		[router, searchParams]
	);

	// Commit power range to URL on slider release
	const commitPowerRange = useCallback(
		(range: number[]) => {
			const params = new URLSearchParams(searchParams.toString());
			const [min, max] = range;

			if (min > POWER_MIN) {
				params.set("power_min", String(min));
			} else {
				params.delete("power_min");
			}

			if (max < POWER_MAX) {
				params.set("power_max", String(max));
			} else {
				params.delete("power_max");
			}

			params.delete("page");
			router.push(`/skill?${params.toString()}`);
		},
		[router, searchParams]
	);

	const isPowerFiltered = powerRange[0] > POWER_MIN || powerRange[1] < POWER_MAX;

	const resetPower = useCallback(() => {
		setPowerRange([POWER_MIN, POWER_MAX]);
		const params = new URLSearchParams(searchParams.toString());
		params.delete("power_min");
		params.delete("power_max");
		params.delete("page");
		router.push(`/skill?${params.toString()}`);
	}, [router, searchParams]);

	return (
		<div className="space-y-4">
			{/* Category Chips */}
			{/* `aria-pressed` plutôt que `aria-selected` : ce sont des bascules de
			    filtre, pas des onglets — la sélection reste, l'URL en témoigne. Sans
			    `type="button"` ces boutons soumettraient le formulaire de recherche
			    qui les entoure. */}
			<div className="flex flex-wrap gap-2" role="group" aria-label="Catégorie de technique">
				{categories.map((f) => {
					const isActive = currentCategory === f.value;
					return (
						<button
							key={f.value}
							type="button"
							aria-pressed={isActive}
							aria-label={f.value ? `Catégorie ${f.label}` : "Toutes les catégories"}
							onClick={() => updateFilters("type", f.value)}
							className={cn(
								"inline-flex items-center justify-center gap-2 px-4 py-2 rounded-full text-sm font-medium",
								"min-h-11 sm:min-h-0 transition-all duration-200 border flex-1 sm:flex-none cursor-pointer",
								isActive
									? "bg-primary text-on-primary border-primary shadow-md"
									: "bg-surface-container hover:bg-surface-container-high border-outline-variant/30 text-on-surface-variant hover:text-on-surface"
							)}
						>
							{f.imageIcon ? (
								<div className="relative size-5">
									<Image src={f.imageIcon} alt="" fill className="object-contain" sizes="20px" />
								</div>
							) : f.sprite ? (
								<SpriteIcon name={f.sprite} scale={0.4} className="-my-2" />
							) : null}
							<span>{f.label}</span>
						</button>
					);
				})}
			</div>

			{/* Sort & Element Filters Row */}
			<div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
				<div className="flex flex-wrap items-center gap-4">
					{/* Video Toggle */}
					<button
						type="button"
						aria-pressed={currentHasVideo === "1"}
						aria-label="N'afficher que les techniques ayant une vidéo"
						onClick={() => updateFilters("has_video", "1")}
						className={cn(
							"inline-flex items-center gap-2 px-4 py-2 rounded-full text-sm font-bold tracking-wide uppercase",
							"min-h-11 sm:min-h-0 transition-all duration-300 border cursor-pointer",
							currentHasVideo === "1"
								? "bg-tertiary text-on-tertiary border-tertiary shadow-lg ring-2 ring-tertiary/20"
								: "bg-surface-container hover:bg-surface-container-high border-outline-variant/30 text-on-surface-variant hover:text-on-surface"
						)}
					>
						<PlayCircle size={24} aria-hidden="true" />
						<span>Vidéo</span>
					</button>

					{/* Show Aura Skills Toggle */}
					<button
						type="button"
						aria-pressed={currentShowAura === "1"}
						aria-label="Inclure les hyper techniques"
						onClick={() => updateFilters("show_aura", "1")}
						className={cn(
							"inline-flex items-center gap-2 px-4 py-2 rounded-full text-sm font-bold tracking-wide uppercase",
							"min-h-11 sm:min-h-0 transition-all duration-300 border cursor-pointer",
							currentShowAura === "1"
								? "bg-tertiary text-on-tertiary border-tertiary shadow-lg ring-2 ring-tertiary/20"
								: "bg-surface-container hover:bg-surface-container-high border-outline-variant/30 text-on-surface-variant hover:text-on-surface"
						)}
					>
						<Wifi size={24} aria-hidden="true" />
						<span>Avec Hyper Technique</span>
					</button>

					{/* Overdrive Toggle */}
					<button
						type="button"
						aria-pressed={currentOverdrive === "1"}
						aria-label="N'afficher que les techniques entrant dans une combinaison Overdrive"
						onClick={() => updateFilters("overdrive", "1")}
						className={cn(
							"inline-flex items-center gap-2 px-4 py-2 rounded-full text-sm font-bold tracking-wide uppercase",
							"min-h-11 sm:min-h-0 transition-all duration-300 border cursor-pointer",
							currentOverdrive === "1"
								? "bg-tertiary text-on-tertiary border-tertiary shadow-lg ring-2 ring-tertiary/20"
								: "bg-surface-container hover:bg-surface-container-high border-outline-variant/30 text-on-surface-variant hover:text-on-surface"
						)}
					>
						<Zap size={24} aria-hidden="true" />
						<span>Overdrive</span>
					</button>

					<div className="w-px h-6 bg-outline-variant/20 hidden sm:block" />

					{/* Sort Options */}
					<div
						role="group"
						aria-label="Ordre de la liste"
						className="flex items-center gap-2 bg-surface-container-high/50 p-1 rounded-full border border-outline-variant/20"
					>
						<button
							type="button"
							aria-pressed={currentSort === "tension" || !currentSort}
							aria-label="Trier par tension décroissante"
							onClick={() => updateFilters("sort", "tension")}
							className={cn(
								"min-h-10 sm:min-h-0 px-3 py-1.5 rounded-full text-xs font-bold uppercase transition-colors cursor-pointer",
								currentSort === "tension" || !currentSort
									? "bg-surface-container-highest text-primary shadow-sm"
									: "text-on-surface-variant hover:text-on-surface hover:bg-surface-container-highest/50"
							)}
						>
							Tension
						</button>
						<button
							type="button"
							aria-pressed={currentSort === "power"}
							aria-label="Trier par puissance maximale décroissante"
							onClick={() => updateFilters("sort", "power")}
							className={cn(
								"min-h-10 sm:min-h-0 px-3 py-1.5 rounded-full text-xs font-bold uppercase transition-colors cursor-pointer",
								currentSort === "power"
									? "bg-surface-container-highest text-primary shadow-sm"
									: "text-on-surface-variant hover:text-on-surface hover:bg-surface-container-highest/50"
							)}
						>
							Puissance
						</button>
					</div>
				</div>

				{/* Element Chips */}
				<div className="flex flex-wrap justify-end gap-2" role="group" aria-label="Élément">
					{elements.map((f) => {
						const isActive = currentElement === f.value;
						return (
							<button
								key={f.value}
								type="button"
								aria-pressed={isActive}
								aria-label={f.value ? `Élément ${f.label}` : "Tous les éléments"}
								onClick={() => updateFilters("element", f.value)}
								className={cn(
									"inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium",
									"min-h-10 sm:min-h-0 transition-all duration-200 border cursor-pointer",
									isActive
										? "bg-secondary text-on-secondary border-secondary shadow-sm"
										: "bg-surface-container-low hover:bg-surface-container border-outline-variant/20 text-on-surface-variant hover:text-on-surface"
								)}
							>
								{f.commonSprite ? (
									<CommonSpriteIcon name={f.commonSprite} scale={0.28} />
								) : f.imageIcon ? (
									<div className="relative size-4">
										<Image src={f.imageIcon} alt="" fill className="object-contain" sizes="16px" />
									</div>
								) : f.icon ? (
									<Icon name={f.icon} size={18} />
								) : null}
								<span>{f.label}</span>
							</button>
						);
					})}
				</div>
			</div>

			{/* Power Range Slider */}
			<div className="flex items-center gap-4">
				<div className="flex items-center gap-2 text-xs font-bold text-on-surface-variant uppercase tracking-wider shrink-0">
					<Zap size={16} aria-hidden="true" />
					<span>Puissance</span>
				</div>

				<div className="flex-1 flex items-center gap-3 min-w-0">
					<span className="text-xs font-mono font-bold text-on-surface-variant tabular-nums w-8 text-right">
						{powerRange[0]}
					</span>
					<Slider
						value={powerRange}
						min={POWER_MIN}
						max={POWER_MAX}
						step={5}
						aria-label="Fourchette de puissance"
						onValueChange={(v) => setPowerRange(v as [number, number])}
						onValueCommitted={(range) => commitPowerRange(range as [number, number])}
						className="flex-1"
					/>
					<span className="text-xs font-mono font-bold text-on-surface-variant tabular-nums w-8">
						{powerRange[1]}
					</span>
				</div>

				{isPowerFiltered && (
					<button
						type="button"
						aria-label="Réinitialiser le filtre de puissance"
						onClick={resetPower}
						className="inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-bold text-error bg-error/10 border border-error/20 hover:bg-error/20 transition-all cursor-pointer"
					>
						<X size={12} aria-hidden="true" />
					</button>
				)}
			</div>
		</div>
	);
}
