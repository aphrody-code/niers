/**
 * Accès serveur à l'**index texture → conteneur g4tx** du namespace icône.
 *
 * Source : NDJSON gzippé tracké `apps/azalee/data/icon-texture-index.ndjson.gz`,
 * généré par `packages/azalee/scripts/build-icon-texture-index.ts` en parsant
 * l'en-tête de chaque `.g4tx` de `dx11/menu/200_icon/**` servi par
 * `nie-model-serve` (`/raw/...`). L'artefact est petit (quelques dizaines de
 * milliers de noms) : on le charge intégralement en mémoire dans deux `Map`,
 * pas besoin de matérialiser un SQLite comme pour l'index CPK.
 *
 * ⚠ module **serveur** (`node:fs`, `node:zlib`) : ne jamais l'importer depuis un
 * composant `"use client"`. Les types et la construction d'URL vivent dans le
 * jumeau pur `./shared.ts`, lui client-safe et ré-exporté par la racine du
 * package.
 */

import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { resolveDataFile } from "../config";
import {
	iconTextureUrl,
	normalizeTextureName,
	selectMainTexture,
	type IconIndexEntry,
	type IconTextureKind,
	type IconTextureRef,
} from "./shared";

export * from "./shared";
export * from "./g4tx-header";

/** Nom de l'artefact dans le dossier de données Azalée. */
export const ICON_INDEX_FILENAME = "icon-texture-index.ndjson.gz";

interface LoadedIndex {
	/** nom normalisé → toutes les résolutions, dans l'ordre de priorité. */
	byName: Map<string, IconTextureRef[]>;
	/** chemin du conteneur → ses entrées. */
	byContainer: Map<string, IconIndexEntry>;
	/** basename du `.g4tx` normalisé → conteneurs portant ce nom (repli). */
	byBasename: Map<string, IconIndexEntry[]>;
	/** Nombre total de noms indexés (principales + régions, doublons compris). */
	textureCount: number;
}

let cache: LoadedIndex | null = null;

function readEntries(): IconIndexEntry[] {
	const src = resolveDataFile(ICON_INDEX_FILENAME);
	if (!src) {
		throw new Error(
			`[icon-index] artefact introuvable (data/${ICON_INDEX_FILENAME}). ` +
				"Lancer `bun packages/azalee/scripts/build-icon-texture-index.ts`.",
		);
	}
	const ndjson = gunzipSync(readFileSync(src)).toString("utf8");
	const entries: IconIndexEntry[] = [];
	for (const line of ndjson.split("\n")) {
		if (!line) continue;
		const parsed = JSON.parse(line) as IconIndexEntry;
		if (Array.isArray(parsed) && parsed.length === 4) entries.push(parsed);
	}
	return entries;
}

/**
 * Ordre de priorité des conteneurs quand un même nom de texture existe dans
 * plusieurs `.g4tx`. Plus le score est bas, plus le conteneur est prioritaire.
 *
 * Les dossiers thématiques (objets, emblèmes) passent avant les conteneurs
 * fourre-tout ; à score égal l'ordre alphabétique du chemin tranche, ce qui rend
 * la résolution **déterministe** d'un build à l'autre.
 */
const CONTAINER_PRIORITY: Array<[prefix: string, score: number]> = [
	["200_icon/02_icon_item/", 0],
	["200_icon/01_icon_emblem/", 1],
	["200_icon/22_icon_town/", 2],
	["200_icon/21_icon_avatar/", 3],
	["200_icon/25_icon_nameplate/", 4],
	["200_icon/10_icon_chr/", 5],
];

function containerScore(g4txPath: string): number {
	for (const [prefix, score] of CONTAINER_PRIORITY) {
		if (g4txPath.startsWith(prefix)) return score;
	}
	return 50;
}

