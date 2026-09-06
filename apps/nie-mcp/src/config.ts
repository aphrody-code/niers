/**
 * Configuration résolue depuis l'environnement.
 *
 * Les valeurs par défaut sont déduites de l'emplacement du serveur lui-même
 * (`<repo>/apps/nie-mcp/src/config.ts`), de sorte que le serveur fonctionne
 * tel quel sur le VPS Linux *et* sur l'installation Steam Windows, sans
 * chemin codé en dur.
 */

import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { DEFAULT_BRIDGE_PORT } from "@niers/bridge";

function env(name: string, fallback: string): string {
  const v = process.env[name];
  return v !== undefined && v.length > 0 ? v : fallback;
}

/** Racine du repo déduite du fichier courant : `<repo>/apps/nie-mcp/src` -> `<repo>`. */
const defaultRepoRoot = resolve(import.meta.dir, "..", "..", "..");

const repoRoot = resolve(env("NIERS_REPO", defaultRepoRoot));

// Un `NIERS_REPO` qui ne désigne pas le dépôt est la panne la plus coûteuse de
// ce serveur, parce qu'elle est MUETTE : `repo_read` répond « fichier
// introuvable » sur un fichier bien présent, et — beaucoup plus grave — les
// gardes anti-traversée passent au vert pour la mauvaise raison, un ENOENT au
// lieu d'un refus. Elles ne prouvent alors plus rien.
//
// Cas vécu : la valeur du VPS (`/home/ubuntu/niers`) héritée sur le poste
// Windows, que `resolve()` transforme en `C:\Program Files\Git\home\ubuntu\niers`
// sous Git Bash. Un chemin POSIX absolu n'est PAS portable ; on le dit tout haut
// au démarrage plutôt que de laisser une suite verte le cacher.
if (!existsSync(join(repoRoot, "CLAUDE.md"))) {
  console.error(
    `[niers-game] AVERTISSEMENT racine de dépôt invalide : ${repoRoot} (aucun CLAUDE.md). ` +
      `repo_read répondra « introuvable » sur tout chemin, et ses gardes ne prouveront rien. ` +
      `Corriger NIERS_REPO (valeur actuelle : ${process.env["NIERS_REPO"] ?? "non définie"}).`,
  );
}

export const config = {
  /** API publique/interne `nie-site` (Aphrody), utilisée par les intégrations clientes. */
  aphrodyApiUrl: env("NIE_APHRODY_API_URL", "http://127.0.0.1:8085").replace(/\/+$/, ""),
  /** URL Redis. La base 3 (index VFS CPK) est sélectionnée explicitement au chargement. */
  redisUrl: env("NIERS_REDIS", "redis://127.0.0.1:6379"),
  /** Numéro de base Redis hébergeant le HASH `iev:file:index`. */
  redisDb: 3,
  /** Clé HASH chemin-logique -> nom de .cpk (250 800 entrées). */
  redisIndexKey: "iev:file:index",

  /** Base de connaissance RE (SQLite, lecture seule). */
  sqlitePath: env("NIERS_SQLITE", join(repoRoot, "var", "niers.sqlite")),

  /** Service de décodage d'assets `nie-model-serve`. */
  modelServeUrl: env("MODEL_SERVE_URL", "http://127.0.0.1:8790").replace(/\/+$/, ""),

  /** Racine du repo niers pour `repo_read`. */
  repoRoot,

  /**
   * Dossier `data/` du jeu — celui qui contient `cpk_list.cfg.bin`, tel que l'attend
   * `nie_vfs_open` (pas la racine du jeu : cf. CLAUDE.md).
   *
   * Sur l'install Steam Windows, le VFS complet est la racine du repo, donc `<repo>/data`
   * convient sans configuration.
   */
  gameDataDir: join(env("NIE_GAME_DIR", repoRoot), "data"),

  /** Port du pont de contrôle vers `nie-explorer` (cf. `@niers/bridge`). */
  bridgePort: Number.parseInt(env("NIERS_BRIDGE_PORT", String(DEFAULT_BRIDGE_PORT)), 10) || DEFAULT_BRIDGE_PORT,

  /** Exécutable du jeu, relatif à la racine du repo (cf. CLAUDE.md : `nie.exe` est à la racine). */
  gameExe: env("NIERS_GAME_EXE", "nie.exe"),

  /** Binaire RE canonique = vue `.pdata` (cf. CLAUDE.md). C'est celui dont parle `coverage`. */
  primaryBinaryId: 2,
} as const;

export type Config = typeof config;
