/**
 * Client HTTP typé de l'API Azalée.
 *
 * Une méthode par route de `src/server/serve.ts` (41), typée par les fonctions
 * serveur elles-mêmes (voir `./types`). Le client est **client-safe** : il ne
 * dépend que de `fetch`, `Request`, `URL` et `AbortSignal`, et se bundle donc
 * dans une webview Tauri, un navigateur ou un worker.
 *
 * ```ts
 * import { createAzaleeClient } from "./";
 *
 * // API publique (défaut)
 * const api = createAzaleeClient();
 * // Sidecar local d'une application Tauri
 * const local = createAzaleeClient({ baseUrl: "http://127.0.0.1:3010" });
 *
 * const { data } = await api.characters({ q: "Mark", limit: 10 });
 * ```
 */

import {
	AZALEE_SIDECAR_API_URL,
	type AzaleeClientOptions,
	type AzaleeQueryValue,
	type AzaleeRequestOptions,
	type AzaleeTransportContext,
	createTransportContext,
	requestJson,
} from "./transport";
import type {
	AzaleeAura,
	AzaleeAuraList,
	AzaleeCapsuleList,
	AzaleeCharacter,
	AzaleeCharacterList,
	AzaleeCoach,
	AzaleeCoachList,
	AzaleeCostumeList,
	AzaleeCpkFileMeta,
	AzaleeCpkListParams,
	AzaleeCpkListing,
	AzaleeCpkSearchParams,
	AzaleeCpkSearchResult,
	AzaleeCrossSearchParams,
	AzaleeCrossStats,
	AzaleeCrossTables,
	AzaleeDataSource,
	AzaleeDrops,
	AzaleeGalleryList,
	AzaleeGalleryParams,
	AzaleeHealth,
	AzaleeIndex,
	AzaleeInvocation,
	AzaleeItem,
	AzaleeItemList,
	AzaleeListParams,
	AzaleePassive,
	AzaleePassiveList,
	AzaleePassiveParams,
	AzaleeQuest,
	AzaleeQuestList,
	AzaleeQuestParams,
	AzaleeSearchParams,
	AzaleeSearchResult,
	AzaleeShopDetail,
	AzaleeShopList,
	AzaleeSkill,
	AzaleeSkillList,
	AzaleeStadium,
	AzaleeStadiumList,
	AzaleeTacticList,
	AzaleeTactic,
	AzaleeTeamDetail,
	AzaleeTeamList,
	AzaleeTextEntry,
	AzaleeTextParams,
	AzaleeTextResult,
	AzaleeTextStats,
	AzaleeTrophy,
	AzaleeTrophyList,
	AzaleeTrophyParams,
} from "./types";

/**
 * Client complet : la surface de lecture commune plus l'accès brut et la base
 * d'URL effective.
 */
export interface AzaleeClient extends AzaleeDataSource {
	/** Base d'URL réellement utilisée, sans `/` final. */
	readonly baseUrl: string;
	/**
	 * Appelle une route arbitraire. Échappatoire pour une route ajoutée au
	 * serveur avant d'avoir sa méthode dédiée ici.
	 */
	request<T>(
		path: string,
		query?: Record<string, AzaleeQueryValue>,
		options?: AzaleeRequestOptions,
	): Promise<T>;
}

/** Convertit des filtres de liste en chaîne de requête (les `undefined` sont omis). */
function listQuery(params?: AzaleeListParams): Record<string, AzaleeQueryValue> {
	return { ...params } as Record<string, AzaleeQueryValue>;
}

/** Encode un segment dynamique : les chemins CPK et certains slugs contiennent `/` ou `#`. */
function seg(value: string | number): string {
	return encodeURIComponent(String(value));
}

/**
 * Crée un client de l'API Azalée.
 *
 * Sans `baseUrl`, l'ordre de résolution est : `AZALEE_API_URL` puis
 * `https://api.rosegriffon.fr/azalee`.
 */
