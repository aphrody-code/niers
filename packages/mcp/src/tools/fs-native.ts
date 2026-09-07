import { mkdir, rename, rm } from "node:fs/promises";
import { dirname } from "node:path";

export async function ensureDir(path: string): Promise<void> {
  await mkdir(path, { recursive: true });
}

export async function deleteRecursive(path: string): Promise<void> {
  await rm(path, { recursive: true, force: true });
}

export async function movePath(from: string, to: string): Promise<void> {
  await ensureDir(dirname(to));
  await rename(from, to);
}
