#!/usr/bin/env bun
/**
 * Génère `src/data/menu-gallery-manifest.json` (tracké git) : la liste exhaustive
 * des illustrations des dossiers jeu `dx11/menu/220_img/<dir>/`.
 *
 * Source = l'**index CPK** (`apps/azalee/data/cpk-index.ndjson.gz`, chemins réels
 * des 250 800 fichiers), et non plus un scan du dump PNG pré-extrait : ce dump a
 * été archivé, et son nommage `<nom>_<nom>.png` n'existe dans AUCUN chemin CPK —
 * il produisait 404 sur toute la galerie.
 *
 * Chaque illustration est un conteneur `.g4tx` à texture unique portant le nom du
 * fichier (vérifié par sondage d'en-tête : `gallery_img2`, `stadium`, `ev_pic`,
 * `vsroute_map`, `telop_waza` → 1 texture principale, nom == basename). L'URL CDN
 * est donc la forme 1:1 du contrat :
 *
 *     https://cdn.rosegriffon.fr/dx11/menu/220_img/<dir>/<nom>.png
 *
 * Run : `bun packages/azalee/scripts/build-menu-gallery-manifest.ts`
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { gunzipSync } from "node:zlib";
import { resolveDataFile } from "../src/config";

const DATA_DIR = path.resolve(import.meta.dir, "../src/data");
const CPK_INDEX = "cpk-index.ndjson.gz";
const IMG_ROOT = "data/dx11/menu/220_img/";

/**
 * Dossiers indexés. `dir` est le chemin relatif sous `220_img/` (segment de langue
 * inclus) → l'URL CDN se construit sans ambiguïté.
 */
const MENU_DIRS: ReadonlyArray<{ category: string; dirs: string[] }> = [
	{ category: "gallery_img2", dirs: ["gallery_img2"] },
	{ category: "ev_pic", dirs: ["ev_pic"] },
	{ category: "stadium", dirs: ["stadium"] },
	{ category: "vsroute_map", dirs: ["vsroute_map"] },
	{ category: "hlp", dirs: ["hlp"] },
	{ category: "telop_waza", dirs: ["telop_waza/fr", "telop_waza/en"] },
];

interface GalleryManifestItem {
	id: string;
	dir: string;
	file: string;
	title: string;
	category: string;
}

/** Titre lisible : retire les préfixes décoratifs, remplace `_` par espaces, Title Case. */
function titleOf(name: string): string {
	const cleaned = name
		.replace(/^(img_|back_|hlp_|grid_)/, "")
		.replaceAll("_", " ")
		.trim();
	return cleaned.replace(/\b\w/g, (c) => c.toUpperCase()) || name;
}

const src = resolveDataFile(CPK_INDEX);
if (!src) {
	console.error(`[gallery-manifest] ${CPK_INDEX} introuvable (apps/azalee/data/).`);
	process.exit(1);
}

/** `220_img/<dir>` → basenames `.g4tx` DIRECTEMENT dedans (pas de descente récursive). */
const byDir = new Map<string, string[]>();
for (const line of gunzipSync(readFileSync(src)).toString("utf8").split("\n")) {
	if (!line) continue;
	const p = (JSON.parse(line) as [string, string])[0];
	if (!p.startsWith(IMG_ROOT) || !p.endsWith(".g4tx")) continue;
	const rest = p.slice(IMG_ROOT.length, -".g4tx".length);
	const slash = rest.lastIndexOf("/");
	if (slash < 0) continue;
	const dir = rest.slice(0, slash);
	const name = rest.slice(slash + 1);
	const bucket = byDir.get(dir);
	if (bucket) bucket.push(name);
	else byDir.set(dir, [name]);
}

const items: GalleryManifestItem[] = [];
const counts: Record<string, number> = {};

for (const def of MENU_DIRS) {
	let n = 0;
	for (const dir of def.dirs) {
		const names = (byDir.get(dir) ?? []).slice().sort();
		for (const name of names) {
			items.push({
				id: `menu_${dir.replaceAll("/", "_")}_${name}`,
				dir,
				file: `${name}.png`,
				title: titleOf(name),
				category: def.category,
			});
			n += 1;
		}
	}
	counts[def.category] = n;
	console.log(`${def.category.padEnd(14)} ${n}`);
}

const out = path.join(DATA_DIR, "menu-gallery-manifest.json");
await Bun.write(out, `${JSON.stringify({ counts, items }, null, 2)}\n`);

console.log("");
console.log(`total illustrations menu : ${items.length}`);
console.log(`écrit : ${out}`);
