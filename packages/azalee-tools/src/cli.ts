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

// `parseAsync`, jamais `parse` : les actions des commandes sont `async`.
// `parse()` rend la main dès le premier `await` de l'action, le process sort,
// et TOUT ce qui vient après cet `await` est perdu — silencieusement, avec un
// code de sortie 0. Mesuré : `search "" --json` doit écrire
// `{"error":"Requête de recherche vide"}` (le chemin passe par un
// `await Bun.stdin.text()`) et n'écrivait rien du tout. Une erreur avalée qui
// se présente comme un succès.
await createAzaleeProgram().parseAsync();
