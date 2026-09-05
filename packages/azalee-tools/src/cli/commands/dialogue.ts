/** `azalee dialogue` — recherche dans les dialogues narratifs et de chronique. */

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

import type { Command } from "commander";

import { colors, errorMessage, getOrReadInput, reportError, restoreLogs, suppressLogs } from "../context";
import type { DialogueOptions } from "../types";

/** Réplique retenue, telle qu'affichée / sérialisée. */
interface DialogueMatch {
	eventId: string;
	dialogueId: string;
	speaker: string | undefined;
	text: { fr?: string; en?: string; ja?: string } | undefined;
}

/** Forme du dump `all-gamedata/story_text_database.json`. */
interface StoryTextDatabase {
	events?: Array<{
		eventId: string;
		dialogues: Array<{
			dialogueId: string;
			speaker?: { charaId?: string; names?: { fr?: string; en?: string; ja?: string } };
			text?: { fr?: string; en?: string; ja?: string };
		}>;
	}>;
}

export function registerDialogueCommand(program: Command): void {
	program
		.command("dialogue [query]")
		.description("Recherche dans les dialogues narratifs et de chronique du jeu")
		.option("-s, --speaker <speaker>", "Filtre par locuteur/personnage")
		.option("-j, --json", "Format de sortie en JSON brute")
		.option("-l, --limit <limit>", "Nombre de résultats max", "10")
		.action(async (query: string | undefined, options: DialogueOptions) => {
			suppressLogs(!!options.json);
			try {
				const inputQuery = await getOrReadInput(query);
				const speakerQuery = options.speaker;
				const limitVal = parseInt(options.limit, 10) || 10;

				const dataRoot = process.env.DATA_ROOT || process.env.DATA_PATH || "/home/ubuntu/niers/data";
				const dbPath = path.join(dataRoot, "all-gamedata/story_text_database.json");
				if (!existsSync(dbPath)) {
					reportError(
						options.json,
						"Story text database not found",
						`${colors.red}Erreur: Base de dialogues introuvable. Exécutez 'azalee sync --push' d'abord.${colors.reset}`,
					);
					return;
				}

				const data = JSON.parse(readFileSync(dbPath, "utf-8")) as StoryTextDatabase;
				const events = data.events || [];
				const matches: DialogueMatch[] = [];

				const textQ = inputQuery.toLowerCase().trim();
				const speakerQ = speakerQuery?.toLowerCase().trim();

				outerLoop: for (const ev of events) {
					const eventId = ev.eventId;
					for (const d of ev.dialogues) {
						let matchesText = true;
						let matchesSpeaker = true;

						if (textQ) {
							const textFr = d.text?.fr?.toLowerCase() || "";
							const textEn = d.text?.en?.toLowerCase() || "";
							const textJa = d.text?.ja?.toLowerCase() || "";
							matchesText = textFr.includes(textQ) || textEn.includes(textQ) || textJa.includes(textQ);
						}

						if (speakerQ) {
							const spNameFr = d.speaker?.names?.fr?.toLowerCase() || "";
							const spNameEn = d.speaker?.names?.en?.toLowerCase() || "";
							const spNameJa = d.speaker?.names?.ja?.toLowerCase() || "";
							matchesSpeaker =
								spNameFr.includes(speakerQ) ||
								spNameEn.includes(speakerQ) ||
								spNameJa.includes(speakerQ) ||
								d.speaker?.charaId?.toLowerCase() === speakerQ;
						}

						if (matchesText && matchesSpeaker) {
							matches.push({
								eventId,
								dialogueId: d.dialogueId,
								speaker: d.speaker?.names?.fr || d.speaker?.names?.en || d.speaker?.names?.ja || d.speaker?.charaId,
								text: d.text,
							});
							if (matches.length >= limitVal) break outerLoop;
						}
					}
				}

				restoreLogs(!!options.json);
				if (options.json) {
					console.log(JSON.stringify(matches, null, 2));
				} else {
					console.log(
						`${colors.cyan}Recherche de dialogues pour : "${textQ}"${speakerQ ? ` (par ${speakerQ})` : ""}...${colors.reset}\n`,
					);
					if (matches.length === 0) {
						console.log(`${colors.yellow}Aucun dialogue trouvé.${colors.reset}`);
						return;
					}

					for (const m of matches) {
						console.log(
							`[${colors.bold}${colors.green}${m.speaker}${colors.reset}] (${colors.yellow}${m.eventId}${colors.reset} | ${colors.yellow}${m.dialogueId}${colors.reset})`,
						);
						if (m.text?.fr) console.log(`  FR: ${m.text.fr}`);
						if (m.text?.en) console.log(`  EN: ${m.text.en}`);
						if (m.text?.ja) console.log(`  JA: ${m.text.ja}`);
						console.log("─".repeat(80));
					}
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur dialogue : ${errorMessage(e)}${colors.reset}`,
				);
			}
		});
}
