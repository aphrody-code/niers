import type { Database } from "bun:sqlite";
import { sql, type SQL } from "drizzle-orm";
import { drizzle as drizzleProxy } from "drizzle-orm/sqlite-proxy";
import { resolveMirrorPath } from "../config";

/**
 * Chemin du miroir SQLite. La résolution (option explicite → `SQLITE_DB_PATH` →
 * `data/backups/mirror.sqlite` → snapshot le plus récent) vit dans `config.ts`,
 * source unique pour toute la lib.
 */
function getSqlitePath(): string | null {
	return resolveMirrorPath();
}

// Surface minimale commune entre `bun:sqlite` (runtime Bun) et `node:sqlite`
// (Node >= 22 — `DatabaseSync`). La façade Drizzle (`drizzle-orm/sqlite-proxy`)
// délègue l'exécution à ce moteur : un seul schéma, un seul driver universel,
// swap derrière le garde `typeof Bun`. Les deux moteurs exposent `prepare(sql)`
// avec `all`/`get` sur le statement → on les unifie derrière ce type.
type SqliteLike = {
	prepare(sql: string): {
		all(...params: unknown[]): unknown[];
		get(...params: unknown[]): unknown;
	};
	exec(sql: string): void;
};

let _sqliteDb: SqliteLike | null = null;
let _openedPath: string | null = null;
let _openedFingerprint = "";
let _lastFreshnessCheck = 0;

/** Intervalle minimal entre deux vérifications de fraîcheur du miroir. */
const FRESHNESS_INTERVAL_MS = 5_000;

/**
 * Empreinte du fichier réellement ouvert : inode, date de modification et
 * taille. Le `mirror.sqlite` est un **lien symbolique** que la synchronisation
 * quotidienne fait pointer sur un nouveau snapshot, puis elle purge les
 * anciens (rétention 2).
 */
function fingerprintOf(path: string): string {
	try {
		// `statSync` existe sous Bun comme sous Node ; la version asynchrone de
		// `Bun.file()` ne conviendrait pas dans ce chemin synchrone.
		// eslint-disable-next-line @typescript-eslint/no-require-imports -- accès synchrone au système de fichiers
		const { statSync } = require("node:fs") as { statSync: (p: string) => { ino: number; mtimeMs: number; size: number } };
		const info = statSync(path);
		return `${info.ino}:${info.mtimeMs}:${info.size}`;
	} catch {
		return "";
	}
}

/**
 * Invalide le handle si le miroir a été remplacé sous nos pieds.
 *
 * `azalee-web` est redémarré par la synchronisation, mais pas les autres
 * consommateurs de la lib (`azalee-api`, `rg-mcp`, un CLI qui tourne longtemps,
 * un sidecar Tauri) : sans cette vérification, ils continuaient à servir le
 * snapshot ouvert au démarrage — et après deux nuits, un fichier supprimé.
 */
function invalidateIfMirrorChanged(): void {
	if (!_sqliteDb) return;

	// Le chemin est comparé à CHAQUE accès, sans appel système : un
	// `configureAzalee({ mirrorPath })` explicite doit prendre effet
	// immédiatement, pas après l'intervalle de fraîcheur.
	const path = getSqlitePath();
	if (path === _openedPath) {
		// Même chemin : seule l'empreinte du fichier peut avoir changé (swap du
		// lien symbolique). Ce contrôle-là coûte un `stat`, donc on l'espace.
		const now = Date.now();
		if (now - _lastFreshnessCheck < FRESHNESS_INTERVAL_MS) return;
		_lastFreshnessCheck = now;
		if (fingerprintOf(path ?? "") === _openedFingerprint) return;
	}

	const ancien = _sqliteDb as unknown as { close?: () => void };
	_sqliteDb = null;
	_drizzle = null;
	// Les lectures sont synchrones : aucune requête n'est en vol ici. Fermer
	// libère le descripteur ; un échec n'est pas bloquant (le GC s'en chargera).
	try {
		ancien.close?.();
	} catch {
		/* handle déjà fermé ou moteur sans close() */
	}
}

