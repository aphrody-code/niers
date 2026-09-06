#!/usr/bin/env bun
/**
 * Banc d'essai : un signal BON MARCHÉ du rapport d'assemblage prédit-il le score de la gate ?
 *
 * La gate `gate-zukan.ts` est la mesure de vérité, mais elle coûte ~60 s par personnage (rendu
 * WebGL logiciel de 8 vues) et exige d'avoir téléchargé 16 planches de référence. À ce prix,
 * les 5 723 personnages demandent près de 100 heures de rendu. La question qui décide de la
 * suite est donc : peut-on PRÉ-TRIER à partir du seul `/model-report/`, qui coûte ~2 s ?
 *
 * Ce script ne répond pas par une opinion. Il calcule plusieurs signaux candidats à partir des
 * rapports, les confronte aux IoU/SSIM déjà mesurés par la gate, et rend pour chacun un
 * coefficient de Spearman. Un signal utile a |ρ| franchement non nul ; les autres sont écartés.
 *
 * RÉSULTAT DÉJÀ MESURÉ, à ne pas redécouvrir : l'emprise horizontale des pièces NE marche PAS.
 * L'intuition était bonne — sur c01002100 (IoU 0,302, l'un des pires) le rendu montre deux
 * rubans partant de part et d'autre, et sa pièce `g000203` fait 1,84 m de large. Mais le même
 * champ vaut 1,55 m chez c01000060 (IoU 0,875, l'un des meilleurs) : les bornes du rapport sont
 * celles du maillage EN BIND, avant skinning, et sont partagées par tous les personnages qui
 * portent la même pièce. Elles ne décrivent donc pas ce qui est rendu. Idem pour
 * `unresolved_hashes` et `vertices_without_bone`, vides et nuls des deux côtés.
 *
 * Usage :
 *   bun --bun scripts/validation/signaux-rapport.ts --gate var/outputs/gate-zukan/resume.json
 */

import { existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const RACINE = resolve(import.meta.dir, "..", "..");
const CACHE = join(RACINE, "var", "model-cache");

interface Piece {
	piece?: string;
	role?: string;
	bounds_min?: number[];
	bounds_max?: number[];
	primitives_kept?: number;
	skin?: {
		bones_used?: string[];
		max_influences?: number;
		unresolved_hashes?: unknown[];
		vertices_without_bone?: number;
		skinned_submeshes?: number;
		static_submeshes?: number;
	};
}

/** Les signaux candidats, tous tirés du seul rapport d'assemblage. */
function signaux(rapport: unknown): Record<string, number> {
	const r = rapport as {
		pieces?: Piece[];
		materials_without_texture?: unknown[];
		notes?: unknown[];
		glb_bytes?: number;
		mode?: string;
	};
	const pieces = r.pieces ?? [];
	const hauteur = Math.max(0, ...pieces.map((p) => p.bounds_max?.[1] ?? 0));
	let empriseMax = 0;
	let os = 0;
	let nonResolus = 0;
	let sansOs = 0;
	let statiques = 0;
	let influencesMax = 0;
	let primitives = 0;
	for (const p of pieces) {
		const mn = p.bounds_min;
		const mx = p.bounds_max;
		if (mn && mx && mn.length >= 3 && mx.length >= 3) {
			empriseMax = Math.max(empriseMax, Math.max(mx[0]! - mn[0]!, mx[2]! - mn[2]!));
		}
		os += p.skin?.bones_used?.length ?? 0;
		nonResolus += p.skin?.unresolved_hashes?.length ?? 0;
		sansOs += p.skin?.vertices_without_bone ?? 0;
		statiques += p.skin?.static_submeshes ?? 0;
		influencesMax = Math.max(influencesMax, p.skin?.max_influences ?? 0);
		primitives += p.primitives_kept ?? 0;
	}
	return {
		"emprise max / hauteur": hauteur > 0 ? empriseMax / hauteur : 0,
		"emprise max (m)": empriseMax,
		"hauteur (m)": hauteur,
		"nombre de pièces": pieces.length,
		"primitives totales": primitives,
		"os référencés": os,
		"hachés non résolus": nonResolus,
		"sommets sans os": sansOs,
		"sous-maillages statiques": statiques,
		"influences max": influencesMax,
		"matériaux sans texture": (r.materials_without_texture ?? []).length,
		notes: (r.notes ?? []).length,
		"octets du GLB": r.glb_bytes ?? 0,
	};
}

function rangs(xs: number[]): number[] {
	const idx = xs.map((v, i) => [v, i] as const).sort((a, b) => a[0] - b[0]);
	const r = new Array<number>(xs.length);
	let i = 0;
	while (i < idx.length) {
		let j = i;
		while (j + 1 < idx.length && idx[j + 1]![0] === idx[i]![0]) j++;
		const moyen = (i + j) / 2 + 1;
		for (let k = i; k <= j; k++) r[idx[k]![1]] = moyen;
		i = j + 1;
	}
	return r;
}

function spearman(xs: number[], ys: number[]): number {
	if (xs.length < 3) return Number.NaN;
	const rx = rangs(xs);
	const ry = rangs(ys);
	const n = xs.length;
	const mx = rx.reduce((a, b) => a + b, 0) / n;
	const my = ry.reduce((a, b) => a + b, 0) / n;
	let num = 0;
	let dx = 0;
	let dy = 0;
	for (let i = 0; i < n; i++) {
		const a = rx[i]! - mx;
		const b = ry[i]! - my;
		num += a * b;
		dx += a * a;
		dy += b * b;
	}
	return dx > 0 && dy > 0 ? num / Math.sqrt(dx * dy) : Number.NaN;
}

async function main() {
	const argv = process.argv.slice(2);
	let gate = join(RACINE, "var", "outputs", "gate-zukan", "resume.json");
	for (let i = 0; i < argv.length; i++) {
		if (argv[i] === "--gate") gate = argv[++i] ?? gate;
		else if (argv[i] === "-h" || argv[i] === "--help") {
			console.log("signaux-rapport — un signal du rapport prédit-il le score de la gate ?\n  --gate F");
			return;
		}
	}
	if (!existsSync(gate)) throw new Error(`résumé de gate introuvable : ${gate}`);
	const g = JSON.parse(await Bun.file(gate).text()) as {
		scores: { code: string; statut: string; iou_moyen?: number; ssim_moyen?: number; iou_min?: number }[];
	};
	const evalues = g.scores.filter((s) => s.statut === "compare");

	const lignes: { code: string; iou: number; ssim: number; s: Record<string, number> }[] = [];
	for (const s of evalues) {
		const f = join(CACHE, `${s.code}.report.json`);
		if (!existsSync(f)) continue;
		try {
			lignes.push({
				code: s.code,
				iou: s.iou_moyen ?? 0,
				ssim: s.ssim_moyen ?? 0,
				s: signaux(JSON.parse(await Bun.file(f).text())),
			});
		} catch {
			/* rapport illisible */
		}
	}
	if (lignes.length < 3) throw new Error(`trop peu de personnages appariés (${lignes.length})`);

	const noms = Object.keys(lignes[0]!.s);
	const ious = lignes.map((l) => l.iou);
	const ssims = lignes.map((l) => l.ssim);

	console.log(`Appariés : ${lignes.length} personnages mesurés à la fois par la gate et par leur rapport.`);
	console.log(`IoU observé : ${Math.min(...ious).toFixed(3)} … ${Math.max(...ious).toFixed(3)}\n`);
	console.log("Signal du rapport                 ρ(IoU)   ρ(SSIM)   verdict");
	const table: { nom: string; rIou: number; rSsim: number }[] = [];
	for (const n of noms) {
		const xs = lignes.map((l) => l.s[n]!);
		table.push({ nom: n, rIou: spearman(xs, ious), rSsim: spearman(xs, ssims) });
	}
	for (const t of table.sort((a, b) => Math.abs(b.rIou || 0) - Math.abs(a.rIou || 0))) {
		const v = Number.isNaN(t.rIou) ? "constant" : Math.abs(t.rIou) >= 0.6 ? "UTILISABLE" : Math.abs(t.rIou) >= 0.35 ? "faible" : "inutile";
		console.log(
			`${t.nom.padEnd(32)} ${fmt(t.rIou).padStart(7)}  ${fmt(t.rSsim).padStart(8)}   ${v}`,
		);
	}
	console.log(
		"\nUn signal n'est retenu que si |ρ| ≥ 0,6 : en dessous, pré-trier avec lui laisserait passer\n" +
			"trop de modèles cassés — et la gate resterait de toute façon nécessaire pour trancher.",
	);

	await Bun.write(
		join(RACINE, "var", "outputs", "signaux-rapport.json"),
		`${JSON.stringify({ horodatage: new Date().toISOString(), apparies: lignes.length, correlations: table, lignes }, null, 2)}\n`,
	);
}

function fmt(x: number): string {
	return Number.isNaN(x) ? "n/a" : x.toFixed(3);
}

await main();
