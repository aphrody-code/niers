// Les règles de la série, portées depuis le bot Discord (`packages/wonderbot`).
//
// ## Pourquoi porter plutôt que réécrire
//
// Le bot répond depuis des années aux mêmes questions que la vue Cinéma pose maintenant : que
// reste-t-il à voir, quel est l'épisode suivant, que manque-t-il au catalogue. Ses réponses sont
// écrites et testées (`wonderbot.test.ts`, 1 531 lignes). Les recopier ici à l'identique évite
// deux vérités sur le même sujet — un « prochain épisode » qui différerait entre le bot et
// l'application serait un défaut invisible et durable.
//
// Trois règles sont reprises telles quelles, et leur origine est citée à chaque fois :
//
// * `lacunes` ← `wonderbot/src/lacunes.ts` (`lacunesDeSaison`) ;
// * `prochainNonVu` et `voisins` ← `wonderbot/src/progression.ts`.
//
// Ce qui N'EST PAS porté : la persistance. Le bot garde la progression par membre Discord, en
// base ; ici elle tient dans `localStorage`, cloisonnée par profil (`lib/profils.ts`) — c'est
// déjà où vivent les positions de lecture des cinématiques.
import { clePourProfil, PROFIL_PRINCIPAL } from "./profils";

/** Clé de persistance des épisodes marqués vus, avant cloisonnement par profil. */
const CLE_VUS = "nie-explorer:cinema:vus";

/** Un épisode, désigné comme le fait le bot : par sa saison et son numéro. */
export interface CleEpisode {
  saison: number;
  episode: number;
}

/** Les trous d'une saison. Porté de `wonderbot/src/lacunes.ts`. */
export interface LacuneSaison {
  saison: number;
  /** Numéros absents entre le premier et le dernier épisode connus. */
  manquants: number[];
  /** Premier et dernier épisodes réellement présents. */
  borne: { debut: number; fin: number };
}

/**
 * Trous d'une saison : les numéros absents entre le premier et le dernier épisode connus.
 *
 * Rend `null` quand la saison est complète, vide, ou n'a qu'un seul épisode identifié — il n'y a
 * alors aucun intervalle où chercher. La borne est celle du CATALOGUE, pas celle que la chaîne
 * annonce : une saison dont les cinq derniers épisodes n'ont jamais été publiés n'a pas de trou,
 * elle est courte, et présenter cela comme un manque serait faux.
 *
 * Porté de `wonderbot/src/lacunes.ts` (`lacunesDeSaison`).
 */
export function lacunesDeSaison(saison: number, numeros: readonly (number | null)[]): LacuneSaison | null {
  const presents = [...new Set(numeros.filter((n): n is number => n !== null))];
  if (presents.length < 2) return null;

  const debut = Math.min(...presents);
  const fin = Math.max(...presents);
  const jeu = new Set(presents);

  const manquants: number[] = [];
  for (let n = debut; n <= fin; n++) if (!jeu.has(n)) manquants.push(n);

  return manquants.length === 0 ? null : { saison, manquants, borne: { debut, fin } };
}

/** `S03 · E07, E12` — porté de `decrireLacune`. */
export function decrireLacune(lacune: LacuneSaison): string {
  const liste = lacune.manquants.map((n) => `E${String(n).padStart(2, "0")}`);
  const apercu = liste.slice(0, 12).join(", ");
  const reste = liste.length - 12;
  return `${apercu}${reste > 0 ? ` (+${reste})` : ""}`;
}

/**
 * Le premier épisode non vu, par numéro croissant.
 *
 * Ce n'est PAS « celui qui suit le dernier vu » : qui a vu E01, E02 puis E08 se voit proposer
 * E03, l'épisode qui lui manque. Et seuls des numéros réellement présents au catalogue sont
 * proposés, donc une saison trouée ne renvoie jamais vers un épisode introuvable.
 *
 * Porté de `wonderbot/src/progression.ts` (`prochainNonVu`).
 */
