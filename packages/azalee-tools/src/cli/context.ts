/**
 * Contexte partagé du CLI `azalee` : tout ce dont plusieurs commandes ont
 * besoin et qui n'est **pas** de la logique métier (celle-ci vit dans la lib).
 *
 * - palette ANSI et rendu de table ASCII ;
 * - contrat de sortie `--json` (silence des logs parasites) ;
 * - lecture d'entrée argument-ou-stdin ;
 * - accès Postgres (`Bun.SQL`) et SQLite (`bun:sqlite`) ;
 * - exécution de sous-processus (`Bun.spawnSync`).
 *
 * Aucune de ces fonctions n'écrit sur stdout d'elle-même : elles renvoient des
 * chaînes, ce sont les commandes qui décident du canal.
 */

import { SQL } from "bun";
import { Database } from "bun:sqlite";

import { resolveMirrorPath } from "../config";

// ─── Couleurs ────────────────────────────────────────────────────────

/** Palette ANSI utilisée par les commandes (chaînes vides si couleur coupée). */
export interface AnsiPalette {
	reset: string;
	green: string;
	yellow: string;
	blue: string;
	magenta: string;
	cyan: string;
	red: string;
	bold: string;
}

/**
 * Couleur active : uniquement sur un vrai terminal et hors `NO_COLOR`
 * (https://no-color.org). Un pipe ou une redirection produit donc du texte nu.
 */
export const isColorEnabled: boolean = Boolean(process.stdout.isTTY && !process.env.NO_COLOR);

const ansi = (code: string): string => (isColorEnabled ? code : "");

export const colors: AnsiPalette = {
	reset: ansi("\x1b[0m"),
	green: ansi("\x1b[32m"),
	yellow: ansi("\x1b[33m"),
	blue: ansi("\x1b[34m"),
	magenta: ansi("\x1b[35m"),
	cyan: ansi("\x1b[36m"),
	red: ansi("\x1b[31m"),
	bold: ansi("\x1b[1m"),
};

// ─── Contrat de sortie `--json` ──────────────────────────────────────

const originalLog = console.log;
const originalWarn = console.warn;
const originalError = console.error;

/**
 * Coupe `console.log/warn/error` le temps de préparer une réponse `--json` :
 * les couches de données (inagle, Redis) journalisent leur chargement sur
 * stdout, ce qui corromprait un flux JSON destiné à être `jq`-é.
 * Idempotent, sans effet quand `json` est faux.
 */
export function suppressLogs(json: boolean): void {
	if (json) {
		console.log = () => {};
		console.warn = () => {};
		console.error = () => {};
	}
}

/** Restaure les `console.*` d'origine juste avant d'émettre la réponse. */
export function restoreLogs(json: boolean): void {
	if (json) {
		console.log = originalLog;
		console.warn = originalWarn;
		console.error = originalError;
	}
}

/**
 * Émet une erreur selon le mode courant : objet `{ error }` sur stdout en
 * `--json`, ligne rouge sur stderr sinon. Restaure les logs au passage.
 */
export function reportError(json: boolean | undefined, message: string, humanLine: string): void {
	restoreLogs(Boolean(json));
	if (json) {
		console.log(JSON.stringify({ error: message }));
	} else {
		console.error(humanLine);
	}
}

// ─── Entrées ─────────────────────────────────────────────────────────

/**
 * Résout une entrée depuis l'argument positionnel, ou depuis stdin quand la
 * commande est en bout de pipe (`cat q.txt | azalee search`). Renvoie une
 * chaîne vide si aucune source n'est disponible.
 */
export async function getOrReadInput(arg: string | undefined): Promise<string> {
	if (arg !== undefined && arg !== null && arg.trim() !== "") {
		return arg;
	}
	if (!process.stdin.isTTY) {
		return await Bun.stdin.text();
	}
	return "";
}

// ─── Rendu tabulaire ─────────────────────────────────────────────────

/** Ligne quelconque renvoyée par une requête SQL. */
export type TableRow = Record<string, unknown>;

/**
 * Rend un tableau de lignes en table ASCII encadrée. Les colonnes proviennent
 * de la première ligne ; les valeurs sont tronquées à 45 caractères (`…` sur
 * 3 points) pour rester lisibles dans un terminal 80 colonnes.
 */
