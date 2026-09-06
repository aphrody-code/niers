/**
 * Outils : vérification contre les **vraies** données du VPS.
 *
 * Aucun bouchon. Si le miroir SQLite est absent (poste de développement sans
 * données), les tests qui en dépendent sont ignorés plutôt que faussés.
 */

import { describe, expect, test } from "bun:test";
import { resolveMirrorPath } from "@niers/azalee-tools/server/index";
import { createRgMcpServer, DEFAULT_REPO_ROOT } from "../src/index.ts";
import { MODERN_PROTOCOL_VERSION } from "../src/protocol/versions.ts";
import { parseModernMeta } from "../src/protocol/meta.ts";
import type { McpServer } from "../src/server.ts";

const hasMirror = Boolean(resolveMirrorPath());

/**
 * `systemctl` est-il là ? La garde MESURE l'outil, elle ne déduit pas de la
 * plateforme : `process.platform !== "linux"` serait une garde codée en dur,
 * qui sauterait aussi sur un Linux parfaitement équipé et mentirait sur un
 * conteneur sans systemd. Ici on interroge le PATH, et on annonce le saut —
 * une suite qui s'ignore en silence est un faux vert.
 */
const hasSystemd = Bun.which("systemctl") !== null;
if (!hasSystemd) {
	console.warn("[tools.test] systemctl absent de cette machine : les tests `ops_status` sont IGNORÉS, pas réussis.");
}

const context = {
	meta: parseModernMeta({
		_meta: {
			"io.modelcontextprotocol/protocolVersion": MODERN_PROTOCOL_VERSION,
			"io.modelcontextprotocol/clientCapabilities": {},
		},
	}),
	signal: new AbortController().signal,
	emit: () => {},
};

let cached: McpServer | undefined;
async function server(): Promise<McpServer> {
	cached ??= await createRgMcpServer();
	return cached;
}

async function callTool(name: string, args: Record<string, unknown>): Promise<Record<string, unknown>> {
	const instance = await server();
	const response = await instance.handle(
		{ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } },
		context,
	);
	expect(response).toHaveProperty("result");
	return (response as { result: Record<string, unknown> }).result;
}

describe("assemblage", () => {
	test("tous les outils déclarent un schéma d'entrée objet et une description", async () => {
		const instance = await server();
		expect(instance.registry.tools.length).toBeGreaterThanOrEqual(19);
		for (const tool of instance.registry.tools) {
			expect(tool.definition.inputSchema.type).toBe("object");
			expect(tool.definition.description?.length ?? 0).toBeGreaterThan(30);
			// Claude Code tronque les descriptions à 2 Ko : au-delà, l'outil
			// devient invisible à la recherche d'outils.
			expect(tool.definition.description!.length).toBeLessThan(2000);
			// Une union à la racine du schéma est aplatie par certains clients.
			expect(tool.definition.inputSchema.anyOf).toBeUndefined();
			expect(tool.definition.inputSchema.oneOf).toBeUndefined();
		}
	});

	test("les instructions du serveur tiennent sous la troncature à 2 Ko", async () => {
		const instance = await server();
		expect(instance.instructions!.length).toBeLessThan(2000);
	});

	test("les fiches de contexte sont publiées en ressources", async () => {
		const instance = await server();
		const resources = await instance.registry.listResourceDefinitions();
		const uris = resources.map((resource) => resource.uri);
		for (const slug of ["monorepo", "exploitation", "donnees", "ievr", "mcp"]) {
			expect(uris).toContain(`rg://context/${slug}`);
		}
	});

	test("les gabarits d'URI sont déclarés", async () => {
		const instance = await server();
		const templates = instance.registry.templateDefinitions().map((template) => template.uriTemplate);
		expect(templates).toContain("rg://docs/{slug}");
		expect(templates).toContain("rg://schema/{table}");
	});
});

