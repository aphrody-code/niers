import "server-only";

/**
 * Index SQLite du texte de jeu (259k entrées).
 *
 * Façade Next au-dessus de `@rosegriffon/azalee/game-text` : la logique vit dans
 * la bibliothèque (utilisable en CLI et en sidecar Tauri), ce fichier n'ajoute
 * que la garde `server-only`.
 */

export * from "@rosegriffon/azalee/game-text";
