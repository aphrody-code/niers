/**
 * Client HTTP distant (`src/remote/`).
 *
 * Les tests de bout en bout montent le VRAI routeur de la lib
 * (`handleAzaleeRequest`) derrière `Bun.serve` sur un port éphémère : le client
 * parle donc à l'API réelle, pas à une maquette. Le reste (délais, annulation,
 * reprises, erreurs typées) passe par un transport injecté, seul moyen de
 * provoquer une panne de façon déterministe.
 */

import { GlobalRegistrator } from "@happy-dom/global-registrator";

import { rendreReseauNatif } from "../../nie-plugin/src/happydom";
import { afterAll, beforeAll, describe, expect, test } from "bun:test";

import {
	AZALEE_DEFAULT_API_URL,
	AZALEE_SIDECAR_API_URL,
	AzaleeRemoteError,
	buildQuery,
	createAzaleeClient,
	defaultAzaleeCandidates,
	isAzaleeNotFound,
	isAzaleeRemoteError,
	isAzaleeRetriable,
	probeAzaleeApi,
	resolveAzaleeApiUrl,
	resolveAzaleeBaseUrl,
	type AzaleeClient,
} from "../src/remote/index";
import { handleAzaleeRequest, listRoutes } from "../src/server/serve";
import { resolveMirrorPath } from "../src/config";

// --- Serveur de test : le vrai routeur, sur un port éphémère ---------------

let server: ReturnType<typeof Bun.serve>;
let client: AzaleeClient;

/**
 * Lancé depuis la racine du dépôt, `bun test` précharge happy-dom
 * (`bunfig.toml` → `happydom.ts`), ce qui remplace les globals `Response` et
 * `fetch` par ceux d'un DOM simulé : `Bun.serve` refuse alors la `Response`
 * renvoyée par `handleAzaleeRequest` (« Expected a Response object ») et le
 * `fetch` du DOM n'arrive pas à lire la réponse. On rend donc ses globals
 * natifs à Bun le temps de ce fichier.
 *
 * Lancé depuis `packages/azalee`, Bun ne remonte pas chercher le `bunfig.toml`
 * de la racine : happy-dom n'est alors pas enregistré et il n'y a rien à
 * défaire — d'où le garde.
 */
let happyDomRestored = false;

beforeAll(async () => {
	try {
		// `unregister()` est asynchrone : sans `await`, l'échec devient un rejet
		// non capté et la restauration des globals n'est pas encore effective.
		await GlobalRegistrator.unregister();
		happyDomRestored = true;
	} catch {
		// happy-dom n'était pas enregistré : les globals sont déjà ceux de Bun.
	}
	server = Bun.serve({ port: 0, hostname: "127.0.0.1", fetch: (request) => handleAzaleeRequest(request) });
	client = createAzaleeClient({ baseUrl: `http://127.0.0.1:${server.port}`, attempts: 1 });
});

afterAll(async () => {
	server?.stop(true);
	if (happyDomRestored) {
		await GlobalRegistrator.register();
		// `register()` réinstalle la pile réseau simulée de happy-dom, et les fichiers de test
		// exécutés APRÈS celui-ci en héritent. On rend aussitôt à Bun ses primitives, comme le
		// fait le préchargement (`happydom.ts`).
		rendreReseauNatif();
	}
});

/** Les routes de données ne répondent que si le miroir SQLite existe sur la machine. */
const hasMirror = resolveMirrorPath() !== null;

// --- Résolution de la base d'URL ------------------------------------------

describe("resolveAzaleeApiUrl — base d'URL", () => {
	test("l'API publique est la valeur par défaut", () => {
		delete process.env.AZALEE_API_URL;
		expect(resolveAzaleeApiUrl()).toBe(AZALEE_DEFAULT_API_URL);
	});

	test("AZALEE_API_URL surcharge le défaut, l'argument surcharge l'environnement", () => {
		process.env.AZALEE_API_URL = "https://env.example/azalee";
		try {
			expect(resolveAzaleeApiUrl()).toBe("https://env.example/azalee");
			expect(resolveAzaleeApiUrl("https://arg.example")).toBe("https://arg.example");
		} finally {
			delete process.env.AZALEE_API_URL;
		}
	});

	test("le / final est retiré — sinon les routes produiraient un double séparateur", () => {
		expect(resolveAzaleeApiUrl("https://example.test/azalee///")).toBe("https://example.test/azalee");
	});

	test("les candidates par défaut placent le sidecar avant l'API publique", () => {
		delete process.env.AZALEE_API_URL;
		expect(defaultAzaleeCandidates()).toEqual([AZALEE_SIDECAR_API_URL, AZALEE_DEFAULT_API_URL]);
	});
});