export function prochainNonVu(disponibles: readonly number[], vus: ReadonlySet<number>): number | null {
  for (const numero of [...disponibles].sort((a, b) => a - b)) {
    if (!vus.has(numero)) return numero;
  }
  return null;
}

/**
 * Épisode précédent et suivant réellement présents au catalogue.
 *
 * Quand l'épisode courant n'est pas au catalogue (source retirée entre-temps), on l'encadre quand
 * même plutôt que de laisser la navigation sans issue.
 *
 * Porté de `wonderbot/src/progression.ts` (`voisins`).
 */
export function voisins(
  disponibles: readonly number[],
  numero: number,
): { precedent: number | null; suivant: number | null } {
  const tries = [...disponibles].sort((a, b) => a - b);
  const rang = tries.indexOf(numero);
  if (rang === -1) {
    const avant = tries.filter((n) => n < numero).at(-1) ?? null;
    const apres = tries.find((n) => n > numero) ?? null;
    return { precedent: avant, suivant: apres };
  }
  return {
    precedent: rang > 0 ? (tries[rang - 1] ?? null) : null,
    suivant: rang < tries.length - 1 ? (tries[rang + 1] ?? null) : null,
  };
}

/** `3:7` = saison 3, épisode 7 — porté de `empreinte`. */
export function empreinte(saison: number, episode: number): string {
  return `${saison}:${episode}`;
}

// ── Titres ────────────────────────────────────────────────────────────────────

/**
 * Le titre d'un épisode SANS le préfixe que la base y a collé.
 *
 * **Le défaut mesuré (2026-09-03, 355 épisodes).** La colonne `title` ne contient pas le titre
 * de l'épisode mais `<saison> — Épisode <n> - <titre>` — un préfixe rapporté par le scraper.
 * Deux problèmes, tous deux visibles à l'écran :
 *
 *  1. **Il contredit le badge.** Le badge affiche `episode`, qui vaut 1 pour le premier épisode
 *     de la saison 2 ; le préfixe, lui, dit « Épisode 27 » (la numérotation continue depuis la
 *     saison 1). La carte affichait donc « É1 » au-dessus de « Saison 2 — Épisode 27 ».
 *     Aucune des deux n'est fausse — elles ne comptent simplement pas la même chose — mais les
 *     mettre côte à côte dans le même cadre ne peut que se lire comme une erreur.
 *  2. **Il est redondant.** La rangée porte déjà le nom de la saison en en-tête.
 *
 * Et il n'est même pas homogène : les saisons 1 à 6 numérotent en continu (1 à 141) tandis
 * qu'`Outer Code`, `Ares` et `Orion` repartent de 1 — un même « Épisode 26 » désigne deux
 * épisodes différents selon la saison. Raison de plus pour ne pas l'afficher.
 *
 * **Pourquoi corriger ici et pas en base.** La donnée brute reste intacte : c'est ce que la
 * source a publié, et la réparer en base demanderait de savoir reconstruire ces titres à
 * l'identique. Le préfixe est une affaire de présentation, il se retire à la présentation.
 *
 * Les deux formes rencontrées, et rien d'autre :
 *
 * ```
 * "Saison 2 — Épisode 27 - Les extraterrestres débarquent" → "Les extraterrestres débarquent"
 * "Films — Inazuma Eleven LE FILM 2025"                    → "Inazuma Eleven LE FILM 2025"
 * ```
 *
 * Un titre qui ne porte aucun préfixe ressort tel quel : la fonction ne devine pas.
 */
export function titreCourt(titre: string): string {
  const brut = titre.trim();

  // Forme complète. `.+?` s'arrête au PREMIER « — Épisode <n> - » : sans cela, un titre qui
  // contient lui-même un tiret cadratin (« … — tout a commencé », Orion 47) serait tronqué.
  const complet = /^.+?\s—\s[ÉE]pisode\s+\d+\s+-\s+(.+)$/u.exec(brut);
  if (complet?.[1]) return complet[1].trim();

  // Forme sans numéro (« Films — … »). La borne de longueur est la garde : elle distingue un
  // préfixe de saison d'un titre qui contiendrait un tiret cadratin en cours de phrase.
  const nu = /^(.{1,20}?)\s—\s(.+)$/u.exec(brut);
  if (nu?.[2]) return nu[2].trim();

  return brut;
}

