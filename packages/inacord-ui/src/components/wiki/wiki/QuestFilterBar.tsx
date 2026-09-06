"use client";

import { Search, X } from "lucide-react";
import { usePathname, useRouter, useSearchParams } from "../../../compat/next";
import { useEffect, useState } from "react";
import { cn } from "../../../lib/utils";

/** Onglets de catégorie de quête (dérivés du `phase`, pas d'une colonne DB). */
const KIND_TABS: Array<{ value: "all" | "main" | "side"; label: string }> = [
	{ value: "all", label: "Toutes" },
	{ value: "main", label: "Histoire" },
	{ value: "side", label: "Annexes" },
];

interface QuestFilterBarProps {
	/** Compteurs pour annoter les onglets. */
	counts: { all: number; main: number; side: number };
}

/**
 * Barre de filtre/recherche dédiée aux quêtes : recherche plein-titre (debounce,
 * pilotée par l'URL `?q=`) + onglets de catégorie (`?kind=`). 182 lignes → tout est
 * filtré côté serveur en mémoire, donc on se contente de réécrire l'URL.
 */
export function QuestFilterBar({ counts }: QuestFilterBarProps) {
	const router = useRouter();
	const pathname = usePathname();
	const searchParams = useSearchParams();

	const activeKind = (searchParams.get("kind") as "all" | "main" | "side") || "all";
	const [query, setQuery] = useState(searchParams.get("q") || "");

	useEffect(() => {
		const q = searchParams.get("q");
		if (q !== null) setQuery(q);
	}, [searchParams]);

	useEffect(() => {
		const timer = setTimeout(() => {
			const currentQ = searchParams.get("q") || "";
			if (query === currentQ) return;
			const params = new URLSearchParams(searchParams.toString());
			if (query) params.set("q", query);
			else params.delete("q");
			router.replace(`${pathname}?${params.toString()}`);
		}, 350);
		return () => clearTimeout(timer);
	}, [query, router, pathname, searchParams]);

	const setKind = (value: "all" | "main" | "side") => {
		const params = new URLSearchParams(searchParams.toString());
		if (value === "all") params.delete("kind");
		else params.set("kind", value);
		router.replace(`${pathname}?${params.toString()}`);
	};

	const countFor = (value: "all" | "main" | "side") =>
		value === "main" ? counts.main : value === "side" ? counts.side : counts.all;

	return (
		<div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
			{/* Onglets catégorie */}
			<div className="flex flex-wrap gap-2">
				{KIND_TABS.map((tab) => {
					const active = activeKind === tab.value;
					return (
						<button
							key={tab.value}
							type="button"
							onClick={() => setKind(tab.value)}
							aria-pressed={active}
							className={cn(
								"rounded-full px-3.5 py-1.5 text-sm font-bold transition-colors",
								active
									? "bg-primary text-on-primary"
									: "bg-surface-container text-on-surface-variant hover:bg-surface-container-high"
							)}
						>
							{tab.label}
							<span
								className={cn(
									"ml-1.5 text-xs font-medium",
									active ? "text-on-primary/70" : "text-on-surface-variant/50"
								)}
							>
								{countFor(tab.value)}
							</span>
						</button>
					);
				})}
			</div>

			{/* Recherche */}
			<div
				className={cn(
					"flex items-center gap-1 rounded-full border border-outline-variant/40 bg-surface-container-high",
					"p-1 pr-2 shadow-sm transition-all focus-within:border-primary/50 sm:max-w-xs"
				)}
			>
				<Search size={20} aria-hidden="true" className="ml-2 text-on-surface-variant" />
				<input
					value={query}
					onChange={(e) => setQuery(e.target.value)}
					placeholder="Rechercher une quête..."
					aria-label="Rechercher une quête"
					className="h-8 flex-1 bg-transparent text-sm text-on-surface outline-none placeholder:text-on-surface-variant/60"
				/>
				{query && (
					<button
						type="button"
						onClick={() => setQuery("")}
						aria-label="Effacer la recherche"
						className="flex size-7 items-center justify-center rounded-full text-on-surface-variant hover:bg-surface-container-highest"
					>
						<X size={16} aria-hidden="true" />
					</button>
				)}
			</div>
		</div>
	);
}
