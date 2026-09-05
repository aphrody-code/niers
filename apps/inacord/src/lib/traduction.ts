// Moteur de recherche multilingue du **Traducteur** — logique PURE (aucun import Tauri, aucun
// I/O), portée depuis `apps/azalee/app/actions/translate.ts` où elle vivait dans une *server
// action* Next.
//
// Ce que la version desktop change, et pourquoi c'est mieux :
//
//   * **le web ne pouvait pas chercher sur le romaji.** Côté serveur, la première passe est un
//     `ilike` PostgREST sur `name_fr`/`name_en`/`name_ja` : un romaji dérivé de `name_ja`
//     (`japaneseToRomaji`) n'existe dans AUCUNE colonne, donc une saisie latine qui ne matche
//     que le romaji ne remontait jamais — le score romaji n'était appliqué qu'aux lignes déjà
//     ramenées par le `ilike`. Ici, le miroir est LOCAL : les ~9 500 noms des six tables tiennent
//     en mémoire (`chargerIndexNoms`), et le score complet — romaji compris — est calculé sur la
//     totalité de l'index. Plus de sur-ensemble à deviner, plus de faux négatif.
//   * **dédoublonnage par nom.** `inagle_characters` porte 6 166 lignes pour 5 242 noms distincts
//     (variantes d'un même personnage) : le web affichait vingt « Mark Evans » identiques.
//
// Ce qui est laissé de côté, et il faut le dire : la passe romaji→kana de la version web
// (`wanakana.toHiragana`/`toKatakana`) n'est pas portée — `wanakana` n'est pas une dépendance de
// l'explorateur. Le chemin qu'elle servait (taper « endou » et matcher えんどう) reste couvert par
// le romaji DÉRIVÉ de `name_ja`, qui lui est comparé à chaque ligne. Ce qui disparaît vraiment,
// c'est la recherche d'un kana par un autre kana translittéré.

/** Seuil minimal de similarité pour retenir un candidat — identique à la version web. */
export const SEUIL_FLOU = 0.62;

/** Au-delà de ce score, le résultat n'est plus « approchant » mais franc. */
export const SEUIL_FRANC = 0.78;

/** Nature d'une entrée de l'index : décide du libellé et de l'icône affichés. */
export type TypeEntite = "chara" | "waza" | "objet" | "tactique" | "equipe" | "keshin" | "totem";

/** Libellés FR des types — mêmes intitulés que `ENTITY_TYPE_CONFIG` du wiki. */
export const LIBELLES_TYPE: Record<TypeEntite, string> = {
  chara: "Personnage",
  waza: "Technique",
  objet: "Objet",
  tactique: "Tactique",
  equipe: "Équipe",
  keshin: "Esprit Guerrier",
  totem: "Totem",
};

/** Une entrée de l'index de noms, telle que le miroir la rend (cf. `wikiQueries`). */
export interface EntreeNoms {
  type: TypeEntite;
  /** Identifiant de la ligne dans sa table (`0x23DC2602`, `whs00340`…). */
  id: string;
  nomFr: string | null;
  nomEn: string | null;
  nomJa: string | null;
  /** Romaji DÉRIVÉ de `nomJa` (aucune colonne `name_roma` n'existe dans le miroir). */
  romaji: string | null;
  /** Code interne quand la table en porte un — sert à ouvrir les fichiers liés. */
  code: string | null;
  /**
   * Noms dans les autres langues du JEU (`de`, `es`, `it`, `pt`, `zh_hans`, `zh_hant`), présents
   * uniquement quand l'index vient du jeu et non du miroir — le wiki ne publie que FR/EN/JA.
   */
  autresLangues?: { langue: string; nom: string }[];
}

/** Libellés des neuf langues de `data/common/text/`, pour l'affichage. */
export const LIBELLES_LANGUE: Record<string, string> = {
  de: "Allemand",
  en: "Anglais",
  es: "Espagnol",
  fr: "Français",
  it: "Italien",
  ja: "Japonais",
  pt: "Portugais",
  zh_hans: "Chinois simplifié",
  zh_hant: "Chinois traditionnel",
};

