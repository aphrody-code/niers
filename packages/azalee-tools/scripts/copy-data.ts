#!/usr/bin/env bun
/**
 * Recopie `src/data/**` dans `dist/data/**` après l'émission TypeScript.
 *
 * `tsc` ne recopie que les JSON réellement *importés* : sans cette étape, un
 * fichier de données consommé uniquement via le sous-chemin public
 * `@rosegriffon/azalee/data/<x>.json` manquerait du package publié.
 */

import { cp, mkdir } from "node:fs/promises";
import path from "node:path";

const pkgRoot = path.resolve(import.meta.dir, "..");
const from = path.join(pkgRoot, "src/data");
const to = path.join(pkgRoot, "dist/data");

await mkdir(to, { recursive: true });
await cp(from, to, { recursive: true });

const { stdout } = Bun.spawnSync(["find", to, "-name", "*.json"]);
console.log(`data copiées=${stdout.toString().trim().split("\n").length} vers ${to}`);
