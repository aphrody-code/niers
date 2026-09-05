/**
 * Matérialisation de l'index CPK en SQLite requêtable.
 *
 * Partagé entre le script générateur (`scripts/build-cpk-index.ts`) et la lib
 * runtime (`cpk/index.ts`). Décompose chaque entrée `[path, cpk]` de l'index
 * Redis en colonnes (`top, sub, dir, name, ext, depth`) et construit la table
 * `cpk_index` + ses index couvrants pour un listing d'arbre en O(log n) par
 * répertoire.
 *
 * ⚠ Module Bun/serveur uniquement (`bun:sqlite`) : jamais depuis un contexte
 * navigateur. La lib n'embarque aucune garde `server-only` (elle casserait le
 * CLI et le sidecar Tauri) — cette garde vit dans la façade Next de l'app,
 * `apps/azalee/lib/cpk/index.ts`.
 */
import type { Database } from "bun:sqlite";

/** Une entrée brute de l'index : `[chemin complet, archive CPK]`. */
export type CpkEntry = [path: string, cpk: string];

/** Une ligne décomposée prête à insérer. */
export interface CpkRow {
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
 * Décompose un chemin index (`data/dx11/menu/.../c.g4tx`) + son CPK en colonnes.
 * `top`/`sub` ignorent le préfixe racine `data/` (présent sur 100 % des entrées)
 * pour que le 1er niveau de l'arbre soit `common` / `dx11` / `movie` / `font`…
 */
export function decompose(filePath: string, cpk: string): CpkRow {
	const segments = filePath.split("/");
	const name = segments[segments.length - 1] ?? filePath;
	const dir = segments.slice(0, -1).join("/");
	const dot = name.lastIndexOf(".");
	const ext = dot > 0 ? name.slice(dot + 1).toLowerCase() : "";

	const logical = segments[0] === "data" ? segments.slice(1) : segments;
	const top = logical[0] ?? "";
	const sub = logical[1] ?? "";

	return { path: filePath, top, sub, dir, name, ext, cpk, depth: segments.length };
}

/**
 * Construit (ou reconstruit) la table `cpk_index` dans `db` à partir des entrées
 * brutes. Crée le schéma, insère en une transaction, pose les index couvrants.
 */
export function buildSqliteFromEntries(db: Database, entries: CpkEntry[]): void {
	db.exec("PRAGMA journal_mode = WAL");
	db.exec("DROP TABLE IF EXISTS cpk_index");
	db.exec(`
		CREATE TABLE cpk_index (
			path  TEXT NOT NULL,
			top   TEXT NOT NULL,
			sub   TEXT NOT NULL,
			dir   TEXT NOT NULL,
			name  TEXT NOT NULL,
			ext   TEXT NOT NULL,
			cpk   TEXT NOT NULL,
			depth INTEGER NOT NULL
		)
	`);

	const insert = db.prepare(
		"INSERT INTO cpk_index (path, top, sub, dir, name, ext, cpk, depth) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
	);
	const insertMany = db.transaction((batch: CpkEntry[]) => {
		for (const [filePath, cpk] of batch) {
			const r = decompose(filePath, cpk);
			insert.run(r.path, r.top, r.sub, r.dir, r.name, r.ext, r.cpk, r.depth);
		}
	});
	insertMany(entries);

	// Index couvrants pour le listing d'arbre :
	//  - `dir`     : fichiers directs d'un répertoire (listDir).
	//  - `path`    : fileMeta(path) lookup + détection « est-ce un dossier ».
	//  - `(top,sub)`: navigation top-level.
	db.exec("CREATE INDEX idx_cpk_dir ON cpk_index (dir)");
	db.exec("CREATE INDEX idx_cpk_path ON cpk_index (path)");
	db.exec("CREATE INDEX idx_cpk_top_sub ON cpk_index (top, sub)");
	db.exec("PRAGMA wal_checkpoint(TRUNCATE)");
	db.exec("ANALYZE");
}