export function createAzaleeClient(options: AzaleeClientOptions = {}): AzaleeClient {
	const context: AzaleeTransportContext = createTransportContext(options);

	const get = <T>(
		path: string,
		query?: Record<string, AzaleeQueryValue>,
		requestOptions?: AzaleeRequestOptions,
	): Promise<T> => requestJson<T>(context, path, query, requestOptions);

	return {
		baseUrl: context.baseUrl,
		request: get,

		index: (o) => get<AzaleeIndex>("/", undefined, o),
		health: (o) => get<AzaleeHealth>("/health", undefined, o),

		characters: (p, o) => get<AzaleeCharacterList>("/api/characters", listQuery(p), o),
		character: (idOrSlug, o) => get<AzaleeCharacter>(`/api/characters/${seg(idOrSlug)}`, undefined, o),
		coordinators: (p, o) => get<AzaleeCharacterList>("/api/coordinators", listQuery(p), o),

		skills: (p, o) => get<AzaleeSkillList>("/api/skills", listQuery(p), o),
		skill: (id, o) => get<AzaleeSkill>(`/api/skills/${seg(id)}`, undefined, o),

		items: (p, o) => get<AzaleeItemList>("/api/items", listQuery(p), o),
		item: (id, o) => get<AzaleeItem>(`/api/items/${seg(id)}`, undefined, o),

		auras: (type, p, o) => get<AzaleeAuraList>(`/api/auras/${seg(type)}`, listQuery(p), o),
		aura: (type, id, o) => get<AzaleeAura>(`/api/auras/${seg(type)}/${seg(id)}`, undefined, o),

		tactics: (p, o) => get<AzaleeTacticList>("/api/tactics", listQuery(p), o),
		tactic: (slug, o) => get<AzaleeTactic>(`/api/tactics/${seg(slug)}`, undefined, o),

		teams: (o) => get<AzaleeTeamList>("/api/teams", undefined, o),
		team: (id, o) => get<AzaleeTeamDetail>(`/api/teams/${seg(id)}`, undefined, o),

		shops: (o) => get<AzaleeShopList>("/api/shops", undefined, o),
		shop: (id, o) => get<AzaleeShopDetail>(`/api/shops/${seg(id)}`, undefined, o),

		quests: (p: AzaleeQuestParams = {}, o) => get<AzaleeQuestList>("/api/quests", { ...p }, o),
		quest: (id, o) => get<AzaleeQuest>(`/api/quests/${seg(id)}`, undefined, o),

		coaches: (p: AzaleeSearchParams = {}, o) => get<AzaleeCoachList>("/api/coaches", { ...p }, o),
		coach: (id, o) => get<AzaleeCoach>(`/api/coaches/${seg(id)}`, undefined, o),

		stadiums: (p: AzaleeSearchParams = {}, o) => get<AzaleeStadiumList>("/api/stadiums", { ...p }, o),
		stadium: (id, o) => get<AzaleeStadium>(`/api/stadiums/${seg(id)}`, undefined, o),

		trophies: (p: AzaleeTrophyParams = {}, o) => get<AzaleeTrophyList>("/api/trophies", { ...p }, o),
		trophy: (id, o) => get<AzaleeTrophy>(`/api/trophies/${seg(id)}`, undefined, o),

		passives: (p: AzaleePassiveParams = {}, o) => get<AzaleePassiveList>("/api/passives", { ...p }, o),
		passive: (id, o) => get<AzaleePassive>(`/api/passives/${seg(id)}`, undefined, o),

		gallery: (p: AzaleeGalleryParams = {}, o) => get<AzaleeGalleryList>("/api/gallery", { ...p }, o),

		drops: (o) => get<AzaleeDrops>("/api/drops", undefined, o),
		capsules: (p: AzaleeSearchParams = {}, o) => get<AzaleeCapsuleList>("/api/capsules", { ...p }, o),
		costumes: (p: AzaleeSearchParams = {}, o) => get<AzaleeCostumeList>("/api/costumes", { ...p }, o),
		invocation: (o) => get<AzaleeInvocation>("/api/invocation", undefined, o),

		cpkList: (p: AzaleeCpkListParams = {}, o) => get<AzaleeCpkListing>("/api/cpk", { ...p }, o),
		cpkSearch: (p: AzaleeCpkSearchParams, o) => get<AzaleeCpkSearchResult>("/api/cpk/search", { ...p }, o),
		cpkFile: (path, o) => get<AzaleeCpkFileMeta>("/api/cpk/file", { path }, o),

		text: (p: AzaleeTextParams, o) => get<AzaleeTextResult>("/api/text", { ...p }, o),
		textEntry: (hashId, o) => get<AzaleeTextEntry>(`/api/text/${seg(hashId)}`, undefined, o),
		textStats: (o) => get<AzaleeTextStats>("/api/text/stats", undefined, o),

		crossTables: (o) => get<AzaleeCrossTables>("/api/cross/tables", undefined, o),
		crossStats: (o) => get<AzaleeCrossStats>("/api/cross/stats", undefined, o),

		search: (p: AzaleeCrossSearchParams, o) => get<AzaleeSearchResult>("/api/search", { ...p }, o),
	};
}