describe("buildQuery — sérialisation des filtres", () => {
	test("omet undefined et null, sérialise nombres et booléens", () => {
		expect(buildQuery({ q: "mark", limit: 10, page: undefined, sort: null, has_video: true })).toBe(
			"?q=mark&limit=10&has_video=true",
		);
	});

	test("renvoie une chaîne vide quand tout est absent", () => {
		expect(buildQuery()).toBe("");
		expect(buildQuery({ q: undefined })).toBe("");
	});
});

// --- Couverture des routes -------------------------------------------------

/**
 * Reproduit la résolution de `serve.ts` (segments, `:param`, la route la plus
 * spécifique gagne) pour vérifier qu'un chemin produit par le client atteint
 * bien une route déclarée.
 */
function matchRoute(pathname: string): string | null {
	const actual = pathname.split("/").filter(Boolean);
	let best: string | null = null;
	let bestParams = Number.POSITIVE_INFINITY;
	for (const pattern of listRoutes()) {
		const expected = pattern.split("/").filter(Boolean);
		if (expected.length !== actual.length) continue;
		let params = 0;
		let ok = true;
		for (let i = 0; i < expected.length; i++) {
			const segment = expected[i] as string;
			if (segment.startsWith(":")) params++;
			else if (segment !== actual[i]) {
				ok = false;
				break;
			}
		}
		if (ok && params < bestParams) {
			best = pattern;
			bestParams = params;
		}
	}
	return best;
}

describe("couverture — une méthode par route de serve.ts", () => {
	test("les 41 routes déclarées sont toutes atteignables depuis le client", async () => {
		const seen: string[] = [];
		const recorder = createAzaleeClient({
			baseUrl: "http://recorder.test",
			transport: (request) => {
				seen.push(new URL(request.url).pathname);
				return Promise.resolve(Response.json(null));
			},
		});

		await Promise.all([
			recorder.index(),
			recorder.health(),
			recorder.characters(),
			recorder.character("mark-evans"),
			recorder.coordinators(),
			recorder.skills(),
			recorder.skill("s1"),
			recorder.items(),
			recorder.item("i1"),
			recorder.auras("keshin"),
			recorder.aura("keshin", "a1"),
			recorder.tactics(),
			recorder.tactic("t1"),
			recorder.teams(),
			recorder.team("0xF01BB293"),
			recorder.shops(),
			recorder.shop(1),
			recorder.quests(),
			recorder.quest("q1"),
			recorder.coaches(),
			recorder.coach(1),
			recorder.stadiums(),
			recorder.stadium("st1"),
			recorder.trophies(),
			recorder.trophy("tr1"),
			recorder.passives(),
			recorder.passive("p1"),
			recorder.gallery(),
			recorder.drops(),
			recorder.capsules(),
			recorder.costumes(),
			recorder.invocation(),
			recorder.cpkList(),
			recorder.cpkSearch({ q: "x" }),
			recorder.cpkFile("data/common/x.bin"),
			recorder.text({ q: "x" }),
			recorder.textEntry("0x1"),
			recorder.textStats(),
			recorder.crossTables(),
			recorder.crossStats(),
			recorder.search({ q: "ma" }),
		]);

		const matched = new Set<string>();
		for (const pathname of seen) {
			const route = matchRoute(pathname);
			expect(route, `chemin sans route correspondante : ${pathname}`).not.toBeNull();
			matched.add(route as string);
		}
		expect(matched.size).toBe(listRoutes().length);
		expect([...matched].sort()).toEqual(listRoutes());
	});

	test("les segments dynamiques sont encodés", async () => {
		let captured = "";
		const recorder = createAzaleeClient({
			baseUrl: "http://recorder.test",
			transport: (request) => {
				captured = new URL(request.url).pathname;
				return Promise.resolve(Response.json(null));
			},
		});
		await recorder.character("mark evans/#1");
		expect(captured).toBe("/api/characters/mark%20evans%2F%231");
	});
});