function getSqliteDb(): Database {
	invalidateIfMirrorChanged();
	if (_sqliteDb) return _sqliteDb as unknown as Database;
	const dbPath = getSqlitePath();
	if (!dbPath) {
		throw new Error("No SQLite database backup found");
	}
	_openedPath = dbPath;
	_openedFingerprint = fingerprintOf(dbPath);
	_lastFreshnessCheck = Date.now();

	if (typeof Bun !== "undefined") {
		// Runtime Bun (prod `azalee-web.service`) → bun:sqlite natif.
		// eslint-disable-next-line @typescript-eslint/no-require-imports -- lazy load bun:sqlite sous garde Bun
		const { Database: DBConstructor } = require("bun:sqlite");
		const db = new DBConstructor(dbPath, { readonly: true }) as SqliteLike;
		// `journal_mode` est une écriture sur l'en-tête du fichier : sur une base
		// qui n'est pas déjà en WAL, la poser depuis une connexion en lecture
		// seule lève `SQLITE_READONLY`. Les snapshots de production sont en WAL
		// (le pragma est alors un simple accusé de réception), mais une base
		// fournie par un tiers — poste de dev, sidecar Tauri — ne l'est pas
		// forcément : le mode par défaut fonctionne très bien en lecture.
		try {
			db.exec("PRAGMA journal_mode = WAL");
		} catch {
			/* base non-WAL ouverte en lecture seule : on garde son mode d'origine */
		}
		db.exec("PRAGMA temp_store = MEMORY");
		_sqliteDb = db;
		return _sqliteDb as unknown as Database;
	}

	// Runtime Node (>= 22) → node:sqlite (`DatabaseSync`). Indispensable pour
	// `next build` sous /usr/bin/node : Bun déclenche un bug de prerender du
	// /_global-error (dispatcher React null), donc le build de prod tourne sous
	// Node. Sans ce fallback, getSqliteDb retombait sur Postgres → timeouts >60s
	// sur les milliers de fiches perso et build avorté.
	// `process.getBuiltinModule` : résolution 100% runtime du builtin, invisible
	// à l'analyse statique de Turbopack (un `require("node:sqlite")` littéral
	// échoue: "Unsupported external type Url for commonjs reference").
	const nodeSqlite = (
		process as unknown as {
			getBuiltinModule?: (id: string) => { DatabaseSync: new (p: string, o?: unknown) => SqliteLike };
		}
	).getBuiltinModule?.("node:sqlite");
	if (!nodeSqlite) {
		throw new Error("[SQLite Client] node:sqlite indisponible (Node < 22)");
	}
	const db = new nodeSqlite.DatabaseSync(dbPath, { readOnly: true });
	db.exec("PRAGMA temp_store = MEMORY");
	_sqliteDb = db;
	return _sqliteDb as unknown as Database;
}

// Instance Drizzle universelle (`sqlite-proxy`). L'exécution est déléguée au
// `SqliteLike` actif (bun:sqlite au runtime, node:sqlite au build). Drizzle
// compile la requête (placeholders `?`, NULLS FIRST/LAST, json_extract, GROUP BY)
// puis nous remet `{ sql, params, method }` ; on prépare et on exécute sur le
// moteur natif. Singleton process-local — pas de réouverture du fichier.
type ProxyDb = ReturnType<typeof drizzleProxy>;
let _drizzle: ProxyDb | null = null;
function getDrizzle(): ProxyDb {
	if (_drizzle) return _drizzle;
	const engine = getSqliteDb() as unknown as SqliteLike;
	// Le callback est typé `AsyncRemoteCallback` (doit renvoyer une Promise). Le
	// moteur sous-jacent (bun:sqlite / node:sqlite) est SYNCHRONE — on enveloppe
	// donc le résultat dans une promesse résolue (aucun I/O async réel).
	_drizzle = drizzleProxy(async (queryString, params, method) => {
		const stmt = engine.prepare(queryString);
		if (method === "get") {
			const row = stmt.get(...params);
			// sqlite-proxy attend `{ rows: <ligne unique en tableau de valeurs OU
			// objet> }`. On utilise UNIQUEMENT `db.all(sql)` côté façade (jamais
			// `db.get`), donc cette branche n'est pas empruntée en pratique ; on la
			// garde correcte par sûreté.
			return { rows: row ? [row] : [] };
		}
		if (method === "values") {
			const rows = stmt.all(...params) as Array<Record<string, unknown>>;
			return { rows: rows.map((r) => Object.values(r)) };
		}
		// method "all" / "run" → on renvoie les lignes en objets. La façade lit
		// `result.rows` comme un tableau d'objets (mode "all" de Drizzle).
		const rows = stmt.all(...params) as unknown[];
		return { rows };
	});
	return _drizzle;
}

