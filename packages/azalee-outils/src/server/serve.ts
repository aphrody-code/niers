/**
 * API HTTP headless d'Azalée — **sans Next**.
 *
 * Même socle de données que le wiki web, exposé en JSON pur pour :
 *   - le CLI (`azalee serve`) ;
 *   - une application de bureau **Tauri** (le binaire Bun tourne en sidecar,
 *     la webview appelle `http://127.0.0.1:<port>/api/...`) ;
 *   - n'importe quel client HTTP (scripts, autres services du VPS).
 *
 * `handleAzaleeRequest` est une fonction pure `Request → Response` : elle
 * s'embarque aussi bien dans `Bun.serve` que dans un autre routeur.
 */

import { fileMeta, listDirPaged, searchFiles, totalFiles } from "../cpk/index";
import { getCrossCatalogStats, getCrossTables } from "@rosegriffon/azalee/cross/data";
import { categoryStats, searchText } from "../game-text/index";
import { resolveMirrorPath, resolveDataDir } from "../config";
import { getCapsuleList, getCostumeList } from "@rosegriffon/azalee/wiki/gacha";
import { getCoach, getCoachesList } from "@rosegriffon/azalee/wiki/coaches";
import { getDropsData } from "@rosegriffon/azalee/wiki/drops";
import { getInvocationRates } from "@rosegriffon/azalee/wiki/invocation";
import { getQuest, getQuestsList } from "@rosegriffon/azalee/wiki/quests";
import { getShop, getShopsList } from "@rosegriffon/azalee/wiki/shops";
import { getStadium, getStadiumsList } from "@rosegriffon/azalee/wiki/stadiums";
import { getTeamDetail, getTeamsList } from "@rosegriffon/azalee/wiki/teams";
import { getTrophiesList, getTrophy } from "@rosegriffon/azalee/wiki/trophies";
import { resolveTextAll } from "../wiki/game-text";
import { wikiService } from "@rosegriffon/azalee/wiki/service";

/**
 * Port d'écoute par défaut de l'API headless.
 *
 * Aligné sur le plan d'adressage des outils internes du VPS
 * (`api.rosegriffon.fr` : n2b 8802, shai 8803, cdn 8804, cdn-variants 8805,
 * rag-api 8806, azalee-api **8807**). 3010 est déjà pris par un autre service.
 */
export const DEFAULT_AZALEE_PORT = 8807;

/** Options du serveur headless. */
export interface AzaleeServerOptions {
	/** Port d'écoute (défaut : `AZALEE_PORT` ou `DEFAULT_AZALEE_PORT`). */
	port?: number;
	/** Interface d'écoute (défaut : `127.0.0.1` — local, adapté au sidecar Tauri). */
	hostname?: string;
	/** Origine autorisée en CORS (`true` = `*`). Défaut : `true`. */
	cors?: boolean | string;
}

/** Une route : méthode + segments (`:param` = capture) + handler. */
interface Route {
	segments: string[];
	handler: (ctx: RouteContext) => Promise<unknown> | unknown;
}

interface RouteContext {
	params: Record<string, string>;
	query: URLSearchParams;
	url: URL;
}

// --- Utilitaires de requête ------------------------------------------------

function num(query: URLSearchParams, key: string, fallback: number): number {
	const raw = query.get(key);
	if (raw === null) return fallback;
	const parsed = Number.parseInt(raw, 10);
	return Number.isFinite(parsed) ? parsed : fallback;
}

/** Convertit les paramètres d'URL en filtres de liste du wiki. */
function listParams(query: URLSearchParams): Record<string, string | number | undefined> {
	const params: Record<string, string | number | undefined> = {
		page: num(query, "page", 1),
		limit: Math.min(num(query, "limit", 24), 200),
	};
	for (const key of [
		"q",
		"element",
		"position",
		"rarity",
		"category",
		"team",
		"series",
		"gender",
		"playstyle",
		"has_video",
		"show_aura",
		"overdrive",
		"sort",
		"power_min",
		"power_max",
		"status",
		"role",
		"ageGroup",
	]) {
		const value = query.get(key);
		if (value !== null) params[key] = value;
	}
	return params;
}

/** Erreur HTTP typée — remonte un statut au lieu d'un 500. */
class HttpError extends Error {
	constructor(
		readonly status: number,
		message: string,
	) {
		super(message);
	}
}

function required<T>(value: T | null | undefined, what: string): T {
	if (value === null || value === undefined) throw new HttpError(404, `${what} introuvable`);
	return value;
}

/**
 * Résout un personnage depuis n'importe quelle forme d'identifiant exposée par
 * l'API, dans le MÊME ordre que la page web `/chara/[id]` :
 *   1. `baseSlug` — URL canonique (`mark-evans`), c'est ce que renvoie la liste ;
 *   2. `slug` de variante (`mark-evans-0x3055CF22`) ;
 *   3. `id` brut de ligne DB.
 *
 * Sans la 1re et la 3e branche, `/api/characters/<baseSlug>` renvoyait 404 alors
 * même que `/api/characters` publie ce `baseSlug` — la fiche était inatteignable
 * depuis un client CLI/Tauri.
 */
