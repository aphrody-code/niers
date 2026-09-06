/**
 * Outils « données de jeu » — le wiki Azalée (Inazuma Eleven: Victory Road).
 *
 * Ils appellent **directement** la bibliothèque `@rosegriffon/azalee`, dans
 * le même processus : pas de saut HTTP, pas de duplication de logique. Les
 * mêmes fonctions servent le site, le CLI et l'API headless — donc une donnée
 * lue ici est, par construction, identique à celle affichée sur le wiki.
 *
 * Le découpage évite l'écueil du « un outil par route » (41 outils plats,
 * illisibles pour un modèle) : trois outils couvrent l'ensemble des
 * collections, avec une énumération explicite qui sert de documentation.
 */

import {
	categoryStats,
	fileMeta,
	getCapsuleList,
	getCoach,
	getCoachesList,
	getCostumeList,
	getCrossCatalogStats,
	getCrossTables,
	getDropsData,
	getInvocationRates,
	getQuest,
	getQuestsList,
	getShop,
	getShopsList,
	getStadium,
	getStadiumsList,
	getTeamDetail,
	getTeamsList,
	getTrophiesList,
	getTrophy,
	listDirPaged,
	queryRag,
	resolveDataDir,
	resolveMirrorPath,
	resolveTextAll,
	searchFiles,
	searchText,
	totalFiles,
	wikiService,
} from "@niers/azalee-tools/server/index";
import { z } from "zod";
import { structured, text, toolError } from "../protocol/types.ts";
import { defineTool, type RegisteredTool } from "../registry.ts";

/** Collections listables, alignées sur les routes de l'API headless. */
const COLLECTIONS = [
	"characters",
	"coordinators",
	"skills",
	"items",
	"tactics",
	"passives",
	"teams",
	"shops",
	"quests",
	"coaches",
	"stadiums",
	"trophies",
	"capsules",
	"costumes",
	"gallery",
	"auras",
] as const;

type Collection = (typeof COLLECTIONS)[number];

/** Types d'aura acceptés par `wikiService.getAurasList` / `getAura`. */
const AURA_TYPES = ["armure", "keshin", "miximax", "totem"] as const;

const listInput = z.object({
	collection: z.enum(COLLECTIONS).describe("Collection à lister."),
	q: z.string().optional().describe("Recherche plein texte (nom français, anglais ou japonais)."),
	page: z.int().min(1).default(1).describe("Page, à partir de 1."),
	limit: z.int().min(1).max(200).default(25).describe("Éléments par page (200 maximum)."),
	element: z.string().optional().describe("Élément : feu, bois, terre, vent (personnages et techniques)."),
	position: z.string().optional().describe("Poste : GB, DF, MF, FW (personnages)."),
	rarity: z.string().optional().describe("Rareté."),
	category: z.string().optional().describe("Catégorie (techniques, passives, trophées, galerie)."),
	team: z.string().optional().describe("Équipe (personnages)."),
	kind: z.string().optional().describe("Type de quête (collection `quests`)."),
	auraType: z.enum(AURA_TYPES).optional().describe("Type d'aura, requis pour la collection `auras`."),
	sort: z.string().optional().describe("Tri, ex. `power_desc`."),
});

const getInput = z.object({
	collection: z
		.enum([...COLLECTIONS, "text"] as const)
		.describe("Collection de l'entité. `text` résout un identifiant de texte de jeu."),
	id: z
		.string()
		.describe(
			"Identifiant : slug canonique (`mark-evans`), slug de variante, identifiant de ligne, ou hash de texte.",
		),
	auraType: z.enum(AURA_TYPES).optional().describe("Type d'aura, requis pour la collection `auras`."),
});

const searchInput = z.object({
	q: z.string().min(2).describe("Terme recherché (2 caractères minimum)."),
	limit: z.int().min(1).max(50).default(10).describe("Résultats par catégorie."),
});

const datasetInput = z.object({
	dataset: z
		.enum(["drops", "invocation", "cross_tables", "cross_stats", "text_stats", "health"] as const)
		.describe(
			"Jeu de données global : taux de drop, taux d'invocation, tables Inazuma Cross, statistiques du texte de jeu, état de la source de données.",
		),
});

/** Applique la même normalisation de filtres que l'API HTTP. */
function listFilters(input: z.output<typeof listInput>): Record<string, unknown> {
	const filters: Record<string, unknown> = { page: input.page, limit: input.limit };
	for (const key of ["q", "element", "position", "rarity", "category", "team", "sort"] as const) {
		const value = input[key];
		if (value !== undefined) filters[key] = value;
	}
	return filters;
}

