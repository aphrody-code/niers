/** `azalee glossary-rebuild` — reconstruction du glossaire consolidé. */

import type { Command } from "commander";

import { colors, errorMessage, reportError, restoreLogs, suppressLogs } from "../context";
import type { GlossaryRebuildOptions } from "../types";

/** Signature attendue du script `scripts/build-glossary.ts`. */
interface GlossaryRebuildModule {
	main: () => Promise<Record<string, number>>;
}

/**
 * Le script de reconstruction vit à la racine du **monorepo**, pas dans le
 * paquet : il agrège des sources qui débordent la bibliothèque (inagle, les
 * binaires de texte du jeu, `fr.json`). On le cherche donc en remontant les
 * répertoires depuis ce module, ce qui reste valide que le CLI soit exécuté
 * depuis les sources, depuis un lien de workspace, ou depuis un binaire
 * compilé posé ailleurs.
 *
 * Sans cette recherche, le chemin relatif codé en dur pointait sur un
 * `packages/azalee/scripts/build-glossary.ts` inexistant et la commande
 * échouait sur un « Cannot find module » peu parlant.
 */
async function resolveGlossaryScript(): Promise<string | undefined> {
	let repertoire = import.meta.dir;
	for (let profondeur = 0; profondeur < 8; profondeur += 1) {
		const candidat = `${repertoire}/scripts/build-glossary.ts`;
		if (await Bun.file(candidat).exists()) return candidat;
		const parent = repertoire.slice(0, repertoire.lastIndexOf("/"));
		if (!parent || parent === repertoire) break;
		repertoire = parent;
	}
	return undefined;
}

export function registerGlossaryRebuildCommand(program: Command): void {
	program
		.command("glossary-rebuild")
		.description("Regroupe et renforce le glossaire (inagle, text binaries, fr.json)")
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (options: GlossaryRebuildOptions) => {
			suppressLogs(!!options.json);
			if (!options.json) {
				console.log(`${colors.cyan}Lancement de la reconstruction complète du glossaire...${colors.reset}`);
			}

			try {
				const script = await resolveGlossaryScript();
				if (!script) {
					throw new Error(
						"scripts/build-glossary.ts introuvable — cette commande nécessite le monorepo, pas seulement le paquet publié.",
					);
				}
				const { main: rebuildGlossary } = (await import(script)) as GlossaryRebuildModule;
				const counts = await rebuildGlossary();

				restoreLogs(!!options.json);
				if (options.json) {
					console.log(JSON.stringify({ success: true, counts }));
				}
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur de reconstruction : ${errorMessage(e)}${colors.reset}`,
				);
			}
		});
}
