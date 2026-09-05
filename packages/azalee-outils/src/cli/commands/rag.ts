/** `azalee rag` — interrogation sémantique de la base de connaissances. */

import type { Command } from "commander";

import { queryRag } from "@rosegriffon/azalee/rag";
import { colors, errorMessage, getOrReadInput, reportError, restoreLogs, suppressLogs } from "../context";
import { exitUnlessRepl } from "../repl-state";
import type { RagOptions } from "../types";

export function registerRagCommand(program: Command): void {
	program
		.command("rag [query]")
		.description("Interroge la base de connaissances sémantique (RAG) sur Inazuma Eleven")
		.option("-j, --json", "Format de sortie en JSON brute")
		.option("-l, --limit <limit>", "Nombre maximum de résultats", "4")
		.action(async (query: string | undefined, options: RagOptions) => {
			suppressLogs(!!options.json);
			try {
				const inputQuery = await getOrReadInput(query);
				if (!inputQuery.trim()) {
					reportError(
						options.json,
						"Requête sémantique vide",
						`${colors.red}Erreur: Requête sémantique vide.${colors.reset}`,
					);
					return;
				}

				if (!options.json) {
					console.log(`${colors.cyan}Recherche RAG pour : "${inputQuery}"...${colors.reset}`);
				}

				const limitVal = parseInt(options.limit, 10) || 4;
				const results = await queryRag(inputQuery, limitVal);

				restoreLogs(!!options.json);

				if (options.json) {
					console.log(JSON.stringify(results, null, 2));
				} else {
					console.log(`\n${colors.bold}${colors.green}Résultats du RAG :${colors.reset}`);
					console.log("─".repeat(80));
					if (results.length === 0) {
						console.log(`${colors.yellow}Aucun résultat pertinent trouvé.${colors.reset}`);
					}
					for (const r of results) {
						console.log(
							`[${colors.cyan}Score: ${(r.score * 100).toFixed(1)}%${colors.reset}] ${colors.bold}${r.title}${colors.reset} (${r.type})`,
						);
						console.log(`${colors.blue}Source: ${r.url || "N/A"}${colors.reset}`);
						console.log(r.text.substring(0, 300) + (r.text.length > 300 ? "..." : ""));
						console.log("─".repeat(80));
					}
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur RAG : ${errorMessage(e)}${colors.reset}`,
				);
			} finally {
				exitUnlessRepl(0);
			}
		});
}