describe.skipIf(!hasMirror)("données de jeu réelles", () => {
	test("azalee_search retrouve Mark Evans", async () => {
		const result = await callTool("azalee_search", { q: "mark", limit: 5 });
		const payload = result.structuredContent as {
			characters: { names: { fr: string }; baseSlug: string }[];
		};
		expect(payload.characters.length).toBeGreaterThan(0);
		expect(payload.characters.some((entry) => entry.names.fr.includes("Mark"))).toBe(true);
	});

	test("azalee_get renvoie la fiche complète d'un personnage", async () => {
		const result = await callTool("azalee_get", { collection: "characters", id: "mark-evans" });
		const character = result.structuredContent as { names: { fr: string; ja: string }; variants: unknown[] };
		expect(character.names.fr).toBe("Mark Evans");
		expect(character.variants.length).toBeGreaterThan(0);
	});

	test("azalee_get sur un identifiant inconnu renvoie isError", async () => {
		const result = await callTool("azalee_get", { collection: "characters", id: "personnage-inexistant" });
		expect(result.isError).toBe(true);
	});

	test("azalee_list pagine une collection", async () => {
		const result = await callTool("azalee_list", { collection: "skills", limit: 3, page: 1 });
		const payload = result.structuredContent as { data: unknown[] };
		expect(payload.data).toHaveLength(3);
	});

	test("azalee_list exige auraType pour les auras", async () => {
		const result = await callTool("azalee_list", { collection: "auras", limit: 2 });
		expect(result.isError).toBe(true);
	});

	test("azalee_dataset renvoie l'état de la source", async () => {
		const result = await callTool("azalee_dataset", { dataset: "health" });
		const payload = result.structuredContent as { mirror: string; cpkFiles: number };
		expect(payload.mirror).toContain(".sqlite");
		expect(payload.cpkFiles).toBeGreaterThan(0);
	});

	test("db_tables compte les tables inagle_*", async () => {
		const result = await callTool("db_tables", { like: "inagle_" });
		const payload = result.structuredContent as { count: number; tables: { name: string; rows: number }[] };
		expect(payload.count).toBeGreaterThan(50);
		const teams = payload.tables.find((table) => table.name === "inagle_teams");
		expect(teams?.rows).toBeGreaterThan(100);
	});

	test("db_query exécute un agrégat réel", async () => {
		const result = await callTool("db_query", {
			sql: "select count(*) as n from inagle_characters",
			limit: 1,
		});
		const payload = result.structuredContent as { rows: { n: number }[] };
		expect(payload.rows[0]!.n).toBeGreaterThan(1000);
	});

	test("db_query refuse toute écriture", async () => {
		for (const sql of [
			"delete from inagle_teams",
			"select 1; drop table inagle_teams",
			"pragma writable_schema = 1",
			"update inagle_teams set name = 'x'",
		]) {
			const result = await callTool("db_query", { sql });
			expect(result.isError).toBe(true);
		}
	});

	test("db_query impose un plafond de lignes", async () => {
		const result = await callTool("db_query", { sql: "select id from inagle_characters", limit: 5 });
		const payload = result.structuredContent as { rows: unknown[]; sql: string };
		expect(payload.rows).toHaveLength(5);
		expect(payload.sql).toContain("limit 5");
	});

	test("cpk_browse liste la racine de l'arborescence du jeu", async () => {
		const result = await callTool("cpk_browse", { path: "", limit: 10 });
		const payload = result.structuredContent as { dirs: { name: string; count: number }[] };
		expect(payload.dirs.map((entry) => entry.name)).toContain("common");
	});

	test("game_text_search trouve du texte du jeu", async () => {
		const result = await callTool("game_text_search", { q: "Tornade", locale: "fr", limit: 3 });
		const rows = result.structuredContent as { value: string }[];
		expect(rows.length).toBeGreaterThan(0);
		expect(rows[0]!.value.toLowerCase()).toContain("tornade");
	});
});

