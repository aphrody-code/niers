import "server-only";

import "@/lib/azalee-runtime";

/**
 * Résolution des textes de jeu par hash.
 *
 * Façade Next au-dessus de `@rosegriffon/azalee/wiki/game-text` : la logique vit dans
 * la bibliothèque (utilisable en CLI et en sidecar Tauri), ce fichier n'ajoute
 * que la garde `server-only` et l'injection du client de données.
 */

export * from "@rosegriffon/azalee/wiki/game-text";