export class SqliteQueryBuilder {
	private db: Database;
	private table: string;
	private selects: string = "*";
	// Conditions WHERE émises comme fragments Drizzle (`sql`). Chaque fragment
	// porte ses propres paramètres bindés via Drizzle (placeholders `?`).
	private wheres: SQL[] = [];
	private limitValue: number | null = null;
	private offsetValue: number | null = null;
	// Fragments ORDER BY dans l'ordre d'insertion (l'ordre des appels `.order()`
	// est significatif — plusieurs tris s'accumulent).
	private orders: SQL[] = [];
	private isSingle: boolean = false;
	private isMaybeSingle: boolean = false;
	private countOption: string | null = null;

	private needsGrouping: boolean = false;

	constructor(db: Database, table: string) {
		this.db = db;
		if (table.endsWith("_clean")) {
			this.table = table.replace("_clean", "");
			this.needsGrouping = true;
		} else {
			this.table = table;
		}
	}

	select(fields: string = "*", options?: { count?: string }) {
		this.selects = fields;
		if (options?.count) {
			this.countOption = options.count;
		}
		return this;
	}

	// Expression de colonne SQL (fragment Drizzle), avec support `->>` → json_extract.
	// IMPORTANT : émis via `sql.raw` car ce sont des identifiants de schéma, jamais
	// des valeurs utilisateur (les colonnes viennent du code, pas d'input runtime).
	private cleanCol(col: string): SQL {
		if (col.includes("->>")) {
			const [left, right] = col.split("->>");
			return sql.raw(`json_extract("${left}", '$.${right}')`);
		}
		return sql.raw(`"${col}"`);
	}

	eq(col: string, val: string | number | boolean | null) {
		const column = this.cleanCol(col);
		if (val === null) {
			this.wheres.push(sql`${column} IS NULL`);
		} else {
			const bound = typeof val === "boolean" ? (val ? 1 : 0) : val;
			this.wheres.push(sql`${column} = ${bound}`);
		}
		return this;
	}

	neq(col: string, val: string | number | boolean | null) {
		const column = this.cleanCol(col);
		if (val === null) {
			this.wheres.push(sql`${column} IS NOT NULL`);
		} else {
			const bound = typeof val === "boolean" ? (val ? 1 : 0) : val;
			this.wheres.push(sql`${column} != ${bound}`);
		}
		return this;
	}

	not(col: string, op: string, val: string | number | boolean | null) {
		const column = this.cleanCol(col);
		if (op === "is" && val === null) {
			this.wheres.push(sql`${column} IS NOT NULL`);
		} else if (op === "like" || op === "ilike") {
			this.wheres.push(sql`${column} NOT LIKE ${val}`);
		} else {
			const bound = typeof val === "boolean" ? (val ? 1 : 0) : val;
			this.wheres.push(sql`${column} != ${bound}`);
		}
		return this;
	}

	ilike(col: string, pattern: string) {
		const column = this.cleanCol(col);
		this.wheres.push(sql`${column} LIKE ${pattern}`);
		return this;
	}

	gte(col: string, val: string | number | boolean | null) {
		const column = this.cleanCol(col);
		const bound = typeof val === "boolean" ? (val ? 1 : 0) : val;
		this.wheres.push(sql`${column} >= ${bound}`);
		return this;
	}

	lte(col: string, val: string | number | boolean | null) {
		const column = this.cleanCol(col);
		const bound = typeof val === "boolean" ? (val ? 1 : 0) : val;
		this.wheres.push(sql`${column} <= ${bound}`);
		return this;
	}

	in(col: string, arr: Array<string | number | boolean | null>) {
		if (!arr || arr.length === 0) {
			this.wheres.push(sql`1 = 0`);
			return this;
		}
		const column = this.cleanCol(col);
		const bound = arr.map((val) => (typeof val === "boolean" ? (val ? 1 : 0) : val));
		// `sql.join` insère un placeholder `?` par valeur (binding paramétré).
		const list = sql.join(
			bound.map((v) => sql`${v}`),
			sql`, `
		);
		this.wheres.push(sql`${column} IN (${list})`);
		return this;
	}

	/**
	 * Découpe une liste PostgREST `.or()` sur les virgules de NIVEAU SUPÉRIEUR
	 * uniquement — les virgules à l'intérieur d'un littéral `[...]`/`{...}` (valeur
	 * JSON pour l'opérateur `cs`) sont préservées. Indispensable pour ne PAS couper
	 * une valeur multi-mots (ex. `name_fr.eq.Tornade de Feu`) : l'ancien regex
	 * s'arrêtait au 1er espace et tronquait silencieusement la valeur.
	 */
	private splitTopLevel(s: string): string[] {
		const out: string[] = [];
		let depth = 0;
		let cur = "";
		for (const ch of s) {
			if (ch === "[" || ch === "{") {
				depth++;
			} else if (ch === "]" || ch === "}") {
				depth = Math.max(0, depth - 1);
			}
			if (ch === "," && depth === 0) {
				out.push(cur);
				cur = "";
			} else {
				cur += ch;
			}
		}
		if (cur) {
			out.push(cur);
		}
		return out;
	}

