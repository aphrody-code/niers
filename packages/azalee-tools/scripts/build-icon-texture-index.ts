#!/usr/bin/env bun
/**
 * Génère `apps/azalee/data/icon-texture-index.ndjson.gz` : l'index
 * **nom de texture → conteneur g4tx** de tout le namespace icône du jeu
 * (`data/dx11/menu/200_icon/**`).
 *
 * ## Pourquoi
 *
 * Un `.g4tx` est un conteneur : `icon_item05.g4tx` porte 80 textures 256×256
 * nommées, chacune avec son propre payload DDS. Le dossier écrit dans
 * `inagle_items.image_url` est faux pour une partie du catalogue
 * (`coa_animal_an000100` est déclaré dans `02_icon_item` alors qu'il vit dans
 * `22_icon_town/icon_animal.g4tx`). Le seul localisateur fiable est le couple
 * **(chemin g4tx, nom de texture)** — d'où cet index matérialisé et committé.
 *
 * ## Source
 *
 * - la liste des `.g4tx` vient de l'artefact `apps/azalee/data/cpk-index.ndjson.gz`
 *   (250 799 chemins CPK réels) ;
 * - les octets de chaque conteneur viennent de `nie-model-serve`
 *   (`http://127.0.0.1:8790/raw/<chemin sans data/>`), qui les lit **live** dans
 *   les CPK — aucun dump disque requis ;
 * - l'en-tête est parsé par le port TypeScript pur `src/icon-index/g4tx-header.ts`,
 *   validé octet à octet contre l'exemple Rust `inspect_g4tx` de `nie-formats`.
 *
 * `10_icon_chr/face` (5676) et `10_icon_chr/uniform` (12 481) sont **exclus** :
 * ce sont des conteneurs 1:1 par personnage, déjà couverts par
 * `character-face-manifest.json` et adressables sans index.
 *
 * Run : `bun packages/azalee/scripts/build-icon-texture-index.ts`
 * Options : `--out <fichier>` `--service <url>` `--concurrency <n>` `--dry-run`
 */

import { gunzipSync, gzipSync } from "node:zlib";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { parseG4tx } from "../src/icon-index/g4tx-header";
import type { IconIndexEntry } from "../src/icon-index/shared";

const REPO_ROOT = path.resolve(import.meta.dir, "../../..");
const CPK_INDEX = path.join(REPO_ROOT, "apps/azalee/data/cpk-index.ndjson.gz");

/** Préfixe du namespace icône dans l'index CPK. */
const ICON_PREFIX = "data/dx11/menu/200_icon/";

/**
 * Sous-arbres exclus : conteneurs 1:1 par personnage, volumineux et sans
 * bénéfice d'index (le nom de fichier suffit à les adresser).
 */
const EXCLUDED = ["data/dx11/menu/200_icon/10_icon_chr/face/", "data/dx11/menu/200_icon/10_icon_chr/uniform/"];

function arg(name: string, fallback: string): string {
	const i = process.argv.indexOf(`--${name}`);
	return i >= 0 ? (process.argv[i + 1] ?? fallback) : fallback;
}

const OUT = path.resolve(arg("out", path.join(REPO_ROOT, "apps/azalee/data/icon-texture-index.ndjson.gz")));
const SERVICE = arg("service", process.env.NIE_MODEL_SERVE_URL ?? "http://127.0.0.1:8790");
const CONCURRENCY = Number(arg("concurrency", "8"));
const DRY_RUN = process.argv.includes("--dry-run");

// --- 1. Lister les conteneurs du namespace icône --------------------------

function listIconContainers(): string[] {
	const ndjson = gunzipSync(readFileSync(CPK_INDEX)).toString("utf8");
	const paths: string[] = [];
	for (const line of ndjson.split("\n")) {
		if (!line) continue;
		const entry = JSON.parse(line) as [string, string];
		const filePath = entry[0];
		if (!filePath?.startsWith(ICON_PREFIX) || !filePath.endsWith(".g4tx")) continue;
		if (EXCLUDED.some((prefix) => filePath.startsWith(prefix))) continue;
		paths.push(filePath);
	}
	return [...new Set(paths)].sort();
}

