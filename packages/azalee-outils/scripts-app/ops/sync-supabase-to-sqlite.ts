#!/usr/bin/env bun
/**
 * Sync INCRÉMENTALE Supabase (distant, source de vérité) → mirror SQLite local.
 *
 * IMPORTANT — pourquoi ce script existe :
 *  `backup-supabase-to-sqlite.ts` (full dump) lit `DATABASE_URL`, qui pointe sur
 *  le Postgres LOCAL `rose_griffon` (socket). Or l'app et les outils d'admin
 *  écrivent sur SUPABASE via REST/service-role (`createClient`/MCP). Le local
 *  peut donc DRIFTER (ex. zukan_order/vidéos écrits côté Supabase seulement).
 *  Ce script lit les DONNÉES depuis SUPABASE (la vraie source) et n'utilise le
 *  pg local QUE pour la découverte de schéma (colonnes/PK/updated_at — stable).
 *
 * Stratégie :
 *  - tables `updated_at` + PK simple → upsert des lignes `updated_at > watermark`
 *    (ON CONFLICT(pk) DO UPDATE), watermark par table dans `_sync_meta`.
 *  - tables sans `updated_at` → full-refresh seulement avec `--full-static`.
 *  - `--deletes` → sweep : supprime localement les PK absentes de Supabase.
 *  - mirror en WAL ⇒ l'app (lecteur readonly) voit les commits SANS restart.
 *
 * Usage :
 *   bun scripts/ops/sync-supabase-to-sqlite.ts
 *   bun scripts/ops/sync-supabase-to-sqlite.ts --tables=inagle_characters,inagle_skills
 *   bun scripts/ops/sync-supabase-to-sqlite.ts --deletes --full-static
 *   bun scripts/ops/sync-supabase-to-sqlite.ts --db=/path/mirror.sqlite
 *
 * Env : DATABASE_URL (pg local, schéma) + SUPABASE URL/SERVICE_ROLE_KEY (données).
 */
import { createSupabaseServiceClient } from "@rosegriffon/db/service";
import { Database } from "bun:sqlite";
import { readdirSync } from "node:fs";
import path from "node:path";
import { Client } from "pg";

const { DATABASE_URL } = process.env;
if (!DATABASE_URL) {
	console.error("❌ Manque DATABASE_URL (découverte de schéma)");
	process.exit(1);
}

const args = new Map<string, string>();
for (const a of Bun.argv.slice(2)) {
	const [k, ...rest] = a.replace(/^--/, "").split("=");
	args.set(k, rest.join("=") || "true");
}
const doDeletes = args.has("deletes");
const doFullStatic = args.has("full-static");
// --full : refresh COMPLET paginé (ignore le watermark updated_at). À utiliser
// quand des écritures distantes n'ont pas bumpé updated_at (ex. UPDATE bruts SQL).
const doFull = args.has("full");
const onlyTables = args
	.get("tables")
	?.split(",")
	.map((s) => s.trim());
// PostgREST plafonne à 1000 lignes/requête → page ≤ 1000.
const pageSize = Math.min(1000, Number(args.get("page") ?? "1000"));

function findActiveMirror(): string {
	if (args.get("db")) return path.resolve(args.get("db")!);
	// `data/backups/` reste volontairement hors dépôt (instantanés pouvant porter de la PII) :
	// son emplacement se donne par l'environnement, jamais par le chemin d'une autre machine.
	const dirs = [
		process.env.AZALEE_DATA_DIR ? path.join(process.env.AZALEE_DATA_DIR, "backups") : undefined,
		path.resolve(process.cwd(), "data/backups"),
		path.resolve(process.cwd(), "apps/azalee/data/backups"),
	].filter((d): d is string => Boolean(d));
	for (const dir of dirs) {
		try {
			const files = readdirSync(dir)
				.filter((f) => f.startsWith("supabase-") && f.endsWith(".sqlite"))
				.sort((a, b) => b.localeCompare(a));
			if (files.length > 0) return path.join(dir, files[0]);
		} catch {
			/* ignore */
		}
	}
	throw new Error("Aucun mirror SQLite actif (lance d'abord backup:supabase)");
}

const dbPath = findActiveMirror();
console.log(`🔄 Sync Supabase (distant) → ${dbPath}`);

type Affinity = "INTEGER" | "REAL" | "TEXT";
function affinityFor(pgType: string): Affinity {
	const t = pgType.toLowerCase();
	if (t === "boolean" || t === "integer" || t === "bigint" || t === "smallint") return "INTEGER";
	if (t === "numeric" || t === "real" || t === "double precision") return "REAL";
	return "TEXT";
}
function coerce(v: unknown, a: Affinity): string | number | null {
	if (v === null || v === undefined) return null;
	if (a === "INTEGER") {
		if (typeof v === "boolean") return v ? 1 : 0;
		if (typeof v === "bigint") return Number(v);
		if (typeof v === "number") return v;
		const n = Number(v);
		return Number.isFinite(n) ? n : String(v);
	}
	if (a === "REAL") {
		if (typeof v === "number") return v;
		const n = Number(v);
		return Number.isFinite(n) ? n : String(v);
	}
	if (typeof v === "object") return JSON.stringify(v);
	if (typeof v === "bigint") return String(v);
	return String(v);
}

