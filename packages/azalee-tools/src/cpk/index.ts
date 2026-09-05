/**
 * Data layer serveur de l'index CPK — `listDir` + `fileMeta`.
 *
 * Source : NDJSON gzippé tracké `data/cpk-index.ndjson.gz` (~7 Mo, généré par
 * `scripts/build-cpk-index.ts` depuis Redis db3 `iev:file:index`). Au premier
 * accès, on MATÉRIALISE la table `cpk_index` dans un SQLite de cache (singleton
 * process-local) puis on requête en O(log n) via index couvrants. Robuste au
 * build standalone fresh-checkout (pas de binaire 93 Mo en git, pas de symlink).
 *
 * ⚠ module **serveur** : ne JAMAIS importer ce module depuis un composant
 * `"use client"` (il touche `bun:sqlite` / Node fs). Les types + helpers purs
 * (URL CDN) vivent dans `lib/cpk/shared.ts`, client-safe.
 */

import type { Database } from "bun:sqlite";
import { gunzipSync } from "node:zlib";
import { existsSync, mkdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { getCacheDir, resolveDataFile } from "../config";
import { buildSqliteFromEntries, type CpkEntry } from "./materialize";
import {
	cpkAssetKind,
	cpkAssetUrl,
	cpkThumbUrl,
	normalizeDir,
	type CpkAssetKind,
	type CpkDir,
	type CpkFile,
	type CpkListing,
} from "@rosegriffon/azalee/cpk/shared";

export type { CpkDir, CpkFile, CpkListing } from "@rosegriffon/azalee/cpk/shared";

/** Métadonnées d'un fichier unique (pour `?meta=1`). */
export interface CpkFileMeta extends CpkFile {
	top: string;
	sub: string;
	dir: string;
	depth: number;
	/** Famille de contenu CDN (`image` | `model` | `raw`). */
	kind: CpkAssetKind;
	/** URL CDN du contenu décodé (ou `null` si pas de mapping fiable). */
	assetUrl: string | null;
	/** URL vignette WebP redimensionnée (images uniquement, sinon `null`). */
	thumbUrl: string | null;
}

// --- Localisation de l'artefact source (NDJSON gz) -------------------------

function resolveSourcePath(): string | null {
	// `CPK_INDEX_PATH` reste prioritaire (compat) ; sinon la résolution commune
	// de `config.ts` (option explicite → `AZALEE_DATA_DIR` → chemins usuels).
	const pinned = process.env.CPK_INDEX_PATH;
	if (pinned && existsSync(pinned)) return pinned;
	return resolveDataFile("cpk-index.ndjson.gz");
}

// --- Chargement / matérialisation (singleton) ------------------------------

let _db: Database | null = null;

/** Charge le NDJSON gz, parse en entrées `[path, cpk]`. */
function readEntries(srcPath: string): CpkEntry[] {
	const gz = readFileSync(srcPath);
	const ndjson = gunzipSync(gz).toString("utf8");
	const entries: CpkEntry[] = [];
	for (const line of ndjson.split("\n")) {
		if (!line) continue;
		const parsed = JSON.parse(line) as CpkEntry;
		if (Array.isArray(parsed) && parsed.length === 2) entries.push(parsed);
	}
	return entries;
}

function getDb(): Database {
	if (_db) return _db;
	const src = resolveSourcePath();
	if (!src) {
		throw new Error(
			"[cpk] artefact introuvable (data/cpk-index.ndjson.gz). Lancer `bun scripts/build-cpk-index.ts`.",
		);
	}

	// SQLite de cache : reconstruit si absent ou plus ancien que la source. On
	// utilise un répertoire de cache stable (réutilisé entre requêtes/process).
	const cacheDir = process.env.CPK_CACHE_DIR ?? getCacheDir("azalee-cpk");
	mkdirSync(cacheDir, { recursive: true });
	const cachePath = path.join(cacheDir, "cpk-index.sqlite");

	const needsBuild =
		!existsSync(cachePath) || statSync(cachePath).mtimeMs < statSync(src).mtimeMs;

	// `bun:sqlite` (runtime Bun, prod `bun server.js`) chargé via un require
	// LITTÉRAL sous garde `typeof Bun` : Turbopack tolère le specifier littéral
	// (marqué external), mais REFUSE un require computé (`Cannot find module as
	// expression is too dynamic`). Le build de prod tourne sous Node mais cette
	// page est `force-dynamic` (jamais prerendue) → `getDb()` n'exécute le require
	// qu'au runtime Bun. Cf. `lib/supabase/sqlite-client.ts` (même garde).
	if (typeof Bun === "undefined") {
		throw new Error("[cpk] bun:sqlite requis (runtime Bun) — indisponible ici.");
	}
	// eslint-disable-next-line @typescript-eslint/no-require-imports -- require littéral sous garde Bun
	const { Database: DBConstructor } = require("bun:sqlite") as typeof import("bun:sqlite");

	if (needsBuild) {
		const db = new DBConstructor(cachePath, { create: true });
		buildSqliteFromEntries(db, readEntries(src));
		_db = db;
	} else {
		_db = new DBConstructor(cachePath, { readonly: true });
		_db.exec("PRAGMA temp_store = MEMORY");
	}
	return _db;
}

// --- API serveur ------------------------------------------------------------

interface SubRow {
	seg: string;
	count: number;
}
interface FileRow {
	name: string;
	ext: string;
	cpk: string;
	path: string;
}

/** Listing paginé : sous-dossiers (toujours complets) + page de fichiers. */
export interface CpkPagedListing extends CpkListing {
	/** Nombre total de fichiers directs du répertoire (avant pagination). */
	fileTotal: number;
	/** Offset de la page de fichiers retournée. */
	fileOffset: number;
}

/**
 * Liste un répertoire de l'index : sous-dossiers directs (avec compte récursif)
 * + fichiers directs. `path` vide ou `data` → racine logique (`common`, `dx11`…).
 *
 * Les sous-dossiers sont calculés par extraction du segment suivant chez les
 * descendants (`path GLOB dir/*`), groupés et comptés. Les fichiers directs
 * matchent `dir = <path>` exactement.
 */
export function listDir(dirPath: string): CpkListing {
	const db = getDb();
	const dir = normalizeDir(dirPath);

	// Racine : on liste les `top` (1er segment après `data/`). On présente sous
	// la forme `data` pour rester cohérent avec les chemins complets de l'index.
	const isRoot = dir === "" || dir === "data";

	if (isRoot) {
		const tops = db
			.query(
				"SELECT top AS seg, COUNT(*) AS count FROM cpk_index GROUP BY top ORDER BY top",
			)
			.all() as SubRow[];
		return {
			dir: "data",
			dirs: tops.map((t) => ({ name: t.seg, count: t.count }) satisfies CpkDir),
			files: [],
		};
	}

	const prefix = `${dir}/`;
	// Sous-dossiers directs : segment immédiatement après `dir/`, chez tout
	// descendant dont le `dir` n'est pas exactement `dir` (= dans un sous-arbre).
	// `substr` extrait le 1er segment relatif, `instr(...||'/', '/')` borne au slash.
	const subs = db
		.query(
			`SELECT seg, COUNT(*) AS count FROM (
				SELECT substr(
					substr(path, ?2),
					1,
					instr(substr(path, ?2) || '/', '/') - 1
				) AS seg
				FROM cpk_index
				WHERE path GLOB ?1 AND dir <> ?3
			)
			GROUP BY seg ORDER BY seg`,
		)
		.all(`${prefix}*`, prefix.length + 1, dir) as SubRow[];

	const files = db
		.query(
			"SELECT name, ext, cpk, path FROM cpk_index WHERE dir = ? ORDER BY name",
		)
		.all(dir) as FileRow[];

	return {
		dir,
		dirs: subs
			.filter((s) => s.seg.length > 0)
			.map((s) => ({ name: s.seg, count: s.count }) satisfies CpkDir),
		files: files.map(
			(f) => ({ name: f.name, ext: f.ext, cpk: f.cpk, path: f.path }) satisfies CpkFile,
		),
	};
}

/**
 * Variante paginée de `listDir` : les sous-dossiers (peu nombreux) sont toujours
 * listés en entier, mais les fichiers directs sont tranchés (`LIMIT/OFFSET`) —
 * indispensable pour les répertoires massifs (`common/event` ≈ 55k fichiers).
 * `fileTotal` permet au caller de paginer.
 */
export function listDirPaged(
	dirPath: string,
	limit = 200,
	offset = 0,
): CpkPagedListing {
	const db = getDb();
	const dir = normalizeDir(dirPath);
	const isRoot = dir === "" || dir === "data";

	// Racine : pas de fichiers directs, on délègue à listDir pour les tops.
	if (isRoot) {
		const base = listDir(dir);
		return { ...base, fileTotal: 0, fileOffset: 0 };
	}

	const prefix = `${dir}/`;
	const subs = db
		.query(
			`SELECT seg, COUNT(*) AS count FROM (
				SELECT substr(
					substr(path, ?2),
					1,
					instr(substr(path, ?2) || '/', '/') - 1
				) AS seg
				FROM cpk_index
				WHERE path GLOB ?1 AND dir <> ?3
			)
			GROUP BY seg ORDER BY seg`,
		)
		.all(`${prefix}*`, prefix.length + 1, dir) as SubRow[];

	const fileTotal = (
		db.query("SELECT COUNT(*) AS n FROM cpk_index WHERE dir = ?").get(dir) as {
			n: number;
		}
	).n;

	const files = db
		.query(
			"SELECT name, ext, cpk, path FROM cpk_index WHERE dir = ? ORDER BY name LIMIT ? OFFSET ?",
		)
		.all(dir, limit, offset) as FileRow[];

	return {
		dir,
		dirs: subs
			.filter((s) => s.seg.length > 0)
			.map((s) => ({ name: s.seg, count: s.count }) satisfies CpkDir),
		files: files.map(
			(f) => ({ name: f.name, ext: f.ext, cpk: f.cpk, path: f.path }) satisfies CpkFile,
		),
		fileTotal,
		fileOffset: offset,
	};
}

interface MetaRow {
	path: string;
	top: string;
	sub: string;
	dir: string;
	name: string;
	ext: string;
	cpk: string;
	depth: number;
}

/**
 * Métadonnées d'un fichier de l'index + URL CDN de son contenu. `null` si le
 * chemin n'existe pas dans l'index (= ce n'est pas un fichier, peut-être un
 * dossier — le caller bascule sur `listDir`).
 */
export function fileMeta(filePath: string): CpkFileMeta | null {
	const db = getDb();
	const target = normalizeDir(filePath);
	const row = db
		.query(
			"SELECT path, top, sub, dir, name, ext, cpk, depth FROM cpk_index WHERE path = ? LIMIT 1",
		)
		.get(target) as MetaRow | undefined;
	if (!row) return null;

	return {
		path: row.path,
		top: row.top,
		sub: row.sub,
		dir: row.dir,
		name: row.name,
		ext: row.ext,
		cpk: row.cpk,
		depth: row.depth,
		kind: cpkAssetKind(row.ext),
		assetUrl: cpkAssetUrl(row.path, row.ext),
		thumbUrl: cpkThumbUrl(row.path, row.ext),
	};
}

/**
 * Recherche globale de fichiers par sous-chaîne de **nom** (case-insensitive), bornée.
 * Sert la search app bar de l'explorateur — recouvre les 250 800 entrées via l'index couvrant.
 */
export function searchFiles(query: string, limit = 200): CpkFile[] {
	const q = query.trim().toLowerCase();
	if (q.length < 2) return [];
	const db = getDb();
	// Recherche sur le CHEMIN complet, pas seulement le nom de fichier : c'est le
	// contrat annoncé, et un dossier comme `telop_waza` ou `220_img` ne porte le
	// nom d'aucun fichier — la recherche renvoyait donc zéro résultat sur 11 208
	// fichiers réellement présents, ce qui fait conclure à tort qu'ils n'existent
	// pas. Le chemin se termine par le nom : chercher un nom continue de marcher.
	// L'ordre reste `length(name)` d'abord, pour que la correspondance la plus
	// précise remonte en tête.
	const rows = db
		.query(
			"SELECT path, name, ext, cpk FROM cpk_index WHERE path LIKE ?1 ESCAPE '\\' ORDER BY length(name), name LIMIT ?2",
		)
		.all(`%${q.replace(/[%_\\]/g, "\\$&")}%`, limit) as {
		path: string;
		name: string;
		ext: string;
		cpk: string;
	}[];
	return rows.map((r) => ({ name: r.name, ext: r.ext, cpk: r.cpk, path: r.path }));
}

/** Nombre total de fichiers indexés (sanity / health). */
export function totalFiles(): number {
	const db = getDb();
	return (db.query("SELECT COUNT(*) AS n FROM cpk_index").get() as { n: number }).n;
}