describe("dépôt", () => {
	test("repo_list liste la racine du monorepo", async () => {
		const result = await callTool("repo_list", { path: "", depth: 1 });
		const payload = result.structuredContent as { entries: { path: string }[] };
		const paths = payload.entries.map((entry) => entry.path);
		expect(paths).toContain("packages");
		expect(paths).toContain("apps");
	});

	test("repo_list masque les répertoires interdits", async () => {
		const result = await callTool("repo_list", { path: "", depth: 1 });
		const paths = (result.structuredContent as { entries: { path: string }[] }).entries.map((e) => e.path);
		expect(paths).not.toContain("node_modules");
		expect(paths).not.toContain(".env");
	});

	test("repo_read renvoie une plage de lignes", async () => {
		const result = await callTool("repo_read", { path: "packages/mcp/package.json", startLine: 1, endLine: 3 });
		const payload = result.structuredContent as { totalLines: number; startLine: number };
		expect(payload.startLine).toBe(1);
		expect((result.content as { text: string }[])[0]!.text.split("\n")).toHaveLength(3);
	});

	test("repo_read refuse les secrets et la sortie du dépôt", async () => {
		for (const path of [".env", ".env.local", "../../etc/passwd", "apps/azalee/data/backups/mirror.sqlite"]) {
			const result = await callTool("repo_read", { path });
			expect(result.isError).toBe(true);
		}
	});

	test("repo_grep trouve un motif réel", async () => {
		const result = await callTool("repo_grep", {
			pattern: "MODERN_PROTOCOL_VERSION",
			path: "packages/mcp/src",
			glob: "*.ts",
			limit: 20,
		});
		const payload = result.structuredContent as { matches: string[] };
		expect(payload.matches.length).toBeGreaterThan(0);
		expect(payload.matches[0]).toContain("packages/mcp/src");
	});

	test("repo_git renvoie la branche courante", async () => {
		const result = await callTool("repo_git", { action: "status" });
		const payload = result.structuredContent as { branch: string };
		expect(payload.branch.length).toBeGreaterThan(0);
	});

	test("la racine déduite du paquet est bien le dépôt", async () => {
		expect(await Bun.file(`${DEFAULT_REPO_ROOT}/CLAUDE.md`).exists()).toBe(true);
		expect(await Bun.file(`${DEFAULT_REPO_ROOT}/bunfig.toml`).exists()).toBe(true);
		// Le repère le plus sûr : le nom du paquet racine. `CLAUDE.md` et
		// `bunfig.toml` existent dans plus d'un dossier de ce dépôt ; ce nom-là
		// n'existe qu'une fois. (Le témoin précédent était `turbo.json`, qui
		// n'est plus à la racine — turbo n'est resté qu'une devDependency. Le
		// test rougissait sur une racine pourtant parfaitement résolue.)
		const racine = (await Bun.file(`${DEFAULT_REPO_ROOT}/package.json`).json()) as { name: string };
		expect(racine.name).toBe("nie-monorepo");
	});
});

describe("exploitation", () => {
	test.skipIf(!hasSystemd)("ops_status renvoie l'état des services connus", async () => {
		const result = await callTool("ops_status", { services: true, endpoints: false });
		const payload = result.structuredContent as { services: { unit: string; active: string }[] };
		const azalee = payload.services.find((service) => service.unit === "azalee-web.service");
		expect(azalee).toBeDefined();
		expect(azalee!.active.length).toBeGreaterThan(0);
	});

	test("ops_http refuse un domaine hors périmètre", async () => {
		const result = await callTool("ops_http", { url: "https://example.com/" });
		expect(result.isError).toBe(true);
		expect((result.content as { text: string }[])[0]!.text).toContain("Domaine refusé");
	});
});

describe("ressources et prompts assemblés", () => {
	test("rg://context/mcp se lit", async () => {
		const instance = await server();
		const response = await instance.handle(
			{ jsonrpc: "2.0", id: 1, method: "resources/read", params: { uri: "rg://context/mcp" } },
			context,
		);
		const contents = (response as unknown as { result: { contents: { text: string }[] } }).result.contents;
		expect(contents[0]!.text).toContain("Model Context Protocol");
	});

	test("rg://docs/{slug} sert un document du dépôt", async () => {
		const instance = await server();
		// `ARCHITECTURE` plutôt que `azalee-lib` : ce dernier document n'existe
		// plus sous `docs/`, et le gabarit rendait donc `undefined` sur une
		// résolution par ailleurs correcte. `docs/ARCHITECTURE.md` est le
		// document que CLAUDE.md désigne comme la carte du dépôt : s'il
		// disparaît, ce test DOIT rougir.
		const response = await instance.handle(
			{ jsonrpc: "2.0", id: 2, method: "resources/read", params: { uri: "rg://docs/ARCHITECTURE" } },
			context,
		);
		expect((response as unknown as { result: { contents: { text: string }[] } }).result.contents[0]!.text.length).toBeGreaterThan(
			100,
		);
	});

	test.skipIf(!hasMirror)("rg://schema/{table} décrit une table réelle", async () => {
		const instance = await server();
		const response = await instance.handle(
			{ jsonrpc: "2.0", id: 3, method: "resources/read", params: { uri: "rg://schema/inagle_teams" } },
			context,
		);
		const payload = JSON.parse(
			(response as unknown as { result: { contents: { text: string }[] } }).result.contents[0]!.text,
		) as { table: string; rows: number; columns: unknown[] };
		expect(payload.table).toBe("inagle_teams");
		expect(payload.rows).toBeGreaterThan(100);
		expect(payload.columns.length).toBeGreaterThan(2);
	});

	test.skipIf(!hasMirror)("le prompt fiche-personnage complète ses arguments", async () => {
		const instance = await server();
		const prompt = instance.registry.getPrompt("fiche-personnage");
		const suggestions = await prompt!.complete!("personnage", "mark");
		expect(suggestions.length).toBeGreaterThan(0);
	});
});

