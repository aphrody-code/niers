/**
 * `azalee shell` (alias `repl`) — terminal interactif.
 *
 * Le shell réutilise **le programme commander existant** : chaque ligne saisie
 * est reparsée comme un `argv`, ce qui garantit qu'une commande se comporte
 * exactement pareil au shell et en ligne de commande. Deux différences
 * assumées : `exitOverride()` (une erreur d'analyse ne doit pas tuer le shell)
 * et le mode REPL (aucune commande n'appelle `process.exit`).
 */

import { appendFileSync, existsSync, readFileSync } from "node:fs";
import path from "node:path";
import readline from "node:readline";

import type { Command } from "commander";

import { colors } from "../context";
import { createInagleService, type InagleService } from "../inagle";
import { renderCharaProfile, renderItemProfile, renderSkillProfile, renderTeamProfile } from "../render";
import { getPendingSelection, setActiveReadline, setPendingSelection, setReplMode } from "../repl-state";

/** Commandes proposées à la complétion. */
const COMPLETION_COMMANDS = [
	"help",
	"chara",
	"compare",
	"search",
	"translate",
	"dialogue",
	"skill",
	"item",
	"team",
	"random-team",
	"team-builder",
	"db",
	"status",
	"audit",
	"wave",
	"sync",
	"repair",
	"redis",
	"glossary-rebuild",
	"exit",
	"quit",
];

/**
 * Découpe une ligne du shell en arguments, en respectant les guillemets
 * simples et doubles (`chara "Mark Evans"` = deux arguments).
 */
export function parseReplArgs(line: string): string[] {
	const args: string[] = [];
	let current = "";
	let inQuotes = false;
	let quoteChar = "";

	for (let i = 0; i < line.length; i++) {
		const char = line[i];
		if (inQuotes) {
			if (char === quoteChar) {
				inQuotes = false;
			} else {
				current += char;
			}
		} else {
			if (char === '"' || char === "'") {
				inQuotes = true;
				quoteChar = char;
			} else if (/\s/.test(char)) {
				if (current) {
					args.push(current);
					current = "";
				}
			} else {
				current += char;
			}
		}
	}
	if (current) {
		args.push(current);
	}
	return args;
}

