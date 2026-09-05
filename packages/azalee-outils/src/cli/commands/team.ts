/** `azalee team` — fiche d'une équipe (nom, uniformes, saisons). */

import type { Command } from "commander";

import { colors, errorMessage, getOrReadInput, reportError, restoreLogs, suppressLogs } from "../context";
import { createInagleService } from "../inagle";
import { renderTeamProfile } from "../render";
import { getActiveReadline, setPendingSelection } from "../repl-state";
import type { TeamOptions } from "../types";

export function registerTeamCommand(program: Command): void {
	program
		.command("team [query]")
		.description("Affiche les détails d'une équipe (nom, uniformes, saisons)")
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (query: string | undefined, options: TeamOptions) => {
			suppressLogs(!!options.json);
			try {
				const inputQuery = await getOrReadInput(query);
				if (!inputQuery.trim()) {
					reportError(
						options.json,
						"Aucune équipe spécifiée",
						`${colors.red}Erreur: Spécifiez un nom ou un ID d'équipe (ex: Raimon).${colors.reset}`,
					);
					return;
				}

				const svc = await createInagleService();

				// Résolution par identifiant d'abord, recherche par nom ensuite.
				let team = svc.teams.getTeam(inputQuery);

				if (!team) {
					const matches = svc.teams.searchTeams(inputQuery);

					if (matches.length === 0) {
						restoreLogs(!!options.json);
						if (options.json) {
							console.log(JSON.stringify([], null, 2));
						} else {
							console.log(`${colors.yellow}Aucune équipe trouvée pour : "${inputQuery}"${colors.reset}`);
						}
						return;
					}

					if (matches.length > 1) {
						restoreLogs(!!options.json);
						if (options.json) {
							console.log(
								JSON.stringify(
									matches.map((m) => ({ id: m.teamId, name: m.name })),
									null,
									2,
								),
							);
						} else {
							const rl = getActiveReadline();
							if (rl) {
								console.log(`${colors.yellow}Plusieurs équipes correspondent à votre recherche :${colors.reset}`);
								for (const [idx, m] of matches.entries()) {
									console.log(
										`  [${colors.bold}${idx + 1}${colors.reset}] ${colors.bold}${m.name}${colors.reset} (ID: ${m.teamId})`,
									);
								}
								setPendingSelection({ type: "team", matches });
								rl.setPrompt(
									`${colors.bold}${colors.yellow}Choisissez [1-${matches.length}] ou tapez une commande > ${colors.reset}`,
								);
								rl.prompt();
							} else {
								console.log(`${colors.yellow}Plusieurs équipes correspondent à votre recherche :${colors.reset}`);
								for (const m of matches) {
									console.log(`  - ${colors.bold}${m.name}${colors.reset} (ID: ${m.teamId})`);
								}
							}
						}
						return;
					}
					team = matches[0];
				}

				restoreLogs(!!options.json);
				if (options.json) {
					console.log(JSON.stringify(team, null, 2));
				} else {
					console.log(renderTeamProfile(team));
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur team : ${errorMessage(e)}${colors.reset}`,
				);
			}
		});
}
