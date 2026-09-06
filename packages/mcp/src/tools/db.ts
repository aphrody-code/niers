/**
 * Outils « base de données » — SQL en lecture seule sur le miroir SQLite.
 *
 * Le miroir (`apps/azalee/data/backups/mirror.sqlite`) contient les 66 tables
 * `inagle_*` extraites du jeu, **sans donnée personnelle** : c'est la copie
 * que sert déjà le wiki. L'ouvrir en SQL donne à un agent la couverture que
 * les outils métier ne peuvent pas atteindre (agrégats, jointures, colonnes
 * rares) sans avoir à multiplier les outils.
 *
 * Trois garde-fous : connexion ouverte en lecture seule par SQLite lui-même,
 * une seule instruction `SELECT`/`WITH` autorisée, et un `LIMIT` imposé.
 */

import { Database } from "bun:sqlite";
import { statSync } from "node:fs";
import { resolveMirrorPath } from "@niers/azalee-tools/server/index";
import { z } from "zod";
import { structured, toolError } from "../protocol/types.ts";
import { defineTool, type RegisteredTool } from "../registry.ts";

let cached: Database | undefined;
let cachedPath: string | undefined;
let cachedFingerprint = "";
let lastCheck = 0;

/** Intervalle minimal entre deux vérifications de fraîcheur du miroir. */
const FRESHNESS_INTERVAL_MS = 5_000;

/**
 * Empreinte du fichier ouvert : inode, date de modification, taille.
 *
 * `mirror.sqlite` est un lien symbolique que la synchronisation quotidienne
 * fait pointer sur un nouveau snapshot, avant de purger les anciens. Ce
 * service n'étant jamais redémarré par la synchronisation, sans cette
 * vérification il servirait indéfiniment le snapshot ouvert au démarrage — et
 * après deux nuits, un fichier supprimé.
 */
function fingerprintOf(path: string): string {
	try {
		// `Bun.file().stat()` est asynchrone : dans ce chemin synchrone, seul
		// `statSync` convient. C'est le même emprunt que fait la bibliothèque
		// `@rosegriffon/azalee` pour la même raison.
		const info = statSync(path);
		return `${info.ino}:${info.mtimeMs}:${info.size}`;
	} catch {
		return "";
	}
}

function openMirror(): Database {
	const now = Date.now();
	if (cached && now - lastCheck >= FRESHNESS_INTERVAL_MS) {
		lastCheck = now;
		const path = resolveMirrorPath();
		if (path !== cachedPath || fingerprintOf(path ?? "") !== cachedFingerprint) {
			try {
				cached.close();
			} catch {
				/* déjà fermée */
			}
			cached = undefined;
		}
	}
	if (cached) return cached;

	const path = resolveMirrorPath();
	if (!path) throw new Error("Miroir SQLite introuvable : aucune base locale disponible.");
	cached = new Database(path, { readonly: true, strict: true });
	cachedPath = path;
	cachedFingerprint = fingerprintOf(path);
	lastCheck = Date.now();
	return cached;
}

/**
 * N'accepte qu'une requête de lecture unique.
 *
 * On refuse tout point-virgule séparateur (pas d'instructions enchaînées),
 * les mots-clés d'écriture et les pragmas — même si l'ouverture en lecture
 * seule les bloquerait déjà, un refus explicite donne un message clair.
 */
const FORBIDDEN = /\b(insert|update|delete|drop|alter|create|replace|attach|detach|pragma|vacuum|reindex)\b/i;

function assertReadOnlySql(sql: string): string {
	const trimmed = sql.trim().replace(/;\s*$/, "");
	if (trimmed.includes(";")) throw new Error("Une seule instruction SQL à la fois.");
	if (!/^(select|with)\b/i.test(trimmed)) throw new Error("Seules les requêtes SELECT (ou WITH … SELECT) sont acceptées.");
	if (FORBIDDEN.test(trimmed)) throw new Error("Mot-clé d'écriture refusé : la base est en lecture seule.");
	return trimmed;
}

export function dbTools(): RegisteredTool[] {
	return [
		defineTool({
			name: "db_tables",
			title: "Lister les tables du miroir",
			description:
				"Liste les tables du miroir SQLite des données de jeu (préfixe `inagle_`) avec leur nombre de lignes réel. Point de départ avant db_schema et db_query.",
			inputSchema: z.object({
				like: z.string().optional().describe("Filtre sur le nom, ex. `skill`."),
			}),
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: ({ like }) => {
				const db = openMirror();
				const rows = db
					.query<{ name: string }, []>("select name from sqlite_master where type = 'table' order by name")
					.all()
					.filter((row) => !like || row.name.includes(like));
				const tables = rows.map((row) => ({
					name: row.name,
					rows: (db.query(`select count(*) as n from "${row.name}"`).get() as { n: number }).n,
				}));
				return structured({ count: tables.length, tables });
			},
		}),

		defineTool({
			name: "db_schema",
			title: "Schéma d'une table",
			description:
				"Colonnes, types et index d'une table du miroir, avec une ligne d'exemple. Indispensable avant d'écrire une requête : beaucoup de colonnes portent des identifiants bruts du jeu.",
			inputSchema: z.object({ table: z.string().min(1).describe("Nom exact de la table.") }),
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: ({ table }) => {
				const db = openMirror();
				const exists = db
					.query("select name from sqlite_master where type = 'table' and name = ?")
					.get(table);
				if (!exists) return toolError(`Table inconnue : ${table}`);
				const columns = db.query(`pragma table_info("${table}")`).all();
				const indexes = db.query(`pragma index_list("${table}")`).all();
				const sample = db.query(`select * from "${table}" limit 1`).get();
				return structured({ table, columns, indexes, sample });
			},
		}),

		defineTool({
			name: "db_query",
			title: "Requête SQL en lecture seule",
			description:
				"Exécute une requête SELECT (ou WITH … SELECT) sur le miroir SQLite des données de jeu et renvoie les lignes. Une seule instruction, aucune écriture possible, résultat plafonné. Sert aux agrégats et jointures que les outils azalee_* ne couvrent pas.",
			inputSchema: z.object({
				sql: z.string().min(6).describe("Requête SELECT. Les paramètres se passent par `?`."),
				params: z
					.array(z.union([z.string(), z.number(), z.boolean(), z.null()]))
					.default([])
					.describe("Valeurs des paramètres positionnels `?`."),
				limit: z.int().min(1).max(1000).default(100).describe("Plafond de lignes renvoyées."),
			}),
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: ({ sql, params, limit }) => {
				const statement = assertReadOnlySql(sql);
				const db = openMirror();
				const bounded = /\blimit\s+\d+/i.test(statement) ? statement : `${statement} limit ${limit}`;
				const started = performance.now();
				const rows = db.query(bounded).all(...(params as never[]));
				return structured({
					sql: bounded,
					rows: rows.slice(0, limit),
					rowCount: rows.length,
					ms: Math.round(performance.now() - started),
				});
			},
		}),
	];
}

/** Ferme la connexion mise en cache (tests, arrêt du processus). */
export function closeMirror(): void {
	cached?.close();
	cached = undefined;
}