	or(orString: string) {
		const conditions: SQL[] = [];

		// Parse chaque condition `colonne.opérateur.valeur` séparément. La colonne
		// (qui peut contenir `->>`) n'a jamais de `.` littéral en PostgREST, donc on
		// coupe au 1er `.`. La valeur est TOUT le reste après l'opérateur (espaces
		// inclus) — corrige la troncature multi-mots de l'ancien parser regex.
		for (const rawToken of this.splitTopLevel(orString)) {
			const token = rawToken.trim();
			if (!token) {
				continue;
			}

			const firstDot = token.indexOf(".");
			if (firstDot === -1) {
				continue;
			}
			const col = token.slice(0, firstDot);
			const rest = token.slice(firstDot + 1);
			const column = this.cleanCol(col);

			// Null checks (PostgREST: `col.is.null` / `col.not.is.null`) — sans param.
			if (rest === "is.null") {
				conditions.push(sql`${column} IS NULL`);
				continue;
			}
			if (rest === "not.is.null") {
				conditions.push(sql`${column} IS NOT NULL`);
				continue;
			}

			const opDot = rest.indexOf(".");
			if (opDot === -1) {
				continue;
			}
			const op = rest.slice(0, opDot);
			const val = rest.slice(opDot + 1);
			// Valeur vide (`id.eq.`) → on saute la condition plutôt que de générer un
			// filtre qui matche tout (qui ferait remonter la 1re ligne en maybeSingle).
			if (val === "") {
				continue;
			}

			switch (op) {
				case "ilike": {
					let cleanVal = val;
					if (cleanVal.startsWith("%")) cleanVal = cleanVal.slice(1);
					if (cleanVal.endsWith("%")) cleanVal = cleanVal.slice(0, -1);
					conditions.push(sql`${column} LIKE ${`%${cleanVal}%`}`);
					break;
				}
				case "like":
					// PostgREST `like` : `%`/`*` = joker. On normalise `*` → `%`.
					conditions.push(sql`${column} LIKE ${val.replaceAll("*", "%")}`);
					break;
				case "eq":
					conditions.push(sql`${column} = ${val === "true" ? 1 : val === "false" ? 0 : val}`);
					break;
				case "neq":
					conditions.push(sql`${column} != ${val}`);
					break;
				case "gt":
					conditions.push(sql`${column} > ${val}`);
					break;
				case "gte":
					conditions.push(sql`${column} >= ${val}`);
					break;
				case "lt":
					conditions.push(sql`${column} < ${val}`);
					break;
				case "lte":
					conditions.push(sql`${column} <= ${val}`);
					break;
				case "cs":
					try {
						const parsed = JSON.parse(val);
						if (Array.isArray(parsed) && parsed[0]?.id) {
							conditions.push(
								sql`EXISTS (SELECT 1 FROM json_each(${column}) WHERE json_each.value->>'id' = ${parsed[0].id})`
							);
						}
					} catch {
						// fallback if JSON parsing fails
					}
					break;
				default:
					break;
			}
		}

		if (conditions.length > 0) {
			const joined = sql.join(conditions, sql` OR `);
			this.wheres.push(sql`(${joined})`);
		} else {
			// Aucune condition valide (`.or()` malformé / valeurs vides) → matcher
			// RIEN. Sinon l'absence de WHERE ferait remonter toute la table, et un
			// .maybeSingle() renverrait silencieusement la 1re ligne (mauvaise entité
			// au lieu d'un 404).
			this.wheres.push(sql`1 = 0`);
		}

		return this;
	}

	limit(n: number) {
		this.limitValue = n;
		return this;
	}

	order(col: string, options?: { ascending?: boolean; nullsFirst?: boolean }) {
		const column = this.cleanCol(col);
		const dir = options?.ascending === false ? sql.raw("DESC") : sql.raw("ASC");
		// SQLite trie les NULL EN PREMIER par défaut (ASC) — l'inverse de PostgREST
		// quand `nullsFirst:false`. On émet NULLS FIRST/LAST explicitement (SQLite
		// >= 3.30) pour matcher la sémantique supabase-js, sinon les lignes à
		// valeur NULL (ex. zukan_order) remontent en tête et cassent l'ordre.
		if (options?.nullsFirst === true) {
			this.orders.push(sql`${column} ${dir} NULLS FIRST`);
		} else if (options?.nullsFirst === false) {
			this.orders.push(sql`${column} ${dir} NULLS LAST`);
		} else {
			this.orders.push(sql`${column} ${dir}`);
		}
		return this;
	}

