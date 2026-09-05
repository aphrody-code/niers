"use client";

/**
 * Panneau de STATISTIQUES de la démo — câble le moteur de croissance niers (`lib/nie-engine`,
 * tables byte-exact ancrées sur nie.exe) sur les paramètres de croissance réels du personnage.
 *
 * L'utilisateur choisit la forme (positions du dossier), la rareté et le pattern de croissance,
 * puis un niveau (1→99) : on affiche les 7 stats + le total + la courbe du total (markup SVG repris
 * de `StatCalculator.TotalCurve`, non exporté), et — en regard — les stats PRÉ-CALCULÉES du dossier
 * (lv1/lv50/lv99) comme repère. Navigateur uniquement (wasm) : îlot `"use client"`.
 */
import { useEffect, useMemo, useState } from "react";
import { calculateStats, statCurve, type GrowthParams, type StatBlock } from "@/lib/nie-engine";
import type { DemoData, DemoFormData } from "./types";

/** Ordre + libellés FR des 7 stats (clés wasm) + clé du dossier pré-calculé. */
const STATS: ReadonlyArray<{ k: keyof StatBlock; fr: string; pre: string }> = [
	{ fr: "Frappe", k: "kc", pre: "kick" },
	{ fr: "Contrôle", k: "cr", pre: "control" },
	{ fr: "Technique", k: "tc", pre: "technique" },
	{ fr: "Pression", k: "pr", pre: "pressure" },
	{ fr: "Physique", k: "ps", pre: "physical" },
	{ fr: "Agilité", k: "ag", pre: "agility" },
	{ fr: "Intelligence", k: "it", pre: "intelligence" },
];

const RARITIES = [
	{ code: 0, label: "N" },
	{ code: 2, label: "R" },
	{ code: 3, label: "SR" },
	{ code: 4, label: "SSR" },
	{ code: 5, label: "UR" },
	{ code: 6, label: "LR" },
	{ code: 7, label: "Légende" },
	{ code: 20, label: "BASARA" },
];

const GROWTH_PATTERNS = [0, 1, 2, 3];
const MAX_STAT = 350; // échelle d'affichage des barres

