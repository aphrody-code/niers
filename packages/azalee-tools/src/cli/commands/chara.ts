/** `azalee chara` — fiche détaillée d'un joueur (profil, stats, techniques). */

import type { Command } from "commander";

import { interpolateVariantStats } from "@rosegriffon/azalee/game/stats-interpolation";
import { colors, errorMessage, getOrReadInput, reportError, restoreLogs, suppressLogs } from "../context";
import { createInagleService } from "../inagle";
import { renderCharaProfile } from "../render";
import { getActiveReadline, setPendingSelection } from "../repl-state";
import type { CharaOptions } from "../types";

export function registerCharaCommand(program: Command): void {
	program
		.command("chara [query]")
		.description("Affiche les détails d'un joueur d'Inazuma Eleven (profil, stats, techniques, variants)")
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (query: string | undefined, options: CharaOptions) => {
			suppressLogs(!!options.json);
			try {
				const inputQuery = await getOrReadInput(query);
				if (!inputQuery.trim()) {
					reportError(
						options.json,
						"Aucun joueur spécifié",
						`${colors.red}Erreur: Spécifiez un nom ou un ID de joueur (ex: Mark).${colors.reset}`,
					);
					return;
				}

				const svc = await createInagleService();
				const chars = svc.characters.baseCharacters();

				const matches = chars.filter((c: any) => {
					const q = inputQuery.toLowerCase().trim();
					if (c.charaId.toLowerCase() === q || c.internalCode?.toLowerCase() === q) {
						return true;
					}
					const nameFr = c.names?.fr?.toLowerCase() || "";
					const nameEn = c.names?.en?.toLowerCase() || "";
					const nameJa = c.names?.ja?.toLowerCase() || "";
					const slug = c.slug?.toLowerCase() || "";
					return nameFr.includes(q) || nameEn.includes(q) || nameJa.includes(q) || slug.includes(q);
				});

				restoreLogs(!!options.json);

				if (matches.length === 0) {
					if (options.json) {
						console.log(JSON.stringify([], null, 2));
					} else {
						console.log(`${colors.yellow}Aucun joueur trouvé pour : "${inputQuery}"${colors.reset}`);
					}
					return;
				}

				if (matches.length > 1) {
					if (options.json) {
						console.log(
							JSON.stringify(
								matches.map((m: any) => ({ id: m.charaId, name: m.names.fr || m.names.en || m.names.ja })),
								null,
								2,
							),
						);
					} else {
						const rl = getActiveReadline();
						if (rl) {
							// En shell : liste numérotée puis attente d'un choix.
							console.log(`${colors.yellow}Plusieurs joueurs correspondent à votre recherche :${colors.reset}`);
							for (const [idx, m] of matches.entries()) {
								console.log(
									`  [${colors.bold}${idx + 1}${colors.reset}] ${colors.bold}${m.names.fr || m.names.en}${colors.reset} (ID: ${m.charaId} | Code: ${m.internalCode})`,
								);
							}
							setPendingSelection({ type: "chara", matches });
							rl.setPrompt(
								`${colors.bold}${colors.yellow}Choisissez [1-${matches.length}] ou tapez une commande > ${colors.reset}`,
							);
							rl.prompt();
						} else {
							console.log(`${colors.yellow}Plusieurs joueurs correspondent à votre recherche :${colors.reset}`);
							for (const m of matches) {
								console.log(
									`  - ${colors.bold}${m.names.fr || m.names.en}${colors.reset} (ID: ${m.charaId} | Code: ${m.internalCode})`,
								);
							}
						}
					}
					return;
				}

				if (options.json) {
					// En JSON on enrichit avec la courbe de stats, absente du dump brut.
					const enhancedChara = {
						...matches[0],
						variants: matches[0].variants?.map((v: any) => ({
							...v,
							statsGrowth: {
								lv1: interpolateVariantStats(v, 1),
								lv50: interpolateVariantStats(v, 50),
								lv99: interpolateVariantStats(v, 99),
							},
						})),
					};
					console.log(JSON.stringify(enhancedChara, null, 2));
				} else {
					console.log(renderCharaProfile(matches[0], svc));
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur chara : ${errorMessage(e)}${colors.reset}`,
				);
			}
		});
}
