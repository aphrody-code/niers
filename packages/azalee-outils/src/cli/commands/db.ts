/** `azalee db` — requête SQL ad hoc sur PostgreSQL ou sur le miroir SQLite. */

import type { Command } from "commander";

import {
	colors,
	createPgClient,
	errorMessage,
	getOrReadInput,
	getSqlitePath,
	openReadonlyDatabase,
	renderAsciiTable,
	reportError,
	restoreLogs,
	suppressLogs,
	type TableRow,
} from "../context";
import type { DbOptions } from "../types";

export function registerDbCommand(program: Command): void {
	program
		.command("db [sql]")
		.description("Exécute une requête SQL (sur PostgreSQL par défaut, ou sur SQLite avec --sqlite)")
		.option("-s, --sqlite", "Exécute sur la base SQLite de sauvegarde locale")
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (sql: string | undefined, options: DbOptions) => {
			suppressLogs(!!options.json);
			try {
				const inputSql = await getOrReadInput(sql);
				if (!inputSql.trim()) {
					reportError(
						options.json,
						"Requête SQL vide",
						`${colors.red}Erreur: Requête SQL vide.${colors.reset}`,
					);
					return;
				}

				if (options.sqlite) {
					const dbPath = getSqlitePath();
					if (!dbPath) {
						reportError(
							options.json,
							"Aucune base de données SQLite localisée",
							`${colors.red}Erreur: Aucune base de données SQLite localisée.${colors.reset}`,
						);
						return;
					}

					if (!options.json) {
						console.log(`${colors.cyan}SQLite DB utilisé : ${dbPath}${colors.reset}`);
					}

					const db = openReadonlyDatabase(dbPath);
					const results = db.query(inputSql).all() as TableRow[];

					restoreLogs(!!options.json);
					if (options.json) {
						console.log(JSON.stringify(results, null, 2));
					} else {
						console.log(`\n${colors.green}Résultats SQLite (${results.length} lignes) :${colors.reset}`);
						console.log(renderAsciiTable(results));
					}
					db.close();
				} else {
					// PostgreSQL par défaut.
					const dbUrl = process.env.DATABASE_URL;
					if (!dbUrl) {
						reportError(
							options.json,
							"DATABASE_URL non définie dans l'environnement",
							`${colors.red}Erreur: DATABASE_URL non définie dans l'environnement.${colors.reset}`,
						);
						return;
					}

					if (!options.json) {
						console.log(`${colors.cyan}PostgreSQL DB utilisé : ${dbUrl}${colors.reset}`);
					}
					const client = createPgClient(dbUrl);
					await client.connect();
					const res = await client.query(inputSql);
					await client.end();

					restoreLogs(!!options.json);
					if (options.json) {
						console.log(JSON.stringify(res.rows, null, 2));
					} else {
						console.log(`\n${colors.green}Résultats PostgreSQL (${res.rowCount} lignes) :${colors.reset}`);
						console.log(renderAsciiTable(res.rows));
					}
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur SQL : ${errorMessage(e)}${colors.reset}`,
				);
			}
		});
}
