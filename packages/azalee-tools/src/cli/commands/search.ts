/** `azalee search` — recherche floue globale sur toutes les entités inagle. */

import type { Command } from "commander";

import { colors, errorMessage, getOrReadInput, reportError, restoreLogs, suppressLogs } from "../context";
import { createInagleService } from "../inagle";
import type { SearchOptions } from "../types";

export function registerSearchCommand(program: Command): void {
	program
		.command("search [query]")
		.description("Fuzzy search ultra-rapide sur l'ensemble des entités du jeu (Inagle)")
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (query: string | undefined, options: SearchOptions) => {
			suppressLogs(!!options.json);
			try {
				const inputQuery = await getOrReadInput(query);
				if (!inputQuery.trim()) {
					reportError(
						options.json,
						"Requête de recherche vide",
						`${colors.red}Erreur: Requête de recherche vide.${colors.reset}`,
					);
					return;
				}

				const svc = await createInagleService();
				const results = svc.search.global(inputQuery, { limit: 15 });

				if (options.json) {
					restoreLogs(true);
					console.log(JSON.stringify(results, null, 2));
					return;
				}

				console.log(`${colors.cyan}Recherche Inagle pour : "${inputQuery}"...${colors.reset}`);
				if (results.length === 0) {
					console.log(`${colors.yellow}Aucun résultat trouvé.${colors.reset}`);
					return;
				}

				console.log(`\n${colors.bold}${colors.blue}Résultats de la recherche :${colors.reset}`);
				console.log("─".repeat(80));
				for (const r of results) {
					const typeLabel = r.type.toUpperCase().padEnd(10);
					const nameLabel = `${colors.bold}${r.name}${colors.reset}`.padEnd(45);
					const idLabel = `${colors.yellow}(ID/Code: ${r.id})${colors.reset}`;
					console.log(`[${colors.green}${typeLabel}${colors.reset}] ${nameLabel} ${idLabel}`);
				}
				console.log("─".repeat(80));
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur lors de la recherche : ${errorMessage(e)}${colors.reset}`,
				);
			}
		});
}
