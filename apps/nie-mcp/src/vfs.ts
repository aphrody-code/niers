/**
 * Index VFS en mémoire des ~255 000 fichiers CPK.
 *
 * Source unique : le paquet `nie` (bindings Bun FFI de `libnie_ffi`), c'est-à-dire
 * exactement les crates Rust — `nie-formats::vfs` — que `nie-explorer` lie côté Tauri.
 * Le serveur MCP et l'explorateur voient donc rigoureusement le même VFS, décodé par
 * le même code.
 *
 * `VfsHandle.listAll()` pagine l'index côté Rust : ~255 000 entrées en ~1 s, sans le
 * plafond de 50 000 de l'ancienne `list()`. On construit en plus un tableau de chemins
 * trié pour la navigation arborescente et la recherche (binary search sur le préfixe).
 *
 * Redis (`NIERS_REDIS`) reste accepté comme source alternative — c'est ainsi que le VPS
 * sert l'index sans monter les CPK — mais n'est plus requis.
 */

import { RedisClient } from "bun";
import { vfsOpen, type VfsHandle } from "nie";
import { config } from "./config.ts";
import { ToolError } from "./security.ts";

export type DecodeKind = "raw" | "tex" | "cfg" | "audio" | "model";

const CFG_EXTS = [".cfg.bin", ".objbin", ".fxbin", ".mevbin"];
const AUDIO_EXTS = [".hca", ".adx", ".acb", ".awb"];

/** Devine le mode de décodage model-serve le plus pertinent pour un chemin. */
export function classifyDecode(path: string): DecodeKind {
  const p = path.toLowerCase();
  if (p.endsWith(".g4tx")) return "tex";
  for (const e of CFG_EXTS) if (p.endsWith(e)) return "cfg";
  for (const e of AUDIO_EXTS) if (p.endsWith(e)) return "audio";
  return "raw";
}

/** Extension « parlante » d'un chemin (gère les doubles extensions type `.cfg.bin`). */
export function extOf(path: string): string {
  const base = path.slice(path.lastIndexOf("/") + 1);
  for (const e of CFG_EXTS) if (base.toLowerCase().endsWith(e)) return e;
  const dot = base.lastIndexOf(".");
  return dot >= 0 ? base.slice(dot) : "";
}

export interface ListEntry {
  name: string;
  path: string;
  cpk: string;
}

export interface ListResult {
  prefix: string;
  directories: string[];
  files: ListEntry[];
  total_directories: number;
  total_files: number;
  truncated: boolean;
}

export interface SearchResult {
  query: string;
  mode: "glob" | "substring";
  matches: ListEntry[];
  total_matches: number;
  truncated: boolean;
}

export interface StatResult {
  path: string;
  kind: "file" | "directory" | "missing";
  cpk?: string;
  ext?: string;
  decode?: DecodeKind;
  decodable?: boolean;
  child_count?: number;
}

export class VfsIndex {
  private readonly map: Map<string, string>;
  private readonly paths: string[];
  /** Handle FFI ouvert, quand l'index vient des CPK — `null` si chargé depuis Redis. */
  private readonly handle: VfsHandle | null;

  private constructor(entries: Map<string, string>, handle: VfsHandle | null = null) {
    this.map = entries;
    this.paths = [...entries.keys()].sort();
    this.handle = handle;
  }

  /** Charge l'index depuis Redis db 3 (un seul HGETALL), puis ferme la connexion. */
  static async load(): Promise<VfsIndex> {
    const client = new RedisClient(config.redisUrl);
    try {
      await client.connect();
      // Sélection explicite de la base 3, robuste quelle que soit l'URL.
      await client.send("SELECT", [String(config.redisDb)]);
      const raw = (await client.send("HGETALL", [config.redisIndexKey])) as unknown;
      const map = new Map<string, string>();
      if (raw && typeof raw === "object" && !Array.isArray(raw)) {
        for (const [k, v] of Object.entries(raw as Record<string, string>)) map.set(k, v);
      } else if (Array.isArray(raw)) {
        // Repli au cas où le client renverrait un tableau plat [field, value, ...].
        const arr = raw as string[];
        for (let i = 0; i + 1 < arr.length; i += 2) map.set(arr[i]!, arr[i + 1]!);
      }
      if (map.size === 0) {
        throw new Error(`HASH ${config.redisIndexKey} vide ou introuvable sur ${config.redisUrl} db${config.redisDb}`);
      }
      return new VfsIndex(map);
    } finally {
      client.close();
    }
  }

  /**
   * Monte le VFS via `nie` (FFI → `nie-formats::vfs`) et matérialise son index.
   *
   * C'est la voie par défaut : elle n'exige aucun service (ni Redis, ni model-serve) et
   * partage le code Rust de `nie-explorer`. Le handle reste ouvert pour servir
   * {@link VfsIndex.read}.
   */
  static loadFromFfi(): VfsIndex {
    const handle = vfsOpen(config.gameDataDir);
    if (handle === null) {
      throw new Error(
        `VFS non montable depuis ${config.gameDataDir} — le dossier doit contenir cpk_list.cfg.bin (cf. NIE_GAME_DIR)`,
      );
    }
    const map = new Map<string, string>();
    for (const e of handle.listAll()) map.set(e.path, e.cpk);
    if (map.size === 0) {
      handle.free();
      throw new Error(`index VFS vide depuis ${config.gameDataDir}`);
    }
    return new VfsIndex(map, handle);
  }

  get size(): number {
    return this.paths.length;
  }

