"use client";

/**
 * Rendu TABLE d'une famille typée dont les données sont un tableau d'objets (item, skill,
 * mission, trophy, gallery…) — bien plus lisible que l'arbre JSON brut. Colonnes = union des
 * clés (champs scalaires d'abord), paginé. Repli sur l'arbre JSON pour les formes non tabulaires.
 */
import { useMemo, useState } from "react";
import { CpkJsonTree } from "@/app/cpk/CpkJsonTree";

const PAGE = 50;

/** Formate une cellule scalaire ; les objets/tableaux imbriqués → JSON compact tronqué. */
function cell(v: unknown): string {
	if (v === null || v === undefined) return "";
	if (typeof v === "object") {
		const s = JSON.stringify(v);
		return s.length > 60 ? `${s.slice(0, 57)}…` : s;
	}
	return String(v);
}

function isHexish(s: unknown): boolean {
	return typeof s === "string" && (/^0x[0-9a-fA-F]+$/.test(s) || /^[0-9A-F]{8,}$/.test(s));
}

export function CpkTypedTable({ data }: { data: unknown }) {
	const [page, setPage] = useState(0);

	const rows = Array.isArray(data) ? (data as Record<string, unknown>[]) : null;

	// Colonnes = union des clés des objets (scalaires en premier, dans l'ordre de découverte).
	const columns = useMemo(() => {
		if (!rows) return [];
		const seen = new Set<string>();
		const cols: string[] = [];
		for (const r of rows.slice(0, 200)) {
			if (r && typeof r === "object" && !Array.isArray(r)) {
				for (const k of Object.keys(r)) {
					if (!seen.has(k)) {
						seen.add(k);
						cols.push(k);
					}
				}
			}
		}
		return cols;
	}, [rows]);

	// Non tabulaire (pas un tableau d'objets) → arbre JSON.
	if (!rows || rows.length === 0 || columns.length === 0) {
		return <CpkJsonTree data={data} rootLabel="data" />;
	}

	const pages = Math.ceil(rows.length / PAGE);
	const slice = rows.slice(page * PAGE, page * PAGE + PAGE);

	return (
		<div className="p-4">
			<div className="overflow-auto max-h-[60vh] rounded-xl border border-outline-variant/20">
				<table className="w-full text-xs">
					<thead className="sticky top-0 bg-surface-container-high text-on-surface-variant">
						<tr>
							<th className="text-left font-semibold px-2 py-1.5">#</th>
							{columns.map((c) => (
								<th key={c} className="text-left font-semibold px-2 py-1.5 whitespace-nowrap">
									{c}
								</th>
							))}
						</tr>
					</thead>
					<tbody className="font-mono">
						{slice.map((r, i) => (
							<tr
								key={page * PAGE + i}
								className="border-t border-outline-variant/10 hover:bg-surface-container-low"
							>
								<td className="px-2 py-1 text-on-surface-variant/50 tabular-nums">
									{page * PAGE + i}
								</td>
								{columns.map((c) => {
									const v = r?.[c];
									return (
										<td
											key={c}
											className={`px-2 py-1 break-all ${isHexish(v) ? "text-secondary" : typeof v === "number" ? "text-primary" : "text-on-surface"}`}
											title={typeof v === "object" && v !== null ? JSON.stringify(v) : undefined}
										>
											{cell(v)}
										</td>
									);
								})}
							</tr>
						))}
					</tbody>
				</table>
			</div>
			{pages > 1 && (
				<div className="flex items-center justify-center gap-3 pt-2 text-xs text-on-surface-variant">
					<button
						type="button"
						disabled={page === 0}
						onClick={() => setPage((p) => Math.max(0, p - 1))}
						className="px-2 py-1 rounded hover:text-primary disabled:opacity-30"
					>
						‹ Précédent
					</button>
					<span className="font-mono">
						{page * PAGE + 1}–{Math.min((page + 1) * PAGE, rows.length)} / {rows.length}
					</span>
					<button
						type="button"
						disabled={page >= pages - 1}
						onClick={() => setPage((p) => Math.min(pages - 1, p + 1))}
						className="px-2 py-1 rounded hover:text-primary disabled:opacity-30"
					>
						Suivant ›
					</button>
				</div>
			)}
		</div>
	);
}
