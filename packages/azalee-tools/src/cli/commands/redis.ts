/** `azalee redis` — lecture/écriture/suppression dans le cache Redis. */

import type { Command } from "commander";

import { cache } from "@rosegriffon/db/redis";

import { colors, errorMessage, reportError, restoreLogs, suppressLogs } from "../context";
import { exitUnlessRepl } from "../repl-state";
import type { RedisOptions } from "../types";

export function registerRedisCommand(program: Command): void {
	program
		.command("redis <cmd> <key> [val]")
		.description("Interagit avec le cache Redis (get, set, del)")
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (cmd: string, key: string, val: string | undefined, options: RedisOptions) => {
			suppressLogs(!!options.json);
			try {
				if (cmd === "get") {
					const res = await cache.get(key);
					if (options.json) {
						restoreLogs(true);
						console.log(JSON.stringify({ key, value: res }));
					} else {
						console.log(`${colors.green}Résultat Redis (${key}) :${colors.reset}`);
						console.log(JSON.stringify(res, null, 2));
					}
				} else if (cmd === "set") {
					let inputVal = val;
					if (!inputVal && !process.stdin.isTTY) {
						inputVal = await Bun.stdin.text();
					}
					if (!inputVal) {
						reportError(
							options.json,
							"Valeur requise pour set",
							`${colors.red}Erreur : Valeur requise pour set.${colors.reset}`,
						);
						process.exit(1);
					}
					// Une valeur JSON est stockée telle quelle ; sinon, chaîne brute.
					let parsedVal: unknown = inputVal;
					try {
						parsedVal = JSON.parse(inputVal);
					} catch {
						// on garde la chaîne
					}
					await cache.set(key, parsedVal);
					if (options.json) {
						restoreLogs(true);
						console.log(JSON.stringify({ success: true, key }));
					} else {
						console.log(`${colors.green}Clé Redis ${key} mise à jour avec succès.${colors.reset}`);
					}
				} else if (cmd === "del") {
					await cache.del(key);
					if (options.json) {
						restoreLogs(true);
						console.log(JSON.stringify({ success: true, key }));
					} else {
						console.log(`${colors.green}Clé Redis ${key} supprimée avec succès.${colors.reset}`);
					}
				} else {
					reportError(
						options.json,
						`Commande inconnue: ${cmd}`,
						`${colors.red}Commande inconnue. Utilisez: get, set ou del${colors.reset}`,
					);
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur Redis : ${errorMessage(e)}${colors.reset}`,
				);
			} finally {
				exitUnlessRepl(0);
			}
		});
}