/**
 * Interroge `/health` sur une base d'URL et renvoie le rapport, ou `null` si
 * l'API ne répond pas dans le délai imparti.
 *
 * Le délai par défaut est court (1,5 s) et sans reprise : c'est une **sonde**,
 * pas un appel de données. Elle sert à choisir une source, pas à en obtenir.
 */
export async function probeAzaleeApi(
	baseUrl: string,
	options: AzaleeRequestOptions & { fetch?: typeof globalThis.fetch } = {},
): Promise<AzaleeHealth | null> {
	const { fetch: fetchImpl, ...requestOptions } = options;
	const client = createAzaleeClient({
		baseUrl,
		attempts: 1,
		timeoutMs: requestOptions.timeoutMs ?? 1_500,
		...(fetchImpl ? { fetch: fetchImpl } : {}),
	});
	try {
		const health = await client.health(requestOptions);
		return health.ok ? health : null;
	} catch {
		return null;
	}
}

/**
 * Choisit la première base d'URL disponible parmi des candidates, dans
 * l'ordre. Sonde `/health` sur chacune et s'arrête à la première qui répond.
 *
 * C'est le pendant **navigateur** de `createAzaleeData` : dans une webview
 * Tauri, on tente le sidecar local puis l'API publique.
 *
 * ```ts
 * const picked = await resolveAzaleeBaseUrl([AZALEE_SIDECAR_API_URL, AZALEE_DEFAULT_API_URL]);
 * const client = createAzaleeClient({ baseUrl: picked?.baseUrl });
 * ```
 *
 * @param requireLocalData n'accepter une base que si elle sert de vraies
 *   données locales (`mirror` non nul) — utile pour écarter un sidecar démarré
 *   sur une machine sans miroir SQLite ni dump du jeu.
 */
export async function resolveAzaleeBaseUrl(
	candidates: readonly string[],
	options: AzaleeRequestOptions & { fetch?: typeof globalThis.fetch; requireLocalData?: boolean } = {},
): Promise<{ baseUrl: string; health: AzaleeHealth } | null> {
	const { requireLocalData = false, ...probeOptions } = options;
	for (const candidate of candidates) {
		const baseUrl = candidate.replace(/\/+$/, "");
		const health = await probeAzaleeApi(baseUrl, probeOptions);
		if (!health) continue;
		if (requireLocalData && health.mirror === null) continue;
		return { baseUrl, health };
	}
	return null;
}

/** Candidates par défaut d'une application de bureau : sidecar local, puis API publique. */
export function defaultAzaleeCandidates(sidecarUrl: string = AZALEE_SIDECAR_API_URL): string[] {
	return [sidecarUrl, createAzaleeClient().baseUrl];
}
