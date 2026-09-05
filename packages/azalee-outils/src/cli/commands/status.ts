/** `azalee status` — diagnostic de santé (miroir SQLite, Redis, git, système). */

import { statSync } from "node:fs";
import os from "node:os";

import type { Command } from "commander";

import { getRedisClient } from "@rosegriffon/db/redis";

import {
	colors,
	errorMessage,
	getSqlitePath,
	openReadonlyDatabase,
	reportError,
	restoreLogs,
	suppressLogs,
} from "../context";
import { exitUnlessRepl } from "../repl-state";
import type { StatusOptions } from "../types";

/** État du miroir SQLite embarqué. */
interface SqliteStatus {
	healthy: boolean;
	path: string;
	fileSize: string;
	tables: number;
	characterCount: number;
	error: string | undefined;
}

/** État du cache Redis. */
interface RedisStatus {
	healthy: boolean;
	latency: string;
	error: string | undefined;
}

/** Position du dépôt au moment du diagnostic. */
interface GitStatus {
	branch: string;
	commit: string;
	clean: boolean;
}

export function registerStatusCommand(program: Command): void {
	program
		.command("status")
		.description("Affiche le statut de santé des services locaux (SQLite, Redis, système)")
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (options: StatusOptions) => {
			suppressLogs(!!options.json);
			try {
				const dbPath = getSqlitePath();
				const sqliteStatus: SqliteStatus = {
					healthy: false,
					path: dbPath || "non trouvé",
					fileSize: "N/A",
					tables: 0,
					characterCount: 0,
					error: undefined,
				};

				if (dbPath) {
					try {
						const db = openReadonlyDatabase(dbPath);
						const stat = statSync(dbPath);
						sqliteStatus.fileSize = (stat.size / (1024 * 1024)).toFixed(2) + " MB";

						const tablesResult = db
							.query("SELECT count(*) as count FROM sqlite_master WHERE type='table'")
							.get() as { count: number };
						sqliteStatus.tables = tablesResult.count;

						try {
							const charResult = db.query("SELECT count(*) as count FROM inagle_characters").get() as {
								count: number;
							};
							sqliteStatus.characterCount = charResult.count;
						} catch {}

						sqliteStatus.healthy = true;
						db.close();
					} catch (e) {
						sqliteStatus.error = errorMessage(e);
					}
				}

				const redisStatus: RedisStatus = {
					healthy: false,
					latency: "N/A",
					error: undefined,
				};
				try {
					const redis = getRedisClient();
					const start = performance.now();
					await redis.get("status:ping");
					const end = performance.now();
					redisStatus.healthy = true;
					redisStatus.latency = (end - start).toFixed(2) + "ms";
				} catch (e) {
					redisStatus.error = errorMessage(e);
				}

				const gitStatus: GitStatus = {
					branch: "unknown",
					commit: "unknown",
					clean: false,
				};
				try {
					const git = (args: string[]) =>
						Bun.spawnSync(["git", ...args], { stdout: "pipe", stderr: "ignore" }).stdout.toString().trim();
					gitStatus.branch = git(["rev-parse", "--abbrev-ref", "HEAD"]) || "unknown";
					gitStatus.commit = git(["rev-parse", "--short", "HEAD"]) || "unknown";
					gitStatus.clean = git(["status", "--porcelain"]) === "";
				} catch {}

				const sysMemTotal = (os.totalmem() / (1024 * 1024 * 1024)).toFixed(2) + " GB";
				const sysMemFree = (os.freemem() / (1024 * 1024 * 1024)).toFixed(2) + " GB";
				const uptime = process.uptime().toFixed(1) + "s";

				const memory = process.memoryUsage();
				const processHeapUsed = (memory.heapUsed / (1024 * 1024)).toFixed(2) + " MB";
				const processRss = (memory.rss / (1024 * 1024)).toFixed(2) + " MB";

				const report = {
					sqlite: sqliteStatus,
					redis: redisStatus,
					git: gitStatus,
					process: {
						uptime,
						memory: {
							heapUsed: processHeapUsed,
							rss: processRss,
						},
					},
					system: {
						totalMemory: sysMemTotal,
						freeMemory: sysMemFree,
						platform: os.platform(),
						arch: os.arch(),
					},
				};

				restoreLogs(!!options.json);

				if (options.json) {
					console.log(JSON.stringify(report, null, 2));
				} else {
					console.log(`${colors.cyan}=== Diagnostic de santé Azalée ===${colors.reset}\n`);

					console.log(`${colors.bold}${colors.blue}[SQLite]${colors.reset}`);
					if (sqliteStatus.healthy) {
						console.log(`  Statut     : ${colors.green}Disponible${colors.reset}`);
						console.log(`  Base       : ${sqliteStatus.path}`);
						console.log(`  Taille     : ${sqliteStatus.fileSize}`);
						console.log(`  Tables     : ${sqliteStatus.tables}`);
						console.log(`  Personnages: ${sqliteStatus.characterCount}`);
					} else {
						console.log(`  Statut     : ${colors.red}Hors-ligne / Erreur${colors.reset}`);
						if (sqliteStatus.error) {
							console.log(`  Erreur     : ${sqliteStatus.error}`);
						}
					}

					console.log(`\n${colors.bold}${colors.blue}[Redis]${colors.reset}`);
					if (redisStatus.healthy) {
						console.log(`  Statut     : ${colors.green}Disponible${colors.reset}`);
						console.log(`  Latence    : ${redisStatus.latency}`);
					} else {
						console.log(`  Statut     : ${colors.red}Hors-ligne / Erreur${colors.reset}`);
						if (redisStatus.error) {
							console.log(`  Erreur     : ${redisStatus.error}`);
						}
					}

					console.log(`\n${colors.bold}${colors.blue}[Git / Version]${colors.reset}`);
					console.log(`  Branche    : ${gitStatus.branch}`);
					console.log(`  Commit     : ${gitStatus.commit}`);
					console.log(
						`  Propre     : ${gitStatus.clean ? colors.green + "Oui" : colors.yellow + "Modifications locales en cours"}${colors.reset}`,
					);

					console.log(`\n${colors.bold}${colors.blue}[Processus & Système]${colors.reset}`);
					console.log(`  Uptime CLI : ${report.process.uptime}`);
					console.log(`  Heap Utilisé: ${report.process.memory.heapUsed}`);
					console.log(`  RSS Process: ${report.process.memory.rss}`);
					console.log(
						`  OS Mémoire : ${report.system.freeMemory} libres / ${report.system.totalMemory} total`,
					);
					console.log(`  Plateforme : ${report.system.platform} (${report.system.arch})`);
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur lors du diagnostic de statut : ${errorMessage(e)}${colors.reset}`,
				);
			} finally {
				exitUnlessRepl(0);
			}
		});
}