function load(): LoadedIndex {
	if (cache) return cache;

	const byName = new Map<string, IconTextureRef[]>();
	const byContainer = new Map<string, IconIndexEntry>();
	const byBasename = new Map<string, IconIndexEntry[]>();
	let textureCount = 0;

	const entries = readEntries();
	// Tri stable des conteneurs → l'ordre d'insertion des candidats est reproductible.
	entries.sort((a, b) => {
		const d = containerScore(a[0]) - containerScore(b[0]);
		return d !== 0 ? d : a[0].localeCompare(b[0]);
	});

	for (const entry of entries) {
		const [g4txPath, mains, regions] = entry;
		byContainer.set(g4txPath, entry);

		const basename = normalizeTextureName(g4txPath.split("/").pop() ?? "");
		if (basename) {
			const bucket = byBasename.get(basename);
			if (bucket) bucket.push(entry);
			else byBasename.set(basename, [entry]);
		}

		const push = (textureName: string, kind: IconTextureKind): void => {
			const key = normalizeTextureName(textureName);
			if (!key) return;
			textureCount += 1;
			const list = byName.get(key);
			if (list) list.push({ g4txPath, textureName, kind });
			else byName.set(key, [{ g4txPath, textureName, kind }]);
		};
		for (const name of mains) push(name, "main");
		for (const name of regions) push(name, "region");
	}

	cache = { byName, byContainer, byBasename, textureCount };
	return cache;
}

/** Vide le cache mémoire (tests, rechargement à chaud). */
export function resetIconIndex(): void {
	cache = null;
}

/**
 * Résout un nom de texture d'icône vers son conteneur g4tx.
 *
 * C'est l'accesseur central : `resolveIconTexture("coa_animal_an000100")` rend
 * `{ g4txPath: "200_icon/22_icon_town/icon_animal.g4tx", textureName:
 * "coa_animal_an000100", kind: "main" }` — le vrai conteneur, pas celui écrit en
 * base. Passer le résultat à `iconTextureUrl()` pour obtenir l'URL CDN.
 *
 * Deux étapes, dans l'ordre :
 *
 * 1. **nom de texture** — le nom figure dans la table de chaînes d'un conteneur,
 *    comme texture principale (`main`) ou comme région d'atlas (`region`). En cas
 *    d'homonymie entre conteneurs, rend le candidat le plus prioritaire (cf.
 *    `CONTAINER_PRIORITY`) ; `resolveIconTextureAll()` rend la liste complète.
 * 2. **basename de conteneur** — repli pour les familles où le code métier nomme
 *    le `.g4tx` et non la texture (plaques de nom : l'objet `nm03103` désigne
 *    `25_icon_nameplate/nm03103.g4tx`, dont les textures s'appellent
 *    `nm03103_01`/`_02`). On rejoue alors `selectMainTexture()`, réplique exacte
 *    de `select_main_texture` côté Rust, pour désigner la texture à servir.
 *
 * @returns la résolution, ou `null` si le nom n'existe dans aucun conteneur indexé.
 */
export function resolveIconTexture(textureName: string): IconTextureRef | null {
	return resolveIconTextureAll(textureName)[0] ?? null;
}

/**
 * Toutes les résolutions d'un nom, par priorité décroissante : d'abord les
 * correspondances par nom de texture, puis le repli par basename de conteneur.
 */
export function resolveIconTextureAll(textureName: string): IconTextureRef[] {
	const index = load();
	const key = normalizeTextureName(textureName);
	const direct = index.byName.get(key);
	if (direct?.length) return direct;

	const containers = index.byBasename.get(key);
	if (!containers?.length) return [];
	const refs: IconTextureRef[] = [];
	for (const entry of containers) {
		const main = selectMainTexture(entry);
		if (main) refs.push({ g4txPath: entry[0], textureName: main, kind: "main" });
	}
	return refs;
}

/** URL CDN directe d'un nom de texture, ou `null` s'il n'est pas indexé. */
export function resolveIconTextureUrl(textureName: string): string | null {
	const ref = resolveIconTexture(textureName);
	return ref ? iconTextureUrl(ref) : null;
}

/** Contenu d'un conteneur indexé (`[chemin, principales, régions]`), ou `null`. */
export function getIconContainer(g4txPath: string): IconIndexEntry | null {
	return load().byContainer.get(g4txPath) ?? null;
}

/** Liste des chemins de conteneurs indexés (ordre de priorité). */
export function listIconContainers(): string[] {
	return [...load().byContainer.keys()];
}

/** Statistiques de l'index chargé. */
export function iconIndexStats(): { containers: number; textures: number; uniqueNames: number } {
	const idx = load();
	return { containers: idx.byContainer.size, textures: idx.textureCount, uniqueNames: idx.byName.size };
}
