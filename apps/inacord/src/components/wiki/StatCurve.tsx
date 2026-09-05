"use client";

import { useId, useMemo, useState } from "react";
import { cn } from "@/lib/utils";

/**
 * Bloc des 7 stats IEVR à un palier de niveau donné.
 * Aligné sur `StatBlock` de l'ingesteur niers `nie-zukan` (mêmes clés, même ordre).
 */
export interface StatBlock {
	kick: number;
	control: number;
	technique: number;
	pressure: number;
	physical: number;
	agility: number;
	intelligence: number;
}

/**
 * Courbes de stats sur les 4 paliers du Zukan (Lv50/100/150/200).
 * Chaque palier est OPTIONNEL : l'ingesteur niers n'extrait de façon fiable
 * que le Lv50 (seule colonne pré-rendue dans le HTML) ; 100/150/200 peuvent
 * manquer. Le composant s'adapte au nombre de paliers réellement fournis.
 */
export interface StatCurves {
	lv50?: StatBlock | null;
	lv100?: StatBlock | null;
	lv150?: StatBlock | null;
	lv200?: StatBlock | null;
}

export interface StatCurveProps {
	curves: StatCurves;
	/** Nom du personnage (a11y / titre). */
	name?: string;
	className?: string;
	/** Hauteur du graphe en px (largeur = 100% responsive). Défaut 220. */
	height?: number;
}

/** Les 7 stats dans l'ordre canonique du jeu + couleur d'accent + libellé FR. */
const STAT_SERIES = [
	{ key: "kick", label: "Frappe", color: "#ef4444" },
	{ key: "control", label: "Contrôle", color: "#3b82f6" },
	{ key: "technique", label: "Technique", color: "#22c55e" },
	{ key: "pressure", label: "Pression", color: "#a855f7" },
	{ key: "physical", label: "Physique", color: "#f97316" },
	{ key: "agility", label: "Agilité", color: "#06b6d4" },
	{ key: "intelligence", label: "Intelligence", color: "#6366f1" },
] as const;

type StatKey = (typeof STAT_SERIES)[number]["key"];

/** Les 4 paliers, dans l'ordre. */
const LEVELS = [
	{ key: "lv50", level: 50 },
	{ key: "lv100", level: 100 },
	{ key: "lv150", level: 150 },
	{ key: "lv200", level: 200 },
] as const;

/**
 * Graphe de COURBES de stats par niveau (7 stats × jusqu'à 4 paliers Lv50→200).
 *
 * SVG pur (cohérent avec `StatHeptagon`, aucune dépendance de charting) :
 * - mobile-first (largeur fluide via `viewBox`, hauteur fixe en prop) ;
 * - DA Azalée / tokens M3 pour le châssis, couleurs de stat héritées du jeu ;
 * - états vides/partiels : si aucun palier → message « bientôt » ; si 1 seul
 *   palier → points isolés (pas de ligne fantôme) ; légende cliquable pour
 *   isoler une stat ;
 * - accessible : chaque série a un `<title>`, légende au clavier, axes labellisés.
 *
 * NB : ce composant n'effectue AUCUN fetch — il reçoit les courbes en props
 * (à câbler sur la fiche perso quand l'ingesteur `nie-zukan` aura peuplé la DB).
 */