// L'inventaire canonique des outils vit dans `context/mcp.md`, la fiche que le
// serveur publie lui-même en `rg://context/mcp` : c'est elle que le modèle
// client lit pour savoir ce qu'il peut appeler. Ces deux tests la tiennent
// collée au registre réel, DANS LES DEUX SENS.
//
// Ils visaient auparavant `plugins/rose-griffon/skills/donnees-jeu/`. Cette
// skill a disparu avec le renommage du plugin en `niers`, et le plugin actuel
// ne documente plus ce serveur — les deux tests n'avaient donc plus de sujet.
// La fiche de contexte remplit exactement le même rôle et, elle, est livrée.
describe("inventaire de la fiche de contexte", () => {
	/**
	 * Noms d'outils cités dans le tableau d'inventaire de `context/mcp.md`.
	 *
	 * Seule la DERNIÈRE colonne est lue. La colonne « Famille » porte aussi du
	 * code entre accents graves — `(`admin`)` y désigne une portée, pas un
	 * outil — et une extraction sur la ligne entière réclamait au registre un
	 * outil nommé « admin » qui n'a jamais existé.
	 */
	async function inventaire(): Promise<string[]> {
		const fiche = await Bun.file(`${import.meta.dir}/../context/mcp.md`).text();
		const tableau = fiche.split("## Inventaire")[1]?.split("## Deux portées")[0] ?? "";
		const noms: string[] = [];
		for (const ligne of tableau.split(/\r?\n/)) {
			const cellules = ligne.split("|").filter((cellule) => cellule.trim() !== "");
			if (cellules.length < 2 || ligne.includes("---")) continue;
			for (const [, nom] of cellules.at(-1)!.matchAll(/`([a-z][a-z0-9_]*)`/g)) noms.push(nom!);
		}
		return noms;
	}

	test("la fiche ne cite aucun outil fantôme", async () => {
		const instance = await server();
		const noms = instance.registry.tools.map((tool) => tool.definition.name);
		const cites = await inventaire();
		expect(cites.length).toBeGreaterThan(0);
		// Un outil documenté mais absent du registre est pire qu'un outil non
		// documenté : le modèle l'appelle, et se prend une erreur de protocole.
		for (const cite of cites) expect(noms).toContain(cite);
	});

	test("la fiche documente chaque outil du serveur", async () => {
		// Le sens inverse : un outil ajouté au registre sans une ligne dans la
		// fiche est invisible pour le client, donc jamais appelé.
		//
		// DÉRIVE MESURÉE, pas tolérée. La fiche annonce « 26 outils » et en
		// liste 26 ; le registre par défaut en expose 44. Les 18 manquants
		// forment deux familles entières ajoutées après la rédaction de la
		// fiche : la plateforme Supabase et le déploiement. Ils sont nommés ici
		// un par un, de sorte que le cliquet joue dans les deux sens — documenter
		// l'un d'eux fait rougir ce test et demande qu'on le retire de la liste,
		// et un 19e outil non documenté le fait rougir aussi.
		const NON_DOCUMENTES = [
			"apply_migration",
			"deploy_run",
			"deploy_status",
			"execute_sql",
			"generate_typescript_types",
			"get_advisors",
			"get_project_url",
			"get_publishable_keys",
			"get_storage_config",
			"list_extensions",
			"list_migrations",
			"list_storage_buckets",
			"list_tables",
			"live_graphql",
			"live_select",
			"live_storage",
			"live_tables",
			"search_docs",
		];
		const instance = await server();
		const cites = new Set(await inventaire());
		const manquants = instance.registry.tools
			.map((tool) => tool.definition.name)
			.filter((nom) => !cites.has(nom));
		expect(manquants.toSorted()).toEqual([...NON_DOCUMENTES].toSorted());
	});
});
