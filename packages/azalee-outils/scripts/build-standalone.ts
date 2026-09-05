#!/usr/bin/env bun
/**
 * Compile le CLI `azalee` en **binaire autonome** (`bun build --compile`).
 *
 * Le binaire embarque le runtime Bun : il tourne sans `node_modules`, ce qui en
 * fait aussi le sidecar idéal d'une application Tauri (on l'ajoute à
 * `tauri.conf.json > bundle.externalBin`, et la webview parle à
 * `azalee serve` en HTTP local).
 *
 *     bun packages/azalee/scripts/build-standalone.ts            # -> bin/azalee
 *     bun packages/azalee/scripts/build-standalone.ts --install  # + copie dans ~/.local/bin
 */

import { $ } from "bun";
import path from "node:path";
import { mkdir } from "node:fs/promises";

const pkgRoot = path.resolve(import.meta.dir, "..");
const entry = path.join(pkgRoot, "src/cli.ts");
const outDir = path.join(pkgRoot, "bin");
const outFile = path.join(outDir, "azalee");

await mkdir(outDir, { recursive: true });

const started = Date.now();
await $`bun build --compile --minify --sourcemap ${entry} --outfile ${outFile}`.cwd(pkgRoot);

const size = Bun.file(outFile).size;
console.log(`built=${outFile} size=${(size / 1024 / 1024).toFixed(1)}Mo ms=${Date.now() - started}`);

if (process.argv.includes("--install")) {
	const target = path.join(process.env.HOME ?? "/home/ubuntu", ".local/bin/azalee");
	await $`install -m 755 ${outFile} ${target}`;
	console.log(`installed=${target}`);
}
