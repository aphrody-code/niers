/**
 * Types de réponse de l'API Azalée, **dérivés des fonctions serveur** qui les
 * produisent — jamais retapés à la main.
 *
 * Chaque route de `src/server/serve.ts` délègue à une fonction de `wiki/`,
 * `cpk/`, `game-text/` ou `cross/`. On récupère donc le type de sortie via
 * `Awaited<ReturnType<typeof …>>` : impossible que le client dérive du serveur,
 * puisque toute évolution d'une fonction serveur se propage ici à la
 * compilation. Les routes de détail passent par le garde `required()` de
 * `serve.ts` (404 sinon), d'où le `NonNullable<…>`.
 *
 * Tous les imports sont **`import type`** : ils sont intégralement effacés à la
 * compilation, ce module reste donc bundlable pour le navigateur alors même
 * qu'il référence le pilote SQLite de Bun.
 */

import type { categoryStats, searchText } from "../game-text/index";
import type { fileMeta, listDirPaged, searchFiles } from "../cpk/index";
import type { getCapsuleList, getCostumeList } from "@rosegriffon/azalee/wiki/gacha";
import type { getCoach, getCoachesList } from "@rosegriffon/azalee/wiki/coaches";
import type { getCrossCatalogStats, getCrossTables } from "@rosegriffon/azalee/cross/data";
import type { getDropsData } from "@rosegriffon/azalee/wiki/drops";
import type { getInvocationRates } from "@rosegriffon/azalee/wiki/invocation";
import type { getQuest, getQuestsList } from "@rosegriffon/azalee/wiki/quests";
import type { getShop, getShopsList } from "@rosegriffon/azalee/wiki/shops";
import type { getStadium, getStadiumsList } from "@rosegriffon/azalee/wiki/stadiums";
import type { getTeamDetail, getTeamsList } from "@rosegriffon/azalee/wiki/teams";
import type { getTrophiesList, getTrophy } from "@rosegriffon/azalee/wiki/trophies";
import type { resolveTextAll } from "../wiki/game-text";
import type { wikiService } from "@rosegriffon/azalee/wiki/service";
import type { AzaleeRequestOptions } from "./transport";

/** Sortie résolue d'une fonction serveur. */
type Out<F extends (...args: never[]) => unknown> = Awaited<ReturnType<F>>;
/** Sortie résolue d'une route de détail (le 404 est levé avant, par `required()`). */
type Detail<F extends (...args: never[]) => unknown> = NonNullable<Out<F>>;

// --- Paramètres ------------------------------------------------------------

/**
 * Filtres communs aux listes paginées (`/api/characters`, `/api/skills`,
 * `/api/items`, `/api/tactics`, `/api/auras/:type`, `/api/coordinators`).
 * Dérivés de `wikiService.getCharactersList`, dont `serve.ts` alimente le
 * paramètre via `listParams()`.
 */
export type AzaleeListParams = Parameters<typeof wikiService.getCharactersList>[0];

/** Filtres de `/api/passives`. */
export type AzaleePassiveParams = Parameters<typeof wikiService.getPassivesList>[0];

/** Filtres de `/api/gallery`. */
export type AzaleeGalleryParams = Parameters<typeof wikiService.getGalleryList>[0];

/** Filtres de `/api/quests`. */
export type AzaleeQuestParams = NonNullable<Parameters<typeof getQuestsList>[0]>;

/** Filtres de `/api/trophies`. */
export type AzaleeTrophyParams = NonNullable<Parameters<typeof getTrophiesList>[0]>;

/** Filtre libre partagé par `/api/coaches`, `/api/stadiums`, `/api/capsules`, `/api/costumes`. */
export interface AzaleeSearchParams {
	q?: string;
}

/** Langue de l'index de texte de jeu. */
export type AzaleeTextLocale = NonNullable<Parameters<typeof searchText>[1]>;

/** Paramètres de `/api/text`. */
export interface AzaleeTextParams {
	q: string;
	locale?: AzaleeTextLocale;
	limit?: number;
}

/** Paramètres de `/api/cpk`. */
export interface AzaleeCpkListParams {
	/** Chemin du dossier dans l'arbre CPK (`""` = racine). */
	path?: string;
	limit?: number;
	offset?: number;
}

/** Paramètres de `/api/cpk/search`. */
export interface AzaleeCpkSearchParams {
	q: string;
	limit?: number;
}

/** Paramètres de `/api/search`. */
export interface AzaleeCrossSearchParams {
	q: string;
	limit?: number;
}

// --- Réponses --------------------------------------------------------------

