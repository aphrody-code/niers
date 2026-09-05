#!/usr/bin/env bun
/**
 * Génère `data/character-model-manifest.json` : la liste EXHAUSTIVE des basenames
 * `<base>` qui possèdent réellement un modèle 3D GLB dans le dump dx11
 * (`data/dx11/model/<base>.glb`). Vérité terrain = scan direct du dossier servi par
 * nginx (`cdn.rosegriffon.fr/model/`), pas la DB.
 *
 * `getCharacterModelGlbUrl(code)` (src/lib/images/utils.ts) strippe le suffixe de
 * variante (`_5000`/`_5100`…) puis lookup le `<base>` dans ce manifest : présent →
 * URL `cdn.rosegriffon.fr/model/<base>.glb`, absent → `null` (pas de bouton 3D, pas
 * de 404). Le basename du GLB = `internalCode` du perso (`c<NNNNNNNN>`) pour les
 * personnages ; le dump contient aussi des accessoires/équipements (`accessory*`,
 * `e*`, `h*`…) gardés ici par cohérence avec le dossier réel.
 *
 * Source dump : $IECODE_DUMP_DIR/dx11/model/ (défaut : le dump Steam local).
 *
 * Run : `bun scripts/build-character-model-manifest.ts`
 */
import { readdir } from "node:fs/promises";

import path from "node:path";

/** Données figées du package (`packages/azalee/src/data`). */
const DATA_DIR = path.resolve(import.meta.dir, "../src/data");

const DUMP_DIR =
	process.env.IECODE_DUMP_DIR ?? "/home/ubuntu/.local/share/Steam/iecode/inazuma/data";
const MODEL_DIR = `${DUMP_DIR}/dx11/model`;

let entries: string[];
try {
	entries = await readdir(MODEL_DIR);
} catch (err) {
	console.error(`Cannot read model dump dir ${MODEL_DIR}: ${(err as Error).message}`);
	process.exit(1);
}

const bases = new Set<string>();
for (const name of entries) {
	if (name.endsWith(".glb")) {
		bases.add(name.slice(0, -".glb".length));
	}
}

const sorted = [...bases].sort();
const out = path.join(DATA_DIR, "character-model-manifest.json");
await Bun.write(out, JSON.stringify(sorted, null, 0));

console.log(`GLB basenames: ${sorted.length}`);
console.log(`Written: ${out}`);