  /**
   * Octets bruts d'un fichier du VFS, ou `null` si absent.
   *
   * Disponible seulement quand l'index vient du FFI ({@link VfsIndex.loadFromFfi}) :
   * un index chargé depuis Redis ne connaît que les chemins, pas le contenu.
   */
  read(internalPath: string): Uint8Array | null {
    if (this.handle === null) {
      throw new ToolError("lecture directe indisponible : index chargé depuis Redis, pas depuis les CPK");
    }
    return this.handle.read(internalPath);
  }

  /** Vrai si les octets des fichiers sont lisibles localement (index monté via FFI). */
  get canRead(): boolean {
    return this.handle !== null;
  }

  cpkOf(path: string): string | undefined {
    return this.map.get(path);
  }

  /** Premier indice i tel que paths[i] >= target (borne inférieure). */
  private lowerBound(target: string): number {
    let lo = 0;
    let hi = this.paths.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (this.paths[mid]! < target) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  /** Liste les enfants immédiats (sous-dossiers + fichiers) sous un préfixe. */
  list(prefix: string, limit: number): ListResult {
    const norm = prefix.replace(/^\/+|\/+$/g, "");
    const dirPrefix = norm === "" ? "" : norm + "/";
    const start = this.lowerBound(dirPrefix);

    const dirs = new Set<string>();
    const files: ListEntry[] = [];
    for (let i = start; i < this.paths.length; i++) {
      const p = this.paths[i]!;
      if (!p.startsWith(dirPrefix)) break;
      const rest = p.slice(dirPrefix.length);
      const slash = rest.indexOf("/");
      if (slash === -1) {
        files.push({ name: rest, path: p, cpk: this.map.get(p)! });
      } else {
        dirs.add(rest.slice(0, slash));
      }
    }

    const allDirs = [...dirs].sort();
    files.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));

    // Le `limit` s'applique à l'ensemble dossiers + fichiers (dossiers d'abord).
    const outDirs = allDirs.slice(0, limit);
    const remaining = Math.max(0, limit - outDirs.length);
    const outFiles = files.slice(0, remaining);
    const truncated = outDirs.length < allDirs.length || outFiles.length < files.length;

    return {
      prefix: dirPrefix === "" ? "(root)" : dirPrefix,
      directories: outDirs,
      files: outFiles,
      total_directories: allDirs.length,
      total_files: files.length,
      truncated,
    };
  }

  /** Recherche sous-chaîne (insensible à la casse) ou glob sur les 250 800 chemins. */
  search(query: string, limit: number): SearchResult {
    const q = query.trim();
    if (q.length === 0) throw new ToolError("query vide");
    const isGlob = /[*?[\]{}]/.test(q);
    let matcher: (p: string) => boolean;
    let mode: "glob" | "substring";
    if (isGlob) {
      const glob = new Bun.Glob(q);
      matcher = (p) => glob.match(p);
      mode = "glob";
    } else {
      const needle = q.toLowerCase();
      matcher = (p) => p.toLowerCase().includes(needle);
      mode = "substring";
    }

    const matches: ListEntry[] = [];
    let total = 0;
    for (const p of this.paths) {
      if (!matcher(p)) continue;
      total++;
      if (matches.length < limit) {
        matches.push({ name: p.slice(p.lastIndexOf("/") + 1), path: p, cpk: this.map.get(p)! });
      }
    }
    return { query: q, mode, matches, total_matches: total, truncated: total > matches.length };
  }

  /** Métadonnées d'un chemin : fichier (cpk + décodage) ou dossier ou absent. */
  stat(path: string): StatResult {
    const cpk = this.map.get(path);
    if (cpk !== undefined) {
      const decode = classifyDecode(path);
      return {
        path,
        kind: "file",
        cpk,
        ext: extOf(path),
        decode,
        decodable: decode !== "raw",
      };
    }
    // Peut-être un dossier ?
    const dirPrefix = path.replace(/\/+$/, "") + "/";
    const start = this.lowerBound(dirPrefix);
    if (this.paths[start]?.startsWith(dirPrefix)) {
      let count = 0;
      for (let i = start; i < this.paths.length && this.paths[i]!.startsWith(dirPrefix); i++) count++;
      return { path, kind: "directory", child_count: count };
    }
    return { path, kind: "missing" };
  }

  /**
   * Extrait le contenu brut d'un fichier virtuel du jeu via FFI/CPK.
   */
  cat(path: string, maxBytes = 262_144): { path: string; cpk: string; size: number; base64?: string; text?: string; truncated: boolean } {
    const cpk = this.map.get(path);
    if (!cpk) throw new ToolError(`chemin introuvable dans le VFS : ${path}`);
    const bytes = this.read(path);
    if (!bytes) throw new ToolError(`impossible de lire le fichier VFS : ${path}`);

    const size = bytes.byteLength;
    const slice = bytes.subarray(0, Math.min(size, maxBytes));
    const isText = extOf(path).includes("txt") || extOf(path).includes("json") || extOf(path).includes("lua") || extOf(path).includes("xml");

    let text: string | undefined;
    let base64: string | undefined;

    if (isText) {
      try {
        text = new TextDecoder("utf-8").decode(slice);
      } catch {
        base64 = Buffer.from(slice).toString("base64");
      }
    } else {
      base64 = Buffer.from(slice).toString("base64");
    }

    return {
      path,
      cpk,
      size,
      truncated: size > maxBytes,
      ...(text === undefined ? {} : { text }),
      ...(base64 === undefined ? {} : { base64 }),
    };
  }
}
