/** `azalee skill` — fiche d'une technique, d'une passive ou d'une aura. */

import type { Command } from "commander";

import { colors, errorMessage, getOrReadInput, reportError, restoreLogs, suppressLogs } from "../context";
import { createInagleService } from "../inagle";
import { renderSkillProfile } from "../render";
import { getActiveReadline, setPendingSelection } from "../repl-state";
import type { SkillOptions } from "../types";

export function registerSkillCommand(program: Command): void {
	program
		.command("skill [query]")
		.description("Affiche les détails d'une technique / compétence / passive")
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (query: string | undefined, options: SkillOptions) => {
			suppressLogs(!!options.json);
			try {
				const inputQuery = await getOrReadInput(query);
				if (!inputQuery.trim()) {
					reportError(
						options.json,
						"Aucun mot-clé spécifié",
						`${colors.red}Erreur: Spécifiez un nom ou un ID de move (ex: Fire Tornado).${colors.reset}`,
					);
					return;
				}

				const svc = await createInagleService();

				// Résolution par identifiant d'abord (technique, passive, aura).
				let skill: any =
					svc.skills.get(inputQuery) || svc.skills.passiveGet(inputQuery) || svc.skills.auraGet(inputQuery);

				// À défaut, recherche par nom sur les techniques puis les auras.
				if (!skill) {
					const mainMatches = svc.skills.search(inputQuery, { limit: 5 });
					const auraMatches = svc.skills.auraSearch(inputQuery).slice(0, 5);

					const matches = [
						...mainMatches.map((s: any) => ({ ...s, skillType: "main" as const })),
						...auraMatches.map((a: any) => ({
							...a,
							skillType: "aura" as const,
							displayName: a.displayName || a.name_FR || a.name_EN,
						})),
					];

					if (matches.length === 0) {
						restoreLogs(!!options.json);
						if (options.json) {
							console.log(JSON.stringify([], null, 2));
						} else {
							console.log(`${colors.yellow}Aucune technique trouvée pour: "${inputQuery}"${colors.reset}`);
						}
						return;
					}

					if (matches.length > 1) {
						restoreLogs(!!options.json);
						if (options.json) {
							console.log(JSON.stringify(matches, null, 2));
						} else {
							const rl = getActiveReadline();
							if (rl) {
								console.log(`${colors.yellow}Plusieurs techniques correspondent :${colors.reset}`);
								for (const [idx, m] of matches.entries()) {
									const typeStr = m.skillType.toUpperCase();
									console.log(
										`  [${colors.bold}${idx + 1}${colors.reset}] [${colors.bold}${colors.green}${typeStr}${colors.reset}] ${colors.bold}${m.displayName || m.name_FR}${colors.reset} (ID: ${m.skillIDStr || m.auraId || m.passiveId || "N/A"})`,
									);
								}
								setPendingSelection({ type: "skill", matches });
								rl.setPrompt(
									`${colors.bold}${colors.yellow}Choisissez [1-${matches.length}] ou tapez une commande > ${colors.reset}`,
								);
								rl.prompt();
							} else {
								console.log(`${colors.yellow}Plusieurs techniques correspondent :${colors.reset}`);
								for (const m of matches) {
									const typeStr = m.skillType.toUpperCase();
									console.log(
										`  - [${colors.bold}${colors.green}${typeStr}${colors.reset}] ${colors.bold}${m.displayName || m.name_FR}${colors.reset} (ID: ${m.skillIDStr || m.auraId || m.passiveId || "N/A"})`,
									);
								}
							}
						}
						return;
					}

					skill = matches[0];
				}

				restoreLogs(!!options.json);
				if (options.json) {
					console.log(JSON.stringify(skill, null, 2));
				} else {
					console.log(renderSkillProfile(skill, svc));
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur skill : ${errorMessage(e)}${colors.reset}`,
				);
			}
		});
}