// --- Bout en bout contre le vrai routeur ----------------------------------

describe("bout en bout — Bun.serve + handleAzaleeRequest", () => {
	test("l'index publie le nom du package et ses 41 routes", async () => {
		const index = await client.index();
		expect(index.name).toBe("@rosegriffon/azalee");
		expect(index.routes).toEqual(listRoutes());
	});

	test("/health décrit l'état des sources locales", async () => {
		const health = await client.health();
		expect(health.ok).toBe(true);
		expect(health.mirror === null || typeof health.mirror === "string").toBe(true);
		expect(typeof health.cpkFiles).toBe("number");
	});

	test.skipIf(!hasMirror)("liste de personnages typée et paginée", async () => {
		const list = await client.characters({ q: "Mark", limit: 3, page: 1 });
		expect(list.page).toBe(1);
		expect(list.limit).toBe(3);
		expect(Array.isArray(list.data)).toBe(true);
		expect(list.data.length).toBeGreaterThan(0);
		expect(typeof list.data[0]?.charaId).toBe("string");
	});

	test.skipIf(!hasMirror)("fiche personnage résolue depuis le baseSlug publié par la liste", async () => {
		const list = await client.characters({ q: "Mark", limit: 1 });
		const slug = (list.data[0] as { baseSlug?: string; id?: string }).baseSlug;
		expect(slug).toBeString();
		const character = await client.character(slug as string);
		expect(character.names).toBeDefined();
	});

	test.skipIf(!hasMirror)("les 208 équipes remontent en un appel", async () => {
		const teams = await client.teams();
		expect(teams.length).toBeGreaterThan(100);
		expect(teams[0]?.id).toBeString();
	});

	test.skipIf(!hasMirror)("recherche transverse : personnages + techniques + objets", async () => {
		const result = await client.search({ q: "ma", limit: 3 });
		expect(result.q).toBe("ma");
		expect(Array.isArray(result.characters)).toBe(true);
		expect(Array.isArray(result.skills)).toBe(true);
		expect(Array.isArray(result.items)).toBe(true);
	});

	test("statistiques de l'index de texte", async () => {
		const stats = await client.textStats();
		expect(Array.isArray(stats)).toBe(true);
	});

	test("un identifiant inconnu remonte une erreur 404 typée", async () => {
		expect.assertions(4);
		try {
			await client.character("ce-personnage-n-existe-pas");
		} catch (error) {
			expect(isAzaleeRemoteError(error)).toBe(true);
			expect(isAzaleeNotFound(error)).toBe(true);
			expect((error as AzaleeRemoteError).kind).toBe("http");
			expect((error as AzaleeRemoteError).detail).toBe("personnage introuvable");
		}
	});

	test("une route inconnue remonte un 404 non réessayé", async () => {
		expect.assertions(2);
		try {
			await client.request("/api/inexistante");
		} catch (error) {
			expect((error as AzaleeRemoteError).status).toBe(404);
			expect(isAzaleeRetriable(error)).toBe(false);
		}
	});

	test("probeAzaleeApi renvoie le rapport de santé, ou null sur une base morte", async () => {
		expect(await probeAzaleeApi(`http://127.0.0.1:${server.port}`)).not.toBeNull();
		// Port fermé : la sonde doit dégrader en `null`, jamais lever.
		expect(await probeAzaleeApi("http://127.0.0.1:1", { timeoutMs: 300 })).toBeNull();
	});

	test("resolveAzaleeBaseUrl choisit la première base vivante", async () => {
		const picked = await resolveAzaleeBaseUrl(["http://127.0.0.1:1", `http://127.0.0.1:${server.port}/`], {
			timeoutMs: 500,
		});
		expect(picked?.baseUrl).toBe(`http://127.0.0.1:${server.port}`);
		expect(picked?.health.ok).toBe(true);
		expect(await resolveAzaleeBaseUrl(["http://127.0.0.1:1"], { timeoutMs: 300 })).toBeNull();
	});
});

