#!/usr/bin/env bun
/**
 * Génère `lib/supabase/drizzle-schema.ts` par introspection du miroir SQLite
 * embarqué (tables `inagle_*` STATIQUES — ne changent qu'à un re-dump du jeu).
 *
 * À RELANCER UNIQUEMENT après un re-dump (la donnée est statique → 0 churn entre
 * deux dumps). Déterministe : lit `pragma_table_info` pour chaque table `inagle_%`
 * et émet une définition `sqliteTable` Drizzle par table.
 *
 *   bun apps/azalee/scripts/ops/gen-drizzle-schema.ts
 *
 * NB : les tables du miroir n'ont PAS de PRIMARY KEY déclarée (dump brut). On
 * n'en ajoute pas — la façade ne fait que des SELECT, Drizzle n'a pas besoin de
 * PK pour `db.select()`. Le mapping de type est volontairement minimal : SQLite
 * stocke en TEXT/INTEGER/REAL/BLOB et la façade reparse le JSON elle-même
 * (parseRow). On mappe donc INTEGER→integer, REAL→real, le reste→text.
 */
import { readdirSync } from "node:fs";
import path from "node:path";

type ColInfo = { name: string; type: string };

function getMirrorPath(): string {
	if (process.env.SQLITE_DB_PATH) return path.resolve(process.env.SQLITE_DB_PATH);
	// Même règle que `sync-supabase-to-sqlite.ts` : l'emplacement des instantanés se donne
	// par l'environnement, jamais par le chemin d'une autre machine.
	const dirs = [
		process.env.AZALEE_DATA_DIR ? path.join(process.env.AZALEE_DATA_DIR, "backups") : undefined,
		path.resolve(process.cwd(), "apps/azalee/data/backups"),
		path.resolve(process.cwd(), "data/backups"),
	].filter((d): d is string => Boolean(d));
	for (const dir of dirs) {
		try {
			const files = readdirSync(dir);
			const mirror = files.find((f) => f === "mirror.sqlite");
			if (mirror) return path.join(dir, mirror);
			const sqliteFiles = files
				.filter((f) => f.startsWith("supabase-") && f.endsWith(".sqlite"))
				.sort((a, b) => b.localeCompare(a));
			if (sqliteFiles.length > 0) return path.join(dir, sqliteFiles[0]);
		} catch {
			/* ignore */
		}
	}
	throw new Error("Miroir SQLite introuvable");
}

// drizzle column builder selon le type SQLite déclaré
function colBuilder(type: string): string {
	const t = type.toUpperCase();
	if (t.includes("INT")) return "integer";
	if (t.includes("REAL") || t.includes("FLOA") || t.includes("DOUB")) return "real";
	if (t.includes("BLOB")) return "blob";
	return "text"; // TEXT, NUMERIC, vide, etc.
}

// Identifiant JS sûr pour la clé de propriété (les colonnes sont alphanum/_)
function propKey(name: string): string {
	return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(name) ? name : JSON.stringify(name);
}

function main() {
	const dbPath = getMirrorPath();
	const { Database } = require("bun:sqlite");
	const db = new Database(dbPath, { readonly: true });

	const tables = (
		db
			.query(
				"SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'inagle_%' ORDER BY name"
			)
			.all() as Array<{ name: string }>
	).map((r) => r.name);

	const usedBuilders = new Set<string>();
	const blocks: string[] = [];

	for (const table of tables) {
		const cols = db.query(`PRAGMA table_info("${table}")`).all() as Array<ColInfo>;
		const lines = cols.map((c) => {
			const b = colBuilder(c.type);
			usedBuilders.add(b);
			// On passe le nom SQL explicite à Drizzle pour rester fidèle au schéma réel.
			return `\t${propKey(c.name)}: ${b}(${JSON.stringify(c.name)}),`;
		});
		// Nom d'export = table en camelCase-ish ; on garde le nom de table tel quel.
		const exportName = table.replace(/[^A-Za-z0-9_]/g, "_");
		blocks.push(
			`export const ${exportName} = sqliteTable(${JSON.stringify(table)}, {\n${lines.join("\n")}\n});`
		);
	}

	db.close();

	const builderImport = [...usedBuilders].sort().join(", ");
	const header = `// GÉNÉRÉ par scripts/ops/gen-drizzle-schema.ts — NE PAS ÉDITER À LA MAIN.
// Introspection du miroir SQLite (tables inagle_* STATIQUES). Re-générer après un
// re-dump du jeu uniquement. Cf. docs/decision-archi-donnees-azalee.md (Phase 4).
import { sqliteTable, ${builderImport} } from "drizzle-orm/sqlite-core";

`;

	const out = header + blocks.join("\n\n") + "\n";
	const target = path.resolve(process.cwd(), "apps/azalee/lib/supabase/drizzle-schema.ts");
	const altTarget = path.resolve(process.cwd(), "lib/supabase/drizzle-schema.ts");
	const finalTarget = process.cwd().endsWith("azalee") ? altTarget : target;
	Bun.write(finalTarget, out);
	console.log(`schema=${finalTarget} tables=${tables.length}`);
}

main();
