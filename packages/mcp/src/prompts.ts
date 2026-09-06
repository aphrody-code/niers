/**
 * Prompts MCP : des amorces de travail réutilisables, invocables par
 * l'utilisateur (dans Claude Code : `/mcp` puis le nom du prompt).
 *
 * Un prompt n'est pas de la documentation : c'est une *procédure* qui dit
 * quels outils appeler, dans quel ordre, et ce qu'il ne faut pas inventer.
 */

import { wikiService } from "@niers/azalee-tools/server/index";
import type { PromptMessage } from "./protocol/types.ts";
import type { PromptSpec } from "./registry.ts";
import { KNOWN_SERVICES } from "./tools/ops.ts";

function user(text: string): PromptMessage {
	return { role: "user", content: { type: "text", text } };
}

export function buildPrompts(): PromptSpec[] {
	return [
		{
			name: "fiche-personnage",
			title: "Fiche complète d'un personnage",
			description:
				"Rassemble tout ce que le wiki sait d'un personnage d'Inazuma Eleven: Victory Road (statistiques, techniques, formes, auras, provenance) et en fait une fiche lisible.",
			arguments: [{ name: "personnage", description: "Nom ou slug du personnage.", required: true }],
			build: (args) => ({
				description: `Fiche du personnage ${args.personnage}`,
				messages: [
					user(
						[
							`Construis la fiche complète du personnage « ${args.personnage} » d'Inazuma Eleven: Victory Road.`,
							"",
							"Marche à suivre :",
							"1. `azalee_search` avec son nom pour récupérer le slug canonique ;",
							"2. `azalee_get` (collection `characters`) sur ce slug pour la fiche brute ;",
							"3. si la fiche référence des techniques ou des auras, `azalee_get` sur chacune plutôt que de deviner leur effet ;",
							"4. `db_query` seulement si une donnée manque aux outils métier.",
							"",
							"Restitution : statistiques chiffrées, techniques avec leur élément et leur puissance, formes alternatives, moyen de l'obtenir.",
							"Toute valeur affichée doit venir d'un appel d'outil — si une information est absente des données, écris-le au lieu de l'inventer.",
						].join("\n"),
					),
				],
			}),
			complete: async (argument, value) => {
				if (argument !== "personnage" || value.length < 2) return [];
				const list = await wikiService.getCharactersList({ q: value, limit: 20, page: 1 } as never);
				return (list.data as unknown as Record<string, unknown>[])
					.map((entry) => String(entry.baseSlug ?? entry.slug ?? entry.id ?? ""))
					.filter(Boolean);
			},
		},

		{
			name: "diagnostic-prod",
			title: "Diagnostiquer un incident de production",
			description:
				"Procédure de diagnostic de la production Rose Griffon : services systemd, points d'entrée HTTP, journaux, puis hypothèse argumentée.",
			arguments: [
				{ name: "symptome", description: "Ce qui est observé (page 500, service arrêté, lenteur…).", required: true },
			],
			build: (args) => ({
				description: "Diagnostic de production",
				messages: [
					user(
						[
							`Symptôme signalé : ${args.symptome}`,
							"",
							"Diagnostique dans cet ordre, sans sauter d'étape :",
							"1. `ops_status` — quel service est tombé, quel point d'entrée répond mal ;",
							"2. `ops_logs` sur le service suspect (commence par `priority: err`) ;",
							"3. `ops_http` sur l'URL concernée pour voir le statut et le début du corps ;",
							"4. `repo_git` (`status` puis `log`) pour savoir si un changement récent coïncide ;",
							"5. `repo_read` sur le fichier de configuration ou d'unité en cause.",
							"",
							`Unités du périmètre : ${KNOWN_SERVICES.join(", ")}.`,
							"",
							"Conclus par : cause la plus probable, preuve qui l'étaye (sortie d'outil citée), et la commande de remise en service à exécuter à la main sur le VPS.",
							"Ces outils sont en lecture seule : ne prétends jamais avoir redémarré quoi que ce soit.",
						].join("\n"),
					),
				],
			}),
		},

		{
			name: "explorer-donnees",
			title: "Répondre à une question sur les données du jeu",
			description:
				"Cadre une exploration des données extraites du jeu : outils métier d'abord, SQL ensuite, réponse chiffrée et vérifiable.",
			arguments: [{ name: "question", description: "La question posée sur les données du jeu.", required: true }],
			build: (args) => ({
				description: "Exploration des données de jeu",
				messages: [
					user(
						[
							`Question : ${args.question}`,
							"",
							"Méthode :",
							"1. essaie d'abord les outils métier (`azalee_search`, `azalee_list`, `azalee_get`, `azalee_dataset`) — ils appliquent les règles du jeu ;",
							"2. si la question demande un agrégat ou une jointure, passe à `db_tables` → `db_schema` → `db_query` ;",
							"3. pour du texte du jeu, `game_text_search` ; pour un fichier du jeu, `cpk_search` puis `cpk_file`.",
							"",
							"Piège connu : dans `inagle_skills`, les colonnes `category_id` et `element_id` sont NULL — filtrer sur les colonnes textuelles françaises `category` et `element`.",
							"",
							"Donne le compte exact renvoyé par la requête, pas une estimation, et montre la requête utilisée.",
						].join("\n"),
					),
				],
			}),
		},

		{
			name: "contexte-monorepo",
			title: "Charger le contexte du monorepo",
			description:
				"Amorce d'une session de travail sur le monorepo Rose Griffon : lit les fiches de contexte et l'état courant avant toute modification.",
			arguments: [
				{ name: "sujet", description: "Sur quoi porte la session (facultatif).", required: false },
			],
			build: (args) => ({
				description: "Contexte du monorepo Rose Griffon",
				messages: [
					user(
						[
							"Avant de travailler sur le monorepo Rose Griffon, charge le contexte :",
							"1. lis les ressources `rg://context/monorepo`, `rg://context/exploitation` et `rg://context/donnees` ;",
							"2. appelle `ops_status` pour connaître l'état réel de la production ;",
							"3. appelle `repo_git` (`status`) pour voir la branche et les fichiers modifiés.",
							args.sujet ? `\nSujet de la session : ${args.sujet}. Lis aussi la ressource \`rg://docs/…\` correspondante s'il en existe une.` : "",
							"",
							"Puis résume en dix lignes maximum : où en est le dépôt, ce qui tourne, et ce qu'il faut savoir avant de toucher au code.",
						]
							.filter(Boolean)
							.join("\n"),
					),
				],
			}),
		},
	];
}