/** `GET /api/characters` et `GET /api/coordinators`. */
export type AzaleeCharacterList = Out<typeof wikiService.getCharactersList>;
/** `GET /api/characters/:slug`. */
export type AzaleeCharacter = Detail<typeof wikiService.getCharacterByBaseSlug>;
/** `GET /api/skills`. */
export type AzaleeSkillList = Out<typeof wikiService.getSkillsList>;
/** `GET /api/skills/:id`. */
export type AzaleeSkill = Detail<typeof wikiService.getSkill>;
/** `GET /api/items`. */
export type AzaleeItemList = Out<typeof wikiService.getItemsList>;
/** `GET /api/items/:id`. */
export type AzaleeItem = Detail<typeof wikiService.getItem>;
/** `GET /api/auras/:type`. */
export type AzaleeAuraList = Out<typeof wikiService.getAurasList>;
/** `GET /api/auras/:type/:id`. */
export type AzaleeAura = Detail<typeof wikiService.getAura>;
/** `GET /api/tactics`. */
export type AzaleeTacticList = Out<typeof wikiService.getTacticsList>;
/** `GET /api/tactics/:slug`. */
export type AzaleeTactic = Detail<typeof wikiService.getTactic>;
/** `GET /api/teams`. */
export type AzaleeTeamList = Out<typeof getTeamsList>;
/** `GET /api/teams/:id`. */
export type AzaleeTeamDetail = Detail<typeof getTeamDetail>;
/** `GET /api/shops`. */
export type AzaleeShopList = Out<typeof getShopsList>;
/** `GET /api/shops/:id`. */
export type AzaleeShopDetail = Detail<typeof getShop>;
/** `GET /api/quests`. */
export type AzaleeQuestList = Out<typeof getQuestsList>;
/** `GET /api/quests/:id`. */
export type AzaleeQuest = Detail<typeof getQuest>;
/** `GET /api/coaches`. */
export type AzaleeCoachList = Out<typeof getCoachesList>;
/** `GET /api/coaches/:id`. */
export type AzaleeCoach = Detail<typeof getCoach>;
/** `GET /api/stadiums`. */
export type AzaleeStadiumList = Out<typeof getStadiumsList>;
/** `GET /api/stadiums/:id`. */
export type AzaleeStadium = Detail<typeof getStadium>;
/** `GET /api/trophies`. */
export type AzaleeTrophyList = Out<typeof getTrophiesList>;
/** `GET /api/trophies/:id`. */
export type AzaleeTrophy = Detail<typeof getTrophy>;
/** `GET /api/passives`. */
export type AzaleePassiveList = Out<typeof wikiService.getPassivesList>;
/** `GET /api/passives/:id`. */
export type AzaleePassive = Detail<typeof wikiService.getPassive>;
/** `GET /api/gallery`. */
export type AzaleeGalleryList = Out<typeof wikiService.getGalleryList>;
/** `GET /api/drops`. */
export type AzaleeDrops = Out<typeof getDropsData>;
/** `GET /api/capsules`. */
export type AzaleeCapsuleList = Out<typeof getCapsuleList>;
/** `GET /api/costumes`. */
export type AzaleeCostumeList = Out<typeof getCostumeList>;
/** `GET /api/invocation`. */
export type AzaleeInvocation = Out<typeof getInvocationRates>;
/** `GET /api/cpk`. */
export type AzaleeCpkListing = Out<typeof listDirPaged>;
/** `GET /api/cpk/search`. */
export type AzaleeCpkSearchResult = Out<typeof searchFiles>;
/** `GET /api/cpk/file`. */
export type AzaleeCpkFileMeta = Detail<typeof fileMeta>;
/** `GET /api/text`. */
export type AzaleeTextResult = Out<typeof searchText>;
/** `GET /api/text/:hash`. */
export type AzaleeTextEntry = Detail<typeof resolveTextAll>;
/** `GET /api/text/stats`. */
export type AzaleeTextStats = Out<typeof categoryStats>;
/** `GET /api/cross/tables`. */
export type AzaleeCrossTables = Out<typeof getCrossTables>;
/** `GET /api/cross/stats`. */
export type AzaleeCrossStats = Out<typeof getCrossCatalogStats>;

/**
 * `GET /api/search` — agrégat construit par `serve.ts` à partir des trois
 * listes ; les tableaux sont donc dérivés de leurs listes respectives.
 */
export interface AzaleeSearchResult {
	q: string;
	characters: AzaleeCharacterList["data"];
	skills: AzaleeSkillList["data"];
	items: AzaleeItemList["data"];
}

/**
 * `GET /health` — sonde de démarrage du sidecar. `mirror` et `dataDir` valent
 * `null` quand la machine n'a **ni miroir SQLite ni artefacts** : c'est le
 * signal qu'un client doit basculer sur l'API distante.
 */
export interface AzaleeHealth {
	ok: boolean;
	mirror: string | null;
	dataDir: string | null;
	cpkFiles: number;
}

/** `GET /` — identité du service et table de routage. */
export interface AzaleeIndex {
	name: string;
	routes: string[];
}

// --- Surface de lecture commune -------------------------------------------

