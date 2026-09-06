/**
 * Ressources MCP : le contexte que le serveur met à disposition en lecture.
 *
 * Trois familles, toutes en URI `rg://` :
 *
 * - `rg://context/<nom>`  fiches d'orientation écrites pour un agent
 *   (architecture, données, exploitation, glossaire du jeu) ;
 * - `rg://docs/<slug>`    la documentation versionnée du dépôt (`docs/**.md`) ;
 * - `rg://schema/<table>` le schéma réel d'une table du miroir SQLite.
 *
 * Une ressource est *lue à la demande* par le client : c'est le bon canal pour
 * du contexte volumineux qu'il serait absurde de charger dans chaque prompt.
 */

import { Database } from "bun:sqlite";
import { resolveMirrorPath } from "@niers/azalee-tools/server/index";
import type { ResourceContents } from "./protocol/types.ts";
import type { ResourceDefinition, ResourceSpec, ResourceTemplateSpec } from "./registry.ts";

export interface ResourceOptions {
	/** Racine du dépôt (pour `docs/**`). */
	repoRoot: string;
	/** Répertoire des fiches de contexte livrées avec le paquet. */
	contextDir: string;
}

const MARKDOWN = "text/markdown";
const JSON_TYPE = "application/json";

async function readMarkdown(path: string, uri: string): Promise<ResourceContents | undefined> {
	const file = Bun.file(path);
	if (!(await file.exists())) return undefined;
	return { uri, mimeType: MARKDOWN, text: await file.text() };
}

/** Titre = premier titre de niveau 1, sinon le nom de fichier. */
function titleOf(markdown: string, fallback: string): string {
	const matched = /^#\s+(.+)$/m.exec(markdown);
	return matched?.[1]?.trim() ?? fallback;
}

async function listMarkdown(root: string, subdir: string): Promise<string[]> {
	const glob = new Bun.Glob("**/*.md");
	const found: string[] = [];
	try {
		for await (const relative of glob.scan({ cwd: `${root}/${subdir}`, onlyFiles: true })) {
			found.push(relative);
		}
	} catch {
		return [];
	}
	return found.sort();
}

export async function buildResources(
	options: ResourceOptions,
): Promise<{ resources: ResourceSpec[]; templates: ResourceTemplateSpec[] }> {
	const resources: ResourceSpec[] = [];

	// ── Fiches de contexte livrées avec le paquet ────────────────────────
	const contextFiles = await listMarkdown(options.contextDir, ".");
	for (const relative of contextFiles) {
		const slug = relative.replace(/\.md$/, "");
		const absolute = `${options.contextDir}/${relative}`;
		const head = await Bun.file(absolute).text();
		resources.push({
			uri: `rg://context/${slug}`,
			name: slug,
			title: titleOf(head, slug),
			description: firstParagraph(head),
			mimeType: MARKDOWN,
			read: async () => (await readMarkdown(absolute, `rg://context/${slug}`))!,
		});
	}

	// ── Documentation versionnée du dépôt ────────────────────────────────
	const docs = await listMarkdown(options.repoRoot, "docs");
	const docTemplate: ResourceTemplateSpec = {
		uriTemplate: "rg://docs/{slug}",
		name: "docs",
		title: "Documentation du monorepo",
		description:
			"Un document de `docs/**.md` du dépôt Rose Griffon : architecture, décisions techniques, procédures de déploiement, format des données du jeu.",
		mimeType: MARKDOWN,
		read: async (uri, variables) => {
			const slug = variables.slug ?? "";
			if (slug.includes("..")) return undefined;
			return await readMarkdown(`${options.repoRoot}/docs/${slug}.md`, uri);
		},
		complete: (variable, value) =>
			variable === "slug"
				? docs.map((relative) => relative.replace(/\.md$/, "")).filter((slug) => slug.startsWith(value))
				: [],
		list: () =>
			docs.map<ResourceDefinition>((relative) => {
				const slug = relative.replace(/\.md$/, "");
				return {
					uri: `rg://docs/${slug}`,
					name: slug,
					title: slug,
					mimeType: MARKDOWN,
				};
			}),
	};

	// ── Schéma des tables du miroir ──────────────────────────────────────
	const tables = listMirrorTables();
	const schemaTemplate: ResourceTemplateSpec = {
		uriTemplate: "rg://schema/{table}",
		name: "schema",
		title: "Schéma d'une table du miroir",
		description:
			"Colonnes, types, index et nombre de lignes d'une table `inagle_*` du miroir SQLite des données de jeu.",
		mimeType: JSON_TYPE,
		read: (uri, variables) => {
			const table = variables.table ?? "";
			if (!tables.includes(table)) return undefined;
			const path = resolveMirrorPath();
			if (!path) return undefined;
			const db = new Database(path, { readonly: true });
			try {
				const payload = {
					table,
					rows: (db.query(`select count(*) as n from "${table}"`).get() as { n: number }).n,
					columns: db.query(`pragma table_info("${table}")`).all(),
					indexes: db.query(`pragma index_list("${table}")`).all(),
				};
				return { uri, mimeType: JSON_TYPE, text: JSON.stringify(payload, null, 2) };
			} finally {
				db.close();
			}
		},
		complete: (variable, value) =>
			variable === "table" ? tables.filter((table) => table.includes(value)) : [],
		list: () =>
			tables.map<ResourceDefinition>((table) => ({
				uri: `rg://schema/${table}`,
				name: table,
				title: `Schéma ${table}`,
				mimeType: JSON_TYPE,
			})),
	};

	// ── Le README du dépôt, utile comme point d'entrée ───────────────────
	resources.push({
		uri: "rg://repo/readme",
		name: "readme",
		title: "README du monorepo",
		description: "Présentation, pile technique et workspaces du monorepo Rose Griffon.",
		mimeType: MARKDOWN,
		read: async () => (await readMarkdown(`${options.repoRoot}/README.md`, "rg://repo/readme")) ?? {
			uri: "rg://repo/readme",
			text: "README indisponible.",
		},
	});

	return { resources, templates: [docTemplate, schemaTemplate] };
}

function firstParagraph(markdown: string): string {
	const body = markdown.replace(/^#.*$/m, "").trim();
	const paragraph = body.split("\n\n")[0] ?? "";
	const flat = paragraph.replaceAll("\n", " ").trim();
	return flat.length > 300 ? `${flat.slice(0, 297)}…` : flat;
}

function listMirrorTables(): string[] {
	const path = resolveMirrorPath();
	if (!path) return [];
	try {
		const db = new Database(path, { readonly: true });
		try {
			return db
				.query<{ name: string }, []>("select name from sqlite_master where type = 'table' order by name")
				.all()
				.map((row) => row.name)
				.filter((name) => !name.startsWith("sqlite_"));
		} finally {
			db.close();
		}
	} catch {
		return [];
	}
}
