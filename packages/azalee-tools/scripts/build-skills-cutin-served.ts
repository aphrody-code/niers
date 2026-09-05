/**
 * Génère `data/skills-cutin-served.json` — la liste des `event_id_name` de cut-in dont les assets
 * 3D `_waza` existent RÉELLEMENT dans cette build du jeu (gate anti-404).
 *
 * Vérité terrain : l'index CPK (`data/cpk-index.ndjson.gz`). Le modèle de cut-in est servi par
 * `cdn.rosegriffon.fr/model-chr/waza/<event>.glb` qui assemble depuis
 * `data/common/chr/_waza/<event>/<event>.g4mg` ; la texture depuis
 * `data/dx11/chr/_waza/<event>/<event>.g4tx`. Sur les 992 hissatsu à cut-in de `skills-cutin.json`,
 * seuls ~157 ont ce dossier `_waza` (les autres n'ont pas de cut-in 3D dans cette build → 404).
 * Le modèle et la texture sont parfaitement corrélés (157 les deux, 0 partiel) → un seul ensemble.
 *
 * Le résultat (petit, ~2 Ko) est importé client-safe par `lib/skills-cutin.ts` pour n'afficher le
 * viewer 3D + la texture que là où ils existent (cf. CDN, pas de viewer « indisponible »).
 *
 * Régénérer après tout rebuild de `cpk-index.ndjson.gz` ou `skills-cutin.json` :
 *   bun apps/azalee/scripts/build-skills-cutin-served.ts
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
const CUTIN = path.join(DATA_DIR, "skills-cutin.json");
const OUT = path.join(DATA_DIR, "skills-cutin-served.json");

interface CutinFile {
	skills: { cutin: { event_id_name: string } | null }[];
}

const indexPaths = new Set(
	gunzipSync(readFileSync(INDEX))
		.toString("utf8")
		.split("\n")
		.filter(Boolean)
		.map((line) => (JSON.parse(line) as [string, string])[0]),
);

const cutin = JSON.parse(readFileSync(CUTIN, "utf8")) as CutinFile;
const events = Array.from(
	new Set(cutin.skills.map((s) => s.cutin?.event_id_name).filter((e): e is string => Boolean(e))),
);

const served = events
	.filter((ev) => indexPaths.has(`data/common/chr/_waza/${ev}/${ev}.g4mg`))
	.sort();

writeFileSync(
	OUT,
	`${JSON.stringify({ meta: { count: served.length, total: events.length }, events: served }, null, 0)}\n`,
);

console.log(`skills-cutin-served: ${served.length}/${events.length} events servis → ${OUT}`);
