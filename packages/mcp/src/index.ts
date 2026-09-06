/**
 * `@rosegriffon/mcp` — serveur MCP du monorepo Rose Griffon.
 *
 * Assemble le registre (outils, ressources, prompts) et le noyau protocole.
 * Les transports se choisissent à l'appel :
 *
 * ```ts
 * const server = await createRgMcpServer();
 * createHttpTransport({ server, port: 8808 });   // Streamable HTTP
 * await runStdioTransport({ server });           // stdio
 * ```
 */

import { fileURLToPath } from "node:url";
import { McpRegistry } from "./registry.ts";
import { buildPrompts } from "./prompts.ts";
import { buildResources } from "./resources.ts";
import { McpServer } from "./server.ts";
import { adminTools } from "./tools/admin.ts";
import { azaleeTools } from "./tools/azalee.ts";
import { dbTools } from "./tools/db.ts";
import { deployTools } from "./tools/deploy.ts";
import { opsTools } from "./tools/ops.ts";
import { supabasePlatformTools } from "./tools/supabase-platform.ts";
import { supabaseTools } from "./tools/supabase.ts";
import { toPosixPath } from "./tools/paths.ts";
import { repoTools } from "./tools/repo.ts";

export interface RgMcpServerOptions {
	/** Racine du monorepo. Déduite du paquet par défaut. */
	repoRoot?: string;
	/** Répertoire des fiches de contexte. `<paquet>/context` par défaut. */
	contextDir?: string;
	/** Outils de données de jeu (wiki, CPK, texte, RAG). */
	azalee?: boolean;
	/** SQL en lecture seule sur le miroir. */
	db?: boolean;
	/** Outils de lecture LIVE (REST, GraphQL, stockage) sur la pile self-host. */
	live?: boolean;
	/** Outils aux noms du serveur MCP officiel Supabase (SQL, audit, types, stockage). */
	platform?: boolean;
	/** Lecture du dépôt (fichiers, grep, git). */
	repo?: boolean;
	/** État de la production (systemd, HTTP, journaux). */
	ops?: boolean;
	/** Publication bleu/vert (état en lecture, lancement en portée admin). */
	deploy?: boolean;
	/**
	 * Outils d'administration (écriture, suppression, exécution). Déclarés en
	 * portée `admin` : ils restent invisibles pour une connexion en lecture
	 * seule, c'est le jeton présenté qui décide.
	 */
	admin?: boolean;
	version?: string;
}

/**
 * Racine du dépôt telle que déduite de l'emplacement du paquet.
 *
 * `URL.pathname` n'est PAS un chemin de fichier : sous Windows il rend
 * `/C:/Users/…`, que ni `Bun.file` ni `readdir` n'ouvrent — les outils dépôt
 * répondaient alors « fichier introuvable » et `repo_list` une liste vide,
 * sans que rien ne signale la cause. `fileURLToPath` rend le vrai chemin, que
 * l'on POSIXifie pour rester homogène avec la prison de `tools/paths.ts`.
 */
function racineDepuisUrl(relatif: string): string {
	return toPosixPath(fileURLToPath(new URL(relatif, import.meta.url))).replace(/\/$/, "");
}

export const DEFAULT_REPO_ROOT = racineDepuisUrl("../../..");
export const DEFAULT_CONTEXT_DIR = racineDepuisUrl("../context");

/**
 * Instructions destinées au modèle client.
 *
 * Volontairement courtes : Claude Code tronque les instructions serveur à
 * 2 Ko quand la recherche d'outils est active — l'essentiel doit tenir au
 * début.
 */
