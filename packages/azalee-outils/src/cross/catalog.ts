/**
 * Data layer serveur pour le catalogue d'assets d'Inazuma Eleven Cross.
 *
 * Source : NDJSON gzippé `data/cross/catalog-index.ndjson.gz` (~1.2 Mo).
 * Matérialisé dans un cache SQLite local au 1er accès pour des requêtes instantanées.
 */

import type { Database } from "bun:sqlite";
import { gunzipSync } from "node:zlib";
import { existsSync, mkdirSync, readFileSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

export interface CatalogAsset {
	guid: string;
	key: string;
	type: string;
	bundle: string;
	size: number;
	n_deps: number;
	deps?: string[];
}

function resolveSourcePath(): string | null {
	const candidates = [
		process.env.AZALEE_DATA_DIR
			? path.join(process.env.AZALEE_DATA_DIR, "cross/catalog-index.ndjson.gz")
			: undefined,
		path.resolve(process.cwd(), "data/cross/catalog-index.ndjson.gz"),
		path.resolve(process.cwd(), "apps/azalee/data/cross/catalog-index.ndjson.gz"),
	].filter((c): c is string => Boolean(c));
	for (const c of candidates) {
		if (existsSync(c)) return c;
	}
	return null;
}

let _db: Database | null = null;

function getDb(): Database {
	if (_db) return _db;
	const src = resolveSourcePath();
	if (!src) {
		throw new Error("[cross-catalog] etape obligatoire: catalog-index.ndjson.gz introuvable.");
	}

	const cacheDir = path.join(tmpdir(), "azalee-cross-catalog-v2");
	mkdirSync(cacheDir, { recursive: true });
	const cachePath = path.join(cacheDir, "catalog-index-v2.sqlite");

	const needsBuild =
		!existsSync(cachePath) || statSync(cachePath).mtimeMs < statSync(src).mtimeMs;

	if (typeof Bun === "undefined") {
		throw new Error("[cross-catalog] Bun requis pour bun:sqlite — indisponible.");
	}

	// eslint-disable-next-line @typescript-eslint/no-require-imports
	const { Database: DBConstructor } = require("bun:sqlite") as typeof import("bun:sqlite");

	if (needsBuild) {
		const db = new DBConstructor(cachePath, { create: true });
		buildSqlite(db, src);
		_db = db;
	} else {
		_db = new DBConstructor(cachePath, { readonly: true });
		_db.exec("PRAGMA temp_store = MEMORY");
	}
	return _db;
}

function buildSqlite(db: Database, srcPath: string): void {
	console.log("[cross-catalog] Matérialisation de l'index v2 des Addressables...");
	db.exec("PRAGMA journal_mode = WAL");
	db.exec("DROP TABLE IF EXISTS cross_catalog");
	db.exec(`
		CREATE TABLE cross_catalog (
			guid   TEXT NOT NULL PRIMARY KEY,
			key    TEXT NOT NULL,
			type   TEXT NOT NULL,
			bundle TEXT NOT NULL,
			size   INTEGER NOT NULL,
			n_deps INTEGER NOT NULL,
			deps   TEXT NOT NULL
		)
	`);

	const gz = readFileSync(srcPath);
	const ndjson = gunzipSync(gz).toString("utf8");

	const insert = db.prepare(
		"INSERT OR REPLACE INTO cross_catalog (guid, key, type, bundle, size, n_deps, deps) VALUES (?, ?, ?, ?, ?, ?, ?)"
	);

	db.transaction(() => {
		for (const line of ndjson.split("\n")) {
			if (!line) continue;
			try {
				const entry = JSON.parse(line);
				if (entry.kind === "asset") {
					insert.run(
						entry.guid,
						entry.key,
						entry.type || "Unknown",
						entry.bundle || "",
						entry.size || 0,
						entry.n_deps || 0,
						JSON.stringify(entry.deps || [])
					);
				}
			} catch {
				// Ignorer les lignes corrompues
			}
		}
	})();

	db.exec("CREATE INDEX idx_catalog_type ON cross_catalog (type)");
	db.exec("CREATE INDEX idx_catalog_key ON cross_catalog (key)");
	db.exec("PRAGMA wal_checkpoint(TRUNCATE)");
	db.exec("ANALYZE");
	console.log("[cross-catalog] Matérialisation v2 terminée.");
}

/** Recherche paginée dans le catalogue d'assets. */
export function searchCatalog(
	q?: string,
	type?: string,
	limit = 50,
	offset = 0
): { assets: CatalogAsset[]; total: number } {
	const db = getDb();
	const cleanQ = (q ?? "").trim().toLowerCase();
	const cleanType = (type ?? "").trim();

	let query = "SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE 1=1";
	let countQuery = "SELECT COUNT(*) AS total FROM cross_catalog WHERE 1=1";
	const params: any[] = [];

	if (cleanQ) {
		const filter = `%${cleanQ}%`;
		query += " AND (lower(key) LIKE ? OR lower(bundle) LIKE ?)";
		countQuery += " AND (lower(key) LIKE ? OR lower(bundle) LIKE ?)";
		params.push(filter, filter);
	}

	if (cleanType) {
		query += " AND type = ?";
		countQuery += " AND type = ?";
		params.push(cleanType);
	}

	// Récupérer le total
	const totalRow = db.query(countQuery).get(...params) as { total: number };
	const total = totalRow?.total ?? 0;

	// Récupérer les données avec pagination
	query += " ORDER BY key LIMIT ? OFFSET ?";
	const dataParams = [...params, limit, offset];
	const rows = db.query(query).all(...dataParams) as any[];

	const assets = rows.map((r) => {
		let parsedDeps: string[] = [];
		try {
			parsedDeps = JSON.parse(r.deps);
		} catch {
			// fallback
		}
		return {
			guid: r.guid,
			key: r.key,
			type: r.type,
			bundle: r.bundle,
			size: r.size,
			n_deps: r.n_deps,
			deps: parsedDeps,
		};
	});

	return { assets, total };
}

/** Récupère un asset unique par son GUID. */
export function getAsset(guid: string): CatalogAsset | null {
	const db = getDb();
	const row = db.query("SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE guid = ?").get(guid) as any;
	if (!row) return null;

	let parsedDeps: string[] = [];
	try {
		parsedDeps = JSON.parse(row.deps);
	} catch {
		// fallback
	}

	return {
		guid: row.guid,
		key: row.key,
		type: row.type,
		bundle: row.bundle,
		size: row.size,
		n_deps: row.n_deps,
		deps: parsedDeps,
	};
}

/** Liste tous les types d'assets distincts présents dans le catalogue. */
export function getCatalogTypes(): string[] {
	const db = getDb();
	const rows = db.query("SELECT DISTINCT type FROM cross_catalog ORDER BY type").all() as { type: string }[];
	return rows.map((r) => r.type);
}
