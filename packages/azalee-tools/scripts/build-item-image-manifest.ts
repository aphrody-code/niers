#!/usr/bin/env bun
/**
 * Génère `src/data/item-image-manifest.json` : `internal_code` d'objet →
 * **(conteneur `.g4tx`, nom de texture)** réels dans les CPK.
 *
 * Pourquoi un manifeste plutôt qu'une règle : le dossier écrit en base
 * (`image_url = 200_icon/02_icon_item/<code>.webp`) est FAUX pour 522 objets sur
 * 1300 — `coa_animal_an000100` vit dans `22_icon_town/icon_animal.g4tx`, les `em*`
 * dans `01_icon_emblem/`, les `ds*` dans `20_icon_deco/`, les plaques `nm*` dans
 * `25_icon_nameplate/`. Le seul localisateur fiable est le couple (conteneur, nom
 * de texture), fourni par l'index `icon-texture-index.ndjson.gz`.
 *
 * Sources :
 *  - miroir SQLite `apps/azalee/data/backups/mirror.sqlite` (`inagle_items`) ;
 *  - index texture → conteneur (`@rose-griffon/azalee/icon-index`).
 *
 * Format (compact, pour ne pas gonfler le bundle navigateur — `utils.ts` est
 * client-safe et importe ce JSON statiquement) :
 *
 * ```json
 * { "containers": ["200_icon/02_icon_item/icon_item05.g4tx", …],
 *   "items": { "eq_ac0100101": 0, "nm03103": [7, "nm03103_01"] } }
 * ```
 * une valeur numérique = index de conteneur, la texture portant le nom de l'objet ;
 * un couple `[index, nom]` = texture au nom différent du code.
 *
 * L'URL finale est `<CDN>/dx11/menu/<conteneur>/<texture>.png` (contrat de texture
 * nommée servi par `nie-model-serve`).
 *
 * Run : `bun packages/azalee/scripts/build-item-image-manifest.ts`
 */
import { Database } from "bun:sqlite";
import path from "node:path";
import { resolveMirrorPath } from "../src/config";
import { resolveIconTexture } from "../src/icon-index";

const DATA_DIR = path.resolve(import.meta.dir, "../src/data");
const OUT = path.join(DATA_DIR, "item-image-manifest.json");

const mirror = resolveMirrorPath();
if (!mirror) {
	console.error("[item-manifest] miroir SQLite introuvable (apps/azalee/data/backups/).");
	process.exit(1);
}

const db = new Database(mirror, { readonly: true });
const rows = db
	.query("select internal_code from inagle_items where internal_code is not null and internal_code != ''")
	.all() as Array<{ internal_code: string }>;

const codes = [...new Set(rows.map((r) => r.internal_code.trim()).filter(Boolean))].sort();

const containers: string[] = [];
const containerIndex = new Map<string, number>();
const items: Record<string, number | [number, string]> = {};
const byFolder = new Map<string, number>();
let unresolved = 0;

for (const code of codes) {
	const ref = resolveIconTexture(code);
	if (!ref) {
		unresolved += 1;
		continue;
	}
	let idx = containerIndex.get(ref.g4txPath);
	if (idx === undefined) {
		idx = containers.length;
		containers.push(ref.g4txPath);
		containerIndex.set(ref.g4txPath, idx);
	}
	items[code] = ref.textureName === code ? idx : [idx, ref.textureName];
	const folder = ref.g4txPath.split("/").slice(0, 2).join("/");
	byFolder.set(folder, (byFolder.get(folder) ?? 0) + 1);
}

await Bun.write(
	OUT,
	`${JSON.stringify({ containers, items: Object.fromEntries(Object.entries(items).sort()) }, null, 2)}\n`,
);

console.log(`codes objets distincts : ${codes.length}`);
console.log(`résolus                : ${Object.keys(items).length}`);
console.log(`non résolus            : ${unresolved}`);
console.log(`conteneurs distincts   : ${containers.length}`);
for (const [folder, n] of [...byFolder].sort((a, b) => b[1] - a[1])) {
	console.log(`  ${String(n).padStart(5)}  ${folder}`);
}
console.log(`écrit : ${OUT}`);