/**
 * Convertit l'index multilingue du JEU (`api.gameDataNoms`) en entrées de traduction.
 *
 * C'est la source de repli — et la plus riche : le miroir du wiki s'arrête à FR/EN/JA, le jeu
 * porte neuf langues. Le romaji reste DÉRIVÉ du japonais (aucune table de romaji n'existe dans
 * le jeu non plus), par la même fonction que pour le miroir.
 */
export function depuisIndexJeu(
  entrees: { kind: string; code: string; noms: { langue: string; nom: string }[] }[],
  versRomaji: (ja: string | null | undefined) => string | null,
): EntreeNoms[] {
  const TYPES: Record<string, TypeEntite> = { chara: "chara", item: "objet", skill: "waza" };
  return entrees.map((e) => {
    const par = (l: string) => e.noms.find((n) => n.langue === l)?.nom ?? null;
    const ja = par("ja");
    return {
      type: TYPES[e.kind] ?? "chara",
      id: e.code,
      nomFr: par("fr"),
      nomEn: par("en"),
      nomJa: ja,
      romaji: ja ? versRomaji(ja) : null,
      code: e.code,
      autresLangues: e.noms.filter((n) => !["fr", "en", "ja"].includes(n.langue)),
    };
  });
}

/** Une entrée retenue par la recherche, avec son score. */
export interface ResultatTraduction extends EntreeNoms {
  score: number;
  /** `true` quand aucune correspondance franche n'a été trouvée (résultat approchant). */
  approchant: boolean;
}

/**
 * Normalise pour la comparaison : minuscules, accents retirés (NFD + suppression des marques
 * combinantes), ponctuation réduite à un espace, espaces compactés.
 */
