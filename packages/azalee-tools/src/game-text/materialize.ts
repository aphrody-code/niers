/**
 * Matérialisation de l'index de texte de jeu IEVR en SQLite requêtable.
 *
 * Partagé entre le script générateur (`scripts/build-game-text-index.ts`) et la
 * lib runtime (`game-text/index.ts`). Chaque entrée de l'artefact NDJSON est
 * une ligne `[hashId, locale, category, value]` ; on construit la table `text`
 * (PK `hashId,locale`) + un index sur `category` pour les listings par domaine.
 *
 * ⚠ Module Bun/serveur uniquement (`bun:sqlite`) : jamais depuis un contexte
 * navigateur. La lib n'embarque aucune garde `server-only` (elle casserait le
 * CLI et le sidecar Tauri) — cette garde vit dans la façade Next de l'app,
 * `apps/azalee/lib/game-text/index.ts`.
 */
import type { Database } from "bun:sqlite";
import { formatGameText } from "@rosegriffon/azalee/game-text/format";

/** Une entrée brute de l'artefact : `[hashId, locale, category, value]`. */
export type TextEntry = [hashId: string, locale: string, category: string, value: string];

/**
 * Construit (ou reconstruit) la table `text` dans `db` à partir des entrées.
 * Crée le schéma, insère en transactions par lots, pose l'index `category`.
 *
 * Le couple `(hashId, locale)` n'est PAS strictement unique en théorie : le même
 * hashId peut apparaître dans plusieurs dicts (ex. un nom partagé item/menu). On
 * insère donc avec `INSERT OR IGNORE` (première occurrence gagne, déterministe
 * via l'ordre d'écriture de l'artefact) afin de respecter la PK demandée
 * `(hashId, locale)`. Le compteur de collisions ignorées est journalisé par le
 * script générateur.
 */
export function buildSqliteFromEntries(db: Database, entries: TextEntry[]): { inserted: number; ignored: number } {
	db.exec("PRAGMA journal_mode = WAL");
	db.exec("DROP TABLE IF EXISTS text");
	db.exec(`
		CREATE TABLE text (
			hashId   TEXT NOT NULL,
			locale   TEXT NOT NULL,
			category TEXT NOT NULL,
			value    TEXT NOT NULL,
			PRIMARY KEY (hashId, locale)
		) WITHOUT ROWID
	`);

	const insert = db.prepare(
		"INSERT OR IGNORE INTO text (hashId, locale, category, value) VALUES (?, ?, ?, ?)",
	);
	let inserted = 0;
	let ignored = 0;
	const insertMany = db.transaction((batch: TextEntry[]) => {
		for (const [hashId, locale, category, value] of batch) {
			// Défense en profondeur : nettoie aussi à l'insert (artefact déjà
			// nettoyé en amont, mais on garantit l'invariant côté SQLite).
			const res = insert.run(hashId, locale, category, formatGameText(value));
			if (res.changes > 0) inserted++;
			else ignored++;
		}
	});

	// Insertion par lots de 50k pour borner la mémoire de la transaction.
	const CHUNK = 50_000;
	for (let i = 0; i < entries.length; i += CHUNK) {
		insertMany(entries.slice(i, i + CHUNK));
	}

	// Index couvrant pour les listings par domaine (`/passive`, `/item`, …) et la
	// recherche plein-texte naïve (`value LIKE`). La PK couvre déjà les lookups
	// `(hashId, locale)`.
	db.exec("CREATE INDEX idx_text_category ON text (category, locale)");
	db.exec("PRAGMA wal_checkpoint(TRUNCATE)");
	db.exec("ANALYZE");

	return { inserted, ignored };
}
