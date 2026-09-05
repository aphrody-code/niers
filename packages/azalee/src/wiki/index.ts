/**
 * API wiki : lecture des entités de jeu (personnages, techniques, objets,
 * auras, équipes, boutiques, quêtes, coachs, stades, drops, gacha, trophées…).
 *
 * ⚠ Module **serveur** : chaque accès passe par le client injecté
 * (`setDatabaseProvider`) ou, par défaut, par le miroir SQLite.
 * Les types et helpers purs correspondants vivent dans les modules `*-shared`,
 * ré-exportés par la racine du package et utilisables côté navigateur.
 */

export * from "./service";
export * from "./chara-stats";
export * from "./coaches";
export * from "./drops";
export * from "./gacha";
export * from "./invocation";
export * from "./quests";
export * from "./shops";
export * from "./stadiums";
export * from "./teams";
export * from "./trophies";
