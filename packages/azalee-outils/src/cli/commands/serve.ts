/**
 * `azalee serve` — API HTTP headless (JSON, sans Next).
 *
 * Sert de sidecar à une GUI Tauri ou à n'importe quel client HTTP. Le serveur
 * n'est chargé qu'à l'exécution (`await import`) : un `--help` ou un `--json`
 * ne doit pas payer l'initialisation de la couche données.
 */

import type { Command } from "commander";

import type { ServeOptions } from "../types";

export function registerServeCommand(program: Command): void {
	program
		.command("serve")
		.description("Démarre l'API HTTP headless d'Azalée (JSON, sans Next) — sidecar d'une GUI Tauri ou client HTTP")
		.option("-p, --port <port>", "Port d'écoute", process.env.AZALEE_PORT ?? "3010")
		.option("-H, --host <host>", "Interface d'écoute", process.env.AZALEE_HOST ?? "127.0.0.1")
		.option("--cors <origin>", "Origine CORS autorisée (défaut: *)")
		.option("-j, --json", "Affiche la table de routage en JSON puis quitte")
		.action(async (options: ServeOptions) => {
			const { createAzaleeServer, listRoutes } = await import("../../server/serve");
			if (options.json) {
				console.log(JSON.stringify({ routes: listRoutes() }, null, 2));
				process.exit(0);
			}
			const server = createAzaleeServer({
				port: Number.parseInt(options.port, 10),
				hostname: options.host,
				cors: options.cors ?? true,
			});
			console.log(`azalee serve url=http://${server.hostname}:${server.port} routes=${listRoutes().length}`);
		});
}