export function normaliser(entree: string): string {
  return entree
    .normalize("NFD")
    .replaceAll(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replaceAll(/[^\p{L}\p{N}]+/gu, " ")
    .replaceAll(/\s+/g, " ")
    .trim();
}

/** Découpe une chaîne normalisée en jetons non vides. */
export function jetons(entree: string): string[] {
  const norme = normaliser(entree);
  return norme ? norme.split(" ").filter(Boolean) : [];
}

/**
 * Distance de Levenshtein, deux lignes de travail (O(n·m) en temps, O(m) en mémoire).
 * Sur un index de ~9 500 entrées comparé à chaque frappe, l'allocation d'une matrice complète
 * serait le coût dominant.
 */
export function levenshtein(a: string, b: string): number {
  if (a === b) return 0;
  if (a.length === 0) return b.length;
  if (b.length === 0) return a.length;

  let prec = Array.from<number>({ length: b.length + 1 });
  let cour = Array.from<number>({ length: b.length + 1 });
  for (let j = 0; j <= b.length; j++) prec[j] = j;

  for (let i = 1; i <= a.length; i++) {
    cour[0] = i;
    const ca = a.charCodeAt(i - 1);
    for (let j = 1; j <= b.length; j++) {
      const cout = ca === b.charCodeAt(j - 1) ? 0 : 1;
      const suppr = prec[j] + 1;
      const insert = cour[j - 1] + 1;
      const subst = prec[j - 1] + cout;
      cour[j] = Math.min(suppr, insert, subst);
    }
    const tmp = prec;
    prec = cour;
    cour = tmp;
  }
  return prec[b.length];
}

/** Similarité 0-1 dérivée de la distance de Levenshtein. */
export function similarite(a: string, b: string): number {
  const max = Math.max(a.length, b.length);
  return max === 0 ? 1 : 1 - levenshtein(a, b) / max;
}

/**
 * Score d'un nom candidat face à une requête, tous deux DÉJÀ normalisés.
 *
 * Barème repris tel quel du wiki : égalité 1, préfixe 0,92, inclusion 0,82, tous les jetons
 * présents 0,78 (0,72 si l'un n'est qu'un préfixe), sinon similarité floue plafonnée à 0,7.
 */
export function scoreNom(nomNorme: string, requeteNorme: string, jetonsRequete: string[]): number {
  if (!nomNorme) return 0;
  if (nomNorme === requeteNorme) return 1;
  if (nomNorme.startsWith(requeteNorme)) return 0.92;
  if (nomNorme.includes(requeteNorme)) return 0.82;

  const jetonsNom = nomNorme.split(" ").filter(Boolean);

  if (jetonsRequete.length > 0) {
    let tousPresents = true;
    let prefixeSeulement = false;
    for (const jr of jetonsRequete) {
      const exact = jetonsNom.includes(jr);
      const prefixe = !exact && jetonsNom.some((jn) => jn.startsWith(jr) || jr.startsWith(jn));
      if (!exact && !prefixe) {
        tousPresents = false;
        break;
      }
      if (!exact) prefixeSeulement = true;
    }
    if (tousPresents) return prefixeSeulement ? 0.72 : 0.78;
  }

  let meilleur = similarite(requeteNorme, nomNorme);
  for (const jn of jetonsNom) {
    const s = similarite(requeteNorme, jn);
    if (s > meilleur) meilleur = s;
  }
  if (jetonsRequete.length > 0) {
    let somme = 0;
    for (const jr of jetonsRequete) {
      let meilleurJeton = 0;
      for (const jn of jetonsNom) {
        const s = similarite(jr, jn);
        if (s > meilleurJeton) meilleurJeton = s;
      }
      somme += meilleurJeton;
    }
    const moyenne = somme / jetonsRequete.length;
    if (moyenne > meilleur) meilleur = moyenne;
  }
  return meilleur * 0.7;
}

/** Meilleur score d'une entrée, toutes langues confondues (FR, EN, JA et romaji dérivé). */
export function scoreEntree(e: EntreeNoms, requeteNorme: string, jetonsRequete: string[]): number {
  let meilleur = 0;
  for (const candidat of [e.nomFr, e.nomEn, e.romaji, e.nomJa]) {
    if (!candidat) continue;
    const s = scoreNom(normaliser(candidat), requeteNorme, jetonsRequete);
    if (s > meilleur) meilleur = s;
  }
  return meilleur;
}

/**
 * Recherche dans l'index complet : score chaque entrée, écarte ce qui passe sous le seuil, trie
 * par pertinence puis par nom, et coupe à `limite`.
 *
 * Le repli du wiki (« si rien ne passe le seuil, montrer quand même les meilleures ») n'a plus
 * lieu d'être : il compensait un sur-ensemble ramené par `ilike` qu'aucun score ne validait.
 * Ici l'index est complet — sous le seuil, il n'y a réellement rien.
 */
export function chercher(
  index: readonly EntreeNoms[],
  requete: string,
  type: TypeEntite | null,
  limite = 60,
): ResultatTraduction[] {
  const requeteNorme = normaliser(requete);
  if (requeteNorme.length < 2) return [];
  const jetonsRequete = jetons(requete);

  const retenus: ResultatTraduction[] = [];
  for (const e of index) {
    if (type && e.type !== type) continue;
    const score = scoreEntree(e, requeteNorme, jetonsRequete);
    if (score < SEUIL_FLOU) continue;
    retenus.push({ ...e, score, approchant: score < SEUIL_FRANC });
  }

  retenus.sort((a, b) => {
    const d = b.score - a.score;
    if (Math.abs(d) > 1e-6) return d;
    const na = (a.nomFr || a.nomEn || "").toLowerCase();
    const nb = (b.nomFr || b.nomEn || "").toLowerCase();
    return na.localeCompare(nb, "fr");
  });
  return retenus.slice(0, limite);
}

/**
 * Dédoublonne un lot d'entrées par triplet de noms : `inagle_characters` porte une ligne PAR
 * VARIANTE (6 166 lignes pour 5 242 noms distincts), et un dictionnaire n'a rien à gagner à
 * répéter vingt fois « Mark Evans ». La première ligne rencontrée gagne — l'ordre de la requête
 * fait donc foi (`zukan_order`).
 */
export function dedoublonnerParNom(entrees: readonly EntreeNoms[]): EntreeNoms[] {
  const vus = new Set<string>();
  const sortie: EntreeNoms[] = [];
  for (const e of entrees) {
    const cle = `${e.type} ${e.nomFr ?? ""} ${e.nomEn ?? ""} ${e.nomJa ?? ""}`;
    if (vus.has(cle)) continue;
    vus.add(cle);
    sortie.push(e);
  }
  return sortie;
}