async function listCollection(input: z.output<typeof listInput>): Promise<unknown> {
	const filters = listFilters(input);
	switch (input.collection) {
		case "characters":
			return await wikiService.getCharactersList(filters as never);
		case "coordinators":
			return await wikiService.getCoordinatorsList(filters as never);
		case "skills":
			return await wikiService.getSkillsList(filters as never);
		case "items":
			return await wikiService.getItemsList(filters as never);
		case "tactics":
			return await wikiService.getTacticsList(filters as never);
		case "passives":
			return wikiService.getPassivesList({
				q: input.q,
				category: input.category,
				page: input.page,
				limit: input.limit,
			} as never);
		case "gallery":
			return await wikiService.getGalleryList({
				page: input.page,
				limit: input.limit,
				category: input.category,
				q: input.q,
			} as never);
		case "auras": {
			if (!input.auraType) throw new Error("`auraType` est requis pour la collection `auras`.");
			return await wikiService.getAurasList({ ...filters, typeSlug: input.auraType } as never);
		}
		case "teams":
			return await getTeamsList();
		case "shops":
			return await getShopsList();
		case "quests":
			return await getQuestsList({ q: input.q, kind: input.kind } as never);
		case "coaches":
			return await getCoachesList({ q: input.q });
		case "stadiums":
			return await getStadiumsList({ q: input.q });
		case "trophies":
			return await getTrophiesList({ q: input.q, category: input.category } as never);
		case "capsules":
			return await getCapsuleList({ q: input.q } as never);
		case "costumes":
			return await getCostumeList({ q: input.q } as never);
	}
}

/** Même ordre de résolution que la page web `/chara/[id]`. */
async function resolveCharacter(idOrSlug: string): Promise<unknown> {
	return (
		(await wikiService.getCharacterByBaseSlug(idOrSlug)) ??
		(await wikiService.getCharacterBySlug(idOrSlug)) ??
		(await wikiService.getCharacter(idOrSlug))
	);
}

async function getEntity(input: z.output<typeof getInput>): Promise<unknown> {
	const { id } = input;
	switch (input.collection) {
		case "characters":
		case "coordinators":
			return await resolveCharacter(id);
		case "skills":
			return await wikiService.getSkill(id);
		case "items":
			return await wikiService.getItem(id);
		case "tactics":
			return await wikiService.getTactic(id);
		case "passives":
			return await wikiService.getPassive(id);
		case "teams":
			return await getTeamDetail(id);
		case "shops":
			return await getShop(Number.parseInt(id, 10));
		case "quests":
			return await getQuest(id);
		case "coaches":
			return await getCoach(Number.parseInt(id, 10));
		case "stadiums":
			return await getStadium(id);
		case "trophies":
			return await getTrophy(id);
		case "auras": {
			if (!input.auraType) throw new Error("`auraType` est requis pour la collection `auras`.");
			return await wikiService.getAura(id, input.auraType);
		}
		case "text":
			return resolveTextAll(id);
		case "capsules":
		case "costumes":
		case "gallery":
			throw new Error(
				`La collection « ${input.collection} » n'a pas de fiche détaillée : utiliser azalee_list avec un filtre \`q\`.`,
			);
	}
}

