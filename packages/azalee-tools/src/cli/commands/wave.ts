/** `azalee wave` — vague d'enrichissement/analyse des données de jeu. */

import path from "node:path";

import type { Command } from "commander";

import { colors } from "../context";
import { exitUnlessRepl } from "../repl-state";
import type { WaveOptions } from "../types";

/** Intervalle entre deux vagues en mode `--loop`. */
const LOOP_INTERVAL_MS = 30_000;

export function registerWaveCommand(program: Command): void {
	program
		.command("wave")
		.description(
			"Lance une vague d'enrichissement, d'analyse et de traitement de données (zukan, glossary, asset xref)",
		)
		.option(
			"-c, --cycle",
			"Lance le runner de cycle complet (avec sync characters, compilation Next.js et restart du service)",
		)
		.option("-l, --loop", "Lance la boucle continue de vagues toutes les 30 secondes")
		.action(async (options: WaveOptions) => {
			// Le cycle complet est un script shell exécutable ; la vague simple est
			// un module TypeScript qu'il faut confier à `bun`.
			const scriptPath = options.cycle
				? path.resolve(process.cwd(), "scripts/inagle/pipeline/run-wave-cycle.sh")
				: path.resolve(process.cwd(), "scripts/inagle/pipeline/wave-processing.ts");
			const argv = [options.cycle ? scriptPath : "bun", options.cycle ? "" : scriptPath].filter(Boolean);

			if (options.loop) {
				console.log(
					`${colors.cyan}Lancement de la boucle infinie de vagues de traitement toutes les 30 secondes...${colors.reset}`,
				);
				console.log(`${colors.yellow}Appuyez sur Ctrl+C pour arrêter.${colors.reset}\n`);

				while (true) {
					console.log(`\n🌊 [${new Date().toISOString()}] Début de la vague de traitement...`);
					const proc = Bun.spawn(argv, {
						stdout: "inherit",
						stderr: "inherit",
					});
					await proc.exited;
					console.log(`💤 En attente de 30 secondes avant la prochaine vague...`);
					await Bun.sleep(LOOP_INTERVAL_MS);
				}
			} else {
				console.log(`${colors.cyan}Lancement de la vague de traitement...${colors.reset}`);
				const proc = Bun.spawn(argv, {
					stdout: "inherit",
					stderr: "inherit",
				});
				const code = await proc.exited;
				exitUnlessRepl(code);
			}
		});
}
