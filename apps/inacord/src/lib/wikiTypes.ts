// Types et libellés des cartes du wiki, définis **localement** pour l'application de bureau.
//
// ## Pourquoi ne pas importer `@rosegriffon/azalee/wiki/*`
//
// Une raison de fond et une raison mesurée :
//
//  1. ces modules sont la couche DONNÉES du site (ils descendent jusqu'à `db/sqlite-client.ts`,
//     qui importe `bun:sqlite`). L'application de bureau lit le JEU, pas la base du wiki : lui
//     faire dépendre de la couche web serait une inversion ;
//  2. `bun run build` (`tsc && vite build`) fait entrer les SOURCES de ces paquets dans le
//     programme TypeScript de l'app, avec la config de l'app. Résultat mesuré : la release 0.5.6
//     a échoué sur `Cannot find module 'bun:sqlite'` et `Property 'dir' does not exist on type
//     'ImportMeta'` — des erreurs de code Bun typé dans un contexte DOM, qui n'ont rien à voir
//     avec l'application et qu'aucune correction locale ne pouvait faire disparaître.
//
// Les formes ci-dessous sont donc les CONTRATS que les cartes attendent, réduits à ce qu'elles
// lisent réellement. Elles n'ont pas à suivre la base du wiki : c'est l'appelant qui adapte.

// ─── Butin (DropsCard) ───────────────────────────────────────────────────────────────────────

/** Une ligne de butin telle que la carte l'affiche. */
export interface DropEntry {
  id: string;
  /** Origine de la ligne dans les données du jeu. */
  source: "win_treasure" | "item_emission";
  sourceId: string;
  itemId: string;
  itemName: string;
  itemCategory: string | null;
  itemRarity: number | null;
  itemInternalCode: string | null;
  itemImageUrl: string | null;
  /** Poids brut dans son groupe de tirage. */
  weight: number;
  /** Part du poids dans le total du groupe, en pourcentage. */
  chance: number;
}

export function dropSourceLabel(source: DropEntry["source"]): string {
  return source === "win_treasure" ? "Coffre de victoire" : "Émission d'objet";
}

/** Catégories d'objets telles que le jeu les nomme → libellé FR. */
const CATEGORIES_OBJET: Record<string, string> = {
  accessory: "Pendentifs",
  consume: "Consommables",
  costume: "Tenues",
  emblem: "Écussons",
  formation: "Formations",
  important: "Objets clés",
  misanga: "Bracelets",
  shoes: "Chaussures",
  special_skill: "Techniques sp.",
  special_tactics: "Tactiques sp.",
  super_tactics: "Hyper tactiques",
};

export function dropCategoryLabel(category: string | null): string {
  if (!category) return "Autre";
  return CATEGORIES_OBJET[category] ?? category;
}

// ─── Capsules (GachaCard) ────────────────────────────────────────────────────────────────────

/** Un lot de capsule, réduit à ce que la carte affiche. */
export interface CapsulePrize {
  id: string;
  /** Référence du contenu tiré (hash ou code interne). */
  contentRef: string;
  /** Référence de la table de tirage d'origine. */
  poolRef: string;
}

/** Une tenue obtenable, réduite à ce que la carte affiche. */
export interface Costume {
  index: number;
  /** `0`, `1`, `2` — la carte colore la pastille selon cette valeur. */
  type: number;
  typeLabel: string;
  modelRef: string;
  /** Les deux drapeaux de déblocage, tels que la carte les compare. */
  flag1: number;
  flag2: number;
}

// ─── Boutiques (ShopCard) ────────────────────────────────────────────────────────────────────

/** Catégories de boutique → libellé FR (mêmes clés que les objets). */
export const SHOP_CATEGORY_FR: Record<string, string> = CATEGORIES_OBJET;

// ─── Équipes (TeamCard) ──────────────────────────────────────────────────────────────────────

/** Une équipe telle que la carte l'affiche. */
export interface TeamListItem {
  id: string;
  name: string;
  nameJa: string | null;
  emblemUrl: string | null;
  rosterCount: number;
  /** Effectif par saison (`{ ie1: 16, v: 22 }`) — la carte trie sur ces nombres. */
  seasons: Record<string, number>;
  /** Clés de série d'origine. */
  seriesKeys: string[];
}

/** Une quête telle que la carte l'affiche. */
export interface Quest {
  id: string;
  title: string;
  /** Titres par langue — la carte affiche `en` en second quand il diffère du titre courant. */
  titles: { en?: string; fr?: string; ja?: string };
  kind: string;
  /** Chapitre (quête principale) ou numéro d'ordre — le jeu le rend sous forme de code. */
  phase: string | null;
  /** Identifiant de zone du monde (`areaLabel` le traduit), `null` pour une quête d'histoire. */
  area: number | null;
}

/** Codes de saison du jeu → libellé affiché (mêmes neuf saisons que `belong_team`). */
export const SEASON_LABELS: Record<string, string> = {
  ares: "Ares",
  go1: "GO 1",
  go2: "GO 2",
  go3: "GO 3",
  ie1: "IE 1",
  ie2: "IE 2",
  ie3: "IE 3",
  orion: "Orion",
  v: "Victory Road",
};

/** Clés de série → libellé affiché. */
export const SERIES_LABELS: Record<string, string> = {
  ares: "Ares no Tenbin",
  go: "GO",
  ie: "Inazuma Eleven",
  orion: "Orion no Kokuin",
  v: "Victory Road",
};

// ─── Stats (CharacterStatsPopover) ───────────────────────────────────────────────────────────

/** Stats d'un personnage sous leur forme la plus simple : un libellé, une valeur. */
export type GameCharacterStats = Record<string, number>;

// ─── Surlignage de recherche (SearchResultHighlight) ─────────────────────────────────────────

/**
 * Découpe `texte` en segments surlignés ou non, selon les mots de `requete`.
 *
 * Version locale, insensible à la casse et aux accents, sans dépendance : le moteur flou du site
 * score des lignes de base de données, ce dont une liste déjà filtrée en mémoire n'a pas besoin.
 */
export function highlightMatches(
  texte: string,
  requete: string,
): { text: string; highlight: boolean }[] {
  const mots = requete
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter((m) => m.length > 0);
  if (mots.length === 0) return [{ text: texte, highlight: false }];

  const sansAccent = (s: string) => s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
  const base = sansAccent(texte);
  // Marque caractère par caractère : deux mots qui se chevauchent ne produisent alors qu'un seul
  // segment, au lieu de segments imbriqués impossibles à rendre.
  const marques = new Array<boolean>(texte.length).fill(false);
  for (const mot of mots) {
    const cible = sansAccent(mot);
    let i = base.indexOf(cible);
    while (i !== -1) {
      for (let k = i; k < i + cible.length && k < marques.length; k++) marques[k] = true;
      i = base.indexOf(cible, i + cible.length);
    }
  }

  const segments: { text: string; highlight: boolean }[] = [];
  let debut = 0;
  for (let i = 1; i <= texte.length; i++) {
    if (i === texte.length || marques[i] !== marques[debut]) {
      segments.push({ text: texte.slice(debut, i), highlight: marques[debut] });
      debut = i;
    }
  }
  return segments;
}