const INSTRUCTIONS = [
	"Accès aux données et à l'infrastructure du projet Rose Griffon : le wiki Azalée d'Inazuma Eleven: Victory Road (personnages, techniques, objets, équipes, quêtes, textes et fichiers extraits du jeu) et le monorepo qui le fait tourner.",
	"",
	"Données de jeu : `azalee_search` pour trouver un slug, `azalee_get` pour la fiche, `azalee_list` pour une collection filtrée, `azalee_dataset` pour les tables globales. Pour un agrégat ou une jointure, `db_tables` → `db_schema` → `db_query` (SQL en lecture seule sur le miroir).",
	"Le miroir SQLite ne couvre que les 65 tables de jeu et date du dernier dump nocturne. Pour le reste du schéma (articles, équipes, quêtes, boutiques, tweets, membres) ou pour une donnée à jour à la seconde, passer par `live_tables` → `live_select`, `live_graphql` pour des relations imbriquées, `live_storage` pour les fichiers.",
	"Fichiers du jeu : `cpk_search` / `cpk_browse` / `cpk_file`. Textes du jeu : `game_text_search`. Question en langue naturelle : `rag_search`.",
	"Monorepo : `repo_list`, `repo_read`, `repo_grep`, `repo_git`. Production : `ops_status`, `ops_logs`, `ops_http`, `deploy_status` (version servie, slots, prévisualisation).",
	"",
	"Les outils ci-dessus sont en LECTURE SEULE. Si la connexion dispose de la portée `admin` (jeton d'administration), s'ajoutent `repo_write`, `repo_edit`, `repo_delete`, `repo_move`, `shell_run`, `ops_service` et `deploy_run` (publication sans coupure) — ils modifient réellement le VPS. `access_info` indique la portée accordée.",
	"Contexte détaillé dans les ressources `rg://context/*` ; documentation du dépôt dans `rg://docs/*` ; schéma d'une table dans `rg://schema/<table>`.",
].join("\n");

export async function createRgMcpServer(options: RgMcpServerOptions = {}): Promise<McpServer> {
	const repoRoot = options.repoRoot ?? DEFAULT_REPO_ROOT;
	const contextDir = options.contextDir ?? DEFAULT_CONTEXT_DIR;
	const registry = new McpRegistry();

	if (options.azalee !== false) registry.addTools(azaleeTools());
	if (options.db !== false) registry.addTools(dbTools());
	if (options.live !== false) registry.addTools(supabaseTools());
	if (options.platform !== false) registry.addTools(supabasePlatformTools(repoRoot));
	if (options.repo !== false) registry.addTools(repoTools({ root: repoRoot }));
	if (options.ops !== false) registry.addTools(opsTools());
	if (options.deploy !== false) registry.addTools(deployTools({ root: repoRoot }));
	if (options.admin !== false) registry.addTools(adminTools({ root: repoRoot }));

	const { resources, templates } = await buildResources({ repoRoot, contextDir });
	registry.addResources(resources);
	for (const template of templates) registry.addTemplate(template);
	for (const prompt of buildPrompts()) registry.addPrompt(prompt);

	return new McpServer({
		serverInfo: { name: "rose-griffon", title: "Rose Griffon — monorepo & wiki Azalée", version: options.version ?? "1.0.0" },
		instructions: INSTRUCTIONS,
		registry,
	});
}

export { McpRegistry, defineTool, matchUriTemplate, toJsonSchema } from "./registry.ts";
export type {
	HandlerContext,
	PromptSpec,
	RegisteredTool,
	ResourceSpec,
	ResourceTemplateSpec,
	ToolSpec,
} from "./registry.ts";
export { McpServer } from "./server.ts";
export type { DispatchContext, McpServerOptions } from "./server.ts";
export { createFetchHandler, createHttpTransport } from "./transport/http.ts";
export type { HttpTransportOptions, RunningHttpTransport } from "./transport/http.ts";
export { runStdioTransport } from "./transport/stdio.ts";
export type { StdioTransportOptions } from "./transport/stdio.ts";
export * from "./protocol/index.ts";
export { KNOWN_ENDPOINTS, KNOWN_SERVICES } from "./tools/ops.ts";