// --- Délais, annulation, reprises ------------------------------------------

describe("robustesse du transport", () => {
	test("le délai dépassé produit une erreur `timeout`", async () => {
		expect.assertions(2);
		const slow = createAzaleeClient({
			baseUrl: "http://slow.test",
			attempts: 1,
			timeoutMs: 25,
			transport: (request) =>
				new Promise((_resolve, reject) => {
					request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true });
				}),
		});
		try {
			await slow.health();
		} catch (error) {
			expect((error as AzaleeRemoteError).kind).toBe("timeout");
			expect(isAzaleeRetriable(error)).toBe(true);
		}
	});

	test("l'annulation par l'appelant produit une erreur `abort`, non réessayée", async () => {
		expect.assertions(2);
		const controller = new AbortController();
		const hanging = createAzaleeClient({
			baseUrl: "http://hang.test",
			transport: (request) =>
				new Promise((_resolve, reject) => {
					request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true });
				}),
		});
		const promise = hanging.health({ signal: controller.signal });
		controller.abort();
		try {
			await promise;
		} catch (error) {
			expect((error as AzaleeRemoteError).kind).toBe("abort");
			expect(isAzaleeRetriable(error)).toBe(false);
		}
	});

	test("une panne réseau produit une erreur `network`", async () => {
		expect.assertions(3);
		const broken = createAzaleeClient({
			baseUrl: "http://broken.test",
			attempts: 1,
			transport: () => Promise.reject(new TypeError("fetch failed")),
		});
		try {
			await broken.teams();
		} catch (error) {
			expect((error as AzaleeRemoteError).kind).toBe("network");
			expect((error as AzaleeRemoteError).url).toBe("http://broken.test/api/teams");
			expect((error as AzaleeRemoteError).cause).toBeInstanceOf(TypeError);
		}
	});

	test("un 503 est réessayé jusqu'à succès", async () => {
		let calls = 0;
		const flaky = createAzaleeClient({
			baseUrl: "http://flaky.test",
			attempts: 3,
			retryDelayMs: 1,
			transport: () => {
				calls++;
				return Promise.resolve(
					calls < 3 ? Response.json({ error: "indisponible" }, { status: 503 }) : Response.json({ ok: true }),
				);
			},
		});
		await expect(flaky.health()).resolves.toEqual({ ok: true } as never);
		expect(calls).toBe(3);
	});

	test("un 400 n'est jamais réessayé", async () => {
		let calls = 0;
		const strict = createAzaleeClient({
			baseUrl: "http://strict.test",
			attempts: 4,
			retryDelayMs: 1,
			transport: () => {
				calls++;
				return Promise.resolve(Response.json({ error: "requête invalide" }, { status: 400 }));
			},
		});
		await expect(strict.health()).rejects.toThrow("requête invalide");
		expect(calls).toBe(1);
	});

	test("un corps JSON illisible produit une erreur `parse`", async () => {
		expect.assertions(1);
		const garbled = createAzaleeClient({
			baseUrl: "http://garbled.test",
			attempts: 1,
			transport: () => Promise.resolve(new Response("<html>oups", { status: 200 })),
		});
		try {
			await garbled.drops();
		} catch (error) {
			expect((error as AzaleeRemoteError).kind).toBe("parse");
		}
	});

	test("l'en-tête User-Agent est envoyé, et supprimable", async () => {
		const captured: Array<string | null> = [];
		const transport = (request: Request) => {
			captured.push(request.headers.get("user-agent"));
			return Promise.resolve(Response.json(null));
		};
		await createAzaleeClient({ baseUrl: "http://ua.test", transport }).health();
		await createAzaleeClient({ baseUrl: "http://ua.test", transport, userAgent: null }).health();
		await createAzaleeClient({ baseUrl: "http://ua.test", transport, userAgent: "gui-azalee/1.0" }).health();
		expect(captured[0]).toContain("@rosegriffon/azalee");
		expect(captured[1]).toBeNull();
		expect(captured[2]).toBe("gui-azalee/1.0");
	});
});