export function azaleeTools(): RegisteredTool[] {
	return [
		defineTool({
			name: "azalee_search",
			title: "Recherche unifiée dans le wiki Azalée",
			description:
				"Recherche un personnage, une technique ou un objet d'Inazuma Eleven: Victory Road par nom (français, anglais ou japonais) et renvoie les meilleures correspondances des trois catégories. Point d'entrée à privilégier quand on ne connaît pas l'identifiant exact : les résultats contiennent le slug à passer ensuite à azalee_get.",
			inputSchema: searchInput,
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: async ({ q, limit }) => {
				const [characters, skills, items] = await Promise.all([
					wikiService.getCharactersList({ q, limit, page: 1 } as never),
					wikiService.getSkillsList({ q, limit, page: 1 } as never),
					wikiService.getItemsList({ q, limit, page: 1 } as never),
				]);
				return structured({
					q,
					characters: characters.data,
					skills: skills.data,
					items: items.data,
				});
			},
		}),

		defineTool({
			name: "azalee_list",
			title: "Lister une collection du wiki",
			description:
				"Liste paginée et filtrable d'une collection du jeu : personnages, coordinateurs, techniques, objets, tactiques, passives, équipes, boutiques, quêtes, coachs, stades, trophées, capsules, costumes, galerie, auras (armure/keshin/miximax/totem). Filtres selon la collection : q, element, position, rarity, category, team, kind, sort.",
			inputSchema: listInput,
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: async (input) => structured(await listCollection(input)),
		}),

		defineTool({
			name: "azalee_get",
			title: "Fiche détaillée d'une entité du wiki",
			description:
				"Fiche complète d'une entité : statistiques, techniques, formes alternatives, provenance… L'identifiant est le slug renvoyé par azalee_search ou azalee_list. La collection `text` résout un identifiant de texte du jeu dans toutes les langues.",
			inputSchema: getInput,
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: async (input) => {
				const entity = await getEntity(input);
				if (entity === undefined || entity === null) {
					return toolError(`Aucune entité « ${input.id} » dans la collection « ${input.collection} ».`);
				}
				return structured(entity);
			},
		}),

		defineTool({
			name: "azalee_dataset",
			title: "Jeux de données globaux du jeu",
			description:
				"Renvoie un jeu de données complet : `drops` (taux de drop), `invocation` (taux d'invocation), `cross_tables` / `cross_stats` (catalogue Inazuma Eleven Cross), `text_stats` (répartition par catégorie des 259 000 entrées de texte du jeu), `health` (source de données réellement utilisée).",
			inputSchema: datasetInput,
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: async ({ dataset }) => {
				switch (dataset) {
					case "drops":
						return structured(await getDropsData());
					case "invocation":
						return structured(await getInvocationRates());
					case "cross_tables":
						return structured(await getCrossTables());
					case "cross_stats":
						return structured(await getCrossCatalogStats());
					case "text_stats":
						return structured(categoryStats());
					case "health":
						return structured({
							mirror: resolveMirrorPath(),
							dataDir: resolveDataDir(),
							cpkFiles: safeCount(),
						});
				}
			},
		}),

		defineTool({
			name: "cpk_browse",
			title: "Parcourir l'arborescence des fichiers du jeu (CPK)",
			description:
				"Liste le contenu d'un répertoire de l'arborescence des 250 800 fichiers extraits des archives CPK du jeu. Chemin vide = racine. Renvoie sous-répertoires et fichiers avec leur taille.",
			inputSchema: z.object({
				path: z.string().default("").describe("Chemin du répertoire, ex. `common/text` ou `dx11/menu`."),
				limit: z.int().min(1).max(1000).default(200).describe("Entrées par page."),
				offset: z.int().min(0).default(0).describe("Décalage de pagination."),
			}),
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: ({ path, limit, offset }) => structured(listDirPaged(path, limit, offset)),
		}),

		defineTool({
			name: "cpk_search",
			title: "Rechercher un fichier du jeu par nom",
			description:
				"Recherche dans l'index des fichiers CPK par sous-chaîne de chemin. Utile pour retrouver une texture (`.g4tx`), un modèle (`.g4md`), un fichier de configuration (`cfg.bin`) ou un script Lua.",
			inputSchema: z.object({
				q: z.string().min(1).describe("Sous-chaîne recherchée dans le chemin."),
				limit: z.int().min(1).max(1000).default(100).describe("Nombre maximum de résultats."),
			}),
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: ({ q, limit }) => structured(searchFiles(q, limit)),
		}),

		defineTool({
			name: "cpk_file",
			title: "Métadonnées d'un fichier du jeu",
			description:
				"Métadonnées d'un fichier CPK : taille, archive d'origine, type, et URL du CDN qui le décode à la volée (image PNG pour une texture g4tx, GLB texturé pour un modèle, JSON pour un cfg.bin).",
			inputSchema: z.object({ path: z.string().min(1).describe("Chemin complet du fichier dans l'arborescence.") }),
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: ({ path }) => {
				const meta = fileMeta(path);
				return meta ? structured(meta) : toolError(`Fichier CPK introuvable : ${path}`);
			},
		}),

		defineTool({
			name: "game_text_search",
			title: "Rechercher dans le texte du jeu",
			description:
				"Recherche plein texte dans les 259 000 entrées de texte extraites du jeu (dialogues, noms, descriptions, interface), en français, anglais ou japonais. Renvoie l'identifiant de hash, la catégorie et les trois traductions.",
			inputSchema: z.object({
				q: z.string().min(1).describe("Texte recherché."),
				locale: z.enum(["fr", "en", "ja"] as const).default("fr").describe("Langue de recherche."),
				limit: z.int().min(1).max(500).default(50).describe("Nombre maximum de résultats."),
			}),
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: ({ q, locale, limit }) => structured(searchText(q, locale, limit)),
		}),

		defineTool({
			name: "rag_search",
			title: "Recherche sémantique dans la base de connaissances",
			description:
				"Recherche vectorielle (RAG) sur le corpus Rose Griffon : fiches du wiki, texte du jeu, configurations et assets. Répond à des questions formulées en langue naturelle plutôt qu'à un mot-clé exact. Nécessite le service d'embeddings local ; renvoie une erreur explicite s'il est arrêté.",
			inputSchema: z.object({
				question: z.string().min(3).describe("Question en langue naturelle."),
				limit: z.int().min(1).max(30).default(8).describe("Nombre de passages renvoyés."),
			}),
			annotations: { readOnlyHint: true, idempotentHint: false, openWorldHint: false },
			handler: async ({ question, limit }) => {
				const results = await queryRag(question, limit);
				if (!Array.isArray(results) || results.length === 0) {
					return { content: [text("Aucun passage pertinent trouvé.")], structuredContent: [] };
				}
				return structured(results);
			},
		}),
	];
}

function safeCount(): number {
	try {
		return totalFiles();
	} catch {
		return 0;
	}
}
