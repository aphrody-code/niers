/**
 * Résolution des artefacts runtime (`src/config.ts`).
 *
 * Ces tests s'appuient sur les VRAIS artefacts présents sur la machine
 * (`apps/azalee/data/`). Chaque test restaure la configuration explicite et les
 * variables d'environnement touchées : le miroir SQLite est un singleton
 * process-local, une fuite de configuration casserait les fichiers suivants.
 */

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import {
	configureAzalee,
	getAzaleeConfig,
	getCacheDir,
	resetAzaleeConfig,
	resolveDataDir,
	resolveDataFile,
	resolveMirrorPath,
} from "@rosegriffon/azalee/config";

/** Sauvegarde/restauration des variables d'environnement lues par le module. */
const ENV_KEYS = ["AZALEE_DATA_DIR", "AZALEE_CACHE_DIR", "SQLITE_DB_PATH"] as const;
let savedEnv: Record<string, string | undefined> = {};

beforeEach(() => {
	savedEnv = Object.fromEntries(ENV_KEYS.map((k) => [k, process.env[k]]));
	resetAzaleeConfig();
});

afterEach(() => {
	for (const key of ENV_KEYS) {
		const value = savedEnv[key];
		if (value === undefined) delete process.env[key];
		else process.env[key] = value;
	}
	resetAzaleeConfig();
});

const dataDir = resolveDataDir();
const hasData = dataDir !== null;

describe("resolveDataDir — découverte du dossier d'artefacts", () => {
	test.skipIf(!hasData)("trouve un dossier réel porteur d'au moins un marqueur", () => {
		expect(dataDir).toBeString();
		expect(path.isAbsolute(dataDir as string)).toBe(true);
		expect(existsSync(dataDir as string)).toBe(true);
		// Au moins un des marqueurs Azalée doit exister — sinon c'est un `data/`
		// homonyme (dumps Postgres à la racine du monorepo) qu'on doit rejeter.
		const markers = [
			"cpk-index.ndjson.gz",
			"game-text-names.ndjson.gz",
			"backups/mirror.sqlite",
			"schema-snapshot",
		];
		expect(markers.some((m) => existsSync(path.join(dataDir as string, m)))).toBe(true);
	});

	test("un dossier sans marqueur Azalée n'est JAMAIS retenu", () => {
		// `/tmp` existe toujours mais ne contient aucun marqueur → la résolution
		// doit l'ignorer et retomber sur les candidats conventionnels.
		configureAzalee({ dataDir: tmpdir() });
		expect(resolveDataDir()).not.toBe(path.resolve(tmpdir()));
	});
});

describe("configureAzalee / getAzaleeConfig / resetAzaleeConfig", () => {
	test("la configuration explicite est absolutisée et fusionnée", () => {
		configureAzalee({ dataDir: "." });
		expect(getAzaleeConfig().dataDir).toBe(path.resolve("."));
		expect(getAzaleeConfig().mirrorPath).toBeUndefined();

		configureAzalee({ cacheDir: "./cache-x" });
		// La 2e passe ne doit PAS effacer la 1re (fusion, pas remplacement).
		expect(getAzaleeConfig().dataDir).toBe(path.resolve("."));
		expect(getAzaleeConfig().cacheDir).toBe(path.resolve("./cache-x"));
	});

	test("resetAzaleeConfig remet les trois clés à zéro", () => {
		configureAzalee({ cacheDir: "/x", dataDir: "/a", mirrorPath: "/b" });
		expect(Object.keys(getAzaleeConfig())).toHaveLength(3);
		resetAzaleeConfig();
		expect(getAzaleeConfig().dataDir).toBeUndefined();
		expect(getAzaleeConfig().mirrorPath).toBeUndefined();
		expect(getAzaleeConfig().cacheDir).toBeUndefined();
	});

	test.skipIf(!hasData)("AZALEE_DATA_DIR est prioritaire sur les chemins conventionnels", () => {
		// On repointe l'env sur le VRAI dossier : la valeur doit être retenue telle
		// quelle (et non un autre candidat), preuve que l'env passe avant le cwd.
		process.env.AZALEE_DATA_DIR = dataDir as string;
		expect(resolveDataDir()).toBe(dataDir as string);
		// Un env pointant sur un dossier sans marqueur est ignoré (pas de source
		// silencieuse) — on retombe sur la découverte conventionnelle.
		process.env.AZALEE_DATA_DIR = tmpdir();
		expect(resolveDataDir()).toBe(dataDir as string);
	});
});

describe("resolveDataFile — localisation d'un artefact nommé", () => {
	test.skipIf(!hasData)("trouve l'index CPK et l'index de texte", () => {
		const cpk = resolveDataFile("cpk-index.ndjson.gz");
		const names = resolveDataFile("game-text-names.ndjson.gz");
		// Au moins un des deux artefacts doit exister sur une machine de dev.
		expect(cpk !== null || names !== null).toBe(true);
		for (const found of [cpk, names]) {
			if (found === null) continue;
			expect(existsSync(found)).toBe(true);
			expect(path.dirname(found)).toBe(dataDir as string);
		}
	});

	test("renvoie null pour un artefact inexistant", () => {
		expect(resolveDataFile("ce-fichier-nexiste-pas.ndjson.gz")).toBeNull();
	});
});

describe("resolveMirrorPath — miroir SQLite des tables inagle_*", () => {
	test.skipIf(!hasData)("résout un fichier SQLite réel", () => {
		const mirror = resolveMirrorPath();
		expect(mirror).toBeString();
		expect(existsSync(mirror as string)).toBe(true);
		expect(mirror as string).toMatch(/\.sqlite$/);
	});

	test("mirrorPath explicite l'emporte sur tout le reste", () => {
		process.env.SQLITE_DB_PATH = "/tmp/depuis-env.sqlite";
		configureAzalee({ mirrorPath: "/tmp/explicite.sqlite" });
		expect(resolveMirrorPath()).toBe("/tmp/explicite.sqlite");
	});

	test("SQLITE_DB_PATH l'emporte sur la découverte conventionnelle", () => {
		process.env.SQLITE_DB_PATH = "relatif/mirror.sqlite";
		// Absolutisé par rapport au cwd (jamais renvoyé relatif).
		expect(resolveMirrorPath()).toBe(path.resolve("relatif/mirror.sqlite"));
	});
});

describe("getCacheDir — dossier de matérialisation", () => {
	test("défaut = os.tmpdir(), sous-dossier joint", () => {
		delete process.env.AZALEE_CACHE_DIR;
		expect(getCacheDir()).toBe(tmpdir());
		expect(getCacheDir("azalee-cpk")).toBe(path.join(tmpdir(), "azalee-cpk"));
	});

	test("AZALEE_CACHE_DIR puis cacheDir explicite prennent la main", () => {
		process.env.AZALEE_CACHE_DIR = "/tmp/depuis-env-cache";
		expect(getCacheDir("x")).toBe("/tmp/depuis-env-cache/x");
		configureAzalee({ cacheDir: "/tmp/explicite-cache" });
		expect(getCacheDir("x")).toBe("/tmp/explicite-cache/x");
	});
});
