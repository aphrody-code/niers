/**
 * Data layer serveur de l'index de texte de jeu IEVR — résout un `hashId` (ou
 * une catégorie) vers le VRAI texte du jeu (noms perso/objet/technique,
 * descriptions, dialogues…) dans les 3 langues décodées (fr/en/ja).
 *
 * Source : NDJSON gzippés trackés `data/game-text-names.ndjson.gz` (43+ dicts
 * racine, ~3,5 Mo) + `data/game-text-dialogue.ndjson.gz` (event/map/phase/purpose,
 * ~2,1 Mo), générés par `scripts/build-game-text-index.ts` depuis le dump via
 * `@rosegriffon/inagle`. Au premier accès on MATÉRIALISE la table `text` dans un
 * SQLite de cache (singleton process-local) puis on requête en O(log n).
 *
 * ⚠ module **serveur** : ne JAMAIS importer ce module depuis un composant
 * `"use client"` (il touche `bun:sqlite` / Node fs). Les types + helpers purs
 * vivent dans `lib/game-text/shared.ts`, client-safe.
 */

import type { Database } from "bun:sqlite";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { getCacheDir, resolveDataFile } from "../config";
import { gunzipSync } from "node:zlib";
import { buildSqliteFromEntries, type TextEntry } from "./materialize";
import {
	type GameText,
	type GameTextLocale,
	type GameTextLocalized,
	normalizeHashId,
} from "./shared";

export type { GameText, GameTextLocalized } from "./shared";
export { normalizeHashId, pickLocale } from "./shared";

// --- Localisation des artefacts source (NDJSON gz) -------------------------

const ARTEFACTS = ["game-text-names.ndjson.gz", "game-text-dialogue.ndjson.gz"];

function resolveSourcePaths(): string[] {
	// `GAME_TEXT_DATA_DIR` reste prioritaire (compat) ; sinon la résolution
	// commune de `config.ts`.
	const pinnedRoot = process.env.GAME_TEXT_DATA_DIR;
	const found: string[] = [];
	for (const name of ARTEFACTS) {
		if (pinnedRoot) {
			const pinned = path.join(pinnedRoot, name);
			if (existsSync(pinned)) {
				found.push(pinned);
				continue;
			}
		}
		const resolved = resolveDataFile(name);
		if (resolved) found.push(resolved);
	}
	return found;
}

// --- Chargement / matérialisation (singleton) ------------------------------

let _db: Database | null = null;

function readEntries(srcPath: string): TextEntry[] {
	const gz = readFileSync(srcPath);
	const ndjson = gunzipSync(gz).toString("utf8");
	const entries: TextEntry[] = [];
	for (const line of ndjson.split("\n")) {
		if (!line) continue;
		const parsed = JSON.parse(line) as TextEntry;
		if (Array.isArray(parsed) && parsed.length === 4) entries.push(parsed);
	}
	return entries;
}

/** Ouvre (ou matérialise) le SQLite de l'index de texte. Singleton process-local. */
function getDb(): Database {
	if (_db) return _db;
	// Import dynamique : `bun:sqlite` n'est résolu que côté serveur Bun.
	const { Database } = require("bun:sqlite") as typeof import("bun:sqlite");

	const sources = resolveSourcePaths();
	if (sources.length === 0) {
		throw new Error(
			"[game-text] aucun artefact game-text-*.ndjson.gz trouvé (lancer scripts/build-game-text-index.ts)",
		);
	}

	const cacheDir = getCacheDir("azalee-game-text");
	mkdirSync(cacheDir, { recursive: true });
	const dbPath = path.join(cacheDir, "game-text.sqlite");

	const db = new Database(dbPath);
	const all: TextEntry[] = [];
	for (const src of sources) all.push(...readEntries(src));
	buildSqliteFromEntries(db, all);

	_db = db;
	return db;
}

// --- API de lecture --------------------------------------------------------

/** Résout un hashId vers ses textes fr/en/ja (+ catégorie). `undefined` si inconnu. */
export function getText(hashId: string | number): GameTextLocalized | undefined {
	const key = normalizeHashId(hashId);
	const rows = getDb()
		.query<{ locale: string; category: string; value: string }, [string]>(
			"SELECT locale, category, value FROM text WHERE hashId = ?",
		)
		.all(key);
	if (rows.length === 0) return undefined;
	const out: GameTextLocalized = { hashId: key, category: rows[0]?.category };
	for (const r of rows) {
		if (r.locale === "fr") out.fr = r.value;
		else if (r.locale === "en") out.en = r.value;
		else if (r.locale === "ja") out.ja = r.value;
	}
	return out;
}

/** Résout un hashId dans une langue précise (avec repli fr → en → ja). */
export function getTextLocale(hashId: string | number, locale: GameTextLocale = "fr"): string | undefined {
	const t = getText(hashId);
	if (!t) return undefined;
	return t[locale] ?? t.fr ?? t.en ?? t.ja;
}

/** Résout plusieurs hashIds d'un coup (1 requête par lot de 500). */
export function getTexts(hashIds: Array<string | number>): Map<string, GameTextLocalized> {
	const out = new Map<string, GameTextLocalized>();
	const keys = [...new Set(hashIds.map(normalizeHashId))];
	const db = getDb();
	for (let i = 0; i < keys.length; i += 500) {
		const batch = keys.slice(i, i + 500);
		const placeholders = batch.map(() => "?").join(",");
		const rows = db
			.query<{ hashId: string; locale: string; category: string; value: string }, string[]>(
				`SELECT hashId, locale, category, value FROM text WHERE hashId IN (${placeholders})`,
			)
			.all(...batch);
		for (const r of rows) {
			let entry = out.get(r.hashId);
			if (!entry) {
				entry = { hashId: r.hashId, category: r.category };
				out.set(r.hashId, entry);
			}
			if (r.locale === "fr") entry.fr = r.value;
			else if (r.locale === "en") entry.en = r.value;
			else if (r.locale === "ja") entry.ja = r.value;
		}
	}
	return out;
}

/** Liste les entrées d'une catégorie pour une langue (pagination simple). */
export function listCategory(
	category: string,
	locale: GameTextLocale = "fr",
	limit = 200,
	offset = 0,
): GameText[] {
	return getDb()
		.query<GameText, [string, string, number, number]>(
			"SELECT hashId, locale, category, value FROM text WHERE category = ? AND locale = ? ORDER BY hashId LIMIT ? OFFSET ?",
		)
		.all(category, locale, limit, offset);
}

/** Recherche plein-texte naïve (`value LIKE %q%`) dans une langue. */
export function searchText(query: string, locale: GameTextLocale = "fr", limit = 50): GameText[] {
	return getDb()
		.query<GameText, [string, string, number]>(
			"SELECT hashId, locale, category, value FROM text WHERE locale = ? AND value LIKE ? LIMIT ?",
		)
		.all(locale, `%${query}%`, limit);
}

/** Stats : nombre d'entrées par catégorie (langue fr). */
export function categoryStats(): Array<{ category: string; count: number }> {
	return getDb()
		.query<{ category: string; count: number }, [string]>(
			"SELECT category, COUNT(*) as count FROM text WHERE locale = ? GROUP BY category ORDER BY count DESC",
		)
		.all("fr");
}
