/**
 * Génère `data/chr-model-manifest.json` — les codes de modèles 3D `common/chr/_<sub>/` réellement
 * servables par `cdn.rosegriffon.fr/model-chr/<sub>/<code>.glb` (gate anti-404 de la galerie
 * `/modeles` et de `getChrModelGlbUrl`).
 *
 * Vérité terrain : l'index CPK (`data/cpk-index.ndjson.gz`). Un code est servable si son dossier
 * `data/common/chr/_<sub>/<code>/` contient un maillage (`.g4mg` libre ou `.g4pkm` packé) — la route
 * `model-chr` assemble depuis ça. Validé par probe CDN (échantillon des 6 préfixes `_waza` → 200).
 *
 * ⚠ L'ancien manifeste était gravement périmé (waza=5 alors que 272 existent réellement) → la
 * galerie ne montrait qu'une poignée de techniques. Régénérer après tout rebuild de
 * `cpk-index.ndjson.gz` :  bun apps/azalee/scripts/build-chr-model-manifest.ts
 */
import { gunzipSync } from "node:zlib";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { resolveDataFile } from "../src/config";

/** Données figées du package (`packages/azalee/src/data`). */
const DATA_DIR = path.resolve(import.meta.dir, "../src/data");

const INDEX =
	resolveDataFile("cpk-index.ndjson.gz") ??
	(() => {
		throw new Error("cpk-index.ndjson.gz introuvable — lancer apps/azalee/scripts/build-cpk-index.ts");
	})();
const OUT = path.join(DATA_DIR, "chr-model-manifest.json");

/** Sous-domaines chr exposés par la galerie générique `/modeles` (keshin/armd ont leur manifeste). */
const SUBS = ["waza", "item", "animal"] as const;

const paths = gunzipSync(readFileSync(INDEX))
	.toString("utf8")
	.split("\n")
	.filter(Boolean)
	.map((line) => (JSON.parse(line) as [string, string])[0]);

const manifest: Record<string, unknown> = {};
/** sub → code → basename RÉEL du `.g4tx` de texture (sans extension). */
const textures: Record<string, Record<string, string>> = {};

for (const sub of SUBS) {
	// code → set d'extensions présentes sous common/chr/_<sub>/<code>/
	const exts = new Map<string, Set<string>>();
	const re = new RegExp(`^data/common/chr/_${sub}/([^/]+)/(.+)$`);
	for (const p of paths) {
		const m = p.match(re);
		if (!m) continue;
		if (!exts.has(m[1])) exts.set(m[1], new Set());
		exts.get(m[1])!.add(m[2].split(".").pop()!.toLowerCase());
	}
	const codes = [...exts.entries()]
		.filter(([, e]) => e.has("g4mg") || e.has("g4pkm"))
		.map(([code]) => code)
		.sort();
	manifest[sub] = codes;

	// Les TEXTURES ne vivent pas à côté des maillages : elles sont sous `dx11/`, pas
	// `common/`. Et leur nom n'est PAS toujours celui du dossier — 11 objets n'existent
	// qu'en `<code>_10`/`_20` (variantes de niveau de détail). Forger `<code>/<code>.png`
	// pour ceux-là donne un 404 ; on relève donc le basename réel.
	const basenames = new Map<string, string[]>();
	const reTex = new RegExp(`^data/dx11/chr/_${sub}/([^/]+)/(.+)\\.g4tx$`);
	for (const p of paths) {
		const m = p.match(reTex);
		if (!m) continue;
		if (!basenames.has(m[1])) basenames.set(m[1], []);
		basenames.get(m[1])!.push(m[2]);
	}
	const map: Record<string, string> = {};
	for (const code of codes) {
		const dispo = basenames.get(code);
		if (!dispo?.length) continue; // 27 objets n'ont AUCUNE texture : trou de données réel.
		// Exact d'abord, sinon la variante de plus petit suffixe (déterministe).
		map[code] = dispo.includes(code) ? code : [...dispo].sort()[0]!;
	}
	textures[sub] = map;
}
manifest.textures = textures;

writeFileSync(OUT, `${JSON.stringify(manifest, null, 0)}\n`);
console.log(
	`chr-model-manifest → ${OUT}\n  ${SUBS.map(
		(s) => `${s}=${(manifest[s] as string[]).length} tex=${Object.keys(textures[s]!).length}`,
	).join("  ")}`,
);