/**
 * Surface de lecture unique de l'encyclopédie, indépendante de la provenance
 * des données. Le client HTTP (`createAzaleeClient`) et le résolveur avec repli
 * (`createAzaleeData`) l'implémentent tous les deux : un appelant — CLI, GUI
 * Tauri, script — écrit le même code que la donnée vienne du disque local ou de
 * `api.rosegriffon.fr`.
 *
 * Chaque méthode rejette avec une `AzaleeRemoteError` typée ; un `status: 404`
 * signale une ressource absente, pas une panne.
 */
export interface AzaleeDataSource {
	/** Identité du service et liste des routes. */
	index(options?: AzaleeRequestOptions): Promise<AzaleeIndex>;
	/** Sonde de disponibilité et état des sources locales. */
	health(options?: AzaleeRequestOptions): Promise<AzaleeHealth>;

	characters(params?: AzaleeListParams, options?: AzaleeRequestOptions): Promise<AzaleeCharacterList>;
	character(idOrSlug: string, options?: AzaleeRequestOptions): Promise<AzaleeCharacter>;
	coordinators(params?: AzaleeListParams, options?: AzaleeRequestOptions): Promise<AzaleeCharacterList>;

	skills(params?: AzaleeListParams, options?: AzaleeRequestOptions): Promise<AzaleeSkillList>;
	skill(id: string, options?: AzaleeRequestOptions): Promise<AzaleeSkill>;

	items(params?: AzaleeListParams, options?: AzaleeRequestOptions): Promise<AzaleeItemList>;
	item(id: string, options?: AzaleeRequestOptions): Promise<AzaleeItem>;

	auras(type: string, params?: AzaleeListParams, options?: AzaleeRequestOptions): Promise<AzaleeAuraList>;
	aura(type: string, id: string, options?: AzaleeRequestOptions): Promise<AzaleeAura>;

	tactics(params?: AzaleeListParams, options?: AzaleeRequestOptions): Promise<AzaleeTacticList>;
	tactic(slug: string, options?: AzaleeRequestOptions): Promise<AzaleeTactic>;

	teams(options?: AzaleeRequestOptions): Promise<AzaleeTeamList>;
	team(id: string, options?: AzaleeRequestOptions): Promise<AzaleeTeamDetail>;

	shops(options?: AzaleeRequestOptions): Promise<AzaleeShopList>;
	shop(id: number, options?: AzaleeRequestOptions): Promise<AzaleeShopDetail>;

	quests(params?: AzaleeQuestParams, options?: AzaleeRequestOptions): Promise<AzaleeQuestList>;
	quest(id: string, options?: AzaleeRequestOptions): Promise<AzaleeQuest>;

	coaches(params?: AzaleeSearchParams, options?: AzaleeRequestOptions): Promise<AzaleeCoachList>;
	coach(id: number, options?: AzaleeRequestOptions): Promise<AzaleeCoach>;

	stadiums(params?: AzaleeSearchParams, options?: AzaleeRequestOptions): Promise<AzaleeStadiumList>;
	stadium(id: string, options?: AzaleeRequestOptions): Promise<AzaleeStadium>;

	trophies(params?: AzaleeTrophyParams, options?: AzaleeRequestOptions): Promise<AzaleeTrophyList>;
	trophy(id: string, options?: AzaleeRequestOptions): Promise<AzaleeTrophy>;

	passives(params?: AzaleePassiveParams, options?: AzaleeRequestOptions): Promise<AzaleePassiveList>;
	passive(id: string, options?: AzaleeRequestOptions): Promise<AzaleePassive>;

	gallery(params?: AzaleeGalleryParams, options?: AzaleeRequestOptions): Promise<AzaleeGalleryList>;

	drops(options?: AzaleeRequestOptions): Promise<AzaleeDrops>;
	capsules(params?: AzaleeSearchParams, options?: AzaleeRequestOptions): Promise<AzaleeCapsuleList>;
	costumes(params?: AzaleeSearchParams, options?: AzaleeRequestOptions): Promise<AzaleeCostumeList>;
	invocation(options?: AzaleeRequestOptions): Promise<AzaleeInvocation>;

	cpkList(params?: AzaleeCpkListParams, options?: AzaleeRequestOptions): Promise<AzaleeCpkListing>;
	cpkSearch(params: AzaleeCpkSearchParams, options?: AzaleeRequestOptions): Promise<AzaleeCpkSearchResult>;
	cpkFile(path: string, options?: AzaleeRequestOptions): Promise<AzaleeCpkFileMeta>;

	text(params: AzaleeTextParams, options?: AzaleeRequestOptions): Promise<AzaleeTextResult>;
	textEntry(hashId: string | number, options?: AzaleeRequestOptions): Promise<AzaleeTextEntry>;
	textStats(options?: AzaleeRequestOptions): Promise<AzaleeTextStats>;

	crossTables(options?: AzaleeRequestOptions): Promise<AzaleeCrossTables>;
	crossStats(options?: AzaleeRequestOptions): Promise<AzaleeCrossStats>;

	search(params: AzaleeCrossSearchParams, options?: AzaleeRequestOptions): Promise<AzaleeSearchResult>;
}
