#!/usr/bin/env bun
/**
 * Serveur MCP `niers-game` — expose Inazuma Eleven: Victory Road (projet RE niers)
 * à un client MCP via stdio :
 *   - VFS des 250 800 fichiers CPK (Redis db3)
 *   - assets décodés (nie-model-serve : textures, cfg.bin, audio, modèles 3D)
 *   - base de connaissance reverse-engineering (SQLite var/niers.sqlite)
 *   - code/docs du repo niers
 *
 * Transport : stdio (stdout réservé au JSON-RPC ; tous les logs vont sur stderr).
 * Lancement : `bun run src/index.ts`.
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

import { EXPLORER_TABS } from "@niers/bridge";
import { config } from "./config.ts";
import { Control, launchGame } from "./control.ts";
import { VfsIndex } from "./vfs.ts";
import { KnowledgeBase } from "./kb.ts";
import { getAsset } from "./assets.ts";
import { repoRead } from "./repo.ts";
import { ToolError } from "./security.ts";

type ToolResult = { content: { type: "text"; text: string }[]; isError?: boolean };

function jsonResult(obj: unknown): ToolResult {
  return { content: [{ type: "text", text: JSON.stringify(obj, null, 2) }] };
}

function errResult(message: string): ToolResult {
  return { content: [{ type: "text", text: message }], isError: true };
}

/** Enveloppe un handler : convertit le retour en contenu MCP et n'explose jamais. */
async function safe(fn: () => Promise<unknown> | unknown): Promise<ToolResult> {
  try {
    return jsonResult(await fn());
  } catch (e) {
    if (e instanceof ToolError) return errResult(`erreur : ${e.message}`);
    return errResult(`erreur interne : ${(e as Error).message ?? String(e)}`);
  }
}

