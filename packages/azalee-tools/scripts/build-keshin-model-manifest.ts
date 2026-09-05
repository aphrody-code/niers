#!/usr/bin/env bun
/**
 * Génère `data/keshin-model-manifest.json` : le gate des modèles 3D COMPLETS
 * keshin (`k<NNNNNN>`) + armures (`ka<NNNNNN>NN`) réellement servis live par
 * `nie-model-serve` sur `cdn.rosegriffon.fr/model-full/<code>.glb`.
 *
 * Vérité terrain = probe HTTP du vrai endpoint (assemblage live depuis les CPK),
 * PAS la DB ni le dump. Un code n'est gardé que si `/model-full/<code>.glb`
 * répond 200 (les keshin en `.g4mg`-seul ne sont pas encore assemblables → 404
 * propre côté serveur → exclus du gate → pas de bouton 3D mort côté azalee).
 *
 * Le viewer (`AuraDetail`, page `/keshin`) gate sur ce manifeste via
 * `getKeshinModelGlbUrl` / `getArmureModelGlbUrl` (src/lib/images/utils.ts).
 * Régénérer quand la couche serving gagne le support g4mg (les 92 keshin restants).
 *
 * Source codes : index Redis db3 `iev:file:index` (HASH path->cpk), filtré sur
 * `_keshin/k*` et `_armd/ka*`. Fallback : scan du dump local si Redis absent.
 *
 * Run : `bun scripts/build-keshin-model-manifest.ts`
 */
import { readdir } from "node:fs/promises";
import { $ } from "bun";

import path from "node:path";

/** Données figées du package (`packages/azalee/src/data`). */
const DATA_DIR = path.resolve(import.meta.dir, "../src/data");

const MODEL_FULL_BASE = "https://cdn.rosegriffon.fr/model-full";
const REDIS_DB = process.env.IEV_REDIS_DB ?? "3";
const DUMP_DIR =
	process.env.IECODE_DUMP_DIR ?? "/home/ubuntu/.local/share/Steam/iecode/inazuma/data";

/** Récupère tous les codes keshin/armd depuis l'index Redis (canonique, via redis-cli). */
async function codesFromRedis(): Promise<{ keshin: string[]; armd: string[] } | null> {
	try {
		const out =
			await $`redis-cli -n ${REDIS_DB} HKEYS iev:file:index`.quiet().text();
		const keshin = new Set<string>();
		const armd = new Set<string>();
		for (const p of out.split("\n")) {
			const k = p.match(/_keshin\/(k\d+)\//);
			if (k?.[1]) keshin.add(k[1]);
			const a = p.match(/_armd\/(ka\d+)\//);
			if (a?.[1]) armd.add(a[1]);
		}
		if (keshin.size === 0 && armd.size === 0) return null;
		return { armd: [...armd].sort(), keshin: [...keshin].sort() };
	} catch {
		return null;
	}
}

/** Fallback : scan du dump local `common/chr/_keshin` + `_armd`. */
async function codesFromDump(): Promise<{ keshin: string[]; armd: string[] }> {
	const keshin = new Set<string>();
	const armd = new Set<string>();
	try {
		for (const d of await readdir(`${DUMP_DIR}/common/chr/_keshin`)) {
			if (/^k\d+$/.test(d)) keshin.add(d);
		}
	} catch {
		/* dossier absent */
	}
	try {
		for (const d of await readdir(`${DUMP_DIR}/common/chr/_armd`)) {
			if (/^ka\d+$/.test(d)) armd.add(d);
		}
	} catch {
		/* dossier absent */
	}
	return { armd: [...armd].sort(), keshin: [...keshin].sort() };
}

/** Probe live : `/model-full/<code>.glb` répond-il 200 ? (concurrence bornée). */
async function probeServeable(codes: string[], concurrency = 16): Promise<string[]> {
	const ok: string[] = [];
	let i = 0;
	async function worker() {
		while (i < codes.length) {
			const code = codes[i++];
			if (!code) continue;
			try {
				const res = await fetch(`${MODEL_FULL_BASE}/${code}.glb`, { method: "HEAD" });
				if (res.ok) ok.push(code);
			} catch {
				/* réseau : code écarté */
			}
		}
	}
	await Promise.all(Array.from({ length: concurrency }, worker));
	return ok.sort();
}

const codes = (await codesFromRedis()) ?? (await codesFromDump());
if (codes.keshin.length === 0 && codes.armd.length === 0) {
	console.error("Aucun code keshin/armd trouvé (ni Redis ni dump).");
	process.exit(1);
}

const [keshin, armures] = await Promise.all([
	probeServeable(codes.keshin),
	probeServeable(codes.armd),
]);

const manifest = { armures, keshin };
const out = path.join(DATA_DIR, "keshin-model-manifest.json");
await Bun.write(out, JSON.stringify(manifest, null, 0));

console.log(`keshin servables : ${keshin.length}/${codes.keshin.length}`);
console.log(`armures servables : ${armures.length}/${codes.armd.length}`);
console.log(`Written: ${out}`);
