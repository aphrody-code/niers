#!/usr/bin/env bun
/**
 * Backup Supabase Postgres (schema public) → fichier SQLite via Bun native API.
 *
 * - Découverte des tables : pg direct via DATABASE_URL → information_schema (frais),
 *   fallback liste hardcodée si pg indisponible.
 * - Lecture : pg direct si DATABASE_URL dispo (curseur, pas de limite REST 1000),
 *   sinon @supabase/supabase-js paginé 1000 rows.
 * - Écriture : bun:sqlite, WAL + transactions batch (prepared INSERT par table).
 * - Typage : INTEGER (boolean / integer / bigint / smallint), REAL (numeric / double),
 *   TEXT (text / uuid / timestamp / json / jsonb / arrays sérialisés JSON).
 *
 * Usage:
 *   bun scripts/ops/backup-supabase-to-sqlite.ts
 *   bun scripts/ops/backup-supabase-to-sqlite.ts --output=/path/to/file.sqlite
 *   bun scripts/ops/backup-supabase-to-sqlite.ts --tables=profiles,inagle_characters
 *   bun scripts/ops/backup-supabase-to-sqlite.ts --schema=public,storage
 *
 * Env requis :
 *   DATABASE_URL                                            (préféré)
 *   ou SUPABASE_INTERNAL_URL / NEXT_PUBLIC_SUPABASE_URL + SUPABASE_SERVICE_ROLE_KEY
 *
 * Output par défaut : data/backups/supabase-<ISO>.sqlite
 */
import { Database } from "bun:sqlite";
import { mkdir } from "node:fs/promises";
import path from "node:path";
import { Client } from "pg";

const { DATABASE_URL } = process.env;

if (!DATABASE_URL) {
	console.error(
		"❌ Manque DATABASE_URL (postgres connection string requise pour la découverte dynamique)"
	);
	process.exit(1);
}

const args = Bun.argv.slice(2);
const argMap = new Map<string, string>();
for (const a of args) {
	const [k, ...rest] = a.replace(/^--/, "").split("=");
	argMap.set(k, rest.join("=") || "true");
}

const stamp = new Date().toISOString().replaceAll(/[:.]/g, "-").slice(0, 19);
const outFile = path.resolve(argMap.get("output") ?? `data/backups/supabase-${stamp}.sqlite`);
const schemas = (argMap.get("schema") ?? "public").split(",").map((s) => s.trim());
const includeTablesArg = argMap.get("tables");
// Filtre PII : ne snapshoter que les tables d'un préfixe (ex --prefix=inagle_).
// Le wiki ne sert QUE les tables inagle_* via le miroir SQLite ; profiles/articles/
// tweets (emails membres = PII) passent par Supabase direct → on les exclut du dump.
const prefixArg = argMap.get("prefix");
const pageSize = Number(argMap.get("page") ?? "5000");

await mkdir(path.dirname(outFile), { recursive: true });

// --- Découverte tables + colonnes via pg ---
interface ColumnInfo {
	name: string;
	pgType: string;
	sqliteAffinity: "INTEGER" | "REAL" | "TEXT";
}
interface TableInfo {
	schema: string;
	name: string;
	columns: ColumnInfo[];
}

function affinityFor(pgType: string): ColumnInfo["sqliteAffinity"] {
	const t = pgType.toLowerCase();
	if (t === "boolean" || t === "integer" || t === "bigint" || t === "smallint") {
		return "INTEGER";
	}
	if (t === "numeric" || t === "real" || t === "double precision") {
		return "REAL";
	}
	return "TEXT";
}

const tables: TableInfo[] = [];
const pg = new Client({ connectionString: DATABASE_URL });
await pg.connect();

{
	const tblQuery = `
		SELECT table_schema, table_name
		FROM information_schema.tables
		WHERE table_schema = ANY($1::text[])
		  AND table_type = 'BASE TABLE'
		ORDER BY table_schema, table_name
	`;
	const tblRes = await pg.query<{ table_schema: string; table_name: string }>(tblQuery, [schemas]);

	let names = tblRes.rows.map((r) => ({
		name: r.table_name,
		schema: r.table_schema,
	}));
	if (includeTablesArg) {
		const allow = new Set(includeTablesArg.split(",").map((t) => t.trim()));
		names = names.filter((t) => allow.has(t.name));
	}
	if (prefixArg) {
		names = names.filter((t) => t.name.startsWith(prefixArg));
	}

	const colQuery = `
		SELECT column_name, udt_name, data_type, ordinal_position
		FROM information_schema.columns
		WHERE table_schema = $1 AND table_name = $2
		ORDER BY ordinal_position
	`;
	for (const t of names) {
		const cols = await pg.query<{
			column_name: string;
			udt_name: string;
			data_type: string;
		}>(colQuery, [t.schema, t.name]);
		tables.push({
			columns: cols.rows.map((r) => ({
				name: r.column_name,
				pgType: r.data_type,
				sqliteAffinity: affinityFor(r.data_type),
			})),
			name: t.name,
			schema: t.schema,
		});
	}
}

