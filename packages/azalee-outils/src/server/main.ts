#!/usr/bin/env bun
/**
 * Point d'entrée du **service** de l'API headless Azalée.
 *
 * Volontairement minimal et indépendant du CLI : c'est ce fichier que lance
 * `azalee-api.service` (systemd), derrière nginx sur
 * `https://api.rosegriffon.fr/azalee/`. Il donne aux clients distants — CLI
 * d'un autre poste, GUI Tauri, scripts — les mêmes données que le wiki web,
 * même quand la machine cliente n'a ni miroir SQLite ni dump du jeu.
 *
 *     AZALEE_PORT=8807 bun packages/azalee/src/server/main.ts
 *
 * Variables : `AZALEE_PORT`, `AZALEE_HOST`, `AZALEE_CORS` (origine autorisée,
 * `*` par défaut), plus celles de `src/config.ts` pour localiser les données.
 */

import { createAzaleeServer, listRoutes } from "./serve";
import { resolveDataDir, resolveMirrorPath } from "../config";

const server = createAzaleeServer({
	port: process.env.AZALEE_PORT ? Number.parseInt(process.env.AZALEE_PORT, 10) : undefined,
	hostname: process.env.AZALEE_HOST,
	cors: process.env.AZALEE_CORS ?? true,
});

// Une ligne terse `clé=valeur` : le service tourne sous journald, pas besoin de
// bannière. Les chemins résolus sont l'information de diagnostic utile.
console.log(
	[
		`azalee-api url=http://${server.hostname}:${server.port}`,
		`routes=${listRoutes().length}`,
		`mirror=${resolveMirrorPath() ?? "absent"}`,
		`data=${resolveDataDir() ?? "absent"}`,
	].join(" "),
);

for (const signal of ["SIGINT", "SIGTERM"] as const) {
	process.on(signal, () => {
		void server.stop(true);
		process.exit(0);
	});
}