// --- 2. Récupérer + parser chaque conteneur -------------------------------

interface Parsed {
	/** Chemin relatif à `dx11/menu/` (celui du contrat d'URL). */
	relative: string;
	mains: string[];
	regions: string[];
	dims: Array<[number, number]>;
	bytes: number;
}

async function fetchAndParse(cpkPath: string): Promise<Parsed | { failure: string }> {
	// `/raw/<chemin>` attend le chemin SANS le préfixe `data/` de l'index CPK.
	const servicePath = cpkPath.replace(/^data\//, "");
	const url = `${SERVICE}/raw/${servicePath}`;
	let response: Response;
	try {
		response = await fetch(url);
	} catch (error) {
		return { failure: `${cpkPath} : réseau (${String(error)})` };
	}
	if (!response.ok) return { failure: `${cpkPath} : HTTP ${response.status}` };

	const bytes = new Uint8Array(await response.arrayBuffer());
	try {
		const g4tx = parseG4tx(bytes);
		const mains: string[] = [];
		const dims: Array<[number, number]> = [];
		const regions: string[] = [];
		for (const texture of g4tx.textures) {
			if (texture.name) {
				mains.push(texture.name);
				dims.push([texture.width, texture.height]);
			}
			for (const sub of texture.subTextures) if (sub.name) regions.push(sub.name);
		}
		return {
			relative: cpkPath.replace(/^data\/dx11\/menu\//, ""),
			mains,
			regions,
			dims,
			bytes: bytes.length,
		};
	} catch (error) {
		return { failure: `${cpkPath} : parse (${(error as Error).message})` };
	}
}

/** Exécute `task` sur `items` avec au plus `limit` requêtes en vol. */
async function mapPool<T, R>(items: T[], limit: number, task: (item: T) => Promise<R>): Promise<R[]> {
	const results: R[] = new Array(items.length);
	let cursor = 0;
	const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
		for (;;) {
			const i = cursor++;
			if (i >= items.length) return;
			results[i] = await task(items[i] as T);
			if ((i + 1) % 100 === 0) console.log(`  … ${i + 1}/${items.length}`);
		}
	});
	await Promise.all(workers);
	return results;
}

// --- 3. Écrire l'artefact -------------------------------------------------

const containers = listIconContainers();
console.log(`Conteneurs g4tx du namespace icône : ${containers.length} (hors face/ et uniform/)`);

const parsed = await mapPool(containers, CONCURRENCY, fetchAndParse);

const ok: Parsed[] = [];
const failures: string[] = [];
for (const result of parsed) {
	if ("failure" in result) failures.push(result.failure);
	else ok.push(result);
}

const lines: string[] = [];
let mainTotal = 0;
let regionTotal = 0;
let downloaded = 0;
for (const entry of ok) {
	mainTotal += entry.mains.length;
	regionTotal += entry.regions.length;
	downloaded += entry.bytes;
	const row: IconIndexEntry = [entry.relative, entry.mains, entry.regions, entry.dims];
	lines.push(JSON.stringify(row));
}

console.log(`Conteneurs lus      : ${ok.length}`);
console.log(`Échecs              : ${failures.length}`);
for (const failure of failures.slice(0, 20)) console.log(`  ! ${failure}`);
if (failures.length > 20) console.log(`  … (+${failures.length - 20} autres)`);
console.log(`Textures principales: ${mainTotal}`);
console.log(`Régions d'atlas     : ${regionTotal}`);
console.log(`Total noms indexés  : ${mainTotal + regionTotal}`);
console.log(`Octets téléchargés  : ${(downloaded / 1024 / 1024).toFixed(1)} Mio`);

if (DRY_RUN) {
	console.log("--dry-run : rien écrit.");
} else {
	const gz = gzipSync(Buffer.from(`${lines.join("\n")}\n`, "utf8"), { level: 9 });
	writeFileSync(OUT, gz);
	console.log(`Écrit ${OUT} (${(gz.length / 1024).toFixed(1)} Kio gz)`);
}
