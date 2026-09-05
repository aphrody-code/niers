/**
 * `azalee repair` — remet la base locale en état d'être lue par le CLI :
 * désactive RLS et redonne les privilèges sur les tables `inagle_*`.
 */

import type { Command } from "commander";

import { colors, createPgClient, errorMessage, runCapture } from "../context";

export function registerRepairCommand(program: Command): void {
	program
		.command("repair")
		.description("Diagnostique et répare la base de données locale (RLS, privilèges)")
		.action(async () => {
			console.log(`${colors.cyan}Diagnostics et réparation de la base de données...${colors.reset}`);
			const dbUrl = process.env.DATABASE_URL;
			if (!dbUrl) {
				console.error(`${colors.red}Erreur: DATABASE_URL non définie dans l'environnement.${colors.reset}`);
				return;
			}

			console.log(`Connexion à la base via : ${dbUrl}`);
			try {
				const client = createPgClient(dbUrl);
				await client.connect();

				const { rows: tables } = await client.query<{ table_name: string }>(
					`SELECT table_name FROM information_schema.tables
					 WHERE table_schema = 'public' AND table_name LIKE 'inagle_%'`,
				);

				console.log(`  Trouvé ${tables.length} tables Inagle. Désactivation RLS et attribution des droits...`);

				for (const t of tables) {
					const tableName = t.table_name;
					try {
						await client.query(`ALTER TABLE "${tableName}" DISABLE ROW LEVEL SECURITY;`);
						await client.query(`GRANT ALL PRIVILEGES ON TABLE "${tableName}" TO ubuntu;`);
						console.log(`  ✓ Table "${tableName}" réparée.`);
					} catch (e) {
						console.log(
							`  ⚠ Impossible de réparer "${tableName}" directement: ${errorMessage(e)}. Tentative via sudo...`,
						);
						runCapture([
							"sudo",
							"-u",
							"postgres",
							"psql",
							"-d",
							"rose_griffon",
							"-c",
							`ALTER TABLE "${tableName}" DISABLE ROW LEVEL SECURITY; GRANT ALL PRIVILEGES ON TABLE "${tableName}" TO ubuntu;`,
						]);
						console.log(`  ✓ Table "${tableName}" réparée via sudo postgres.`);
					}
				}

				try {
					await client.query(`ALTER TABLE patch_notes DISABLE ROW LEVEL SECURITY;`);
					await client.query(`GRANT ALL PRIVILEGES ON TABLE patch_notes TO ubuntu;`);
					console.log(`  ✓ Table "patch_notes" réparée.`);
				} catch {
					runCapture([
						"sudo",
						"-u",
						"postgres",
						"psql",
						"-d",
						"rose_griffon",
						"-c",
						"ALTER TABLE patch_notes DISABLE ROW LEVEL SECURITY; GRANT ALL PRIVILEGES ON TABLE patch_notes TO ubuntu;",
					]);
					console.log(`  ✓ Table "patch_notes" réparée via sudo postgres.`);
				}

				await client.end();
				console.log(`\n${colors.bold}${colors.green}Base de données locale réparée avec succès !${colors.reset}`);
			} catch (e) {
				console.error(`${colors.red}Erreur de connexion / privilèges : ${errorMessage(e)}${colors.reset}`);
			}
		});
}
