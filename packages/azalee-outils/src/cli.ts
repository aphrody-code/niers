#!/usr/bin/env bun
/**
 * CLI Azalée — boîte à outils Inazuma Eleven: Victory Road.
 *
 * Point d'entrée volontairement mince : il construit le programme
 * (`src/cli/program.ts`) et l'exécute. Toute la logique vit dans
 * `src/cli/commands/*` (une commande par module) et dans la bibliothèque
 * `@rosegriffon/azalee` pour ce qui relève des règles de jeu.
 *
 * APIs Bun natives de bout en bout (`Bun.SQL`, `bun:sqlite`, `Bun.spawn`,
 * `Bun.file`) : les variables d'environnement (`.env`, `.env.local`) sont
 * chargées nativement par Bun avant l'exécution, y compris pour le binaire
 * compilé — aucun `dotenv` nécessaire.
 */

import { createAzaleeProgram } from "./cli/program";

createAzaleeProgram().parse();
