/** `azalee compare` — comparaison côte à côte de deux joueurs. */

import type { Command } from "commander";

import { buildAuraHashSet } from "@rosegriffon/inagle/skills/mapper-aura";

import { interpolateVariantStats } from "@rosegriffon/azalee/game/stats-interpolation";
import type { CharaStats } from "@rosegriffon/azalee/wiki/chara-stats-shared";
import { colors, errorMessage, reportError, restoreLogs, suppressLogs } from "../context";
import { createInagleService, getSkillDetails, type InagleService, type SkillDetails } from "../inagle";
import { exitUnlessRepl } from "../repl-state";
import type { CompareOptions } from "../types";

/**
 * Techniques fantômes présentes dans les dumps mais jamais jouables :
 * les afficher donnerait un moveset faux.
 */
const PHANTOM_SKILL_IDS = new Set(["0xDBEDB6B8"]);

/** Technique résolue d'un moveset, avec son niveau d'apprentissage. */
interface MovesetEntry extends SkillDetails {
	learnLevel: number;
}

/** Les 7 stats dans l'ordre d'affichage du jeu. */
const STAT_ROWS: ReadonlyArray<{ key: keyof CharaStats; label: string }> = [
	{ key: "kick", label: "Frappe" },
	{ key: "control", label: "Contrôle" },
	{ key: "technique", label: "Technique" },
	{ key: "pressure", label: "Pression" },
	{ key: "physical", label: "Physique" },
	{ key: "agility", label: "Agilité" },
	{ key: "intelligence", label: "Intelligence" },
];

