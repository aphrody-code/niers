#!/usr/bin/env bun
/**
 * Génère les **garde-fous d'assets menu** à partir de l'index CPK réel
 * (`apps/azalee/data/cpk-index.ndjson.gz`, 250 799 chemins), et NON plus depuis
 * `inagle_game_assets` — cette table indexait le dump PNG pré-extrait qui a été
 * archivé : ses 40 471 lignes `exists = 1` valident aujourd'hui des URL mortes.
 *
 * Sorties (toutes trackées en git, importées statiquement par `src/images/utils.ts`) :
 *
 * - `src/data/menu-asset-manifest.json` — listes EXHAUSTIVES des basenames `.g4tx`
 *   réellement présents dans les CPK pour les familles qui ont besoin d'un gate
 *   (emblèmes, telops par langue, icônes d'aura). Un code absent de ces listes
 *   renvoie un placeholder au lieu d'une URL 404 forgée.
 * - `src/data/emblem-crc-map.json` — `crc32_std(nom de fichier) → nom`, calculé sur
 *   les **543** emblèmes du CPK (l'ancienne carte n'en couvrait que 253).
 *
 * Le CRC est le crc32 STANDARD (IEEE, polynôme réfléchi `0xEDB88320`, init/xorout
 * `0xFFFFFFFF`) du nom de fichier ASCII sans extension — même convention que
 * `uniform-model-map` côté niers.
 *
 * Run : `bun packages/azalee/scripts/build-menu-asset-manifest.ts`
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { gunzipSync } from "node:zlib";
import { resolveDataFile } from "../src/config";

const DATA_DIR = path.resolve(import.meta.dir, "../src/data");
const CPK_INDEX = "cpk-index.ndjson.gz";

/** Racine du namespace menu dans l'index CPK. */
const MENU_ROOT = "data/dx11/menu/";

/** Table CRC32 (IEEE, polynôme réfléchi) — même algorithme que `crc32_std` côté Rust. */
const CRC_TABLE = (() => {
	const t = new Uint32Array(256);
	for (let i = 0; i < 256; i += 1) {
		let c = i;
		for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
		t[i] = c >>> 0;
	}
	return t;
})();

/** `crc32_std` d'une chaîne ASCII, rendu en `0xXXXXXXXX` majuscule. */
export function crc32Std(input: string): string {
	let crc = 0xffffffff;
	for (let i = 0; i < input.length; i += 1) {
		crc = (CRC_TABLE[(crc ^ input.charCodeAt(i)) & 0xff] as number) ^ (crc >>> 8);
	}
	const value = (crc ^ 0xffffffff) >>> 0;
	return `0x${value.toString(16).toUpperCase().padStart(8, "0")}`;
}

/** Lit tous les chemins de l'index CPK. */
function readCpkPaths(): string[] {
	const src = resolveDataFile(CPK_INDEX);
	if (!src) {
		throw new Error(`[menu-manifest] ${CPK_INDEX} introuvable (apps/azalee/data/).`);
	}
	const text = gunzipSync(readFileSync(src)).toString("utf8");
	const out: string[] = [];
	for (const line of text.split("\n")) {
		if (!line) continue;
		out.push((JSON.parse(line) as [string, string])[0]);
	}
	return out;
}

/**
 * Basenames `.g4tx` DIRECTEMENT sous `dx11/menu/<dir>/` (sans descendre dans les
 * sous-dossiers : les langues de `telop_waza` sont demandées explicitement).
 */
function basenamesOf(paths: string[], dir: string): string[] {
	const prefix = `${MENU_ROOT}${dir}/`;
	const out: string[] = [];
	for (const p of paths) {
		if (!p.startsWith(prefix) || !p.endsWith(".g4tx")) continue;
		const rest = p.slice(prefix.length, -".g4tx".length);
		if (rest.includes("/")) continue;
		out.push(rest);
	}
	return out.sort();
}

const paths = readCpkPaths();

const emblems = basenamesOf(paths, "200_icon/01_icon_emblem");
const telopFr = basenamesOf(paths, "220_img/telop_waza/fr");
const telopEn = basenamesOf(paths, "220_img/telop_waza/en");
const auraFs = basenamesOf(paths, "200_icon/10_icon_chr/aura_fs");
const auraSoul = basenamesOf(paths, "200_icon/10_icon_chr/aura_soul");
const auraMixi = basenamesOf(paths, "200_icon/10_icon_chr/aura_mixi");
const auraArmed = basenamesOf(paths, "200_icon/10_icon_chr/aura_armed");
const uniforms = basenamesOf(paths, "200_icon/10_icon_chr/uniform");
const faces = basenamesOf(paths, "200_icon/10_icon_chr/face");

/**
 * Les fichiers d'aura/visage/uniforme portent le suffixe `_l` (« large »). Le gate
 * travaille sur le code métier, donc on le retire ici une bonne fois.
 */
function stripL(names: string[]): string[] {
	return names.filter((n) => n.endsWith("_l")).map((n) => n.slice(0, -2));
}

/** Uniquement les codes uniforme PERSONNELS `u<8>_l` (cf. règle des uniformes). */
const personalUniforms = stripL(uniforms).filter((n) => /^u\d{8}$/.test(n));

const manifest = {
	source: CPK_INDEX,
	emblems,
	telop: { en: telopEn, fr: telopFr },
	aura: {
		armed: stripL(auraArmed),
		fs: stripL(auraFs),
		mixi: stripL(auraMixi),
		soul: stripL(auraSoul),
	},
	uniformsPersonal: personalUniforms,
	counts: {
		emblems: emblems.length,
		telopEn: telopEn.length,
		telopFr: telopFr.length,
		auraArmed: auraArmed.length,
		auraFs: auraFs.length,
		auraMixi: auraMixi.length,
		auraSoul: auraSoul.length,
		uniformsPersonal: personalUniforms.length,
		facesInCpk: faces.length,
	},
};

const crcMap: Record<string, string> = {};
const collisions: string[] = [];
for (const name of emblems) {
	const key = crc32Std(name);
	if (crcMap[key] && crcMap[key] !== name) {
		collisions.push(`${key}: ${crcMap[key]} / ${name}`);
		continue;
	}
	crcMap[key] = name;
}

await Bun.write(
	path.join(DATA_DIR, "menu-asset-manifest.json"),
	`${JSON.stringify(manifest, null, 2)}\n`,
);
await Bun.write(
	path.join(DATA_DIR, "emblem-crc-map.json"),
	`${JSON.stringify(Object.fromEntries(Object.entries(crcMap).sort()), null, 2)}\n`,
);

console.log(`chemins CPK lus            : ${paths.length}`);
console.log(`emblèmes                   : ${emblems.length}`);
console.log(`telop fr / en              : ${telopFr.length} / ${telopEn.length}`);
console.log(
	`aura fs/soul/mixi/armed    : ${auraFs.length}/${auraSoul.length}/${auraMixi.length}/${auraArmed.length}`,
);
console.log(`uniformes personnels u<8>  : ${personalUniforms.length}`);
console.log(`visages dans le CPK        : ${faces.length}`);
console.log(`entrées CRC emblème        : ${Object.keys(crcMap).length}${collisions.length ? ` (collisions: ${collisions.join(", ")})` : ""}`);