if (tables.length === 0) {
	console.error("❌ Aucune table trouvée");
	process.exit(1);
}

// --- Setup SQLite ---
const db = new Database(outFile, { create: true });
db.exec("PRAGMA journal_mode = WAL");
db.exec("PRAGMA synchronous = NORMAL");
db.exec("PRAGMA cache_size = -64000"); // 64 MB
db.exec("PRAGMA temp_store = MEMORY");

db.exec(`CREATE TABLE IF NOT EXISTS _meta (key TEXT PRIMARY KEY, value TEXT)`);
const meta = db.prepare("INSERT OR REPLACE INTO _meta (key, value) VALUES (?, ?)");
meta.run("source", "pg:DATABASE_URL");
meta.run("backup_stamp", stamp);
meta.run("backup_started_at", new Date().toISOString());
meta.run("page_size", String(pageSize));
meta.run("schemas", schemas.join(","));
meta.run("tables_count", String(tables.length));

console.log(`📦 Backup Supabase → ${outFile}`);
console.log(`   ${tables.length} tables, page size ${pageSize}, schemas=${schemas.join(",")}`);

let totalRows = 0;
const summary: Array<{ table: string; rows: number; ms: number }> = [];

function coerce(v: unknown, affinity: ColumnInfo["sqliteAffinity"]): string | number | null {
	if (v === null || v === undefined) {
		return null;
	}
	if (affinity === "INTEGER") {
		if (typeof v === "boolean") {
			return v ? 1 : 0;
		}
		if (typeof v === "bigint") {
			return Number(v);
		}
		if (typeof v === "number") {
			return v;
		}
		const n = Number(v);
		return Number.isFinite(n) ? n : String(v);
	}
	if (affinity === "REAL") {
		if (typeof v === "number") {
			return v;
		}
		const n = Number(v);
		return Number.isFinite(n) ? n : String(v);
	}
	if (typeof v === "object") {
		return JSON.stringify(v);
	}
	if (typeof v === "bigint") {
		return String(v);
	}
	if (v instanceof Date) {
		return v.toISOString();
	}
	return String(v);
}

for (const t of tables) {
	const tableId = `"${t.schema}"."${t.name}"`;
	const sqliteName = t.schema === "public" ? t.name : `${t.schema}__${t.name}`;
	const t0 = performance.now();

	const colsDecl = t.columns.map((c) => `"${c.name}" ${c.sqliteAffinity}`).join(", ");
	db.exec(`DROP TABLE IF EXISTS "${sqliteName}"`);
	db.exec(`CREATE TABLE "${sqliteName}" (${colsDecl})`);

	const colNames = t.columns.map((c) => `"${c.name}"`).join(", ");
	const placeholders = t.columns.map((c) => `$${c.name}`).join(", ");
	const insert = db.prepare(`INSERT INTO "${sqliteName}" (${colNames}) VALUES (${placeholders})`);

	const insertMany = db.transaction((rows: Array<Record<string, unknown>>) => {
		for (const r of rows) {
			const p: Record<string, string | number | null> = {};
			for (const c of t.columns) {
				p[`$${c.name}`] = coerce(r[c.name], c.sqliteAffinity);
			}
			insert.run(p);
		}
	});

	let count = 0;
	// Pagination par OFFSET (ORDER BY ctid pour stabilité). Pas de curseur (simple).
	for (let offset = 0; ; offset += pageSize) {
		const res = await pg.query<Record<string, unknown>>(
			`SELECT * FROM ${tableId} ORDER BY ctid LIMIT $1 OFFSET $2`,
			[pageSize, offset]
		);
		if (res.rows.length === 0) {
			break;
		}
		insertMany(res.rows);
		count += res.rows.length;
		if (res.rows.length < pageSize) {
			break;
		}
	}

	const ms = Math.round(performance.now() - t0);
	totalRows += count;
	summary.push({ ms, rows: count, table: t.name });
	console.log(`  ✓ ${t.name.padEnd(34)} ${String(count).padStart(7)} rows  (${ms}ms)`);
}

await pg.end();

meta.run("backup_ended_at", new Date().toISOString());
meta.run("total_rows", String(totalRows));
db.exec("PRAGMA wal_checkpoint(TRUNCATE)");
db.close();

const stat = await Bun.file(outFile).stat();
const mb = (stat.size / 1024 / 1024).toFixed(2);
console.log(`\n✓ Done: ${totalRows.toLocaleString()} rows → ${outFile} (${mb} MB)`);
console.log(
	`  Top 5 tables: ${summary
		.toSorted((a, b) => b.rows - a.rows)
		.slice(0, 5)
		.map((s) => `${s.table}=${s.rows}`)
		.join(", ")}`
);

// Hint pour usage offline (lecture seule)
console.log(
	`\nQuery example:\n  bun -e 'import { Database } from "bun:sqlite"; const d = new Database("${outFile}"); console.log(d.prepare("SELECT name FROM sqlite_master WHERE type=\\"table\\" LIMIT 5").all())'`
);