export function registerCompareCommand(program: Command): void {
	program
		.command("compare <chara1> <chara2>")
		.description("Compare deux joueurs côte à côte (stats interpolées et moveset)")
		.option("-l, --level <level>", "Niveau à comparer (1-99)", "99")
		.option("-j, --json", "Format de sortie en JSON brut")
		.action(async (chara1: string, chara2: string, options: CompareOptions) => {
			suppressLogs(!!options.json);
			try {
				const level = parseInt(options.level, 10);
				if (isNaN(level) || level < 1 || level > 99) {
					reportError(
						options.json,
						"Le niveau doit être compris entre 1 et 99.",
						`${colors.red}Erreur: Le niveau doit être compris entre 1 et 99.${colors.reset}`,
					);
					return;
				}

				const svc = await createInagleService();
				const chars = svc.characters.baseCharacters();

				// Identifiant/code/slug exact d'abord, sous-chaîne de nom ensuite.
				const findMatch = (query: string) => {
					const q = query.toLowerCase().trim();
					const exactMatches = chars.filter(
						(c: any) =>
							c.charaId.toLowerCase() === q ||
							c.internalCode?.toLowerCase() === q ||
							c.slug?.toLowerCase() === q ||
							c.names?.fr?.toLowerCase() === q ||
							c.names?.en?.toLowerCase() === q,
					);
					if (exactMatches.length > 0) return exactMatches;

					return chars.filter((c: any) => {
						const nameFr = c.names?.fr?.toLowerCase() || "";
						const nameEn = c.names?.en?.toLowerCase() || "";
						const nameJa = c.names?.ja?.toLowerCase() || "";
						const slug = c.slug?.toLowerCase() || "";
						return nameFr.includes(q) || nameEn.includes(q) || nameJa.includes(q) || slug.includes(q);
					});
				};

				const matches1 = findMatch(chara1);
				const matches2 = findMatch(chara2);

				if (matches1.length === 0 || matches2.length === 0) {
					const errorMsg =
						matches1.length === 0 && matches2.length === 0
							? `Aucun joueur trouvé pour "${chara1}" et "${chara2}".`
							: matches1.length === 0
								? `Aucun joueur trouvé pour "${chara1}".`
								: `Aucun joueur trouvé pour "${chara2}".`;
					reportError(options.json, errorMsg, `${colors.red}Erreur: ${errorMsg}${colors.reset}`);
					return;
				}

				// Plusieurs correspondances : on prend la première (la plus pertinente).
				const c1 = matches1[0];
				const c2 = matches2[0];

				const v1 = c1.variants?.[0];
				const v2 = c2.variants?.[0];

				if (!v1 || !v2) {
					const errorMsg = "Données de variante introuvables pour l'un des joueurs.";
					reportError(options.json, errorMsg, `${colors.red}Erreur: ${errorMsg}${colors.reset}`);
					return;
				}

				const stats1 = interpolateVariantStats(v1, level);
				const stats2 = interpolateVariantStats(v2, level);

				const skills1 = resolveMoves(svc, v1, c1);
				const skills2 = resolveMoves(svc, v2, c2);

				if (options.json) {
					restoreLogs(true);
					console.log(
						JSON.stringify(
							{
								level,
								chara1: {
									id: c1.charaId,
									name: c1.names.fr || c1.names.en || c1.names.ja,
									position: v1.position,
									element: v1.element,
									rarity: v1.rarity,
									stats: stats1,
									skills: skills1,
								},
								chara2: {
									id: c2.charaId,
									name: c2.names.fr || c2.names.en || c2.names.ja,
									position: v2.position,
									element: v2.element,
									rarity: v2.rarity,
									stats: stats2,
									skills: skills2,
								},
							},
							null,
							2,
						),
					);
					return;
				}

				// Rendu ASCII 3 colonnes : joueur 1 | libellé | joueur 2.
				restoreLogs(true);
				const colWidthL = 35;
				const colWidthM = 16;
				const colWidthR = 35;

				const padCenter = (text: string, width: number): string => {
					const pad = width - text.length;
					if (pad <= 0) return text.substring(0, width);
					const left = Math.floor(pad / 2);
					const right = pad - left;
					return " ".repeat(left) + text + " ".repeat(right);
				};

				const formatRow = (
					leftText: string,
					midText: string,
					rightText: string,
					leftColor = "",
					midColor = "",
					rightColor = "",
				) => {
					const rawLeft = leftText.substring(0, colWidthL);
					const rawMid = midText.substring(0, colWidthM);
					const rawRight = rightText.substring(0, colWidthR);

					const padL = rawLeft.padEnd(colWidthL);
					const padM = padCenter(rawMid, colWidthM);
					const padR = rawRight.padStart(colWidthR);

					const coloredL = leftColor ? `${leftColor}${padL}${colors.reset}` : padL;
					const coloredM = midColor ? `${midColor}${padM}${colors.reset}` : padM;
					const coloredR = rightColor ? `${rightColor}${padR}${colors.reset}` : padR;

					return `│ ${coloredL} │ ${coloredM} │ ${coloredR} │`;
				};

				const borderTop = `┌─${"─".repeat(colWidthL)}─┬─${"─".repeat(colWidthM)}─┬─${"─".repeat(colWidthR)}─┐`;
				const borderDivider = `├─${"─".repeat(colWidthL)}─┼─${"─".repeat(colWidthM)}─┼─${"─".repeat(colWidthR)}─┤`;
				const borderBottom = `└─${"─".repeat(colWidthL)}─┴─${"─".repeat(colWidthM)}─┴─${"─".repeat(colWidthR)}─┘`;

				console.log(`\n${colors.cyan}=== COMPARAISON DE JOUEURS (Niveau ${level}) ===${colors.reset}`);
				console.log(borderTop);

				const name1 = c1.names.fr || c1.names.en || c1.names.ja || c1.charaId;
				const name2 = c2.names.fr || c2.names.en || c2.names.ja || c2.charaId;

				console.log(
					formatRow(
						name1,
						"VS",
						name2,
						colors.bold + colors.green,
						colors.bold + colors.magenta,
						colors.bold + colors.green,
					),
				);
				console.log(borderDivider);

				console.log(formatRow(v1.position, "Position", v2.position));
				console.log(formatRow(v1.element, "Élément", v2.element));
				console.log(formatRow(v1.rarity, "Rareté", v2.rarity));
				console.log(borderDivider);

				let total1 = 0;
				let total2 = 0;

				for (const { key, label } of STAT_ROWS) {
					const val1 = stats1[key];
					const val2 = stats2[key];
					total1 += val1;
					total2 += val2;

					let cL = "";
					let cR = "";
					let showVal1 = String(val1);
					let showVal2 = String(val2);

					if (val1 > val2) {
						cL = colors.green + colors.bold;
						cR = colors.red;
						showVal1 += " (▲)";
					} else if (val2 > val1) {
						cL = colors.red;
						cR = colors.green + colors.bold;
						showVal2 = "(▲) " + showVal2;
					}

					console.log(formatRow(showVal1, label, showVal2, cL, colors.cyan, cR));
				}

				console.log(borderDivider);
				let totalCL = "";
				let totalCR = "";
				let showTotal1 = String(total1);
				let showTotal2 = String(total2);
				if (total1 > total2) {
					totalCL = colors.green + colors.bold;
					totalCR = colors.red;
					showTotal1 += " (▲)";
				} else if (total2 > total1) {
					totalCL = colors.red;
					totalCR = colors.green + colors.bold;
					showTotal2 = "(▲) " + showTotal2;
				}
				console.log(formatRow(showTotal1, "TOTAL", showTotal2, totalCL, colors.bold + colors.yellow, totalCR));
				console.log(borderDivider);

				const maxMoves = Math.max(skills1.length, skills2.length);
				console.log(formatRow("", "TECHNIQUES", "", "", colors.bold + colors.magenta));

				for (let i = 0; i < maxMoves; i++) {
					const sk1 = skills1[i];
					const sk2 = skills2[i];

					const text1 = sk1 ? `${sk1.name} (L. ${sk1.learnLevel}${sk1.power ? `, ${sk1.power}P` : ""})` : "";
					const text2 = sk2 ? `${sk2.name} (L. ${sk2.learnLevel}${sk2.power ? `, ${sk2.power}P` : ""})` : "";

					console.log(formatRow(text1, `Slot ${i + 1}`, text2));
				}
				console.log(borderBottom);
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur lors de la comparaison: ${errorMessage(e)}${colors.reset}`,
				);
			} finally {
				exitUnlessRepl(0);
			}
		});
}

/**
 * Moveset réel d'une variante : on retire les techniques fantômes et les auras
 * (qui ne sont pas des techniques), puis on hérite du moveset d'une variante
 * sœur si la variante courante n'en déclare aucun.
 */
function resolveMoves(svc: InagleService, variant: any, baseChara: any): MovesetEntry[] {
	const auraHashes = buildAuraHashSet();
	const rawSkills = variant.skills || [];
	let skillList = rawSkills.filter(
		(sk: any) => sk.skillId && !PHANTOM_SKILL_IDS.has(sk.skillId) && !auraHashes.has(sk.skillId),
	);

	if (skillList.length === 0 && baseChara.variants) {
		for (const sibling of baseChara.variants) {
			if (sibling.skills && sibling.skills.length > 0) {
				const sibSkills = sibling.skills.filter(
					(sk: any) => sk.skillId && !PHANTOM_SKILL_IDS.has(sk.skillId) && !auraHashes.has(sk.skillId),
				);
				if (sibSkills.length > 0) {
					skillList = sibSkills;
					break;
				}
			}
		}
	}

	return skillList.map((sk: any) => {
		const details = getSkillDetails(svc, sk.skillId);
		return {
			learnLevel: sk.learnLevel,
			...details,
		};
	});
}
