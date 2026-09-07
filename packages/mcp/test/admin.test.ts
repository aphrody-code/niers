/**
 * Portée d'accès et outils d'administration.
 *
 * Deux choses à prouver : qu'une connexion en lecture seule ne peut ni voir ni
 * appeler les outils d'écriture, et que ces outils fonctionnent réellement
 * (fichiers écrits, déplacés, supprimés) quand la portée est accordée.
 */

import { afterAll, describe, expect, test } from "bun:test";
import { ErrorCode, type JsonRpcResponse } from "../src/protocol/json-rpc.ts";
import { parseModernMeta } from "../src/protocol/meta.ts";
import { MODERN_PROTOCOL_VERSION } from "../src/protocol/versions.ts";
import { McpRegistry, type McpScope } from "../src/registry.ts";
import { McpServer } from "../src/server.ts";
import { adminTools } from "../src/tools/admin.ts";
import { normalizePath } from "../src/tools/paths.ts";
import { createFetchHandler } from "../src/transport/http.ts";

const RACINE = `${import.meta.dir}/..`;
const BAC = `${RACINE}/test/.bac-admin`;

const meta = parseModernMeta({
	_meta: {
		"io.modelcontextprotocol/protocolVersion": MODERN_PROTOCOL_VERSION,
		"io.modelcontextprotocol/clientCapabilities": {},
	},
});

function serveur(): McpServer {
	const registry = new McpRegistry();
	registry.addTools(adminTools({ root: RACINE, onAudit: () => {} }));
	return new McpServer({ serverInfo: { name: "test", version: "0.0.1" }, registry });
}

function contexte(scope: McpScope) {
	return { meta, scope, signal: new AbortController().signal, emit: () => {} };
}

async function appeler(
	instance: McpServer,
	name: string,
	args: Record<string, unknown>,
	scope: McpScope = "admin",
): Promise<Record<string, unknown>> {
	const response = await instance.handle(
		{ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } },
		contexte(scope),
	);
	expect(response).toHaveProperty("result");
	return (response as { result: Record<string, unknown> }).result;
}

afterAll(async () => {
	const { rm } = await import("node:fs/promises");
	await rm(BAC, { recursive: true, force: true }).catch(() => {});
});

describe("cloisonnement des portées", () => {
	const instance = serveur();

	test("la lecture seule ne voit que access_info", async () => {
		const response = await instance.handle({ jsonrpc: "2.0", id: 1, method: "tools/list" }, contexte("read"));
		const noms = ((response as unknown as { result: { tools: { name: string }[] } }).result.tools).map((t) => t.name);
		expect(noms).toEqual(["access_info"]);
		expect(noms).not.toContain("shell_run");
		expect(noms).not.toContain("repo_write");
	});

	test("l'administration voit tous les outils", async () => {
		const response = await instance.handle({ jsonrpc: "2.0", id: 2, method: "tools/list" }, contexte("admin"));
		const noms = ((response as unknown as { result: { tools: { name: string }[] } }).result.tools).map((t) => t.name);
		expect(noms).toContain("repo_write");
		expect(noms).toContain("repo_edit");
		expect(noms).toContain("repo_delete");
		expect(noms).toContain("repo_move");
		expect(noms).toContain("shell_run");
		expect(noms).toContain("ops_service");
	});

	test("appeler un outil admin en lecture seule est refusé", async () => {
		const response = (await instance.handle(
			{
				jsonrpc: "2.0",
				id: 3,
				method: "tools/call",
				params: { name: "shell_run", arguments: { command: "echo pwn" } },
			},
			contexte("read"),
		)) as JsonRpcResponse;
		expect(response).toHaveProperty("error");
		const error = (response as { error: { code: number; data: { requiredScope: string } } }).error;
		expect(error.code).toBe(ErrorCode.InvalidParams);
		expect(error.data.requiredScope).toBe("admin");
	});

	test("access_info annonce la portée réelle", async () => {
		const lecture = await appeler(instance, "access_info", {}, "read");
		expect((lecture.structuredContent as { scope: string; writable: boolean }).scope).toBe("read");
		expect((lecture.structuredContent as { writable: boolean }).writable).toBe(false);

		const admin = await appeler(instance, "access_info", {}, "admin");
		expect((admin.structuredContent as { writable: boolean }).writable).toBe(true);
	});
});

describe("le jeton décide de la portée (HTTP)", () => {
	const handler = createFetchHandler({
		server: serveur(),
		tokens: ["jeton-lecture"],
		adminTokens: ["jeton-admin"],
	});

	async function lister(authorization?: string): Promise<Response> {
		return await handler(
			new Request("http://mcp.test/mcp", {
				method: "POST",
				headers: {
					"content-type": "application/json",
					...(authorization ? { authorization } : {}),
				},
				body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list", params: {} }),
			}),
		);
	}

	test("sans jeton : 401", async () => {
		expect((await lister()).status).toBe(401);
	});

	test("jeton de lecture : portée read", async () => {
		const response = await lister("Bearer jeton-lecture");
		const payload = (await response.json()) as { result: { tools: { name: string }[] } };
		expect(payload.result.tools.map((t) => t.name)).toEqual(["access_info"]);
	});

	test("jeton d'administration : portée admin", async () => {
		const response = await lister("Bearer jeton-admin");
		const payload = (await response.json()) as { result: { tools: { name: string }[] } };
		expect(payload.result.tools.length).toBeGreaterThan(5);
		expect(payload.result.tools.map((t) => t.name)).toContain("shell_run");
	});

	test("un jeton inconnu ne dégrade pas en lecture : 401", async () => {
		expect((await lister("Bearer autre-chose")).status).toBe(401);
	});
});

