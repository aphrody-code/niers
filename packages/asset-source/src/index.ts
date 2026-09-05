/**
 * `@niers/asset-source` — la porte unique de l'interface partagée vers les ressources du jeu.
 *
 * Inacord (Tauri) et Aphrody (navigateur) montent la MÊME interface ; ce paquet est ce qui
 * rend cela possible sans que les composants sachent lequel des deux les héberge.
 *
 * Le paquet ne dépend pas de Tauri, et c'est délibéré : il doit rester consommable par un
 * navigateur. L'hôte desktop fournit donc son implémentation du contrat en enveloppant ses
 * liaisons `tauri-specta`, qui vivent chez lui et sont régénérées à chaque build.
 */
export * from "./contract";
export { creerWebSource, type OptionsWebSource } from "./web-source";
export type { Capacites, Fichier, Page, SanteApi, VueCatalogue } from "./nie-site";
