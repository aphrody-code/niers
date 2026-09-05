/** `azalee sync` — synchronisation des fichiers de données locaux ↔ PostgreSQL. */

import type { Command } from "commander";

import { colors } from "../context";
import { exitUnlessRepl } from "../repl-state";
import type { SyncOptions } from "../types";

export function registerSyncCommand(program: Command): void {
	program
		.command("sync")
		.description("Synchronise les fichiers locaux de données avec la base PostgreSQL locale")
		.option("-p, --push", "Pousse les données Inagle locales vers PostgreSQL (inagle push)")
		.option("-j, --json", "Synchronise les corrections SQL vers characters.json (sync-db-to-json)")
		.action(async (options: SyncOptions) => {
			if (options.push) {
				console.log(
					`${colors.cyan}Exécution de inagle push pour synchroniser les configs de données...${colors.reset}`,
				);
				const proc = Bun.spawn(["bun", "packages/inagle/src/cli-push.ts"], {
					stdout: "inherit",
					stderr: "inherit",
				});
				const code = await proc.exited;
				exitUnlessRepl(code);
			} else if (options.json) {
				console.log(
					`${colors.cyan}Synchronisation de la base locale vers le fichier characters.json...${colors.reset}`,
				);
				const proc = Bun.spawn(["bun", "scripts/inagle/pipeline/sync-db-to-json.ts"], {
					stdout: "inherit",
					stderr: "inherit",
				});
				const code = await proc.exited;
				exitUnlessRepl(code);
			} else {
				console.log(`${colors.yellow}Veuillez spécifier une option: --push ou --json.${colors.reset}`);
			}
		});
}
