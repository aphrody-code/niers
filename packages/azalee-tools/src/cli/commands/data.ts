/**
 * `azalee data` — pipeline de données unifié.
 *
 * Sous-commandes : `push`, `migrate`, `load`, `sync`, `typecheck`, `verify`,
 * `all`. Chacune **orchestre l'outil canonique existant** plutôt que de
 * dupliquer sa logique : le pipeline reste une façade, pas une seconde source
 * de vérité.
 */

import { existsSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

import type { Command } from "commander";

import { colors } from "../context";
import type { DataMigrateOptions, DataSyncOptions } from "../types";

/**
 * Racine du monorepo (les scripts orchestrés y sont ancrés).
 *
 * Résolue à l'exécution, jamais compilée : `AZALEE_REPO_ROOT` d'abord, sinon la remontée
 * jusqu'au répertoire qui porte `bun.lock`. Ce marqueur-là et pas `turbo.json` : le
 * workspace Bun n'a qu'un seul lockfile, à la racine, dans les deux dépôts — alors que
 * `turbo.json` n'existe que dans `rg`, si bien qu'un marqueur turbo aurait toujours échoué
 * ici. La valeur était auparavant le chemin d'une machine précise, absent partout ailleurs.
 */
const REPO_ROOT = ((): string => {
	if (process.env.AZALEE_REPO_ROOT) return path.resolve(process.env.AZALEE_REPO_ROOT);
	let d = process.cwd();
	while (d !== path.dirname(d)) {
		if (existsSync(path.join(d, "bun.lock"))) return d;
		d = path.dirname(d);
	}
	return process.cwd();
})();
/** Fichier d'environnement chargé par le pousseur inagle. */
const AZALEE_ENV = `${REPO_ROOT}/apps/azalee/.env`;
/** Racine des dumps de jeu. */
const DATA_ROOT_DEFAULT = process.env.DATA_ROOT || process.env.DATA_PATH || "/home/ubuntu/niers/data";
/** Âge maximal toléré pour le miroir SQLite, en heures. */
const MIRROR_MAX_AGE_H = 48;

/**
 * Exécute une étape du pipeline et journalise son issue.
 *
 * `cmd` est une ligne de shell complète (pipes, `&&`) → confiée à `sh -c`, en
 * **synchrone** pour préserver l'ordre d'affichage des étapes.
 */
function runDataStep(label: string, cmd: string, env: Record<string, string> = {}): boolean {
	const t0 = Date.now();
	console.log(`${colors.cyan}▸ ${label}${colors.reset}\n  $ ${cmd}`);
	const res = Bun.spawnSync(["sh", "-c", cmd], {
		cwd: REPO_ROOT,
		stdout: "inherit",
		stderr: "inherit",
		env: { ...process.env, ...env },
	});
	if (res.exitCode === 0) {
		console.log(`${colors.green}✓ ${label} (${((Date.now() - t0) / 1000).toFixed(1)}s)${colors.reset}`);
		return true;
	}
	console.log(`${colors.red}✗ ${label} — échec${colors.reset}`);
	return false;
}

export function registerDataCommand(program: Command): void {
	const dataCmd = program
		.command("data")
		.description("Pipeline de données unifié (push/migrate/load/sync/typecheck/verify/all)");

	dataCmd
		.command("push")
		.description("Push inagle → Supabase (parse live du dump, delete+reinsert)")
		.action(() => {
			const ok = runDataStep(
				"data push (inagle → Supabase)",
				`bun packages/inagle/src/cli.ts push --env ${AZALEE_ENV}`,
				{ DATA_ROOT: DATA_ROOT_DEFAULT, DATA_PATH: DATA_ROOT_DEFAULT },
			);
			process.exit(ok ? 0 : 1);
		});

	dataCmd
		.command("migrate")
		.description("Migrations SQL (better-auth_migrations/*.sql) — liste par défaut, --apply pour exécuter")
		.option("--apply", "Applique les migrations via psql (rose_griffon local)")
		.action((opts: DataMigrateOptions) => {
			const dir = `${REPO_ROOT}/apps/azalee/better-auth_migrations`;
			let files: string[] = [];
			try {
				files = readdirSync(dir)
					.filter((f: string) => f.endsWith(".sql"))
					.sort();
			} catch {
				console.log(`${colors.yellow}Aucun dossier de migrations: ${dir}${colors.reset}`);
				return;
			}
			if (files.length === 0) {
				console.log(`${colors.green}Aucune migration SQL en attente.${colors.reset}`);
				return;
			}
			console.log(`${colors.cyan}${files.length} migration(s) SQL:${colors.reset}`);
			for (const f of files) console.log(`  - ${f}`);
			if (!opts.apply) {
				console.log(`${colors.yellow}(dry-run — relancer avec --apply pour exécuter)${colors.reset}`);
				return;
			}
			let allOk = true;
			for (const f of files) {
				allOk = runDataStep(`migrate ${f}`, `sudo -u postgres psql -d rose_griffon -f ${dir}/${f}`) && allOk;
			}
			process.exit(allOk ? 0 : 1);
		});

	dataCmd
		.command("load")
		.description("Régénère le miroir SQLite local depuis Supabase (backup:supabase)")
		.action(() => {
			const ok = runDataStep(
				"data load (Supabase → miroir SQLite)",
				`bun --filter @rosegriffon/azalee-web backup:supabase`,
			);
			process.exit(ok ? 0 : 1);
		});

	dataCmd
		.command("sync")
		.description("Synchronise Supabase ↔ miroir SQLite (incrémental ; --full pour complet)")
		.option("--full", "Resync complet")
		.option("--deletes", "Propage les suppressions")
		.action((opts: DataSyncOptions) => {
			const flags = `${opts.full ? " --full" : ""}${opts.deletes ? " --deletes" : ""}`;
			const ok = runDataStep(
				"data sync (Supabase ↔ SQLite)",
				`bun apps/azalee/scripts/ops/sync-supabase-to-sqlite.ts${flags}`,
			);
			process.exit(ok ? 0 : 1);
		});

	dataCmd
		.command("typecheck")
		.description("Type-check strict (azalee + inagle, tsc --noEmit)")
		.action(() => {
			const a = runDataStep("typecheck azalee", `bun --filter @rosegriffon/azalee-web type-check`);
			const b = runDataStep("typecheck inagle", `bun --filter @rosegriffon/inagle type-check`);
			process.exit(a && b ? 0 : 1);
		});

	dataCmd
		.command("verify")
		.description("Vérifie les chemins/données : miroir présent+récent, DATA_ROOT, entries, snapshot schéma")
		.action(() => {
			let ok = true;
			const check = (label: string, cond: boolean, detail = "") => {
				console.log(
					`  ${cond ? colors.green + "✓" : colors.red + "✗"} ${label}${colors.reset}${detail ? ` — ${detail}` : ""}`,
				);
				if (!cond) ok = false;
			};
			const backupsDir = `${REPO_ROOT}/apps/azalee/data/backups`;
			let mirror: string | null = null;
			let ageH = Infinity;
			try {
				const m = readdirSync(backupsDir)
					.filter((f: string) => f.startsWith("supabase-") && f.endsWith(".sqlite"))
					.sort()
					.pop();
				if (m) {
					mirror = `${backupsDir}/${m}`;
					ageH = (Date.now() - statSync(mirror).mtimeMs) / 3_600_000;
				}
			} catch {}
			check("miroir SQLite présent", !!mirror, mirror ? mirror.split("/").pop()! : "absent");
			check("miroir récent (< 48h)", ageH < MIRROR_MAX_AGE_H, ageH === Infinity ? "?" : `${ageH.toFixed(1)}h`);
			check("DATA_ROOT existe", existsSync(DATA_ROOT_DEFAULT), DATA_ROOT_DEFAULT);
			check("dump chara_param présent", existsSync(`${DATA_ROOT_DEFAULT}/common/gamedata/character`));
			check("inagle entries/characters.json", existsSync(`${REPO_ROOT}/packages/inagle/src/entries/characters.json`));
			check("snapshot schéma DB", existsSync(`${REPO_ROOT}/apps/azalee/data/schema-snapshot`));
			check(
				"standalone embarque un miroir",
				existsSync(`${REPO_ROOT}/apps/azalee/.next/standalone/apps/azalee/data/backups`),
			);
			console.log(ok ? `${colors.green}verify OK${colors.reset}` : `${colors.red}verify: anomalies détectées${colors.reset}`);
			process.exit(ok ? 0 : 1);
		});

	dataCmd
		.command("all")
		.description("Pipeline complet ordonné : push → load → typecheck → verify")
		.action(() => {
			const steps: Array<[string, string, Record<string, string>]> = [
				[
					"push",
					`bun packages/inagle/src/cli.ts push --env ${AZALEE_ENV}`,
					{ DATA_ROOT: DATA_ROOT_DEFAULT, DATA_PATH: DATA_ROOT_DEFAULT },
				],
				["load", `bun --filter @rosegriffon/azalee-web backup:supabase`, {}],
				["typecheck azalee", `bun --filter @rosegriffon/azalee-web type-check`, {}],
				["typecheck inagle", `bun --filter @rosegriffon/inagle type-check`, {}],
			];
			for (const [label, cmd, env] of steps) {
				if (!runDataStep(label, cmd, env)) {
					console.log(`${colors.red}Pipeline interrompu à '${label}'.${colors.reset}`);
					process.exit(1);
				}
			}
			runDataStep("verify", `bun ${REPO_ROOT}/packages/azalee/src/cli.ts data verify`);
			console.log(`${colors.green}✓ pipeline data complet${colors.reset}`);
		});
}