// --- Découverte schéma via pg local (parité de schéma avec Supabase) ---
interface Col {
	name: string;
	affinity: Affinity;
}
interface Tbl {
	name: string;
	cols: Col[];
	pk: string | null;
	hasUpdated: boolean;
}

const pg = new Client({ connectionString: DATABASE_URL });
await pg.connect();
const colRes = await pg.query<{ table_name: string; column_name: string; data_type: string }>(`
	SELECT table_name, column_name, data_type FROM information_schema.columns
	WHERE table_schema='public' ORDER BY table_name, ordinal_position
`);
const baseTables = new Set(
	(
		await pg.query<{ table_name: string }>(
			`SELECT table_name FROM information_schema.tables
			 WHERE table_schema='public' AND table_type='BASE TABLE'`
		)
	).rows.map((r) => r.table_name)
);
const pkRes = await pg.query<{ table_name: string; cols: string; col: string }>(`
	SELECT tc.table_name, count(*) OVER (PARTITION BY tc.table_name)::text AS cols, kcu.column_name AS col
	FROM information_schema.table_constraints tc
	JOIN information_schema.key_column_usage kcu
	  ON tc.constraint_name=kcu.constraint_name AND tc.table_schema=kcu.table_schema
	WHERE tc.constraint_type='PRIMARY KEY' AND tc.table_schema='public'
`);
await pg.end();

const pkMap = new Map<string, string | null>();
for (const r of pkRes.rows) pkMap.set(r.table_name, Number(r.cols) === 1 ? r.col : null);

const tblMap = new Map<string, Tbl>();
for (const r of colRes.rows) {
	if (!baseTables.has(r.table_name)) continue;
	let t = tblMap.get(r.table_name);
	if (!t) {
		t = { name: r.table_name, cols: [], pk: pkMap.get(r.table_name) ?? null, hasUpdated: false };
		tblMap.set(r.table_name, t);
	}
	t.cols.push({ name: r.column_name, affinity: affinityFor(r.data_type) });
	if (r.column_name === "updated_at") t.hasUpdated = true;
}
let tables = [...tblMap.values()];
if (onlyTables) tables = tables.filter((t) => onlyTables.includes(t.name));

// --- Source de DONNÉES : Supabase REST (service role) ---
const supabase = createSupabaseServiceClient();

// --- Mirror SQLite en read-write + pragmas (docs aphrody) ---
const db = new Database(dbPath, { create: false, readwrite: true });
for (const p of [
	"journal_mode = WAL",
	"synchronous = NORMAL",
	"temp_store = MEMORY",
	"busy_timeout = 8000",
	"mmap_size = 268435456",
]) {
	db.exec(`PRAGMA ${p}`);
}
db.exec(
	`CREATE TABLE IF NOT EXISTS _sync_meta (table_name TEXT PRIMARY KEY, watermark TEXT, synced_at TEXT)`
);
const getWm = db.prepare("SELECT watermark FROM _sync_meta WHERE table_name=?");
const setWm = db.prepare(
	"INSERT OR REPLACE INTO _sync_meta (table_name, watermark, synced_at) VALUES (?,?,?)"
);
// Floor = date du full backup (le mirror contient déjà tout jusque-là) → 1er run
// incrémental = deltas seulement, pas un re-pull de 67k lignes.
let backupFloor = "1970-01-01T00:00:00Z";
try {
	const m = db.query("SELECT value FROM _meta WHERE key='backup_started_at'").get() as {
		value: string;
	} | null;
	if (m?.value) backupFloor = new Date(m.value).toISOString();
} catch {
	/* legacy */
}
const sqliteHasTable = (name: string): boolean =>
	db.query("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?").get(name) !== null;
const ensureTable = (t: Tbl) => {
	if (!sqliteHasTable(t.name)) {
		const decl = t.cols.map((c) => `"${c.name}" ${c.affinity}`).join(", ");
		db.exec(`CREATE TABLE "${t.name}" (${decl}${t.pk ? `, PRIMARY KEY ("${t.pk}")` : ""})`);
	}
	// Les tables du full backup n'ont PAS de PK déclarée → ON CONFLICT échoue.
	// On garantit un index UNIQUE sur la PK (non-destructif) pour activer l'upsert.
	if (t.pk) {
		const idx = `ux_${t.name}_${t.pk}`.replace(/[^a-zA-Z0-9_]/g, "_");
		db.exec(`CREATE UNIQUE INDEX IF NOT EXISTS "${idx}" ON "${t.name}" ("${t.pk}")`);
	}
};

let totalUpserts = 0;
let totalDeletes = 0;
const summary: string[] = [];