export function DemoStatsPanel({
	forms,
	precomputed,
}: {
	forms: DemoFormData[];
	precomputed: DemoData["precomputedStats"];
}) {
	const [formIdx, setFormIdx] = useState(0);
	const [charaRank, setCharaRank] = useState(7);
	const [level, setLevel] = useState(99);
	const form = forms[formIdx] ?? forms[0];
	const [growthPattern, setGrowthPattern] = useState(form?.growth.growthPattern ?? 0);

	const [block, setBlock] = useState<StatBlock | null>(null);
	const [total, setTotal] = useState(0);
	const [curve, setCurve] = useState<number[]>([]);
	const [error, setError] = useState(false);

	const params = useMemo<GrowthParams>(
		() => ({
			charaRank,
			growthPattern,
			mainPosition: form?.growth.mainPosition ?? 3,
			playStyle: 0,
			subPosition: form?.growth.subPosition ?? 0,
		}),
		[charaRank, growthPattern, form],
	);

	// Stats au niveau courant.
	useEffect(() => {
		let on = true;
		setError(false);
		calculateStats(params, level)
			.then((r) => {
				if (!on) return;
				setBlock(r.stats);
				setTotal(r.total);
			})
			.catch(() => on && setError(true));
		return () => {
			on = false;
		};
	}, [params, level]);

	// Courbe du total Lv1→99.
	useEffect(() => {
		let on = true;
		statCurve(params, 1, 99)
			.then((rows) => on && setCurve(rows.map((r) => r.total)))
			.catch(() => on && setCurve([]));
		return () => {
			on = false;
		};
	}, [params]);

	return (
		<div className="space-y-4 rounded-2xl border border-outline-variant/20 bg-surface-container p-4 sm:p-5">
			<h3 className="text-sm font-bold text-on-surface">Statistiques (moteur niers, byte-exact)</h3>

			{/* Paramètres. */}
			<div className="space-y-3">
				<div className="space-y-1.5">
					<p className="text-xs font-medium uppercase tracking-wider text-on-surface-variant">Forme</p>
					<div className="flex flex-wrap gap-1.5">
						{forms.map((f, i) => (
							<button
								key={f.code}
								type="button"
								onClick={() => setFormIdx(i)}
								className={`h-8 rounded-full px-3 text-xs font-medium transition-colors ${
									i === formIdx
										? "bg-primary text-on-primary"
										: "bg-surface-container-high text-on-surface-variant hover:bg-surface-container-highest"
								}`}
							>
								{f.label}
							</button>
						))}
					</div>
				</div>
				<div className="space-y-1.5">
					<p className="text-xs font-medium uppercase tracking-wider text-on-surface-variant">Rareté</p>
					<div className="flex flex-wrap gap-1.5">
						{RARITIES.map((r) => (
							<button
								key={r.code}
								type="button"
								onClick={() => setCharaRank(r.code)}
								className={`h-8 rounded-full px-3 text-xs font-medium transition-colors ${
									charaRank === r.code
										? "bg-primary text-on-primary"
										: "bg-surface-container-high text-on-surface-variant hover:bg-surface-container-highest"
								}`}
							>
								{r.label}
							</button>
						))}
					</div>
				</div>
				<div className="space-y-1.5">
					<p className="text-xs font-medium uppercase tracking-wider text-on-surface-variant">
						Pattern de croissance
					</p>
					<div className="flex flex-wrap gap-1.5">
						{GROWTH_PATTERNS.map((p) => (
							<button
								key={p}
								type="button"
								onClick={() => setGrowthPattern(p)}
								className={`h-8 rounded-full px-3 text-xs font-medium transition-colors ${
									growthPattern === p
										? "bg-primary text-on-primary"
										: "bg-surface-container-high text-on-surface-variant hover:bg-surface-container-highest"
								}`}
							>
								Type {p}
							</button>
						))}
					</div>
				</div>
				<div className="space-y-1.5">
					<div className="flex items-center justify-between">
						<p className="text-xs font-medium uppercase tracking-wider text-on-surface-variant">Niveau</p>
						<span className="text-sm font-bold tabular-nums text-primary">Lv {level}</span>
					</div>
					<input
						type="range"
						min={1}
						max={99}
						value={level}
						onChange={(e) => setLevel(Number(e.target.value))}
						className="w-full accent-primary"
						aria-label="Niveau"
					/>
				</div>
			</div>

			{error ? (
				<p className="p-4 text-center text-sm text-on-surface-variant">
					Moteur de stats indisponible (wasm).
				</p>
			) : (
				<div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
					{/* 7 stats + repère pré-calculé. */}
					<div>
						<div className="mb-3 flex items-center justify-between">
							<h4 className="text-sm font-bold text-on-surface">Stats · Lv {level}</h4>
							<span className="text-sm text-on-surface-variant">
								Total <span className="font-bold tabular-nums text-primary">{total}</span>
							</span>
						</div>
						<div className="space-y-2">
							{STATS.map((s) => {
								const v = block?.[s.k] ?? 0;
								const ref = precomputed.lv99[s.pre];
								return (
									<div key={s.k} className="flex items-center gap-3">
										<span className="w-24 shrink-0 text-xs text-on-surface-variant">{s.fr}</span>
										<div className="h-2.5 flex-1 overflow-hidden rounded-full bg-surface-container-highest">
											<div
												className="h-full rounded-full bg-primary transition-all"
												style={{ width: `${Math.min(100, (v / MAX_STAT) * 100)}%` }}
											/>
										</div>
										<span className="w-10 shrink-0 text-right text-sm font-medium tabular-nums text-on-surface">
											{v}
										</span>
										{ref != null && (
											<span
												className="w-12 shrink-0 text-right text-[11px] tabular-nums text-on-surface-variant/60"
												title="Référence dossier (Lv99)"
											>
												/{ref}
											</span>
										)}
									</div>
								);
							})}
						</div>
						<p className="mt-2 text-[11px] text-on-surface-variant/70">
							Colonne grise = stats du dossier (Lv99) en repère.
						</p>
					</div>

					{/* Courbe du total Lv1→99 (markup repris de StatCalculator.TotalCurve). */}
					<div>
						<h4 className="mb-3 text-sm font-bold text-on-surface">Courbe du total (Lv1→99)</h4>
						<TotalCurve curve={curve} level={level} />
					</div>
				</div>
			)}
		</div>
	);
}

/** Petit graphe SVG du total + marqueur de niveau (repris de `StatCalculator`, non exporté). */
function TotalCurve({ curve, level }: { curve: number[]; level: number }) {
	if (curve.length < 2) {
		return (
			<div className="flex h-40 items-center justify-center text-xs text-on-surface-variant/60">…</div>
		);
	}
	const W = 320;
	const H = 140;
	const min = Math.min(...curve);
	const max = Math.max(...curve);
	const span = Math.max(1, max - min);
	const pts = curve
		.map((v, i) => {
			const x = (i / (curve.length - 1)) * W;
			const y = H - ((v - min) / span) * H;
			return `${x.toFixed(1)},${y.toFixed(1)}`;
		})
		.join(" ");
	const cx = ((level - 1) / (curve.length - 1)) * W;
	const cv = curve[level - 1] ?? max;
	const cy = H - ((cv - min) / span) * H;
	return (
		<svg
			viewBox={`0 0 ${W} ${H}`}
			className="h-40 w-full"
			preserveAspectRatio="none"
			role="img"
			aria-label="Courbe du total des stats"
		>
			<polyline points={pts} fill="none" stroke="currentColor" strokeWidth={2} className="text-primary" />
			<line x1={cx} y1={0} x2={cx} y2={H} stroke="currentColor" strokeWidth={1} className="text-on-surface-variant/30" />
			<circle cx={cx} cy={cy} r={3.5} className="fill-primary" />
		</svg>
	);
}
