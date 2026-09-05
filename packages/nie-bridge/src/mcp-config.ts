/**
 * Description de l'entrée de configuration du serveur MCP `niers-game`.
 *
 * Partagée entre `nie-explorer` (qui l'écrit dans la config d'un client MCP depuis ses
 * Paramètres) et `nie-mcp` (qui s'en sert pour se diagnostiquer) : une seule définition de
 * la commande de lancement, au lieu d'un `.mcp.json` recopié à la main dans trois README.
 */

/** Nom sous lequel le serveur apparaît chez les clients MCP. */
export const MCP_SERVER_NAME = "niers-game";

/** Point d'entrée du serveur, relatif à la racine du repo. */
export const MCP_ENTRYPOINT = "apps/nie-mcp/src/index.ts";

/** Entrée `mcpServers[...]` telle qu'attendue par Claude Code et Claude Desktop. */
export interface McpServerEntry {
  type: "stdio";
  command: string;
  args: string[];
  env: Record<string, string>;
}

/** Options de génération. */
export interface McpEntryOptions {
  /**
   * Racine du repo niers. Requise pour Claude Desktop, qui lance le serveur depuis un
   * répertoire courant arbitraire ; laisser vide pour Claude Code, dont le `.mcp.json` de
   * projet s'exécute déjà à la racine.
   */
  repoRoot?: string | undefined;
  /** Dossier du jeu, si différent de la racine du repo. */
  gameDir?: string | undefined;
  /** URL de l'API `nie-site` à transmettre au serveur MCP. */
  aphrodyApiUrl?: string | undefined;
}

/**
 * Construit l'entrée de configuration du serveur.
 *
 * Sans `repoRoot`, le chemin reste relatif — c'est la forme versionnée dans le `.mcp.json`
 * du repo, valable sur toutes les machines.
 */
export function mcpServerEntry(options: McpEntryOptions = {}): McpServerEntry {
  const root = options.repoRoot?.trim() ?? "";
  const entry = root === "" ? MCP_ENTRYPOINT : joinPath(root, MCP_ENTRYPOINT);
  const env: Record<string, string> = {};
  if (root !== "") env["NIERS_REPO"] = root;
  if (options.gameDir !== undefined && options.gameDir.trim() !== "") env["NIE_GAME_DIR"] = options.gameDir.trim();
  if (options.aphrodyApiUrl !== undefined && options.aphrodyApiUrl.trim() !== "") {
    env["NIE_APHRODY_API_URL"] = options.aphrodyApiUrl.trim().replace(/\/+$/, "");
  }
  return { type: "stdio", command: "bun", args: ["run", entry], env };
}

/** Objet complet `{ mcpServers: { "niers-game": … } }`, à fusionner dans une config existante. */
export function mcpConfigFragment(options: McpEntryOptions = {}): {
  mcpServers: Record<string, McpServerEntry>;
} {
  return { mcpServers: { [MCP_SERVER_NAME]: mcpServerEntry(options) } };
}

/**
 * Concatène deux segments de chemin en conservant le séparateur de la base.
 *
 * `replace(/\//g, …)` plutôt que `replaceAll` : ce module est aussi compilé par
 * `nie-explorer`, dont la cible TypeScript est ES2020 — `replaceAll` est ES2021.
 */
function joinPath(base: string, rest: string): string {
  const windows = base.includes("\\");
  const trimmed = base.replace(/[\\/]+$/, "");
  const tail = windows ? rest.replace(/\//g, "\\") : rest;
  return `${trimmed}${windows ? "\\" : "/"}${tail}`;
}
