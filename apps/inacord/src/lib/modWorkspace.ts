// Espace de travail des mods sur disque — `tauri-plugin-fs`, TOUJOURS scopé à
// `BaseDirectory.AppData` (`mods/<modId>/…`, jamais le dossier du jeu, cf. capabilities
// `fs:scope` dans `tauri.conf.json`/`capabilities/default.json`). `copyFile` du plugin `fs` reste
// utilisé pour `exportMod` (copie DEPUIS AppData, dans la portée déclarée, VERS une destination
// choisie par dialogue). Pour la direction INVERSE — copier VERS AppData depuis une source
// arbitraire (dialogue natif OU presse-papiers, cf. `stageReplacementFromPath`) — on passe par la
// commande Rust `copy_disk_file_to_appdata` (`std::fs` direct, hors du plugin `fs` JS) : `copyFile`
// n'accepte comme source que la portée déclarée ou un chemin temporairement accordé par
// `@tauri-apps/plugin-dialog`, ce qui ne couvre PAS un chemin venu du presse-papiers (Ctrl+V).
import { BaseDirectory, copyFile, exists, mkdir, remove, stat, writeFile } from "@tauri-apps/plugin-fs";
import { open, confirm } from "@tauri-apps/plugin-dialog";
import { modsDb } from "./modsDb";
import { api } from "./api";
import { b64ToBytes } from "./bytes";

export type StageTarget = { kind: "vfs" | "disk"; path: string };