describe("outils d'écriture", () => {
	const instance = serveur();
	const fichier = "test/.bac-admin/note.conf";

	test("repo_write crée le fichier et ses répertoires", async () => {
		const result = await appeler(instance, "repo_write", { path: fichier, content: "cle=valeur\n" });
		expect((result.structuredContent as { created: boolean }).created).toBe(true);
		expect(await Bun.file(`${RACINE}/${fichier}`).text()).toBe("cle=valeur\n");
	});

	test("repo_edit remplace une chaîne exacte", async () => {
		const result = await appeler(instance, "repo_edit", {
			path: fichier,
			oldString: "valeur",
			newString: "autre",
		});
		expect((result.structuredContent as { replaced: number }).replaced).toBe(1);
		expect(await Bun.file(`${RACINE}/${fichier}`).text()).toBe("cle=autre\n");
	});

	test("repo_edit refuse une chaîne absente ou ambiguë", async () => {
		expect((await appeler(instance, "repo_edit", { path: fichier, oldString: "zzz", newString: "x" })).isError).toBe(
			true,
		);
		await appeler(instance, "repo_write", { path: fichier, content: "a\na\n" });
		const ambigu = await appeler(instance, "repo_edit", { path: fichier, oldString: "a", newString: "b" });
		expect(ambigu.isError).toBe(true);
		const force = await appeler(instance, "repo_edit", {
			path: fichier,
			oldString: "a",
			newString: "b",
			replaceAll: true,
		});
		expect((force.structuredContent as { replaced: number }).replaced).toBe(2);
	});

	test("repo_move déplace le fichier", async () => {
		const result = await appeler(instance, "repo_move", {
			from: fichier,
			to: "test/.bac-admin/renomme.conf",
		});
		expect((result.structuredContent as { moved: boolean }).moved).toBe(true);
		expect(await Bun.file(`${RACINE}/test/.bac-admin/renomme.conf`).exists()).toBe(true);
	});

	test("repo_delete supprime un fichier puis un dossier", async () => {
		expect(
			(await appeler(instance, "repo_delete", { path: "test/.bac-admin/renomme.conf" })).structuredContent,
		).toMatchObject({ deleted: true, directory: false });

		await appeler(instance, "repo_write", { path: "test/.bac-admin/sous/x.conf", content: "x" });
		const sansRecursif = await appeler(instance, "repo_delete", { path: "test/.bac-admin/sous" });
		expect(sansRecursif.isError).toBe(true);
		const avecRecursif = await appeler(instance, "repo_delete", {
			path: "test/.bac-admin/sous",
			recursive: true,
		});
		expect((avecRecursif.structuredContent as { directory: boolean }).directory).toBe(true);
	});

	test("la prison de chemin tient même en administration", async () => {
		// `temoin.conf` est là pour que ce test prouve une ABSENCE sur toute
		// plateforme : `/etc/passwd` n'existe pas sous Windows, et le témoin
		// historique — relire ce fichier système — n'y mesurait donc rien.
		// Le témoin vise le parent du paquet, qui existe partout.
		const temoin = normalizePath(`${RACINE}/../temoin-hors-prison.conf`);
		for (const chemin of ["../../etc/passwd", "/etc/passwd", "../../../root/.ssh/id_rsa", "../temoin-hors-prison.conf"]) {
			const ecriture = await appeler(instance, "repo_write", { path: chemin, content: "compromis" });
			expect(ecriture.isError).toBe(true);
			const suppression = await appeler(instance, "repo_delete", { path: chemin });
			expect(suppression.isError).toBe(true);
		}
		// Rien n'a été écrit hors de la prison.
		expect(await Bun.file(temoin).exists()).toBe(false);
	});

	test("la racine du dépôt ne peut pas être supprimée", async () => {
		expect((await appeler(instance, "repo_delete", { path: ".", recursive: true })).isError).toBe(true);
	});
});

describe("shell_run", () => {
	const instance = serveur();

	test("exécute et renvoie code de sortie, sortie et durée", async () => {
		const result = await appeler(instance, "shell_run", { command: "echo bonjour && pwd" });
		const payload = result.structuredContent as { exitCode: number; stdout: string; ms: number };
		expect(payload.exitCode).toBe(0);
		expect(payload.stdout).toContain("bonjour");
		expect(payload.ms).toBeGreaterThanOrEqual(0);
		expect(result.isError).toBeUndefined();
	});

	test("un code de retour non nul est signalé comme erreur", async () => {
		const result = await appeler(instance, "shell_run", { command: "exit 3" });
		expect(result.isError).toBe(true);
		expect((result.structuredContent as { exitCode: number }).exitCode).toBe(3);
	});

	test("le répertoire de travail reste dans le dépôt", async () => {
		expect((await appeler(instance, "shell_run", { command: "pwd", cwd: "../../etc" })).isError).toBe(true);
	});

	test("une commande bloquée est interrompue par le délai", async () => {
		const result = await appeler(instance, "shell_run", { command: "sleep 30", timeoutMs: 1000 });
		expect(result.isError).toBe(true);
	});
});