// ── Recherche approchée ───────────────────────────────────────────────────────

/**
 * Distance de Levenshtein — portée de `ietv/src/video-search.ts`.
 *
 * Deux lignes de matrice suffisent : l'implémentation d'origine en alloue une par caractère de la
 * cible, ce qui, sur les 355 résumés du catalogue, allouait des milliers de tableaux par frappe.
 * Le résultat est identique, à la ligne près.
 */
function distanceLevenshtein(a: string, b: string): number {
  if (a === b) return 0;
  if (a.length === 0) return b.length;
  if (b.length === 0) return a.length;

  let precedente = Array.from({ length: a.length + 1 }, (_, i) => i);
  let courante = new Array<number>(a.length + 1);

  for (let i = 1; i <= b.length; i++) {
    courante[0] = i;
    for (let j = 1; j <= a.length; j++) {
      const substitution = (precedente[j - 1] ?? 0) + (b.charCodeAt(i - 1) === a.charCodeAt(j - 1) ? 0 : 1);
      courante[j] = Math.min(substitution, (courante[j - 1] ?? 0) + 1, (precedente[j] ?? 0) + 1);
    }
    [precedente, courante] = [courante, precedente];
  }
  return precedente[a.length] ?? 0;
}

/** Score de proximité, de 0 à 100 — porté de `fuzzyScore` (`ietv/src/video-search.ts`). */
export function scoreApproche(requete: string, cible: string): number {
  const distance = distanceLevenshtein(requete.toLowerCase(), cible.toLowerCase());
  const longueur = Math.max(requete.length, cible.length);
  return longueur === 0 ? 0 : Math.max(0, 100 - (distance / longueur) * 100);
}

/**
 * Vrai si `cible` ressemble assez à `requete` pour être proposée en repli.
 *
 * Le score brut compare deux chaînes ENTIÈRES : « raimon » contre un titre de soixante caractères
 * tombe très bas, quelle que soit la qualité de la correspondance. On l'évalue donc mot par mot,
 * ce qui est la manière dont une personne cherche — un mot dont elle n'est pas sûre de
 * l'orthographe, pas une phrase complète.
 */
export function ressemble(requete: string, cible: string, seuil = 72): boolean {
  const q = requete.trim().toLowerCase();
  if (q.length < 3) return false;
  return cible
    .toLowerCase()
    .split(/[\s'’,.!?:;()«»"–—-]+/)
    .some((mot) => mot.length >= 3 && scoreApproche(q, mot) >= seuil);
}

// ── Progression locale ────────────────────────────────────────────────────────

/**
 * Les épisodes marqués vus, sous forme d'empreintes `saison:episode`.
 *
 * La progression est cloisonnée par PROFIL depuis que la vue en propose (`lib/profils.ts`) :
 * deux personnes devant la même fenêtre ne partagent plus le même « déjà vu ». Le profil
 * `principal` lit la clé historique, donc rien de ce qui a été accumulé avant n'est perdu — et
 * l'argument reste facultatif pour les appelants qui n'ont pas de profil sous la main.
 */
export function lireVus(profilId: string = PROFIL_PRINCIPAL): Set<string> {
  try {
    const brut = JSON.parse(localStorage.getItem(clePourProfil(CLE_VUS, profilId)) ?? "[]") as unknown;
    return new Set(Array.isArray(brut) ? brut.filter((x): x is string => typeof x === "string") : []);
  } catch {
    return new Set();
  }
}

/** Enregistre l'ensemble des épisodes vus. Un quota plein reste sans conséquence : la progression
 * est un confort, comme les positions de reprise. */
export function ecrireVus(vus: ReadonlySet<string>, profilId: string = PROFIL_PRINCIPAL): void {
  try {
    localStorage.setItem(clePourProfil(CLE_VUS, profilId), JSON.stringify([...vus]));
  } catch {
    // Ignoré volontairement.
  }
}