/** Nom de fichier sûr sous Windows (les chemins VFS utilisent `/`, on les aplatit). */
function sanitize(vfsPath: string): string {
  return vfsPath.replace(/[\\/:*?"<>|]/g, "_");
}

function modDir(modId: string): string {
  return `mods/${modId}`;
}

/**
 * Choisit un fichier de remplacement sur disque et le copie dans l'espace de travail du mod,
 * avec une sauvegarde de l'original (pour diff/restauration). N'écrit jamais dans le CPK ni
 * dans le dossier du jeu — uniquement dans `AppData/mods/<modId>/`.
 */
export async function stageReplacement(modId: string, target: StageTarget, gameDir?: string): Promise<boolean> {
  const picked = await open({ title: "Fichier de remplacement" });
  if (typeof picked !== "string") return false;
  return stageReplacementFromPath(modId, target, picked, gameDir);
}

/**
 * Comme [`stageReplacement`], mais avec un chemin source déjà connu (pas de sélecteur natif) —
 * utilisé par le VRAI « Coller » (Ctrl+V, `editBus.paste()`) : le chemin vient du presse-papiers
 * plutôt que d'un dialogue de fichier, cf. demande utilisatrice « editer doit vraiment copier
 * coller ». Factorisé pour ne pas dupliquer la logique de staging.
 */
export async function stageReplacementFromPath(modId: string, target: StageTarget, picked: string, gameDir?: string): Promise<boolean> {
  await mkdir(modDir(modId), { baseDir: BaseDirectory.AppData, recursive: true });

  const safe = sanitize(target.path);
  const stagedRel = `${modDir(modId)}/${safe}`;
  await api.copyDiskFileToAppdata(picked, stagedRel);

  let originalRel: string | null = null;
  try {
    const b64 = target.kind === "vfs" ? await api.readB64(target.path, gameDir) : await api.readDiskFileB64(target.path);
    originalRel = `${modDir(modId)}/${safe}.original`;
    await writeFile(originalRel, b64ToBytes(b64), { baseDir: BaseDirectory.AppData });
  } catch {
    // Fichier trop volumineux pour l'aperçu (> plafond `vfs_read_b64`) : pas de sauvegarde de
    // l'original, la copie de remplacement reste tout de même enregistrée.
  }

  const info = await stat(stagedRel, { baseDir: BaseDirectory.AppData });
  await modsDb.addFile(modId, target.path, stagedRel, originalRel, info.size);
  return true;
}

/**
 * Remplace la texture d'un `.g4tx` VFS **mono-texture, sans région d'atlas** (§2.2 roadmap) par
 * un PNG choisi via un sélecteur natif — même schéma de staging que [`stageReplacementFromPath`]
 * (sauvegarde de l'original + entrée `modsDb`), mais la copie brute est remplacée par
 * `stage_texture_replacement` (Rust : décode PNG → encode DDS/G4TX → écrit directement dans
 * l'espace de travail du mod) au lieu d'une simple copie d'octets.
 */
export async function stageTextureReplacement(modId: string, vfsPath: string, gameDir?: string): Promise<boolean> {
  const picked = await open({ title: "Texture de remplacement (PNG)", filters: [{ name: "PNG", extensions: ["png"] }] });
  if (typeof picked !== "string") return false;

  await mkdir(modDir(modId), { baseDir: BaseDirectory.AppData, recursive: true });

  const safe = sanitize(vfsPath);
  const stagedRel = `${modDir(modId)}/${safe}`;
  await api.stageTextureReplacement(vfsPath, picked, stagedRel, gameDir);

  let originalRel: string | null = null;
  try {
    const b64 = await api.readB64(vfsPath, gameDir);
    originalRel = `${modDir(modId)}/${safe}.original`;
    await writeFile(originalRel, b64ToBytes(b64), { baseDir: BaseDirectory.AppData });
  } catch {
    // Fichier trop volumineux pour l'aperçu : pas de sauvegarde de l'original (même repli que
    // stageReplacementFromPath).
  }

  const info = await stat(stagedRel, { baseDir: BaseDirectory.AppData });
  await modsDb.addFile(modId, vfsPath, stagedRel, originalRel, info.size);
  return true;
}

/** Envoie des fichiers de l'espace de travail à la VRAIE Corbeille Windows (`api.trashAppdataFiles`,
 * `trash` crate — même mécanisme que cosmic-files `trash.rs`, recherche 2026-08-08 « lis vraiment
 * le code de cosmic… les interactions os et le filesystem ») plutôt qu'un `remove()` permanent
 * (`@tauri-apps/plugin-fs`) : un clic accidentel sur « Retirer »/« Supprimer le mod » était
 * jusqu'ici irrattrapable, alors que ce sont parfois de VRAIES heures de remplacement de texture/
 * modèle. Repli sur `remove()` permanent SEULEMENT si la Corbeille échoue (ex. hors Windows) —
 * jamais un échec silencieux qui laisserait le fichier orphelin sur disque.
 */
async function trashOrRemove(paths: string[]): Promise<void> {
  const existing: string[] = [];
  for (const p of paths) {
    if (await exists(p, { baseDir: BaseDirectory.AppData }).catch(() => false)) existing.push(p);
  }
  if (existing.length === 0) return;
  try {
    await api.trashAppdataFiles(existing);
  } catch {
    // Repli : suppression permanente (ex. hors Windows, `trash_appdata_files` non implémenté) —
    // mieux vaut supprimer sans rattrapage que laisser un fichier fantôme référencé nulle part.
    for (const p of existing) {
      await remove(p, { baseDir: BaseDirectory.AppData }).catch(() => {});
    }
  }
}

export async function removeStagedFile(fileId: number, stagedFile: string, originalFile: string | null): Promise<void> {
  await modsDb.removeFile(fileId);
  await trashOrRemove(originalFile ? [stagedFile, originalFile] : [stagedFile]);
}

export async function deleteModWorkspace(modId: string, files: { staged_file: string; original_file: string | null }[]): Promise<void> {
  const paths: string[] = [];
  for (const f of files) {
    paths.push(f.staged_file);
    if (f.original_file) paths.push(f.original_file);
  }
  await trashOrRemove(paths);
  // Le dossier du mod lui-même (`mods/<modId>/`) reste un `remove()` récursif direct : une fois
  // ses fichiers passés par la Corbeille ci-dessus, il ne reste qu'un dossier VIDE à nettoyer
  // (pas de contenu utilisatrice à protéger — la Corbeille n'a de sens que pour des fichiers).
  await remove(modDir(modId), { baseDir: BaseDirectory.AppData, recursive: true }).catch(() => {});
  await modsDb.deleteMod(modId);
}

/**
 * Exporte les fichiers d'un mod, en préservant l'arborescence VFS relative, vers un dossier
 * choisi. Par défaut un dossier NEUTRE (pas le jeu) : si `intoGameDir` est vrai, exporte
 * directement dans `<gameDir>/<vfs_path>` (overlay loose-file) — comportement du VRAI moteur
 * FACE à un fichier loose non confirmé par RE, et EAC est présent sur cette installation :
 * demande une confirmation explicite avant d'écrire quoi que ce soit dans le dossier du jeu.
 */
export async function exportMod(
  files: { vfs_path: string; staged_file: string }[],
  opts: { intoGameDir: boolean; gameDir?: string },
): Promise<string | null> {
  if (files.length === 0) return null;

  let destRoot: string;
  if (opts.intoGameDir) {
    const ok = await confirm(
      "Ceci copie les fichiers du mod DIRECTEMENT dans le dossier du jeu (overlay « loose file »).\n\n" +
        "⚠️ Le comportement réel de nie.exe face à un fichier loose à la place d'un fichier de CPK " +
        "n'est PAS confirmé par rétro-ingénierie, et Easy Anti-Cheat est présent sur cette installation " +
        "— une modification du dossier du jeu peut être détectée. Continuer ?",
      { title: "Écrire dans le dossier du jeu", kind: "warning" },
    );
    if (!ok) return null;
    destRoot = opts.gameDir ?? (await api.defaultGameDir());
  } else {
    const picked = await open({ title: "Dossier d'export du mod", directory: true });
    if (typeof picked !== "string") return null;
    destRoot = picked;
  }

  for (const f of files) {
    const dest = `${destRoot}/${f.vfs_path}`;
    const parent = dest.slice(0, dest.lastIndexOf("/"));
    await mkdir(parent, { recursive: true }).catch(() => {});
    await copyFile(f.staged_file, dest, { fromPathBaseDir: BaseDirectory.AppData });
  }
  return destRoot;
}

