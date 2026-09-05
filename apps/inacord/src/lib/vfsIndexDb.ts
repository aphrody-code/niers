// Index SQL persistant de la TOTALITÉ du VFS (~255 800 fichiers) — table `vfs_files` (migration
// v2, `mods.db`, `tauri-plugin-sql`). Objectif : PRÉCISION. `vfs_related` (commande Rust) fait un
// `.contains()` substring en mémoire sur le code interne — un code peut apparaître par hasard
// ailleurs dans un chemin sans rapport (faux positif). Ici, `code` (basename sans extension) est
// une colonne indexée et interrogée par ÉGALITÉ ou par suffixe de variante exact
// (`code = ? OR code LIKE ?||'_%'`), pas par sous-chaîne libre.
import Database from "@tauri-apps/plugin-sql";
import { listen } from "@tauri-apps/api/event";
import { api, type VfsEntry } from "./api";

export interface VfsFileRow {
  path: string;
  name: string;
  ext: string;
  code: string;
  cpk: string;
  size: number;
}

export interface VfsIndexMeta {
  total: number;
  reindexed_at: string;
}

let dbPromise: Promise<Database> | null = null;
function db(): Promise<Database> {
  return (dbPromise ??= Database.load("sqlite:mods.db"));
}

/** `code` = basename sans extension (`c01000100.g4md` → `c01000100`, sans le point final si `.cfg.bin`).
 * Exporté : réutilisé par `nameResolve.ts` pour résoudre le nom (perso/technique/objet) lié à
 * un fichier — même définition de « code » des deux côtés, pas de logique dupliquée. */
export function codeOf(name: string): string {
  const i = name.indexOf(".");
  return i === -1 ? name : name.slice(0, i);
}

function extOf(name: string): string {
  const i = name.indexOf(".");
  return i === -1 ? "" : name.slice(i + 1).toLowerCase();
}

/** Échappe `_`/`%` (jokers `LIKE` SQLite) dans un `code` avant de l'utiliser en motif de préfixe. */
function escapeLike(s: string): string {
  return s.replace(/[\\_%]/g, (c) => `\\${c}`);
}

export const vfsIndexDb = {
  async meta(): Promise<VfsIndexMeta | null> {
    const d = await db();
    const rows = await d.select<VfsIndexMeta[]>("SELECT total, reindexed_at FROM vfs_index_meta WHERE id = 1");
    return rows[0] ?? null;
  },

  /**
   * Scanne tout le VFS et matérialise `vfs_files` par lots — appelé explicitement (bouton
   * « Réindexer »), jamais au démarrage (255 800 lignes, quelques secondes). `onProgress` reçoit
   * `(fait, total)` deux fois : d'abord pour le SCAN côté Rust (job `nie-tasks` annulable,
   * cf. `vfs_index_scan_start`/`onTaskId` — permet un bouton « Annuler » réel côté UI), puis pour
   * l'INSERTION SQL par lots ci-dessous (deux phases distinctes, mais même callback : l'UI
   * n'affiche qu'une seule barre de progression, peu importe la phase en cours).
   */
  async reindex(
    gameDir: string | undefined,
    onProgress?: (done: number, total: number) => void,
    onTaskId?: (taskId: string) => void,
  ): Promise<VfsIndexMeta> {
    const taskId = await api.indexScanStart(gameDir);
    onTaskId?.(taskId);
    const entries: VfsEntry[] = await new Promise((resolve, reject) => {
      const unlisten: Array<() => void> = [];
      const cleanup = () => unlisten.forEach((u) => u());
      listen<{ task_id: string; done: number; total: number }>("vfs-index-progress", (e) => {
        if (e.payload.task_id === taskId) onProgress?.(e.payload.done, e.payload.total);
      }).then((u) => unlisten.push(u));
      listen<string>("vfs-index-done", (e) => {
        if (e.payload !== taskId) return;
        cleanup();
        api.indexScanTake(taskId).then(resolve).catch(reject);
      }).then((u) => unlisten.push(u));
      listen<string>("vfs-index-canceled", (e) => {
        if (e.payload !== taskId) return;
        cleanup();
        reject(new Error("Réindexation annulée"));
      }).then((u) => unlisten.push(u));
      listen<string>("vfs-index-error", (e) => {
        if (!e.payload.startsWith(taskId)) return;
        cleanup();
        reject(new Error(e.payload));
      }).then((u) => unlisten.push(u));
    });
    const d = await db();

    await d.execute("DELETE FROM vfs_files");

    const BATCH = 400; // 400 lignes × 6 colonnes = 2400 paramètres, sous la limite SQLite (~32k)
    for (let i = 0; i < entries.length; i += BATCH) {
      const chunk = entries.slice(i, i + BATCH);
      const placeholders: string[] = [];
      const params: unknown[] = [];
      for (const e of chunk) {
        placeholders.push("(?, ?, ?, ?, ?, ?)");
        params.push(e.path, e.name, extOf(e.name), codeOf(e.name), e.cpk, e.size);
      }
      await d.execute(`INSERT INTO vfs_files (path, name, ext, code, cpk, size) VALUES ${placeholders.join(",")}`, params);
      onProgress?.(Math.min(i + BATCH, entries.length), entries.length);
    }

    const meta: VfsIndexMeta = { total: entries.length, reindexed_at: new Date().toISOString() };
    await d.execute(
      `INSERT INTO vfs_index_meta (id, total, reindexed_at) VALUES (1, ?, ?)
       ON CONFLICT(id) DO UPDATE SET total = excluded.total, reindexed_at = excluded.reindexed_at`,
      [meta.total, meta.reindexed_at],
    );
    return meta;
  },

  /** Annule un scan en cours (bouton « Annuler » pendant `reindex`) — cf. `onTaskId`. */
  cancelReindex(taskId: string): Promise<void> {
    return api.indexScanCancel(taskId).then(() => {});
  },

  /** Résolution PRÉCISE : `code` exact, ou `code` suivi d'un suffixe de variante (`_...`). */
  async related(code: string, limit = 200): Promise<VfsFileRow[]> {
    const d = await db();
    return d.select<VfsFileRow[]>(
      "SELECT * FROM vfs_files WHERE code = $1 OR code LIKE $2 ESCAPE '\\' ORDER BY path LIMIT $3",
      [code, `${escapeLike(code)}\\_%`, limit],
    );
  },

  /** Recherche libre par sous-chaîne de nom/chemin (plus rapide que `vfs_find` en mémoire une fois indexé). */
  async find(query: string, ext: string | undefined, limit = 200): Promise<VfsFileRow[]> {
    const d = await db();
    const like = `%${query.toLowerCase()}%`;
    if (ext) {
      return d.select<VfsFileRow[]>(
        "SELECT * FROM vfs_files WHERE lower(path) LIKE $1 AND ext = $2 ORDER BY path LIMIT $3",
        [like, ext.toLowerCase(), limit],
      );
    }
    return d.select<VfsFileRow[]>("SELECT * FROM vfs_files WHERE lower(path) LIKE $1 ORDER BY path LIMIT $2", [like, limit]);
  },
};