export function renderAsciiTable(rows: readonly TableRow[]): string {
	if (!rows || rows.length === 0) return "Aucun résultat (0 ligne).";
	const keys = Object.keys(rows[0]);

	const stringifiedRows = rows.map((row) => {
		const newRow: Record<string, string> = {};
		for (const key of keys) {
			const val = row[key];
			if (val === null || val === undefined) {
				newRow[key] = "NULL";
			} else if (typeof val === "object") {
				newRow[key] = JSON.stringify(val);
			} else {
				newRow[key] = String(val);
			}
			if (newRow[key].length > 45) {
				newRow[key] = newRow[key].substring(0, 42) + "...";
			}
		}
		return newRow;
	});

	const colWidths = keys.map((k) => Math.max(k.length, ...stringifiedRows.map((r) => r[k].length)));

	const topBorder = "┌─" + colWidths.map((w) => "─".repeat(w)).join("─┬─") + "─┐";
	const headerLine = "│ " + keys.map((k, i) => k.padEnd(colWidths[i])).join(" │ ") + " │";
	const midBorder = "├─" + colWidths.map((w) => "─".repeat(w)).join("─┼─") + "─┤";
	const bottomBorder = "└─" + colWidths.map((w) => "─".repeat(w)).join("─┴─") + "─┘";

	const formattedRows = stringifiedRows.map(
		(r) => "│ " + keys.map((k, i) => r[k].padEnd(colWidths[i])).join(" │ ") + " │",
	);

	return [topBorder, headerLine, midBorder, ...formattedRows, bottomBorder].join("\n");
}

// ─── Postgres (Bun.SQL) ──────────────────────────────────────────────

/** Résultat d'une requête Postgres, calqué sur la surface `pg` historique. */
export interface PgQueryResult<Row = TableRow> {
	rows: Row[];
	rowCount: number;
}

/**
 * Client Postgres minimal (`connect`/`query`/`end`) au-dessus de `Bun.SQL` :
 * zéro dépendance npm, mêmes placeholders `$1` et mêmes lignes renvoyées que
 * l'ancien client `pg`.
 */
export interface PgClient {
	connect(): Promise<void>;
	query<Row = TableRow>(text: string, params?: unknown[]): Promise<PgQueryResult<Row>>;
	end(): Promise<void>;
}

export function createPgClient(connectionString: string): PgClient {
	const sql = new SQL({ url: connectionString, max: 1 });
	return {
		async connect(): Promise<void> {},
		async query<Row = TableRow>(text: string, params: unknown[] = []): Promise<PgQueryResult<Row>> {
			const rows = (await sql.unsafe(text, params)) as Row[];
			return { rows, rowCount: rows.length };
		},
		async end(): Promise<void> {
			await sql.end();
		},
	};
}

// ─── SQLite (miroir `inagle_*`) ──────────────────────────────────────

/**
 * Miroir SQLite des tables `inagle_*`. Résolution déléguée à la lib
 * (`SQLITE_DB_PATH` épinglé → `mirror.sqlite` → snapshot le plus récent) :
 * une seule implémentation partagée par le CLI, l'API et le wiki web.
 */
export function getSqlitePath(): string | null {
	return resolveMirrorPath();
}

/** Ouvre le miroir en lecture seule. Lève si le chemin est invalide. */
export function openReadonlyDatabase(dbPath: string): Database {
	return new Database(dbPath, { readonly: true });
}

// ─── Sous-processus ──────────────────────────────────────────────────

/**
 * Exécute une commande et renvoie sa sortie standard. Lève avec le contenu de
 * stderr en cas de code de retour non nul.
 */
export function runCapture(cmd: string[], options: { cwd?: string } = {}): string {
	const res = Bun.spawnSync(cmd, { cwd: options.cwd, stdout: "pipe", stderr: "pipe" });
	if (res.exitCode !== 0) {
		throw new Error(res.stderr?.toString().trim() || `échec: ${cmd.join(" ")}`);
	}
	return res.stdout.toString();
}

/**
 * Message d'erreur lisible pour une valeur `catch` de type inconnu.
 *
 * On lit `.message` par structure et non via `instanceof Error` : Bun lève des
 * objets qui portent un `message` sans dériver d'`Error` (`ResolveMessage`,
 * `BuildMessage`), dont le `String()` préfixerait le nom de classe.
 */
export function errorMessage(e: unknown): string {
	const message = (e as { message?: unknown } | null | undefined)?.message;
	return typeof message === "string" ? message : String(e);
}
