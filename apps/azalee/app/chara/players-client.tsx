"use client";

import { Eye, EyeOff, Gamepad2, Loader2, X } from "lucide-react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useLanguage } from "@/components/providers/language-provider";
import { FadeInItem } from "@/components/ui/fade-in";
import { BaseCharacterTable } from "@/components/wiki/BaseCharacterTable";
import { CharacterFilters } from "@/components/wiki/filters/CharacterFilters";
import type { TeamOption } from "@/components/wiki/filters/TeamFilter";
import { WikiPagination } from "@/components/wiki/WikiPagination";
import { WikiSearchToolbar } from "@/components/wiki/WikiSearchToolbar";
import { FilterTransitionContext, useFilterTransition } from "@/lib/hooks/use-filter-navigation";
import { cn } from "@/lib/utils";

interface PlayersClientProps {
	paginatedCharacters: any[];
	totalItems: number;
	itemsPerPage: number;
	currentPage: number;
	allVariantsCount: number;
	teams?: TeamOption[];
	showIncomplete?: boolean;
	showControllable?: boolean;
}

export function PlayersClient({
	paginatedCharacters,
	totalItems,
	itemsPerPage,
	currentPage,
	teams,
	showIncomplete = false,
	showControllable = false,
}: PlayersClientProps) {
	const { t } = useLanguage();
	const router = useRouter();
	const searchParams = useSearchParams();
	const filterTransition = useFilterTransition();
	const { isPending } = filterTransition;
	const startIndex = (currentPage - 1) * itemsPerPage;

	const toggleIncomplete = () => {
		const params = new URLSearchParams(searchParams.toString());
		if (showIncomplete) {
			params.delete("status");
		} else {
			params.set("status", "all");
		}
		params.delete("page");
		filterTransition.startTransition(() => {
			router.replace(`?${params.toString()}`, { scroll: false });
		});
	};

	const toggleControllable = () => {
		const params = new URLSearchParams(searchParams.toString());
		if (showControllable) {
			params.delete("status");
		} else {
			params.set("status", "jouable");
		}
		params.delete("page");
		filterTransition.startTransition(() => {
			router.replace(`?${params.toString()}`, { scroll: false });
		});
	};

	const removeFilter = (key: string) => {
		const params = new URLSearchParams(searchParams.toString());
		params.delete(key);
		params.delete("page");
		filterTransition.startTransition(() => {
			router.replace(`?${params.toString()}`, { scroll: false });
		});
	};

	const clearAllFilters = () => {
		const params = new URLSearchParams(searchParams.toString());
		const filterKeys = [
			"element",
			"position",
			"rarity",
			"gender",
			"playstyle",
			"series",
			"team",
			"role",
		];
		filterKeys.forEach((key) => params.delete(key));
		params.delete("page");
		filterTransition.startTransition(() => {
			router.replace(`?${params.toString()}`, { scroll: false });
		});
	};

	const getFilterLabel = (key: string, value: string) => {
		switch (key) {
			case "element": {
				if (value === "Fire") return t("wiki.elements.fire");
				if (value === "Wind") return t("wiki.elements.wind");
				if (value === "Forest") return t("wiki.elements.forest");
				if (value === "Mountain") return t("wiki.elements.mountain");
				return value;
			}
			case "position": {
				if (value === "GK") return t("wiki.positions.gk");
				if (value === "DF") return t("wiki.positions.df");
				if (value === "MF") return t("wiki.positions.mf");
				if (value === "FW") return t("wiki.positions.fw");
				return value;
			}
			case "gender": {
				return value === "1" ? t("wiki.genders.male") : t("wiki.genders.female");
			}
			case "playstyle": {
				if (value === "Bond") return t("wiki.playstyles.bond");
				if (value === "Counter") return t("wiki.playstyles.counter");
				if (value === "Breach") return t("wiki.playstyles.breach");
				if (value === "Rough Play") return t("wiki.playstyles.rough_play");
				return value;
			}
			case "role": {
				return value === "Coordinator" ? "Coordinateur" : "Entraîneur";
			}
			case "team": {
				return teams?.find((t) => t.id === value)?.name || value;
			}
			case "ageGroup": {
				if (value === "Child") return t("wiki.age_groups.child");
				if (value === "Elementary") return t("wiki.age_groups.elementary");
				if (value === "Middle School") return t("wiki.age_groups.middle_school");
				if (value === "High School") return t("wiki.age_groups.high_school");
				if (value === "College") return t("wiki.age_groups.college");
				if (value === "Adult") return t("wiki.age_groups.adult");
				if (value === "Elder") return t("wiki.age_groups.elder");
				if (value === "Exobeing") return t("wiki.age_groups.exobeing");
				return value;
			}
			default:
				return value;
		}
	};

	const filterKeys = [
		"element",
		"position",
		"rarity",
		"gender",
		"playstyle",
		"series",
		"team",
		"role",
		"ageGroup",
	];
	const activeFilters = filterKeys
		.map((key) => {
			const value = searchParams.get(key);
			if (!value) return null;
			return { key, value, label: getFilterLabel(key, value) };
		})
		.filter(Boolean) as Array<{ key: string; value: string; label: string }>;

	return (
		<FilterTransitionContext.Provider value={filterTransition}>
			<div className="space-y-6">
				<div className="flex flex-col gap-3">
					{/* Mobile/tablette : recherche + filtres dans un Drawer (bouton "Filtres"). */}
					<div className="md:hidden flex flex-col gap-4">
						<WikiSearchToolbar
							placeholder={t("wiki.players.search_placeholder")}
							filterKeys={filterKeys}
							showFilters={true}
							containerClassName="w-full"
						>
							<CharacterFilters teams={teams} />
						</WikiSearchToolbar>
					</div>

					{/* Desktop : recherche sans bouton + TOUS les filtres affichés en permanence. */}
					<div className="hidden md:flex md:flex-col gap-4">
						<WikiSearchToolbar
							placeholder={t("wiki.players.search_placeholder")}
							filterKeys={filterKeys}
							showFilters={false}
							containerClassName="max-w-md w-full"
						/>
						<div className="rounded-3xl border border-outline-variant/30 bg-surface-container-low/60 px-5 py-4">
							<CharacterFilters teams={teams} />
						</div>
					</div>

					{activeFilters.length > 0 && (
						<div className="flex flex-wrap items-center gap-1.5 px-1 animate-in fade-in slide-in-from-top-1 duration-200">
							<span className="text-[11px] font-bold text-on-surface-variant/60 uppercase tracking-wider mr-1">
								Filtres actifs :
							</span>
							{activeFilters.map((filter) => (
								<button
									key={filter.key}
									onClick={() => removeFilter(filter.key)}
									className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs font-medium bg-secondary-container text-on-secondary-container hover:bg-secondary-container/80 transition-colors"
								>
									<span>{filter.label}</span>
									<X size={12} className="opacity-75 hover:opacity-100" />
								</button>
							))}
							{activeFilters.length > 1 && (
								<button
									onClick={clearAllFilters}
									className="text-[11px] font-bold text-primary hover:text-primary/80 transition-colors ml-1 px-1 py-0.5 rounded hover:bg-primary/5"
								>
									Tout effacer
								</button>
							)}
						</div>
					)}
				</div>

				<div className="flex flex-wrap items-center gap-2 md:gap-3 text-xs font-medium text-on-surface-variant px-1">
					<span className="tabular-nums uppercase tracking-wider">
						{isPending ? <Loader2 size={12} className="inline animate-spin mr-1" /> : null}
						{totalItems.toLocaleString("fr-FR")} personnage{totalItems !== 1 ? "s" : ""}
					</span>
					<button
						onClick={toggleIncomplete}
						disabled={isPending}
						className={cn(
							"inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] md:text-xs font-medium normal-case tracking-normal transition-all duration-200",
							showIncomplete
								? "bg-tertiary-container text-on-tertiary-container"
								: "bg-surface-container-high text-on-surface-variant hover:bg-surface-container-highest",
							isPending && "opacity-50 pointer-events-none"
						)}
					>
						{showIncomplete ? (
							<Eye size={13} aria-hidden="true" />
						) : (
							<EyeOff size={13} aria-hidden="true" />
						)}
						<span className="hidden sm:inline">
							{showIncomplete ? "Tous affichés" : "Incomplets masqués"}
						</span>
						<span className="sm:hidden">{showIncomplete ? "Tous" : "Incomplets"}</span>
					</button>
					<button
						onClick={toggleControllable}
						disabled={isPending}
						className={cn(
							"inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] md:text-xs font-medium normal-case tracking-normal transition-all duration-200",
							showControllable
								? "bg-emerald-500/20 text-emerald-300 border border-emerald-400/30"
								: "bg-surface-container-high text-on-surface-variant hover:bg-surface-container-highest",
							isPending && "opacity-50 pointer-events-none"
						)}
					>
						<Gamepad2 size={13} aria-hidden="true" />
						<span className="hidden sm:inline">Jouables</span>
					</button>
				</div>

				{/* Content area with loading overlay */}
				<div className="relative">
					{isPending && (
						<div className="absolute inset-0 z-10 flex items-start justify-center pt-32 bg-surface/60 backdrop-blur-[1px] rounded-2xl transition-opacity duration-150 animate-in fade-in">
							<div className="flex items-center gap-2 px-4 py-2 rounded-full bg-surface-container-high shadow-level-2">
								<Loader2 size={16} className="animate-spin text-primary" />
								<span className="text-xs font-medium text-on-surface-variant">Chargement...</span>
							</div>
						</div>
					)}

					{paginatedCharacters.length > 0 ? (
						<div className={cn("transition-opacity duration-200", isPending && "opacity-40")}>
							<FadeInItem>
								<BaseCharacterTable characters={paginatedCharacters} startIndex={startIndex} />
							</FadeInItem>

							<WikiPagination
								totalItems={totalItems}
								itemsPerPage={itemsPerPage}
								currentPage={currentPage}
								perPageOptions={[50, 200]}
							/>
						</div>
					) : (
						<div className="py-20 text-center rounded-[32px] border border-outline-variant bg-surface-container-low border-dashed">
							<p className="text-on-surface-variant italic">{t("wiki.players.no_results")}</p>
						</div>
					)}
				</div>
			</div>
		</FilterTransitionContext.Provider>
	);
}
