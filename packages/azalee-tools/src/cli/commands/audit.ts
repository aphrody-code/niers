/**
 * `azalee audit` et `azalee test-variants` — contrôles d'intégrité des données.
 *
 * Même famille : `audit` compte les champs manquants, `test-variants` vérifie
 * les invariants d'ordonnancement des variantes de cartes.
 */

import type { Command } from "commander";

import { colors, errorMessage, reportError, restoreLogs, suppressLogs } from "../context";
import { createInagleService } from "../inagle";
import type { AuditOptions } from "../types";

export function registerAuditCommand(program: Command): void {
	program
		.command("audit")
		.description("Audit et diagnostics sur l'intégrité des traductions et de la base de données")
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (options: AuditOptions) => {
			suppressLogs(!!options.json);
			try {
				const svc = await createInagleService();
				const chars = svc.characters.all();
				const skills = svc.skills.all();

				const missingName = chars.filter((c) => !c.names?.fr && !c.names?.en);
				// Champs hérités des anciens dumps (absents du type inagle courant) : on les
				// teste quand même pour ne pas signaler à tort un personnage illustré.
				const legacyImage = (c: unknown) =>
					(c as { image_url?: string; zukan_hash?: string; image?: string }).image_url ??
					(c as { zukan_hash?: string }).zukan_hash ??
					(c as { image?: string }).image;
				const missingImg = chars.filter((c) => !c.icons?.face && !legacyImage(c) && !c.zukanHash);
				const missingStats = chars.filter((c) => !c.stats || Object.keys(c.stats).length === 0);
				const missingSkillName = skills.filter((s) => !s.name_FR && !s.name_EN);

				if (options.json) {
					restoreLogs(true);
					console.log(
						JSON.stringify(
							{
								characters: {
									total: chars.length,
									missingNameFrEn: missingName.length,
									missingImage: missingImg.length,
									missingStats: missingStats.length,
								},
								skills: {
									total: skills.length,
									missingNameFrEn: missingSkillName.length,
								},
							},
							null,
							2,
						),
					);
					return;
				}

				console.log(`${colors.cyan}Démarrage de l'audit de base de données...${colors.reset}`);
				console.log(`\n${colors.bold}${colors.blue}Diagnostics Characters :${colors.reset}`);
				console.log(`- Total Characters : ${chars.length}`);
				console.log(
					`- Sans nom FR/EN   : ${missingName.length === 0 ? colors.green + "0 (Excellent)" : colors.red + missingName.length} ${colors.reset}`,
				);
				console.log(
					`- Sans image       : ${missingImg.length === 0 ? colors.green + "0 (Excellent)" : colors.yellow + missingImg.length} ${colors.reset}`,
				);
				console.log(
					`- Sans stats       : ${missingStats.length === 0 ? colors.green + "0 (Excellent)" : colors.red + missingStats.length} ${colors.reset}`,
				);

				console.log(`\n${colors.bold}${colors.blue}Diagnostics Skills/Techniques :${colors.reset}`);
				console.log(`- Total Skills     : ${skills.length}`);
				console.log(
					`- Sans nom FR/EN   : ${missingSkillName.length === 0 ? colors.green + "0 (Excellent)" : colors.red + missingSkillName.length} ${colors.reset}`,
				);

				console.log(`\n${colors.bold}${colors.green}Audit complété avec succès !${colors.reset}`);
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur lors de l'audit : ${errorMessage(e)}${colors.reset}`,
				);
			}
		});
}

export function registerTestVariantsCommand(program: Command): void {
	program
		.command("test-variants")
		.description("Exécute des tests de cohérence de haut niveau sur les personnages et leurs variantes")
		.action(async () => {
			console.log(`${colors.cyan}Démarrage des tests de cohérence de haut niveau...${colors.reset}`);
			const svc = await createInagleService();
			const baseChars = svc.characters.baseCharacters();

			let passed = 0;
			let failed = 0;
			const errors: string[] = [];

			for (const bc of baseChars) {
				if (!bc.variants || bc.variants.length === 0) {
					errors.push(`Personnage ${bc.names.fr || bc.names.en} (ID: ${bc.charaId}) sans variantes !`);
					failed++;
					continue;
				}

				const youngest = bc.variants[0];

				// 1. La variante d'origine doit rester la première du tri.
				for (let i = 1; i < bc.variants.length; i++) {
					const current = bc.variants[i];
					const orderYoungest =
						youngest.zukanOrder !== undefined && youngest.zukanOrder !== null ? youngest.zukanOrder : 999999;
					const orderCurrent =
						current.zukanOrder !== undefined && current.zukanOrder !== null ? current.zukanOrder : 999999;
					if (orderYoungest > orderCurrent) {
						errors.push(
							`Personnage ${bc.names.fr || bc.names.en} : variante ${current.charaParamId} a un zukanOrder (${orderCurrent}) plus petit que la variante d'origine ${youngest.charaParamId} (${orderYoungest}) !`,
						);
						failed++;
						break;
					} else if (orderYoungest === orderCurrent) {
						const idYoungest = parseInt(youngest.charaParamId, 16) || 0;
						const idCurrent = parseInt(current.charaParamId, 16) || 0;
						if (idYoungest > idCurrent) {
							errors.push(
								`Personnage ${bc.names.fr || bc.names.en} : variante ${current.charaParamId} a un ID numérique plus petit que la variante d'origine ${youngest.charaParamId} !`,
							);
							failed++;
							break;
						}
					}
				}

				// 2. Les champs représentatifs doivent pointer sur cette variante.
				if (bc.image !== youngest.image) {
					errors.push(
						`Personnage ${bc.names.fr} : image de couverture désignée (${bc.image}) ne correspond pas à la variante d'origine (${youngest.image}) !`,
					);
					failed++;
					continue;
				}

				passed++;
			}

			console.log(`\n${colors.bold}Rapport des tests :${colors.reset}`);
			console.log(`  - Réussis : ${colors.green}${passed}${colors.reset}`);
			console.log(`  - Échecs  : ${failed > 0 ? colors.red : colors.green}${failed}${colors.reset}`);

			if (errors.length > 0) {
				console.log(`\n${colors.red}Erreurs détectées :${colors.reset}`);
				for (const err of errors.slice(0, 10)) {
					console.log(`  - ${err}`);
				}
				if (errors.length > 10) {
					console.log(`  ... et ${errors.length - 10} autres erreurs.`);
				}
				process.exit(1);
			} else {
				console.log(`\n${colors.green}✅ Tous les tests de cohérence ont réussi !${colors.reset}`);
			}
		});
}