async function resolveCharacter(idOrSlug: string) {
	return (
		(await wikiService.getCharacterByBaseSlug(idOrSlug)) ??
		(await wikiService.getCharacterBySlug(idOrSlug)) ??
		(await wikiService.getCharacter(idOrSlug))
	);
}

// --- Table de routage ------------------------------------------------------

const ROUTES: Route[] = [];

function route(pattern: string, handler: Route["handler"]): void {
	ROUTES.push({ segments: pattern.split("/").filter(Boolean), handler });
}

route("health", () => ({
	ok: true,
	mirror: resolveMirrorPath(),
	dataDir: resolveDataDir(),
	cpkFiles: safe(() => totalFiles(), 0),
}));

route("api/characters", ({ query }) => wikiService.getCharactersList(listParams(query) as never));
route("api/characters/:slug", async ({ params }) =>
	required(await resolveCharacter(params.slug), "personnage"),
);
route("api/coordinators", ({ query }) => wikiService.getCoordinatorsList(listParams(query) as never));

route("api/skills", ({ query }) => wikiService.getSkillsList(listParams(query) as never));
route("api/skills/:id", async ({ params }) =>
	required(await wikiService.getSkill(params.id), "technique"),
);

route("api/items", ({ query }) => wikiService.getItemsList(listParams(query) as never));
route("api/items/:id", async ({ params }) => required(await wikiService.getItem(params.id), "objet"));

route("api/auras/:type", ({ params, query }) =>
	wikiService.getAurasList({ ...listParams(query), typeSlug: params.type } as never),
);
route("api/auras/:type/:id", async ({ params }) =>
	required(await wikiService.getAura(params.id, params.type), "aura"),
);

route("api/tactics", ({ query }) => wikiService.getTacticsList(listParams(query) as never));
route("api/tactics/:slug", async ({ params }) =>
	required(await wikiService.getTactic(params.slug), "tactique"),
);

route("api/teams", () => getTeamsList());
route("api/teams/:id", async ({ params }) => required(await getTeamDetail(params.id), "équipe"));

route("api/shops", () => getShopsList());
route("api/shops/:id", async ({ params }) =>
	required(await getShop(Number.parseInt(params.id, 10)), "boutique"),
);

route("api/quests", ({ query }) =>
	getQuestsList({ q: query.get("q") ?? undefined, kind: query.get("kind") ?? undefined } as never),
);
route("api/quests/:id", async ({ params }) => required(await getQuest(params.id), "quête"));

route("api/coaches", ({ query }) => getCoachesList({ q: query.get("q") ?? undefined }));
route("api/coaches/:id", async ({ params }) =>
	required(await getCoach(Number.parseInt(params.id, 10)), "coach"),
);

route("api/stadiums", ({ query }) => getStadiumsList({ q: query.get("q") ?? undefined }));
route("api/stadiums/:id", async ({ params }) => required(await getStadium(params.id), "stade"));

route("api/trophies", ({ query }) =>
	getTrophiesList({ q: query.get("q") ?? undefined, category: query.get("category") ?? undefined } as never),
);
route("api/trophies/:id", async ({ params }) => required(await getTrophy(params.id), "trophée"));

route("api/passives", ({ query }) =>
	wikiService.getPassivesList({
		q: query.get("q") ?? undefined,
		category: query.get("category") ?? undefined,
		page: num(query, "page", 1),
		limit: Math.min(num(query, "limit", 50), 500),
	} as never),
);
route("api/passives/:id", async ({ params }) =>
	required(await wikiService.getPassive(params.id), "passive"),
);

route("api/gallery", ({ query }) =>
	wikiService.getGalleryList({
		page: num(query, "page", 1),
		limit: Math.min(num(query, "limit", 60), 300),
		category: query.get("category") ?? undefined,
		q: query.get("q") ?? undefined,
	} as never),
);

route("api/drops", () => getDropsData());
route("api/capsules", ({ query }) => getCapsuleList({ q: query.get("q") ?? undefined } as never));
route("api/costumes", ({ query }) => getCostumeList({ q: query.get("q") ?? undefined } as never));
route("api/invocation", () => getInvocationRates());

route("api/cpk", ({ query }) =>
	listDirPaged(query.get("path") ?? "", Math.min(num(query, "limit", 200), 1000), num(query, "offset", 0)),
);
route("api/cpk/search", ({ query }) =>
	searchFiles(query.get("q") ?? "", Math.min(num(query, "limit", 200), 1000)),
);
route("api/cpk/file", ({ query }) =>
	required(fileMeta(query.get("path") ?? ""), "fichier CPK"),
);

