#!/usr/bin/env bun
/**
 * Génère `data/miximax-icon-manifest.json` : la VRAIE correspondance entre une aura
 * Miximax (`asset_code` `wmm00XXX` / `icon_code` `c05028XXX` stocké en DB) et le code
 * d'icône RÉEL présent dans le dump dx11 `icon_chr/aura_mixi/` (`ca/cn/cp/iau`).
 *
 * Problème corrigé : `inagle_miximax.icon_code` vaut `c05028XXX` (dérivé du code perso),
 * mais AUCUN fichier `c05028XXX_l_..._l00.png` n'existe dans le dump. Les 17 vrais
 * fichiers Miximax portent des codes hétérogènes (`ca0201`, `cn0221`, `cp1811`,
 * `iau0010a`…) NON dérivables par règle depuis l'`asset_code`. `getMiximaxIconUrl`
 * produisait donc des URLs en 404 garanti.
 *
 * Correspondance (non dérivable, d'où ce manifeste) :
 *   inagle_miximax.config.skillId2 (ou skillId1)  ──┐
 *                                                   │ = hash de ressource menu
 *   inagle_chara_menu_resource.id (`0x…`) ──────────┘
 *     └─ paths.faceSmall = `200_icon/10_icon_chr/aura_mixi/<realIcon>`  → code réel
 *
 * On ne conserve QUE les entrées dont le code réel existe RÉELLEMENT sur disque (les
 * 17 fichiers `aura_mixi/*_l00.png` du dump). Les ~49 autres Miximax pointent vers un
 * hash de ressource absent du dump (icône non extraite) → data-gap → pas d'entrée
 * (le caller bascule sur le placeholder/telop, jamais un 404 forgé).
 *
 * Le manifeste est indexé par DEUX clés pour robustesse côté caller :
 *   - `icon_code`  (`c05028010`)  — ce que stocke `inagle_miximax.icon_code`
 *   - `asset_code` base (`wmm00010`, suffixe de variante retiré)
 *
 * Sources :
 *   - dump : ~/.local/share/Steam/iecode/inazuma/data/dx11/menu/icon_chr/aura_mixi/
 *   - miroir SQLite : data/backups/mirror.sqlite (inagle_miximax + inagle_chara_menu_resource)
 *
 * Run : `bun scripts/build-miximax-icon-manifest.ts`
 */
import { readdirSync } from "node:fs";
import { Database } from "bun:sqlite";

import path from "node:path";

/** Données figées du package (`packages/azalee/src/data`). */
const DATA_DIR = path.resolve(import.meta.dir, "../src/data");

const DUMP_DIR =
	process.env.IEVR_DUMP_DIR ??
	"/home/ubuntu/.local/share/Steam/iecode/inazuma/data/dx11/menu/icon_chr/aura_mixi";
const MIRROR =
	process.env.AZALEE_MIRROR ?? "data/backups/mirror.sqlite";
const OUT = path.join(DATA_DIR, "miximax-icon-manifest.json");

// 1) Codes réels présents sur disque (`<code>_l_<code>_l00.png` → `<code>`).
const realIcons = new Set<string>();
for (const f of readdirSync(DUMP_DIR)) {
	if (f.endsWith("_l00.png")) {
		realIcons.add(f.split("_l_")[0]!);
	}
}
if (realIcons.size === 0) {
	console.error(`Aucun fichier aura_mixi trouvé dans ${DUMP_DIR}`);
	process.exit(1);
}

const db = new Database(MIRROR, { readonly: true });

// 2) hash de ressource menu (`0x…`) → code d'icône réel (aura_mixi uniquement).
const iconByHash = new Map<string, string>();
const resRows = db
	.query<{ id: string; paths: string }, []>(
		"SELECT id, paths FROM inagle_chara_menu_resource",
	)
	.all();
for (const r of resRows) {
	let paths: { faceSmall?: string };
	try {
		paths = JSON.parse(r.paths);
	} catch {
		continue;
	}
	const fs = paths.faceSmall ?? "";
	if (fs.includes("aura_mixi/")) {
		iconByHash.set(r.id.toLowerCase(), fs.split("/").pop()!);
	}
}

// 3) Miximax → code réel via config.skillId2 (fallback skillId1).
const manifest: Record<string, string> = {};
const mixiRows = db
	.query<{ asset_code: string | null; icon_code: string | null; data: string }, []>(
		"SELECT asset_code, icon_code, data FROM inagle_miximax",
	)
	.all();

for (const m of mixiRows) {
	let parsed: { config?: { skillId1?: string; skillId2?: string } };
	try {
		parsed = JSON.parse(m.data);
	} catch {
		continue;
	}
	const cfg = parsed.config ?? {};
	const sid2 = (cfg.skillId2 ?? "").toLowerCase();
	const sid1 = (cfg.skillId1 ?? "").toLowerCase();
	const real = iconByHash.get(sid2) ?? iconByHash.get(sid1);
	if (!real || !realIcons.has(real)) {
		continue; // data-gap : pas de fichier sur disque
	}
	if (m.icon_code) {
		manifest[m.icon_code] = real;
	}
	if (m.asset_code) {
		manifest[m.asset_code.split("_")[0]!] = real; // base sans suffixe de variante
	}
}

const covered = new Set(Object.values(manifest));
const uncovered = [...realIcons].filter((i) => !covered.has(i));

const sorted = Object.fromEntries(
	Object.entries(manifest).sort(([a], [b]) => a.localeCompare(b)),
);
await Bun.write(OUT, JSON.stringify(sorted, null, 0));

console.log(`Fichiers Miximax réels (disque) : ${realIcons.size}`);
console.log(`Entrées de manifeste (icon_code + asset_code base) : ${Object.keys(sorted).length}`);
console.log(`Icônes réelles couvertes : ${covered.size}/${realIcons.size}`);
if (uncovered.length) {
	console.warn(`Icônes réelles NON couvertes (à investiguer) : ${uncovered.join(", ")}`);
}
console.log(`Écrit : ${OUT}`);
