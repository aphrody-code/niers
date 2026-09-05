/**
 * `@rosegriffon/azalee` — cœur de l'encyclopédie Azalée (Inazuma Eleven:
 * Victory Road), utilisable sans aucun framework.
 *
 * **Cette racine est client-safe** : uniquement des types, des règles de jeu
 * pures, la résolution d'URLs CDN, le glossaire de traduction et la recherche
 * floue. Aucun pilote SQLite, aucun accès disque — elle se bundle donc dans une
 * webview Tauri, un navigateur ou un worker.
 *
 * L'accès réel aux données (miroir SQLite, index CPK, index de texte, API HTTP)
 * vit dans `@rosegriffon/azalee/server`, réservé à un runtime Bun/Node.
 *
 * ```ts
 * // Frontend (Tauri, navigateur) — pur
 * import { FORMATIONS, translateEffect, getCharacterFaceUrl } from "@rosegriffon/azalee";
 *
 * // Backend (CLI, sidecar Tauri, serveur) — accès données
 * import { wikiService, createAzaleeServer } from "@rosegriffon/azalee/server";
 * ```
 */

// --- Règles de jeu, texte, assets, recherche (purs) ------------------------
export * from "./game";
export * from "./images";
export * from "./search";
export * from "./text";

// --- Types et helpers partagés des modules serveur ------------------------
export * from "./cpk/shared";
export * from "./cross/assets-shared";
export * from "./cross/data-shared";
export * from "./game-text/format";
export * from "./game-text/shared";
export * from "./icon-index/g4tx-header";
export * from "./icon-index/shared";
export * from "./wiki/chara-stats-shared";
export * from "./wiki/drops-shared";
export * from "./wiki/gacha-shared";
export * from "./wiki/game-text-shared";
export * from "./wiki/invocation-shared";
export * from "./wiki/shops-shared";
export * from "./wiki/teams-shared";
export * from "./wiki/trophies-shared";
