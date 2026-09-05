// Filtrage par sous-chaîne d'une liste affichée — extrait de `GameDataView`, où il était défini
// en local alors que la vue Outils en a exactement le même besoin (roster du calculateur de
// stats, du comparateur, du constructeur). Une seule implémentation, deux appelants.
import { useMemo } from "react";

/**
 * Filtre `liste` sur les champs rendus par `champs`, insensible à la casse et aux espaces de
 * bord. Une requête vide rend la liste telle quelle (même référence : pas de re-rendu inutile).
 */
export function useFiltered<T>(
  liste: T[],
  requete: string,
  champs: (item: T) => string[],
): T[] {
  return useMemo(() => {
    const q = requete.trim().toLowerCase();
    if (!q) return liste;
    return liste.filter((item) => champs(item).some((f) => f.toLowerCase().includes(q)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [liste, requete]);
}
