/** `azalee item` — fiche d'un objet (consommable, équipement, cosmétique). */

import type { Command } from "commander";

import { colors, errorMessage, getOrReadInput, reportError, restoreLogs, suppressLogs } from "../context";
import { createInagleService } from "../inagle";
import { renderItemProfile } from "../render";
import { getActiveReadline, setPendingSelection } from "../repl-state";
import type { ItemOptions } from "../types";

export function registerItemCommand(program: Command): void {
	program
		.command("item [query]")
		.description("Affiche les détails d'un objet (consommable, équipement, cosmétique)")
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (query: string | undefined, options: ItemOptions) => {
			suppressLogs(!!options.json);
			try {
				const inputQuery = await getOrReadInput(query);
				if (!inputQuery.trim()) {
					reportError(
						options.json,
						"Aucun objet spécifié",
						`${colors.red}Erreur: Spécifiez un nom ou un ID d'objet (ex: Bottes).${colors.reset}`,
					);
					return;
				}

				const svc = await createInagleService();

				// Résolution par identifiant d'abord, recherche par nom ensuite.
				let item = svc.items.getItem(inputQuery);

				if (!item) {
					const matches = svc.items.searchItems(inputQuery, { limit: 15 });

					if (matches.length === 0) {
						restoreLogs(!!options.json);
						if (options.json) {
							console.log(JSON.stringify([], null, 2));
						} else {
							console.log(`${colors.yellow}Aucun objet trouvé pour : "${inputQuery}"${colors.reset}`);
						}
						return;
					}

					if (matches.length > 1) {
						restoreLogs(!!options.json);
						if (options.json) {
							console.log(
								JSON.stringify(
									matches.map((m) => ({ id: m.itemId, name: m.names?.fr || m.name })),
									null,
									2,
								),
							);
						} else {
							const rl = getActiveReadline();
							if (rl) {
								console.log(`${colors.yellow}Plusieurs objets correspondent à votre recherche :${colors.reset}`);
								for (const [idx, m] of matches.entries()) {
									console.log(
										`  [${colors.bold}${idx + 1}${colors.reset}] ${colors.bold}${m.names?.fr || m.name}${colors.reset} (ID: ${m.itemId} | Code: ${m.internalCode})`,
									);
								}
								setPendingSelection({ type: "item", matches });
								rl.setPrompt(
									`${colors.bold}${colors.yellow}Choisissez [1-${matches.length}] ou tapez une commande > ${colors.reset}`,
								);
								rl.prompt();
							} else {
								console.log(`${colors.yellow}Plusieurs objets correspondent à votre recherche :${colors.reset}`);
								for (const m of matches) {
									console.log(
										`  - ${colors.bold}${m.names?.fr || m.name}${colors.reset} (ID: ${m.itemId} | Code: ${m.internalCode})`,
									);
								}
							}
						}
						return;
					}
					item = matches[0];
				}

				restoreLogs(!!options.json);
				if (options.json) {
					console.log(JSON.stringify(item, null, 2));
				} else {
					console.log(renderItemProfile(item));
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur item : ${errorMessage(e)}${colors.reset}`,
				);
			}
		});
}
