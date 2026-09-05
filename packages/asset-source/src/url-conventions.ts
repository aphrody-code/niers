/**
 * Les conventions d'URL des ressources du jeu.
 *
 * Elles ne sont PAS redefinies ici : `@niers/catalog/jeu` les porte deja, 69 fonctions sur
 * 757 lignes, et c'est la seule source de verite sur la facon d'adresser un fichier du jeu.
 * Ce module n'est qu'une porte, pour que l'interface partagee n'ait pas a connaitre la
 * topologie des paquets.
 *
 * Deux familles cohabitent, et il ne faut pas les confondre :
 *
 * - celles de `@niers/catalog/jeu` visent `nie-model-serve` (le decodage a la volee) ;
 * - celles de `./nie-site` visent la crate Rust d'Aphrody (`/f`, `/b`, `/api/v1`).
 *
 * Un hote choisit sa famille ; les composants, eux, passent par `AssetSource` et n'en voient
 * aucune.
 */
export * from "@niers/catalog/jeu";