export function registerShellCommand(program: Command): void {
	program
		.command("shell")
		.aliases(["repl"])
		.description("Démarre le terminal interactif Azalée (mode REPL)")
		.action(async () => {
			console.clear();
			console.log(`
${colors.bold}${colors.magenta}🌸=======================================================🌸
    Bienvenue dans le shell interactif AZALÉE CLI !
    Version 1.1.0 (Bun Runtime)
=======================================================🌸${colors.reset}
Entrez une commande pour interagir avec le projet (ou 'help').
Tapez '${colors.bold}exit${colors.reset}' ou '${colors.bold}quit${colors.reset}' pour quitter.
`);

			let svc: InagleService | null = null;
			let characterCache: string[] = [];
			let skillCache: string[] = [];
			let itemCache: string[] = [];
			let teamCache: string[] = [];

			// Caches de complétion : sans eux le shell reste utilisable, on ignore donc
			// silencieusement un échec de chargement.
			try {
				svc = await createInagleService();
				characterCache = svc.characters
					.baseCharacters()
					.map((c: any) => c.names?.fr || c.names?.en || c.charaId)
					.filter(Boolean);
				skillCache = svc.skills
					.all()
					.map((s: any) => s.displayName || s.name_FR || s.name_EN || s.skillIDStr)
					.filter(Boolean);
				itemCache = svc.items
					.allItems()
					.map((i: any) => i.names?.fr || i.names?.en || i.itemId)
					.filter(Boolean);
				teamCache = svc.teams
					.allTeams()
					.map((t: any) => t.name || t.displayName || t.teamId)
					.filter(Boolean);
			} catch {
				// complétion dégradée
			}

			function completer(line: string): [string[], string] {
				const trimmed = line.trim();
				if (!trimmed) {
					return [COMPLETION_COMMANDS, line];
				}
				const parts = trimmed.split(/\s+/);
				const cmd = parts[0].toLowerCase();
				if (parts.length === 1) {
					const hits = COMPLETION_COMMANDS.filter((c) => c.startsWith(cmd));
					return [hits.length ? hits : COMPLETION_COMMANDS, line];
				}
				const query = parts.slice(1).join(" ").toLowerCase();
				if (cmd === "chara" || cmd === "c") {
					const hits = characterCache.filter((name) => name.toLowerCase().startsWith(query));
					return [hits.map((name) => `${cmd} "${name}"`), line];
				}
				if (cmd === "skill" || cmd === "k") {
					const hits = skillCache.filter((name) => name.toLowerCase().startsWith(query));
					return [hits.map((name) => `${cmd} "${name}"`), line];
				}
				if (cmd === "item" || cmd === "i") {
					const hits = itemCache.filter((name) => name.toLowerCase().startsWith(query));
					return [hits.map((name) => `${cmd} "${name}"`), line];
				}
				if (cmd === "team" || cmd === "t") {
					const hits = teamCache.filter((name) => name.toLowerCase().startsWith(query));
					return [hits.map((name) => `${cmd} "${name}"`), line];
				}
				return [[], line];
			}

			const historyPath = path.resolve(process.cwd(), ".azalee_history");
			let history: string[] = [];
			if (existsSync(historyPath)) {
				try {
					history = readFileSync(historyPath, "utf-8").split("\n").filter(Boolean);
				} catch {}
			}

			const rl = readline.createInterface({
				input: process.stdin,
				output: process.stdout,
				prompt: `${colors.bold}${colors.green}azalee 🌸 > ${colors.reset}`,
				completer: completer,
			});

			if (history.length > 0) {
				(rl as unknown as { history: string[] }).history = history.reverse();
			}

			setActiveReadline(rl);
			rl.prompt();

			rl.on("line", async (line: string) => {
				const input = line.trim();
				if (!input) {
					rl.prompt();
					return;
				}

				try {
					appendFileSync(historyPath, input + "\n");
				} catch {}

				// Une sélection numérotée est en attente : le chiffre saisi la résout.
				const pending = getPendingSelection();
				if (pending) {
					const index = parseInt(input, 10) - 1;
					if (!isNaN(index) && index >= 0 && index < pending.matches.length) {
						const match = pending.matches[index];
						rl.pause();
						try {
							if (!svc) svc = await createInagleService();
							if (pending.type === "chara") {
								console.log(renderCharaProfile(match, svc));
							} else if (pending.type === "skill") {
								console.log(renderSkillProfile(match, svc));
							} else if (pending.type === "item") {
								console.log(renderItemProfile(match));
							} else if (pending.type === "team") {
								console.log(renderTeamProfile(match));
							}
						} catch (e) {
							console.error(`${colors.red}Erreur d'affichage : ${(e as Error).message}${colors.reset}`);
						}
						setPendingSelection(null);
						rl.setPrompt(`${colors.bold}${colors.green}azalee 🌸 > ${colors.reset}`);
						rl.resume();
						rl.prompt();
						return;
					} else if (/^\d+$/.test(input)) {
						console.log(`${colors.yellow}Sélection invalide (index hors limites). Sélection annulée.${colors.reset}`);
						setPendingSelection(null);
						rl.setPrompt(`${colors.bold}${colors.green}azalee 🌸 > ${colors.reset}`);
						rl.prompt();
						return;
					} else {
						console.log(`${colors.yellow}Sélection annulée.${colors.reset}`);
						setPendingSelection(null);
						rl.setPrompt(`${colors.bold}${colors.green}azalee 🌸 > ${colors.reset}`);
						// puis on exécute la saisie comme une commande normale
					}
				}

				const parts = parseReplArgs(input);
				const cmd = parts[0].toLowerCase();

				if (cmd === "exit" || cmd === "quit" || cmd === "q") {
					console.log(`\nAu revoir ! 🌸`);
					process.exit(0);
				}

				if (cmd === "help" || cmd === "h") {
					console.log(`
Commandes disponibles :
  ${colors.bold}help${colors.reset}, ${colors.bold}h${colors.reset}          Affiche cette aide
  ${colors.bold}chara${colors.reset} <query>     Détails d'un personnage (nom, stats, techniques)
  ${colors.bold}compare${colors.reset} <c1> <c2>  Compare deux personnages (stats, techniques, moveset)
  ${colors.bold}search${colors.reset} <query>    Recherche globale d'entités (joueurs, items, techniques)
  ${colors.bold}translate${colors.reset} <text>   Traduit du texte via glossaire consolidé
  ${colors.bold}dialogue${colors.reset} <query>   Recherche dans les répliques et dialogues narratifs
  ${colors.bold}skill${colors.reset} <query>      Détails d'une technique / compétence / passive
  ${colors.bold}item${colors.reset} <query>       Détails d'un objet (consommable, équipement, etc.)
  ${colors.bold}team${colors.reset} <query>       Détails d'une équipe (kits, saisons, etc.)
  ${colors.bold}random-team${colors.reset}       Génère une équipe aléatoire (options dispo)
  ${colors.bold}team-builder${colors.reset}      Gère et génère des équipes tactiques (options dispo)
  ${colors.bold}db${colors.reset} <sql>          Exécute une requête SQL PostgreSQL
  ${colors.bold}status${colors.reset}            Diagnostic de santé système et services
  ${colors.bold}audit${colors.reset}             Audit de cohérence de la base
  ${colors.bold}wave${colors.reset} [--cycle]    Lance une vague de traitement de données
  ${colors.bold}sync${colors.reset} [--push]     Synchronise les données locales vers PostgreSQL
  ${colors.bold}repair${colors.reset}            Répare les permissions et privilèges PostgreSQL
  ${colors.bold}exit${colors.reset}, ${colors.bold}quit${colors.reset}        Quitte le shell
`);
					rl.prompt();
					return;
				}

				// Exécution asynchrone : readline reste en pause le temps de la commande.
				rl.pause();

				try {
					program.exitOverride();
					setReplMode(true);

					// commander attend un argv complet (`node script …`).
					const argv = ["bun", "azalee", ...parts];
					await program.parseAsync(argv);
				} catch (err) {
					const code = (err as { code?: string }).code;
					if (code === "commander.helpDisplayed" || code === "commander.help") {
						// déjà affiché par commander
					} else if (code === "commander.unknownCommand") {
						console.log(`${colors.red}Commande inconnue: '${cmd}'. Tapez 'help' pour voir la liste.${colors.reset}`);
					} else {
						console.error(`${colors.red}Erreur d'exécution: ${(err as Error).message}${colors.reset}`);
					}
				} finally {
					setReplMode(false);
					rl.resume();
					rl.prompt();
				}
			});
		});
}
