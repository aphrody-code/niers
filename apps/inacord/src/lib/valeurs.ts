// Conventions d'affichage des valeurs brutes du jeu — partagées par l'encyclopédie et les outils.
//
// Le décodeur `.cfg.bin` rend les champs TELS QUELS, y compris les sentinelles : le format n'a pas
// de valeur nulle, l'absence s'y écrit `0xFFFFFFFF`. Affichée sans traitement, cette sentinelle
// remplissait la colonne « Condition » des 759 lignes du butin et laissait croire à une condition
// mystérieuse, là où la donnée dit simplement « aucune » (constaté à l'écran, pas supposé).

/** Sentinelle « aucune valeur » du format `.cfg.bin`. */
export const SENTINELLE_AUCUNE = "0xFFFFFFFF";

/** `true` si la chaîne est une sentinelle d'absence (ou vide). */
export function estAbsent(v: string | null | undefined): boolean {
  return !v || v === SENTINELLE_AUCUNE;
}

/** Condition du jeu telle qu'on l'affiche : la chaîne, ou `repli` quand il n'y en a pas. */
export function conditionLisible(v: string | null | undefined, repli = "—"): string {
  return estAbsent(v) ? repli : (v as string);
}