for (const t of tables) {
	ensureTable(t);
	const colNames = t.cols.map((c) => `"${c.name}"`).join(", ");
	const placeholders = t.cols.map((c) => `$${c.name}`).join(", ");
	const bind = (r: Record<string, unknown>) => {
		const p: Record<string, string | number | null> = {};
		for (const c of t.cols) p[`$${c.name}`] = coerce(r[c.name], c.affinity);
		return p;
	};

	if (!doFull && t.hasUpdated && t.pk) {
		// --- Incrémental par watermark updated_at ---
		const updates = t.cols
			.filter((c) => c.name !== t.pk)
			.map((c) => `"${c.name}"=excluded."${c.name}"`)
			.join(", ");
		const upsert = db.prepare(
			`INSERT INTO "${t.name}" (${colNames}) VALUES (${placeholders}) ON CONFLICT("${t.pk}") DO UPDATE SET ${updates}`
		);
		const upsertMany = db.transaction((rows: Record<string, unknown>[]) => {
			for (const r of rows) upsert.run(bind(r));
		});
		const wmRow = getWm.get(t.name) as { watermark: string } | null;
		let watermark = wmRow?.watermark ?? backupFloor;
		let changed = 0;
		for (;;) {
			const { data, error } = await supabase
				.from(t.name)
				.select("*")
				.gt("updated_at", watermark)
				.order("updated_at", { ascending: true })
				.limit(pageSize);
			if (error) {
				summary.push(`  ! ${t.name} erreur: ${error.message}`);
				break;
			}
			if (!data || data.length === 0) break;
			upsertMany(data);
			changed += data.length;
			const last = data.at(-1)?.updated_at as string | undefined;
			if (!last || last === watermark) break; // anti-boucle (timestamps égaux)
			watermark = last;
			if (data.length < pageSize) break;
		}
		setWm.run(t.name, watermark, new Date().toISOString());
		totalUpserts += changed;
		if (changed > 0)
			summary.push(`  ↑ ${t.name.padEnd(32)} ${String(changed).padStart(6)} upserts`);
	} else if (t.pk && (doFull || doFullStatic)) {
		// --- Full-refresh paginé (--full toute table, --full-static les sans-updated_at) ---
		const insert = db.prepare(
			`INSERT OR REPLACE INTO "${t.name}" (${colNames}) VALUES (${placeholders})`
		);
		const insertMany = db.transaction((rows: Record<string, unknown>[]) => {
			for (const r of rows) insert.run(bind(r));
		});
		let n = 0;
		for (let from = 0; ; from += pageSize) {
			const { data, error } = await supabase
				.from(t.name)
				.select("*")
				.order(t.pk, { ascending: true })
				.range(from, from + pageSize - 1);
			if (error) {
				summary.push(`  ! ${t.name} erreur: ${error.message}`);
				break;
			}
			if (!data || data.length === 0) break;
			insertMany(data);
			n += data.length;
			if (data.length < pageSize) break;
		}
		// Watermark = now après full-refresh → incrémental ultérieur repart propre.
		if (t.hasUpdated) setWm.run(t.name, new Date().toISOString(), new Date().toISOString());
		totalUpserts += n;
		summary.push(`  ⟳ ${t.name.padEnd(32)} ${String(n).padStart(6)} refresh`);
	}

	if (doDeletes && t.pk && sqliteHasTable(t.name)) {
		// --- Sweep : PK locales absentes de Supabase → DELETE ---
		const remote = new Set<string>();
		for (let from = 0; ; from += 1000) {
			const { data, error } = await supabase
				.from(t.name)
				.select(t.pk)
				.range(from, from + 999);
			if (error || !data || data.length === 0) break;
			for (const r of data) remote.add(String((r as Record<string, unknown>)[t.pk]));
			if (data.length < 1000) break;
		}
		if (remote.size > 0) {
			const localIds = db.query(`SELECT "${t.pk}" AS k FROM "${t.name}"`).all() as { k: unknown }[];
			const toDelete = localIds.map((r) => String(r.k)).filter((k) => !remote.has(k));
			if (toDelete.length > 0) {
				const del = db.prepare(`DELETE FROM "${t.name}" WHERE "${t.pk}"=?`);
				const delMany = db.transaction((ids: string[]) => ids.forEach((id) => del.run(id)));
				delMany(toDelete);
				totalDeletes += toDelete.length;
				summary.push(`  ✗ ${t.name.padEnd(32)} ${String(toDelete.length).padStart(6)} deletes`);
			}
		}
	}
}

db.exec("PRAGMA wal_checkpoint(TRUNCATE)");
db.close();

console.log(summary.join("\n") || "  (rien à synchroniser)");
console.log(
	`\n✓ Sync : ${totalUpserts} upserts, ${totalDeletes} deletes${doDeletes ? "" : " (deletes off → --deletes)"}`
);
console.log("  Mirror en WAL → azalee voit les changements sans restart.");
