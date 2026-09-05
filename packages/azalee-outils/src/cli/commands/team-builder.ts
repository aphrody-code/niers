/**
 * `azalee team-builder` — gestion et génération de compositions d'équipe.
 *
 * Actions : `list`, `show <id>`, `delete <id>`, `save <name> <code>`,
 * `generate`. Les compositions vivent dans la table `user_teams` (PostgreSQL),
 * partagées avec l'éditeur web ; le code de partage utilise le codec commun
 * `@rosegriffon/azalee/game` (`encodeTeamCode` / `decodeTeamCode`).
 */

import type { Command } from "commander";

import { LEGACY_FORMATIONS, type PositionCoord } from "@rosegriffon/azalee/game/formations";
import { decodeTeamCode, encodeTeamCode } from "@rosegriffon/azalee/game/team-code";
import {
	colors,
	createPgClient,
	errorMessage,
	renderAsciiTable,
	reportError,
	restoreLogs,
	suppressLogs,
} from "../context";
import { createInagleService } from "../inagle";
import { exitUnlessRepl } from "../repl-state";
import type { TeamBuilderOptions } from "../types";

/**
 * Coordonnées par identifiant de formation persisté. Dérivé des formations
 * héritées de la lib : une seule table de vérité, plus de copie locale.
 */
const FORMATION_POSITIONS: Record<string, PositionCoord[]> = Object.fromEntries(
	LEGACY_FORMATIONS.map((f) => [f.id, f.positions]),
);

/** Formations proposées au tirage de l'action `generate`. */
const GENERATABLE_FORMATIONS = LEGACY_FORMATIONS.map((f) => f.id);

/** Éléments jouables, thème possible d'une composition générée. */
const ELEMENTS = ["Fire", "Forest", "Mountain", "Wind"] as const;

/** Distance (en % de terrain) sous laquelle deux joueurs sont « liés ». */
const LINK_DISTANCE = 25;

/** Utilisateur par défaut des compositions créées depuis le CLI. */
const CLI_USER_ID = "00000000-0000-0000-0000-000000000000";

/** Base CDN historique des vignettes de zukan. */
const ZUKAN_CDN = "https://dxi4wb638ujep.cloudfront.net/1";

