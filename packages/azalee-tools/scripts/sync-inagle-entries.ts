#!/usr/bin/env bun
/**
 * Resynchronise les entrées inagle embarquées dans `src/data/`.
 *
 * La lib est **autonome** (CLI, sidecar Tauri, publication npm) : elle ne lit
 * jamais un chemin relatif hors de son propre package. Les quelques entrées
 * inagle dont elle dépend sont donc copiées ici, et rafraîchies par ce script
 * après un re-dump du jeu :
 *
 *     bun packages/azalee/scripts/sync-inagle-entries.ts
 */

import path from "node:path";

const ENTRIES: Array<{ from: string; to: string }> = [
	{ from: "change_aura_skills.json", to: "change-aura-skills.json" },
];

const pkgRoot = path.resolve(import.meta.dir, "..");
const inagleEntries = path.resolve(pkgRoot, "../inagle/src/entries");

let changed = 0;
for (const entry of ENTRIES) {
	const src = Bun.file(path.join(inagleEntries, entry.from));
	if (!(await src.exists())) {
		console.error(`source absente: ${entry.from}`);
		process.exit(1);
	}
	const destPath = path.join(pkgRoot, "src/data", entry.to);
	const before = await Bun.file(destPath)
		.text()
		.catch(() => "");
	const after = await src.text();
	if (before !== after) {
		await Bun.write(destPath, after);
		changed++;
	}
	console.log(`${entry.to} ${before === after ? "à jour" : "mis à jour"}`);
}
console.log(`entries=${ENTRIES.length} changed=${changed}`);