	range(from: number, to: number) {
		this.limitValue = to - from + 1;
		this.offsetValue = from;
		return this;
	}

	single() {
		this.isSingle = true;
		return this;
	}

	maybeSingle() {
		this.isMaybeSingle = true;
		return this;
	}

	// Make the class then-able (Promise-like)
	async then<TResult1 = unknown>(
		onfulfilled?:
			| ((value: {
					data: Record<string, unknown> | Array<Record<string, unknown>> | null;
					error: Error | null;
					count: number | null;
			  }) => TResult1 | PromiseLike<TResult1>)
			| undefined
			| null
	) {
		try {
			const data = await this.execute();
			const count = this.countOption ? await this.executeCount() : null;

			const response = {
				data,
				error: null as Error | null,
				count,
			};

			if (onfulfilled) {
				return onfulfilled(response);
			}
			return response;
		} catch (err) {
			const response = {
				data: null,
				error: err instanceof Error ? err : new Error(String(err)),
				count: null,
			};
			if (onfulfilled) {
				return onfulfilled(response);
			}
			return response;
		}
	}

	// Assemble la requête SELECT complète comme fragment Drizzle unique. La
	// projection (`this.selects`) reste une liste de colonnes PostgREST brute
	// (`*`, `id, name_fr`, …) émise via `sql.raw` — identique au SQL maison
	// d'origine (`SELECT ${this.selects} FROM "table" …`).
	private compileSelect(): SQL {
		const parts: SQL[] = [
			sql`SELECT ${sql.raw(this.selects)} FROM ${sql.raw(`"${this.table}"`)}`,
		];
		if (this.wheres.length > 0) {
			parts.push(sql`WHERE ${sql.join(this.wheres, sql` AND `)}`);
		}
		if (this.needsGrouping) {
			parts.push(sql`GROUP BY "name_fr"`);
		}
		if (this.orders.length > 0) {
			parts.push(sql`ORDER BY ${sql.join(this.orders, sql`, `)}`);
		}
		if (this.limitValue !== null) {
			parts.push(sql`LIMIT ${sql.raw(String(this.limitValue))}`);
		}
		if (this.offsetValue !== null) {
			parts.push(sql`OFFSET ${sql.raw(String(this.offsetValue))}`);
		}
		return sql.join(parts, sql` `);
	}

	private async execute(): Promise<
		Record<string, unknown> | Array<Record<string, unknown>> | null
	> {
		const drizzle = getDrizzle();
		const rawRows = (await drizzle.all(this.compileSelect())) as Array<Record<string, unknown>>;

		const parsedRows = rawRows.map((r) => this.parseRow(r));

		if (this.isSingle) {
			if (parsedRows.length === 0) {
				throw new Error("No rows found");
			}
			return parsedRows[0];
		}

		if (this.isMaybeSingle) {
			return parsedRows.length > 0 ? parsedRows[0] : null;
		}

		return parsedRows;
	}

	private async executeCount(): Promise<number> {
		const drizzle = getDrizzle();
		const projection = this.needsGrouping
			? sql.raw(`COUNT(DISTINCT "name_fr") as count`)
			: sql.raw(`COUNT(*) as count`);
		const parts: SQL[] = [sql`SELECT ${projection} FROM ${sql.raw(`"${this.table}"`)}`];
		if (this.wheres.length > 0) {
			parts.push(sql`WHERE ${sql.join(this.wheres, sql` AND `)}`);
		}
		const rows = (await drizzle.all(sql.join(parts, sql` `))) as Array<{ count: number }>;
		return rows[0]?.count ?? 0;
	}

	private parseRow(row: Record<string, unknown>): Record<string, unknown> {
		const parsed: Record<string, unknown> = {};
		for (const [key, val] of Object.entries(row)) {
			if (typeof val === "string" && (val.startsWith("{") || val.startsWith("["))) {
				try {
					parsed[key] = JSON.parse(val);
				} catch {
					parsed[key] = val;
				}
			} else {
				parsed[key] = val;
			}
		}
		return parsed;
	}
}

export function createSqliteClient() {
	return {
		from(table: string) {
			const db = getSqliteDb();
			return new SqliteQueryBuilder(db, table);
		},
	};
}
