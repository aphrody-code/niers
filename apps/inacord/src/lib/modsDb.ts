// Registre local des mods — `tauri-plugin-sql`, base `mods.db` dans `BaseDirectory::AppData`
// (jamais dans le dossier du jeu). Un mod = un ensemble de copies de remplacement pour des
// chemins VFS donnés ; `nie-formats` n'a pas d'encodeur CPK, donc ce registre n'écrit RIEN en
// place — il organise des fichiers destinés à un export explicite (cf. `ModsView`).
import Database from "@tauri-apps/plugin-sql";

export interface ModRow {
  id: string;
  name: string;
  description: string;
  enabled: number;
  priority: number;
  created_at: string;
  updated_at: string;
  file_count: number;
}

export interface ModFileRow {
  id: number;
  mod_id: string;
  vfs_path: string;
  staged_file: string;
  original_file: string | null;
  staged_size: number | null;
  created_at: string;
}

let dbPromise: Promise<Database> | null = null;
function db(): Promise<Database> {
  return (dbPromise ??= Database.load("sqlite:mods.db"));
}

function newId(): string {
  return crypto.randomUUID();
}

export const modsDb = {
  async listMods(): Promise<ModRow[]> {
    const d = await db();
    return d.select<ModRow[]>(
      `SELECT m.*, (SELECT COUNT(*) FROM mod_files f WHERE f.mod_id = m.id) AS file_count
       FROM mods m ORDER BY m.priority DESC, m.created_at DESC`,
    );
  },

  async createMod(name: string, description: string): Promise<string> {
    const id = newId();
    const d = await db();
    await d.execute("INSERT INTO mods (id, name, description) VALUES ($1, $2, $3)", [id, name, description]);
    return id;
  },

  async setEnabled(id: string, enabled: boolean): Promise<void> {
    const d = await db();
    await d.execute("UPDATE mods SET enabled = $1, updated_at = datetime('now') WHERE id = $2", [enabled ? 1 : 0, id]);
  },

  async rename(id: string, name: string, description: string): Promise<void> {
    const d = await db();
    await d.execute("UPDATE mods SET name = $1, description = $2, updated_at = datetime('now') WHERE id = $3", [
      name,
      description,
      id,
    ]);
  },

  async deleteMod(id: string): Promise<void> {
    const d = await db();
    await d.execute("DELETE FROM mods WHERE id = $1", [id]);
  },

  async listFiles(modId: string): Promise<ModFileRow[]> {
    const d = await db();
    return d.select<ModFileRow[]>("SELECT * FROM mod_files WHERE mod_id = $1 ORDER BY vfs_path", [modId]);
  },

  async addFile(modId: string, vfsPath: string, stagedFile: string, originalFile: string | null, stagedSize: number): Promise<void> {
    const d = await db();
    await d.execute(
      `INSERT INTO mod_files (mod_id, vfs_path, staged_file, original_file, staged_size)
       VALUES ($1, $2, $3, $4, $5)
       ON CONFLICT(mod_id, vfs_path) DO UPDATE SET staged_file = $3, original_file = $4, staged_size = $5`,
      [modId, vfsPath, stagedFile, originalFile, stagedSize],
    );
  },

  async removeFile(id: number): Promise<void> {
    const d = await db();
    await d.execute("DELETE FROM mod_files WHERE id = $1", [id]);
  },

  async addRecent(path: string, kind: "vfs" | "disk"): Promise<void> {
    const d = await db();
    await d.execute(
      `INSERT INTO recent_paths (path, kind, opened_at) VALUES ($1, $2, datetime('now'))
       ON CONFLICT(path) DO UPDATE SET opened_at = datetime('now'), kind = $2`,
      [path, kind],
    );
  },

  async listRecent(limit = 20): Promise<{ path: string; kind: string; opened_at: string }[]> {
    const d = await db();
    return d.select("SELECT * FROM recent_paths ORDER BY opened_at DESC LIMIT $1", [limit]);
  },
};