route("api/text/:hash", ({ params }) => required(resolveTextAll(params.hash), "texte"));
route("api/text", ({ query }) =>
	searchText(query.get("q") ?? "", (query.get("locale") ?? "fr") as never, Math.min(num(query, "limit", 50), 500)),
);
route("api/text/stats", () => categoryStats());

route("api/cross/tables", () => getCrossTables());
route("api/cross/stats", () => getCrossCatalogStats());

/** Recherche transverse : personnages + techniques + objets. */
route("api/search", async ({ query }) => {
	const q = (query.get("q") ?? "").trim();
	if (q.length < 2) return { q, characters: [], skills: [], items: [] };
	const limit = Math.min(num(query, "limit", 10), 50);
	const [characters, skills, items] = await Promise.all([
		wikiService.getCharactersList({ q, limit, page: 1 } as never),
		wikiService.getSkillsList({ q, limit, page: 1 } as never),
		wikiService.getItemsList({ q, limit, page: 1 } as never),
	]);
	return { q, characters: characters.data, skills: skills.data, items: items.data };
});

/** Liste des routes déclarées (utile pour `/` et la doc du CLI). */
export function listRoutes(): string[] {
	return ROUTES.map((r) => `/${r.segments.join("/")}`).sort();
}

route("", () => ({ name: "@rosegriffon/azalee", routes: listRoutes() }));

// --- Résolution ------------------------------------------------------------

function safe<T>(fn: () => T, fallback: T): T {
	try {
		return fn();
	} catch {
		return fallback;
	}
}

function match(pathSegments: string[]): { route: Route; params: Record<string, string> } | null {
	let best: { route: Route; params: Record<string, string> } | null = null;
	let bestParams = Number.POSITIVE_INFINITY;

	for (const candidate of ROUTES) {
		if (candidate.segments.length !== pathSegments.length) continue;
		const params: Record<string, string> = {};
		let ok = true;
		for (let i = 0; i < candidate.segments.length; i++) {
			const expected = candidate.segments[i];
			const actual = pathSegments[i];
			if (expected.startsWith(":")) {
				params[expected.slice(1)] = decodeURIComponent(actual);
			} else if (expected !== actual) {
				ok = false;
				break;
			}
		}
		// La route la plus SPÉCIFIQUE gagne : `/api/text/stats` prime sur
		// `/api/text/:hash`, quel que soit l'ordre de déclaration.
		const count = Object.keys(params).length;
		if (ok && count < bestParams) {
			best = { route: candidate, params };
			bestParams = count;
			if (count === 0) break;
		}
	}
	return best;
}

function corsHeaders(cors: boolean | string): Record<string, string> {
	if (cors === false) return {};
	return {
		"Access-Control-Allow-Origin": cors === true ? "*" : cors,
		"Access-Control-Allow-Methods": "GET, OPTIONS",
		"Access-Control-Allow-Headers": "Content-Type",
	};
}

/**
 * Traite une requête et renvoie la réponse JSON correspondante. Aucune
 * dépendance à `Bun.serve` : embarquable dans n'importe quel routeur.
 */
export async function handleAzaleeRequest(
	request: Request,
	options: Pick<AzaleeServerOptions, "cors"> = {},
): Promise<Response> {
	const headers = { "Content-Type": "application/json; charset=utf-8", ...corsHeaders(options.cors ?? true) };

	if (request.method === "OPTIONS") return new Response(null, { status: 204, headers });
	if (request.method !== "GET") {
		return Response.json({ error: "méthode non supportée" }, { status: 405, headers });
	}

	const url = new URL(request.url);
	const segments = url.pathname.split("/").filter(Boolean);
	const found = match(segments);
	if (!found) return Response.json({ error: "route inconnue", routes: listRoutes() }, { status: 404, headers });

	try {
		const body = await found.route.handler({ params: found.params, query: url.searchParams, url });
		return Response.json(body, { headers });
	} catch (error) {
		if (error instanceof HttpError) {
			return Response.json({ error: error.message }, { status: error.status, headers });
		}
		const message = error instanceof Error ? error.message : String(error);
		return Response.json({ error: message }, { status: 500, headers });
	}
}

/**
 * Démarre le serveur HTTP headless (Bun natif).
 *
 * ```ts
 * import { createAzaleeServer } from "./";
 * createAzaleeServer({ port: 3010 }); // sidecar Tauri
 * ```
 */
export function createAzaleeServer(options: AzaleeServerOptions = {}) {
	const port = options.port ?? Number.parseInt(process.env.AZALEE_PORT ?? String(DEFAULT_AZALEE_PORT), 10);
	const hostname = options.hostname ?? process.env.AZALEE_HOST ?? "127.0.0.1";
	return Bun.serve({
		port,
		hostname,
		fetch: (request) => handleAzaleeRequest(request, { cors: options.cors }),
	});
}
