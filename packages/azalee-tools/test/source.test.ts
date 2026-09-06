/**
 * Résolution de la source de données avec repli (`src/server/source.ts`).
 *
 * Trois scénarios réels sont couverts :
 *   1. artefacts présents → lecture locale, **aucun** appel réseau ;
 *   2. artefacts absents → bascule automatique sur l'API distante ;
 *   3. artefacts présents mais base hors ligne → dégradation à chaud vers
 *      l'API distante, en cours d'exécution.
 *
 * La base « hors ligne » est simulée en injectant une fabrique de client qui
 * lève (`setDatabaseProvider`) : c'est le point d'injection officiel de la lib,
 * donc le chemin d'échec traversé est le vrai (`wiki/*` → `createClient()` →
 * `handleAzaleeRequest` → 500).
 */

import { afterEach, describe, expect, test } from "bun:test";

import { configureAzalee, resetAzaleeConfig, resolveMirrorPath } from "../src/config";
import { createAzaleeData, inspectLocalData } from "../src/server/source";
import { setDatabaseProvider } from "@rosegriffon/azalee/db";
import type { AzaleeTeamList } from "../src/remote/index";

/** Base d'URL distante fictive : aucun appel ne sort, le `fetch` est un double. */
const REMOTE = "https://api.example.test/azalee";

/** Charge utile renvoyée par le double de `fetch`, reconnaissable dans les assertions. */
const REMOTE_TEAMS = [{ id: "0xDISTANT", name: "Équipe distante" }] as unknown as AzaleeTeamList;

/** Double de `fetch` qui enregistre les URL appelées et répond du JSON canné. */
function stubFetch(payload: unknown = REMOTE_TEAMS): { fetch: typeof globalThis.fetch; urls: string[] } {
	const urls: string[] = [];
	const impl = (input: Request | string | URL) => {
		urls.push(typeof input === "string" ? input : input instanceof URL ? input.href : input.url);
		return Promise.resolve(Response.json(payload));
	};
	return { fetch: impl as unknown as typeof globalThis.fetch, urls };
}

/** `fetch` interdit : toute sortie réseau fait échouer le test. */
const forbiddenFetch = (() => {
	throw new Error("appel réseau interdit dans ce scénario");
}) as unknown as typeof globalThis.fetch;

afterEach(() => {
	resetAzaleeConfig();
	setDatabaseProvider(null);
});

const hasMirror = resolveMirrorPath() !== null;

describe("inspectLocalData — état des sources locales", () => {
	test("décrit le miroir, le dossier d'artefacts et le motif du choix", () => {
		const local = inspectLocalData();
		expect(typeof local.available).toBe("boolean");
		expect(local.reason).toBeString();
		expect(local.mirror === null || local.mirror.endsWith(".sqlite")).toBe(true);
		expect(local.available).toBe(local.injectedProvider || local.mirror !== null);
	});

	test("un chemin de miroir épinglé mais inexistant n'est PAS considéré disponible", () => {
		configureAzalee({ mirrorPath: "/var/empty/azalee-inexistant.sqlite" });
		const local = inspectLocalData();
		expect(local.mirror).toBeNull();
		expect(local.available).toBe(false);
		expect(local.reason).toContain("aucun miroir SQLite");
	});

	test("un client injecté par l'hôte rend la source locale disponible", () => {
		configureAzalee({ mirrorPath: "/var/empty/azalee-inexistant.sqlite" });
		setDatabaseProvider((() => {
			throw new Error("jamais appelé ici");
		}) as never);
		expect(inspectLocalData().available).toBe(true);
		expect(inspectLocalData().injectedProvider).toBe(true);
	});
});

describe("createAzaleeData — données locales présentes", () => {
	test.skipIf(!hasMirror)("mode auto : source locale, zéro appel réseau", async () => {
		const data = createAzaleeData({ baseUrl: REMOTE, fetch: forbiddenFetch, fallbackOnError: false });
		expect(data.source).toBe("local");
		expect(data.reason).toContain("source locale retenue");
		const health = await data.health();
		expect(health.ok).toBe(true);
		expect(health.mirror).toBe(resolveMirrorPath());
		expect(data.isDegraded()).toBe(false);
	});

	test.skipIf(!hasMirror)("le préfixe de chemin de la base distante est retiré côté local", async () => {
		// `https://api.example.test/azalee` → la passerelle retire `/azalee` ;
		// sans ce retrait, la route locale cherchée serait `/azalee/api/teams`.
		const data = createAzaleeData({ mode: "local", baseUrl: REMOTE, fetch: forbiddenFetch });
		const teams = await data.teams();
		expect(Array.isArray(teams)).toBe(true);
		expect(teams.length).toBeGreaterThan(100);
	});

	test.skipIf(!hasMirror)("la surface de lecture est celle du client distant", () => {
		const data = createAzaleeData({ mode: "local" });
		for (const method of ["characters", "skills", "teams", "cpkList", "text", "search", "health"] as const) {
			expect(typeof data[method]).toBe("function");
		}
		expect(data.baseUrl).toBeString();
	});
});

describe("createAzaleeData — données locales absentes", () => {
	test("mode auto : bascule sur l'API distante, à la bonne URL", async () => {
		configureAzalee({ mirrorPath: "/var/empty/azalee-inexistant.sqlite" });
		const stub = stubFetch();
		const data = createAzaleeData({ baseUrl: REMOTE, fetch: stub.fetch });

		expect(data.source).toBe("remote");
		expect(data.reason).toContain("repli distant");
		await expect(data.teams()).resolves.toEqual(REMOTE_TEAMS as never);
		expect(stub.urls).toEqual([`${REMOTE}/api/teams`]);
	});

	test("mode remote imposé : la source locale n'est jamais consultée", async () => {
		const stub = stubFetch();
		const data = createAzaleeData({ mode: "remote", baseUrl: REMOTE, fetch: stub.fetch });
		expect(data.source).toBe("remote");
		expect(data.reason).toContain("imposée par l'appelant");
		await data.characters({ q: "Mark", limit: 2 });
		expect(stub.urls[0]).toBe(`${REMOTE}/api/characters?q=Mark&limit=2`);
	});
});

describe("createAzaleeData — base locale hors ligne", () => {
	test("dégradation à chaud vers l'API distante quand la lecture locale échoue", async () => {
		// Client injecté qui lève : `wiki/teams` → `createClient()` → 500 local.
		setDatabaseProvider((() => {
			throw new Error("base hors ligne");
		}) as never);

		const stub = stubFetch();
		const data = createAzaleeData({ baseUrl: REMOTE, fetch: stub.fetch, attempts: 1 });

		expect(data.source).toBe("local");
		expect(data.isDegraded()).toBe(false);

		await expect(data.teams()).resolves.toEqual(REMOTE_TEAMS as never);
		expect(data.isDegraded()).toBe(true);
		expect(stub.urls).toEqual([`${REMOTE}/api/teams`]);

		// Une fois dégradée, la source reste distante : plus d'aller-retour local.
		await data.teams();
		expect(stub.urls).toHaveLength(2);
	});

	test("fallbackOnError: false — l'échec local remonte tel quel", async () => {
		setDatabaseProvider((() => {
			throw new Error("base hors ligne");
		}) as never);

		const stub = stubFetch();
		const data = createAzaleeData({
			mode: "local",
			baseUrl: REMOTE,
			fetch: stub.fetch,
			fallbackOnError: false,
			attempts: 1,
		});

		await expect(data.teams()).rejects.toThrow("base hors ligne");
		expect(stub.urls).toHaveLength(0);
	});
});
