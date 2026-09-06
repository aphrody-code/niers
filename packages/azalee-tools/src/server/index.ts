/**
 * Surface **serveur** de la lib : accès données réel + API HTTP headless.
 *
 * Runtime Bun (ou Node ≥ 22 pour le miroir SQLite via `node:sqlite`). C'est ce
 * qu'importent le CLI `azalee`, le sidecar d'une app Tauri, et l'app Next.
 */

export * from "../config";
export * from "@rosegriffon/azalee/db";
export * from "@rosegriffon/azalee/wiki";
export * from "../cpk";
export * from "@rosegriffon/azalee/cross";
export * from "../game-text";
export { queryRag, type RagResult } from "@rosegriffon/azalee/rag";
// `resolveTextAll` est une fonction SERVEUR (elle lit l'index de textes du jeu) :
// `serve.ts` l'utilise pour la route `api/text/:hash`, mais la surface publique ne
// l'exposait pas. Un consommateur legitime — le serveur MCP — echouait donc en TS2305
// sur un symbole pourtant present dans le paquet.
export { resolveTextAll } from "../wiki/game-text";
export { createAzaleeServer, handleAzaleeRequest, type AzaleeServerOptions } from "./serve";
export {
	createAzaleeData,
	inspectLocalData,
	type AzaleeData,
	type AzaleeDataOptions,
	type AzaleeLocalAvailability,
	type AzaleeSourceKind,
	type AzaleeSourceMode,
} from "./source";
