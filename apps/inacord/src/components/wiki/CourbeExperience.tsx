"use client";

/**
 * Courbe d'expérience — graphe SVG de la table `inagle_exp_table`.
 *
 * Deux lectures de la MÊME donnée, sélectionnables par l'appelant :
 * - `cumulative` : EXP totale pour atteindre chaque niveau depuis le niveau 1 ;
 * - `palier` : EXP du seul passage `niveau → niveau + 1` (la colonne `need_exp` brute).
 *
 * Aucune couleur en dur : le tracé hérite de `currentColor` via les classes de
 * tokens (`text-primary`, `text-on-surface-variant`), exactement comme la courbe
 * du calculateur de stats (`components/wiki/StatCalculator.tsx`).
 *
 * Accessibilité : le SVG est `role="img"` avec un `aria-label` décrivant les
 * bornes réelles ; la donnée chiffrée complète reste disponible dans le tableau
 * de la page, le graphe n'est donc jamais le seul porteur d'information.
 */

import { useId, useMemo } from "react";
import type { ExpCurvePoint } from "@/lib/wiki/exp-table-shared";
import { formatExp } from "@/lib/wiki/exp-table-shared";

/** Repère lu sur la courbe : cumul depuis le niveau 1, ou coût du palier seul. */
export type CourbeMode = "cumulative" | "palier";

export interface CourbeExperienceProps {
	/** Points de la courbe (un par niveau), issus de `buildExpCurve`. */
	points: ExpCurvePoint[];
	/** Grandeur tracée. */
	mode: CourbeMode;
	/** Niveau mis en évidence (marqueur vertical + point). */
	niveau: number;
	/** Appelé quand l'utilisateur clique ou glisse sur le graphe. */
	onNiveauChange?: (niveau: number) => void;
}

const VIEW_W = 320;
const VIEW_H = 150;

export function CourbeExperience({ points, mode, niveau, onNiveauChange }: CourbeExperienceProps) {
	const titleId = useId();

	const geometrie = useMemo(() => {
		if (points.length < 2) {
			return null;
		}
		const valeurs = points.map((p) => (mode === "cumulative" ? p.cumulative : p.needExp));
		const max = Math.max(...valeurs);
		const min = Math.min(...valeurs);
		const amplitude = Math.max(1, max - min);
		const x = (index: number) => (index / (points.length - 1)) * VIEW_W;
		const y = (valeur: number) => VIEW_H - ((valeur - min) / amplitude) * VIEW_H;

		const ligne = valeurs.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
		const aire = `0,${VIEW_H} ${ligne} ${VIEW_W},${VIEW_H}`;

		return { aire, ligne, max, min, valeurs, x, y };
	}, [points, mode]);

	if (!geometrie) {
		return (
			<div className="flex h-40 items-center justify-center text-sm text-on-surface-variant">
				Aucune donnée d'expérience à tracer.
			</div>
		);
	}

	const index = Math.max(
		0,
		points.findIndex((p) => p.level === niveau)
	);
	const valeurCourante = geometrie.valeurs[index] ?? 0;
	const cx = geometrie.x(index);
	const cy = geometrie.y(valeurCourante);
	const premier = points[0]!;
	const dernier = points[points.length - 1]!;

	/** Traduit une abscisse de pointeur en niveau, puis remonte le changement. */
	function pointerVersNiveau(event: React.PointerEvent<SVGSVGElement>) {
		if (!onNiveauChange) {
			return;
		}
		const rect = event.currentTarget.getBoundingClientRect();
		if (rect.width === 0) {
			return;
		}
		const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
		const cible = points[Math.round(ratio * (points.length - 1))];
		if (cible && cible.level !== niveau) {
			onNiveauChange(cible.level);
		}
	}

	return (
		<figure className="m-0 space-y-2">
			<svg
				viewBox={`0 0 ${VIEW_W} ${VIEW_H}`}
				className="h-40 w-full sm:h-56"
				preserveAspectRatio="none"
				role="img"
				aria-labelledby={titleId}
				onPointerDown={pointerVersNiveau}
				onPointerMove={(e) => {
					if (e.buttons === 1) {
						pointerVersNiveau(e);
					}
				}}
			>
				<title id={titleId}>
					{mode === "cumulative"
						? `Expérience cumulée du niveau ${premier.level} au niveau ${dernier.level} : de ${formatExp(geometrie.min)} à ${formatExp(geometrie.max)} points.`
						: `Expérience par palier du niveau ${premier.level} au niveau ${dernier.level} : de ${formatExp(geometrie.min)} à ${formatExp(geometrie.max)} points.`}
				</title>
				<polygon points={geometrie.aire} className="fill-primary/10" />
				<polyline
					points={geometrie.ligne}
					fill="none"
					stroke="currentColor"
					strokeWidth={2}
					strokeLinejoin="round"
					vectorEffect="non-scaling-stroke"
					className="text-primary"
				/>
				<line
					x1={cx}
					y1={0}
					x2={cx}
					y2={VIEW_H}
					stroke="currentColor"
					strokeWidth={1}
					vectorEffect="non-scaling-stroke"
					className="text-on-surface-variant/40"
				/>
				<circle cx={cx} cy={cy} r={4} className="fill-primary" />
			</svg>
			<figcaption className="flex items-baseline justify-between gap-3 text-xs text-on-surface-variant">
				<span>Niveau {premier.level}</span>
				<span className="text-center font-medium text-on-surface">
					Niveau {niveau} · {formatExp(valeurCourante)} EXP
				</span>
				<span>Niveau {dernier.level}</span>
			</figcaption>
		</figure>
	);
}