async function main(): Promise<void> {
  // --- Chargement des sources (échecs non fatals : tools concernés renvoient une erreur propre) ---
  // Voie par défaut : les CPK, via le paquet `nie` (mêmes crates Rust que nie-explorer).
  // Redis ne sert plus que de repli, pour un hôte qui a l'index mais pas les packs.
  let vfs: VfsIndex | null = null;
  try {
    vfs = VfsIndex.loadFromFfi();
    console.error(`[niers-game] index VFS chargé : ${vfs.size} fichiers (CPK via nie/FFI, lecture directe)`);
  } catch (e) {
    console.error(`[niers-game] CPK indisponibles (${(e as Error).message}) — repli Redis`);
    try {
      vfs = await VfsIndex.load();
      console.error(`[niers-game] index VFS chargé : ${vfs.size} fichiers (Redis db${config.redisDb}, chemins seuls)`);
    } catch (e2) {
      console.error(`[niers-game] AVERTISSEMENT index VFS indisponible : ${(e2 as Error).message}`);
    }
  }

  let kb: KnowledgeBase | null = null;
  try {
    kb = new KnowledgeBase();
    console.error(`[niers-game] KB SQLite ouverte : ${config.sqlitePath}`);
  } catch (e) {
    console.error(`[niers-game] AVERTISSEMENT KB indisponible : ${(e as Error).message}`);
  }

  const requireVfs = (): VfsIndex => {
    if (!vfs) throw new ToolError("index VFS indisponible (ni CPK locaux, ni Redis au démarrage)");
    return vfs;
  };
  const requireKb = (): KnowledgeBase => {
    if (!kb) throw new ToolError(`KB indisponible (${config.sqlitePath})`);
    return kb;
  };

  const server = new McpServer({ name: "niers-game", version: "0.1.0" });

  server.registerTool(
    "aphrody_api_health",
    {
      title: "Vérifier l'API Aphrody nie-site",
      description: "Teste GET /api/v1/health sur nie-site et renvoie son état réel.",
      inputSchema: {},
    },
    () =>
      safe(async () => {
        const url = `${config.aphrodyApiUrl}/api/v1/health`;
        const response = await fetch(url, { signal: AbortSignal.timeout(5_000) });
        const body = await response.text();
        if (!response.ok) throw new ToolError(`nie-site ${response.status} (${url}) : ${body.slice(0, 500)}`);
        try {
          return { url, status: response.status, health: JSON.parse(body) };
        } catch {
          throw new ToolError(`réponse non JSON de nie-site (${url})`);
        }
      }),
  );

  // ---------------------------------------------------------------- VFS ----
  server.registerTool(
    "vfs_list",
    {
      title: "Lister un dossier VFS",
      description:
        "Liste les sous-dossiers et fichiers immédiats sous un préfixe de chemin VFS (navigation arborescente des 250 800 fichiers CPK). prefix vide = racine. Renvoie directories[] + files[] (avec leur .cpk).",
      inputSchema: {
        prefix: z.string().optional().describe("préfixe de chemin, ex. 'data/dx11/chr' (vide = racine)"),
        limit: z.number().int().positive().max(5000).default(200).describe("nb max d'entrées renvoyées"),
      },
    },
    ({ prefix, limit }) => safe(() => requireVfs().list(prefix ?? "", limit)),
  );

  server.registerTool(
    "vfs_search",
    {
      title: "Rechercher dans le VFS",
      description:
        "Recherche sur les 250 800 chemins VFS. Sous-chaîne insensible à la casse, ou glob si la requête contient * ? [ ] { }. Renvoie les chemins matchés et leur .cpk.",
      inputSchema: {
        query: z.string().min(1).describe("sous-chaîne (ex. 'c01000010') ou glob (ex. 'data/dx11/chr/**/*.g4md')"),
        limit: z.number().int().positive().max(2000).default(100).describe("nb max de résultats"),
      },
    },
    ({ query, limit }) => safe(() => requireVfs().search(query, limit)),
  );

  server.registerTool(
    "vfs_stat",
    {
      title: "Métadonnées d'un chemin VFS",
      description:
        "Pour un fichier : le .cpk conteneur, l'extension, et le mode de décodage applicable (tex/cfg/audio). Pour un préfixe : indique que c'est un dossier et son nombre d'enfants.",
      inputSchema: {
        path: z.string().min(1).describe("chemin VFS complet ou préfixe de dossier"),
      },
    },
    ({ path }) => safe(() => requireVfs().stat(path)),
  );

  // -------------------------------------------------------------- Assets ----
  server.registerTool(
    "asset_get",
    {
      title: "Décoder/récupérer un asset",
      description:
        "Récupère un asset via nie-model-serve. decode=cfg -> JSON texte (cfg.bin/objbin/fxbin/mevbin). decode=tex -> PNG (texture g4tx ; passer le chemin AVEC .g4tx, la route gère la conversion). decode=audio -> WAV (hca/adx/acb). decode=model -> .glb (path = code perso ex. 'c01000010'). decode=raw -> octets bruts. Pour le binaire : renvoie taille + content-type + URL model-serve à ouvrir, et le contenu inline (base64/texte) seulement s'il tient sous maxBytes.",
      inputSchema: {
        path: z.string().min(1).describe("chemin VFS (ou code perso si decode=model)"),
        decode: z.enum(["raw", "tex", "cfg", "audio", "model"]).default("raw").describe("mode de décodage"),
        maxBytes: z
          .number()
          .int()
          .positive()
          .optional()
          .describe("seuil d'inlining (défaut 262144 ≈ 256 Ko ; au-delà, URL seule)"),
      },
    },
    ({ path, decode, maxBytes }) => safe(() => getAsset({ path, decode, maxBytes }, vfs)),
  );

  // ------------------------------------------------------------------ RE ----
  server.registerTool(
    "re_query",
    {
      title: "Requête SQL (SELECT) sur la KB RE",
      description:
        "Exécute une requête SELECT (ou WITH ... SELECT) en lecture seule sur var/niers.sqlite. Toute mutation/DDL est refusée. Tables : function, coverage, rtti_class, xref, str, pdata_func, hash_name, symbol. Les colonnes d'adresse (vaddr, from_addr, to_addr…) sont renvoyées en hexadécimal.",
      inputSchema: {
        sql: z.string().min(1).describe("requête SELECT unique"),
        limit: z.number().int().positive().max(1000).default(50).describe("nb max de lignes"),
      },
    },
    ({ sql, limit }) => safe(() => requireKb().query(sql, limit)),
  );

  server.registerTool(
    "re_function",
    {
      title: "Détail d'une fonction reversée",
      description:
        "Cherche une fonction par nom (LIKE) ou par vaddr (hex 0x… ou décimal). Renvoie ses métadonnées (subsystem, role, pagerank, n_calls…) et un échantillon d'xrefs entrants/sortants du meilleur match.",
      inputSchema: {
        name: z.string().optional().describe("nom ou fragment de nom (ex. 'CSceneSoccer')"),
        vaddr: z.string().optional().describe("adresse virtuelle, hex '0x140333a90' ou décimal"),
      },
    },
    ({ name, vaddr }) => safe(() => requireKb().findFunction({ name, vaddr })),
  );

  server.registerTool(
    "re_coverage",
    {
      title: "Couverture du reverse-engineering",
      description:
        "Dernière ligne de la table coverage (total_funcs / named / classified / pct) pour le binaire RE canonique (.pdata), plus les comptes réels par binaire de la table function.",
      inputSchema: {},
    },
    () => safe(() => requireKb().coverage()),
  );

  // ---------------------------------------------------------------- Repo ----
  server.registerTool(
    "repo_read",
    {
      title: "Lire un fichier du repo niers",
      description:
        "Lit un fichier source/docs du repo niers (crates Rust, docs/, scripts/, package…). Anti-traversal strict ; refs/ data/ var/ .git/ target/ node_modules/ sont interdits. Chemin relatif à la racine du repo ou absolu sous celle-ci.",
      inputSchema: {
        path: z.string().min(1).describe("ex. 'crates/engine/nie-formats/src/lib.rs' ou 'docs/PLAN.md'"),
        maxBytes: z.number().int().positive().optional().describe("octets max lus (défaut 262144)"),
      },
    },
    ({ path, maxBytes }) => safe(() => repoRead({ path, maxBytes })),
  );

  // ------------------------------------------------------------- Contrôle ----
  // Pilotage de nie-explorer via `@niers/bridge` : le protocole est le même module des deux
  // côtés, donc une commande ajoutée ici doit être gérée par le client, ou ça ne compile pas.
  const control = new Control((m) => console.error(`[niers-game] ${m}`));
  control.start();

  server.registerTool(
    "explorer_status",
    {
      title: "État du pont vers nie-explorer",
      description:
        "Indique si le pont de contrôle écoute, si nie-explorer y est connecté, et depuis quand. À appeler avant les autres outils `explorer_*` pour savoir s'ils aboutiront.",
      inputSchema: {},
    },
    () => safe(() => control.status()),
  );

  server.registerTool(
    "explorer_navigate",
    {
      title: "Naviguer dans nie-explorer",
      description:
        "Ouvre l'explorateur sur un dossier VFS et bascule sur l'onglet Explorateur. Renvoie l'état de l'interface après la navigation.",
      inputSchema: {
        prefix: z.string().describe("dossier VFS, ex. 'data/common/chr' (vide = racine)"),
        select: z.string().optional().describe("chemin complet d'une entrée à sélectionner dans ce dossier"),
      },
    },
    ({ prefix, select }) => safe(() => control.send({ cmd: "navigate", prefix, select })),
  );

  server.registerTool(
    "explorer_open",
    {
      title: "Ouvrir un fichier dans nie-explorer",
      description:
        "Ouvre un fichier du VFS dans le panneau de détail de l'explorateur (aperçu décodé selon son format).",
      inputSchema: {
        path: z.string().min(1).describe("chemin VFS complet du fichier"),
      },
    },
    ({ path }) => safe(() => control.send({ cmd: "open", path })),
  );

  server.registerTool(
    "explorer_tab",
    {
      title: "Changer d'onglet dans nie-explorer",
      description: `Bascule l'explorateur sur un onglet. Valeurs : ${EXPLORER_TABS.join(", ")}.`,
      inputSchema: {
        tab: z.enum(EXPLORER_TABS).describe("onglet cible"),
      },
    },
    ({ tab }) => safe(() => control.send({ cmd: "tab", tab })),
  );

  server.registerTool(
    "explorer_toast",
    {
      title: "Notifier dans nie-explorer",
      description: "Affiche une notification dans l'interface de l'explorateur.",
      inputSchema: {
        message: z.string().min(1).describe("texte affiché"),
        kind: z.enum(["info", "success", "error"]).default("info").describe("style de la notification"),
      },
    },
    ({ message, kind }) => safe(() => control.send({ cmd: "toast", message, kind })),
  );

  server.registerTool(
    "game_launch",
    {
      title: "Lancer le jeu",
      description:
        "Démarre `nie.exe` (racine du repo) en process détaché et renvoie son PID. N'attend pas la fin du jeu.",
      inputSchema: {
        args: z.array(z.string()).default([]).describe("arguments passés à l'exécutable"),
      },
    },
    ({ args }) => safe(() => launchGame(args)),
  );

  const transport = new StdioServerTransport();
  await server.connect(transport);
  // Le compte est LU sur le registre, pas récité : écrit en dur, il annonçait
  // encore 14 outils pour 15 enregistrés. Un journal doit décrire ce qui est,
  // pas ce qui était (la garde du compte, elle, vit dans `test/smoke.ts`).
  const exposes = Object.keys((server as unknown as { _registeredTools: Record<string, unknown> })._registeredTools);
  console.error(`[niers-game] serveur MCP prêt (stdio) — ${exposes.length} outils exposés`);
}

main().catch((e) => {
  console.error(`[niers-game] échec fatal : ${(e as Error).stack ?? e}`);
  process.exit(1);
});