export function registerTeamBuilderCommand(program: Command): void {
	program
		.command("team-builder <action> [args...]")
		.description(
			"Gère et génère des compositions d'équipes (actions: list, show <id>, delete <id>, save <name> <encoded-team>, generate)",
		)
		.option("-j, --json", "Format de sortie en JSON brute")
		.option("-f, --formation <formation>", "Spécifie la formation pour l'action 'generate' (ex: diamond442)")
		.action(async (action: string, args: string[], options: TeamBuilderOptions) => {
			suppressLogs(!!options.json);
			try {
				const dbUrl = process.env.DATABASE_URL;
				if (!dbUrl) {
					throw new Error("DATABASE_URL non définie dans l'environnement");
				}
				const client = createPgClient(dbUrl);

				const svc = await createInagleService();
				const characters = svc.characters.baseCharacters();

				if (action === "list") {
					await client.connect();
					const res = await client.query(
						"SELECT id, name, formation_id, updated_at FROM user_teams ORDER BY updated_at DESC",
					);
					await client.end();

					restoreLogs(!!options.json);
					if (options.json) {
						console.log(JSON.stringify(res.rows, null, 2));
					} else {
						console.log(`\n${colors.bold}${colors.green}=== Compositions d'Équipes Sauvegardées ===${colors.reset}`);
						if (res.rows.length === 0) {
							console.log("Aucune équipe enregistrée.");
						} else {
							console.log(renderAsciiTable(res.rows));
						}
					}
				} else if (action === "show") {
					const teamId = args[0];
					if (!teamId) {
						throw new Error("ID de l'équipe manquant. Usage: azalee team-builder show <id>");
					}
					await client.connect();
					const res = await client.query<any>("SELECT * FROM user_teams WHERE id = $1", [teamId]);
					await client.end();

					if (res.rows.length === 0) {
						throw new Error(`Aucune équipe trouvée avec l'ID: ${teamId}`);
					}
					const team = res.rows[0];
					const formationData =
						typeof team.formation_data === "string" ? JSON.parse(team.formation_data) : team.formation_data;

					// Les membres persistés ne stockent qu'un charaId : on réhydrate
					// depuis inagle pour afficher des données à jour.
					const resolvedMembers = (formationData.members || []).map((m: any) => {
						const char = characters.find((c: any) => c.charaId === m.charaId);
						return {
							slot: m.slot,
							charaId: m.charaId,
							name: char ? char.names?.fr || char.names?.en || m.name : m.name,
							position: char ? char.variants?.[0]?.position || m.position : m.position,
							element: char ? char.variants?.[0]?.element || m.element : m.element,
							rarity: char ? char.bestRarity || m.rarity : m.rarity,
							stats: char?.variants?.[0]?.stats?.lv99 || m.stats,
						};
					});

					// Synergies : dominance (4+ d'un élément), harmonie (4 éléments).
					const fieldMembers = resolvedMembers.filter((m: any) => m.slot.startsWith("field-"));
					const elements: Record<string, number> = { Fire: 0, Forest: 0, Mountain: 0, Wind: 0 };
					for (const m of fieldMembers) {
						if (elements[m.element] !== undefined) elements[m.element]++;
					}

					const dominance = Object.entries(elements)
						.filter(([, count]) => count >= 4)
						.map(([el]) => el);
					const harmony = Object.values(elements).every((c) => c > 0);

					// Liens de proximité : même élément et distance euclidienne courte.
					const links: Array<[string, string]> = [];
					const positions = FORMATION_POSITIONS[team.formation_id] || [];
					for (let i = 0; i < fieldMembers.length; i++) {
						const m1 = fieldMembers[i];
						const slotIdx1 = parseInt(m1.slot.replace("field-", ""), 10);
						const pos1 = positions.find((p) => p.index === slotIdx1);
						if (!pos1 || pos1.role === "GK") continue;

						for (let j = i + 1; j < fieldMembers.length; j++) {
							const m2 = fieldMembers[j];
							if (m1.element !== m2.element) continue;

							const slotIdx2 = parseInt(m2.slot.replace("field-", ""), 10);
							const pos2 = positions.find((p) => p.index === slotIdx2);
							if (!pos2 || pos2.role === "GK") continue;

							const dist = Math.sqrt(Math.pow(pos1.top - pos2.top, 2) + Math.pow(pos1.left - pos2.left, 2));
							if (dist < LINK_DISTANCE) {
								links.push([m1.name, m2.name]);
							}
						}
					}

					restoreLogs(!!options.json);
					if (options.json) {
						console.log(
							JSON.stringify(
								{
									id: team.id,
									name: team.name,
									formation_id: team.formation_id,
									updated_at: team.updated_at,
									members: resolvedMembers,
									synergy: {
										elements,
										dominance,
										harmony,
										links: links.map(([n1, n2]) => `${n1} <-> ${n2}`),
									},
								},
								null,
								2,
							),
						);
					} else {
						console.log(`\n${colors.bold}${colors.green}=== Fiche d'Équipe : ${team.name} ===${colors.reset}`);
						console.log(`ID : ${colors.cyan}${team.id}${colors.reset}`);
						console.log(`Formation : ${colors.yellow}${team.formation_id}${colors.reset}`);
						console.log(`Mise à jour : ${team.updated_at}`);
						console.log(`\n${colors.bold}${colors.blue}Membres de l'équipe :${colors.reset}`);
						const tableRows = resolvedMembers.map((m: any) => ({
							Emplacement: m.slot,
							Joueur: m.name,
							Poste: m.position,
							Élément: m.element,
							Rareté: m.rarity,
						}));
						console.log(renderAsciiTable(tableRows));

						console.log(`\n${colors.bold}${colors.cyan}Synergies Élémentaires :${colors.reset}`);
						for (const [el, count] of Object.entries(elements)) {
							console.log(`  - ${el} : ${count} joueur(s)`);
						}
						if (dominance.length > 0) {
							console.log(
								`  - ${colors.green}Dominance détectée${colors.reset} : ${dominance.join(", ")} (+5% puissance des techniques associés)`,
							);
						}
						if (harmony) {
							console.log(`  - ${colors.green}Harmonie élémentaire active${colors.reset} (+3% stats globales)`);
						}
						if (links.length > 0) {
							console.log(`  - ${colors.green}Liens de proximité élémentaires (${links.length}) :${colors.reset}`);
							for (const [n1, n2] of links) {
								console.log(`    * ${n1} <-> ${n2}`);
							}
						}
					}
				} else if (action === "delete") {
					const teamId = args[0];
					if (!teamId) {
						throw new Error("ID de l'équipe manquant. Usage: azalee team-builder delete <id>");
					}
					await client.connect();
					await client.query("DELETE FROM user_teams WHERE id = $1", [teamId]);
					await client.end();

					restoreLogs(!!options.json);
					if (options.json) {
						console.log(JSON.stringify({ success: true, deletedId: teamId }));
					} else {
						console.log(`\n${colors.green}Équipe ${teamId} supprimée avec succès.${colors.reset}`);
					}
				} else if (action === "save") {
					const name = args[0];
					const encodedTeam = args[1];
					if (!name || !encodedTeam) {
						throw new Error("Usage: azalee team-builder save <name> <encoded-team>");
					}

					const decoded = decodeTeamCode(encodedTeam);
					const members: any[] = [];
					for (const { slot, charaId } of decoded.slots) {
						const char = characters.find((c: any) => c.charaId === charaId);
						if (char) {
							const variant = char.variants?.[0];
							members.push({
								slot,
								charaId: char.charaId,
								name: char.names?.fr || char.names?.en || char.names?.ja,
								position: variant?.position || "MF",
								element: variant?.element || "Void",
								rarity: char.bestRarity || variant?.rarity || "Normal",
								imageUrl: `${ZUKAN_CDN}/${char.zukanHash || variant?.zukanHash}.png`,
								slug: char.slug || charaId,
								stats: variant?.stats?.lv99,
							});
						}
					}

					const teamData = {
						formationId: decoded.formationId,
						members,
					};

					const id = crypto.randomUUID();

					await client.connect();
					await client.query(
						`INSERT INTO user_teams(id, user_id, name, formation_id, formation_data, is_public, created_at, updated_at)
						 VALUES ($1, $2, $3, $4, $5, TRUE, NOW(), NOW())`,
						[id, CLI_USER_ID, name, decoded.formationId, JSON.stringify(teamData)],
					);
					await client.end();

					restoreLogs(!!options.json);
					if (options.json) {
						console.log(JSON.stringify({ success: true, id, name }));
					} else {
						console.log(`\n${colors.green}Équipe "${name}" sauvegardée avec succès sous l'ID: ${id}.${colors.reset}`);
					}
				} else if (action === "generate") {
					// Génération cohérente : une formation, un thème d'élément.
					const chosenFormationId =
						options.formation || GENERATABLE_FORMATIONS[Math.floor(Math.random() * GENERATABLE_FORMATIONS.length)];

					const chosenElement = ELEMENTS[Math.floor(Math.random() * ELEMENTS.length)];

					const charsByPosition: Record<string, any[]> = { FW: [], MF: [], DF: [], GK: [] };
					for (const char of characters) {
						const variant = char.variants?.[0];
						if (variant && variant.position && charsByPosition[variant.position]) {
							charsByPosition[variant.position].push({
								charaId: char.charaId,
								name: char.names?.fr || char.names?.en,
								position: variant.position,
								element: variant.element,
								rarity: char.bestRarity || variant.rarity,
								slug: char.slug,
								zukanHash: char.zukanHash || variant.zukanHash,
								stats: variant.stats?.lv99,
							});
						}
					}

					const positions = FORMATION_POSITIONS[chosenFormationId] || FORMATION_POSITIONS.diamond442;
					const selectedCharaIds = new Set<string>();
					const members: any[] = [];

					/** Tire un joueur du poste demandé, en privilégiant le thème. */
					const selectChara = (role: string) => {
						const candidates = charsByPosition[role] || [];
						const elementCandidates = candidates.filter(
							(c) => c.element === chosenElement && !selectedCharaIds.has(c.charaId),
						);
						const fallbackCandidates = candidates.filter((c) => !selectedCharaIds.has(c.charaId));

						const pool = elementCandidates.length > 0 ? elementCandidates : fallbackCandidates;
						if (pool.length === 0) return null;
						const picked = pool[Math.floor(Math.random() * pool.length)];
						selectedCharaIds.add(picked.charaId);
						return picked;
					};

					// Onze titulaires.
					for (const pos of positions) {
						const picked = selectChara(pos.role);
						if (picked) {
							members.push({
								slot: `field-${pos.index}`,
								charaId: picked.charaId,
								name: picked.name,
								position: picked.position,
								element: picked.element,
								rarity: picked.rarity,
								imageUrl: `${ZUKAN_CDN}/${picked.zukanHash}.png`,
								slug: picked.slug,
								stats: picked.stats,
							});
						}
					}

					// Cinq remplaçants, postes tirés au hasard.
					for (let i = 0; i < 5; i++) {
						const allPos = ["FW", "MF", "DF", "GK"];
						const randomRole = allPos[Math.floor(Math.random() * allPos.length)];
						const picked = selectChara(randomRole);
						if (picked) {
							members.push({
								slot: `reserve-${i}`,
								charaId: picked.charaId,
								name: picked.name,
								position: picked.position,
								element: picked.element,
								rarity: picked.rarity,
								imageUrl: `${ZUKAN_CDN}/${picked.zukanHash}.png`,
								slug: picked.slug,
								stats: picked.stats,
							});
						}
					}

					// Staff technique.
					await client.connect();
					const resCoordinators = await client.query<any>("SELECT * FROM inagle_coordinators");
					await client.end();

					const coaches = resCoordinators.rows.filter((r: any) => r.role === "Manager" || r.role === "Coach");
					const managers = resCoordinators.rows.filter((r: any) => r.role === "Coordinator");

					const matchedCoach = coaches[Math.floor(Math.random() * coaches.length)];
					if (matchedCoach) {
						members.push({
							slot: "manager-0",
							charaId: `coord_${matchedCoach.id}`,
							name: matchedCoach.name_localised,
							position: "COACH",
							element: "Void",
							rarity: "Coach",
							imageUrl: `${ZUKAN_CDN}/k/c/b/cb88nfolkwe.png`,
							slug: `coord-${matchedCoach.id}`,
						});
					}

					const shuffledManagers = managers.sort(() => 0.5 - Math.random());
					for (let i = 0; i < Math.min(3, shuffledManagers.length); i++) {
						const m = shuffledManagers[i];
						members.push({
							slot: `support-${i}`,
							charaId: `coord_${m.id}`,
							name: m.name_localised,
							position: "COORD",
							element: "Void",
							rarity: "Coordinator",
							imageUrl: `${ZUKAN_CDN}/k/z/p/zpndk6oxjls.png`,
							slug: `coord-${m.id}`,
						});
					}

					// Le code de partage ne contient que les joueurs (pas le staff).
					const encodedTeam = encodeTeamCode(
						chosenFormationId,
						members
							.filter((m) => !m.slot.startsWith("manager-") && !m.slot.startsWith("support-"))
							.map((m) => ({ slot: m.slot, charaId: m.charaId })),
					);

					restoreLogs(!!options.json);
					if (options.json) {
						console.log(
							JSON.stringify(
								{
									formationId: chosenFormationId,
									elementTheme: chosenElement,
									encodedTeam,
									members,
								},
								null,
								2,
							),
						);
					} else {
						console.log(`\n${colors.bold}${colors.green}=== Compo Générée (Thème : ${chosenElement}) ===${colors.reset}`);
						console.log(`Formation : ${chosenFormationId}`);
						console.log(`Chaîne d'encodage : ${colors.yellow}${encodedTeam}${colors.reset}`);
						console.log(`\n${colors.bold}${colors.blue}Joueurs terrain et remplaçants :${colors.reset}`);
						const tableRows = members.map((m: any) => ({
							Emplacement: m.slot,
							Joueur: m.name,
							Poste: m.position,
							Élément: m.element,
							Rareté: m.rarity,
						}));
						console.log(renderAsciiTable(tableRows));
					}
				} else {
					throw new Error(`Action inconnue: ${action}. Utilisez list, show, delete, save ou generate.`);
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur team-builder : ${errorMessage(e)}${colors.reset}`,
				);
			} finally {
				exitUnlessRepl(0);
			}
		});
}