export function StatCurve({ curves, name, className, height = 220 }: StatCurveProps) {
	const gradientId = useId();
	const [isolated, setIsolated] = useState<StatKey | null>(null);

	// Paliers réellement présents (avec un StatBlock non-null).
	const presentLevels = useMemo(
		() =>
			LEVELS.filter((l) => {
				const block = curves[l.key];
				return block != null;
			}).map((l) => ({ ...l, block: curves[l.key] as StatBlock })),
		[curves]
	);

	// Borne haute de l'axe Y : on arrondit au-dessus pour des graduations lisibles.
	const maxValue = useMemo(() => {
		let max = 0;
		for (const lvl of presentLevels) {
			for (const s of STAT_SERIES) {
				const v = lvl.block[s.key];
				if (v > max) max = v;
			}
		}
		if (max <= 0) return 100;
		// Arrondi au multiple de 50 supérieur (les stats IEVR vont jusqu'à ~999).
		return Math.ceil(max / 50) * 50;
	}, [presentLevels]);

	// État vide : aucune courbe disponible.
	if (presentLevels.length === 0) {
		return (
			<div
				className={cn(
					`
				flex flex-col items-center justify-center gap-2 rounded-2xl border border-outline-variant/40
				bg-surface-container-low p-6 text-center
			`,
					className
				)}
				style={{ minHeight: height }}
			>
				<span className="type-title-small text-on-surface-variant">
					Courbes de stats indisponibles
				</span>
				<span className="type-body-small text-on-surface-variant/70">
					Les progressions Niv. 50 → 200 du Zukan arrivent bientôt.
				</span>
			</div>
		);
	}

	// Géométrie du tracé (viewBox interne fixe ; rendu fluide via width 100%).
	const VB_W = 320;
	const VB_H = height;
	const padLeft = 34;
	const padRight = 12;
	const padTop = 12;
	const padBottom = 26;
	const plotW = VB_W - padLeft - padRight;
	const plotH = VB_H - padTop - padBottom;

	// X : position d'un palier (réparti uniformément sur les paliers présents).
	const xAt = (index: number) =>
		presentLevels.length === 1
			? padLeft + plotW / 2
			: padLeft + (plotW * index) / (presentLevels.length - 1);

	// Y : valeur → pixel (inversé, 0 en bas).
	const yAt = (value: number) => padTop + plotH * (1 - value / maxValue);

	// 4 graduations Y.
	const yTicks = [0, 0.25, 0.5, 0.75, 1].map((t) => ({
		value: Math.round(maxValue * t),
		y: padTop + plotH * (1 - t),
	}));

	return (
		<figure className={cn("w-full", className)} aria-label={`Courbes de stats de ${name ?? "personnage"}`}>
			<svg
				viewBox={`0 0 ${VB_W} ${VB_H}`}
				width="100%"
				height={VB_H}
				role="img"
				aria-label={`Progression des 7 statistiques sur ${presentLevels.length} palier(s) de niveau`}
				className="overflow-visible"
			>
				<defs>
					<linearGradient id={`${gradientId}-grid`} x1="0" y1="0" x2="0" y2="1">
						<stop offset="0%" stopColor="currentColor" stopOpacity="0.08" />
						<stop offset="100%" stopColor="currentColor" stopOpacity="0.02" />
					</linearGradient>
				</defs>

				{/* Fond de la zone de tracé */}
				<rect
					x={padLeft}
					y={padTop}
					width={plotW}
					height={plotH}
					rx={8}
					className="fill-surface-container-low text-on-surface"
					fill={`url(#${gradientId}-grid)`}
				/>

				{/* Graduations Y + labels */}
				{yTicks.map((tick) => (
					<g key={tick.value}>
						<line
							x1={padLeft}
							y1={tick.y}
							x2={padLeft + plotW}
							y2={tick.y}
							className="stroke-outline-variant/30"
							strokeWidth={0.5}
						/>
						<text
							x={padLeft - 5}
							y={tick.y + 3}
							textAnchor="end"
							className="fill-on-surface-variant/60 text-[8px] tabular-nums"
						>
							{tick.value}
						</text>
					</g>
				))}

				{/* Graduations X (paliers de niveau) */}
				{presentLevels.map((lvl, i) => (
					<text
						key={lvl.key}
						x={xAt(i)}
						y={VB_H - 8}
						textAnchor="middle"
						className="fill-on-surface-variant/70 text-[9px] font-medium"
					>
						Niv. {lvl.level}
					</text>
				))}

				{/* Une polyligne par stat (ou points isolés si 1 seul palier) */}
				{STAT_SERIES.map((serie) => {
					const dimmed = isolated !== null && isolated !== serie.key;
					const points = presentLevels.map((lvl, i) => ({
						x: xAt(i),
						y: yAt(lvl.block[serie.key]),
						v: lvl.block[serie.key],
						level: lvl.level,
					}));
					const path = points.map((p) => `${p.x},${p.y}`).join(" ");

					return (
						<g key={serie.key} opacity={dimmed ? 0.12 : 1} className="transition-opacity">
							<title>{serie.label}</title>
							{points.length > 1 && (
								<polyline
									points={path}
									fill="none"
									stroke={serie.color}
									strokeWidth={isolated === serie.key ? 2.5 : 1.75}
									strokeLinejoin="round"
									strokeLinecap="round"
								/>
							)}
							{points.map((p) => (
								<circle
									key={p.level}
									cx={p.x}
									cy={p.y}
									r={isolated === serie.key ? 3 : 2.25}
									fill={serie.color}
									className="stroke-surface"
									strokeWidth={1}
								>
									<title>{`${serie.label} — Niv. ${p.level} : ${p.v}`}</title>
								</circle>
							))}
						</g>
					);
				})}
			</svg>

			{/* Légende cliquable (isole une stat) */}
			<figcaption className="mt-3 flex flex-wrap justify-center gap-1.5">
				{STAT_SERIES.map((serie) => {
					const active = isolated === serie.key;
					return (
						<button
							key={serie.key}
							type="button"
							onClick={() => setIsolated((cur) => (cur === serie.key ? null : serie.key))}
							aria-pressed={active}
							className={cn(
								`
							flex items-center gap-1.5 rounded-full px-2.5 py-1 type-label-small transition-colors
							focus-visible:outline-2 focus-visible:outline-primary
						`,
								active
									? "bg-surface-container-highest text-on-surface"
									: "bg-surface-container text-on-surface-variant hover:bg-surface-container-high"
							)}
						>
							<span
								className="size-2.5 rounded-full"
								style={{ backgroundColor: serie.color }}
								aria-hidden="true"
							/>
							{serie.label}
						</button>
					);
				})}
			</figcaption>

			{/* Avertissement honnête si données partielles (Zukan ne donne souvent que Lv50). */}
			{presentLevels.length === 1 && (
				<p className="mt-2 text-center type-body-small text-on-surface-variant/60">
					Seul le palier Niv. {presentLevels[0]?.level} est disponible pour ce personnage.
				</p>
			)}
		</figure>
	);
}
