// La mise à jour du catalogue d'épisodes depuis le VPS.
//
// ## Le problème qu'elle règle
//
// `data/anime/episodes.db` voyage dans l'installeur : les 355 épisodes qu'il contient sont ceux
// du jour du build. Le VPS, lui, remoissonne chaque nuit (`packages/cron/src/tasks/ietv-cache`).
// Sans ce module, la seule façon de voir un épisode publié après l'installation était de
// réinstaller l'application.
//
// ## Ce qu'elle fait, et ce qu'elle ne fait pas
//
// Elle demande à `GET <base>/api/ietv?since=<dernière moisson locale>` ce qui a changé DEPUIS, et
// fusionne le résultat dans la base locale (`animeDb.fusionner`). Elle ne remplace jamais le
// fichier : `sqlx` le tient ouvert, et échanger une base sous les pieds d'une application est le
// genre de manœuvre qui ne casse qu'une fois sur dix.
//
// Elle est **silencieuse quand tout va bien** et ne bloque jamais l'affichage : le catalogue
// s'ouvre sur ce qui est déjà là, et se complète après. Un VPS injoignable, hors ligne ou qui ne
// moissonne pas la série ne produit aucune erreur visible — seulement un état, que la barre
// affiche si on le lui demande.
//
// ## Fréquence
//
// Un contrôle toutes les six heures au plus, mémorisé dans `localStorage`. Un serveur qui répond
// 503 (« ce serveur-ci ne moissonne pas la série ») est mis de côté vingt-quatre heures : le
// redemander toutes les six heures serait acharné pour une réponse qui ne changera pas.
import { animeDb, type LotCatalogue } from "./animeDb";

/** Base par défaut — le wiki, qui tourne sur le même VPS que la moisson. */
const BASE_DEFAUT = "https://azalee.rosegriffon.fr";

/** Réglage local, pour viser un autre serveur (développement, instance privée). */
const CLE_URL = "nie-explorer:cinema:catalogue-url";
const CLE_CONTROLE = "nie-explorer:cinema:dernier-controle";

/** Six heures entre deux contrôles ; vingt-quatre après un refus franc du serveur. */
const INTERVALLE = 6 * 3600 * 1000;
const INTERVALLE_REFUS = 24 * 3600 * 1000;

/** Au-delà, on considère que le serveur ne répondra pas — la vue n'attend pas. */
const DELAI_MAX = 8000;

export type EtatMaj =
  | { etat: "a-jour"; quand: number }
  | { etat: "maj"; quand: number; ajoutes: number }
  | { etat: "indisponible"; quand: number; raison: string }
  | { etat: "hors-ligne"; quand: number };

export function baseCatalogue(): string {
  try {
    return localStorage.getItem(CLE_URL)?.replace(/\/+$/, "") || BASE_DEFAUT;
  } catch {
    return BASE_DEFAUT;
  }
}

export function definirBaseCatalogue(url: string | null): void {
  try {
    if (url) localStorage.setItem(CLE_URL, url);
    else localStorage.removeItem(CLE_URL);
  } catch {
    // Ignoré volontairement.
  }
}

function lireControle(): { quand: number; refus: boolean } {
  try {
    const brut = localStorage.getItem(CLE_CONTROLE);
    if (!brut) return { quand: 0, refus: false };
    return JSON.parse(brut) as { quand: number; refus: boolean };
  } catch {
    return { quand: 0, refus: false };
  }
}

function ecrireControle(quand: number, refus: boolean): void {
  try {
    localStorage.setItem(CLE_CONTROLE, JSON.stringify({ quand, refus }));
  } catch {
    // Ignoré volontairement.
  }
}

/** L'heure est-elle venue d'un nouveau contrôle ? */
export function controleDu(): boolean {
  const { quand, refus } = lireControle();
  return Date.now() - quand > (refus ? INTERVALLE_REFUS : INTERVALLE);
}

/**
 * Interroge le VPS et fusionne ce qu'il rend.
 *
 * `force` court-circuite l'espacement — c'est le bouton « vérifier maintenant », pas le contrôle
 * automatique.
 */
export async function verifier(cheminDb: string, force = false): Promise<EtatMaj | null> {
  if (!force && !controleDu()) return null;
  const maintenant = Date.now();

  let depuis = 0;
  try {
    depuis = await animeDb.derniereMoisson(cheminDb);
  } catch {
    // Base locale illisible : ce n'est pas au module de mise à jour de le signaler.
    return null;
  }

  const url = `${baseCatalogue()}/api/ietv?since=${depuis}`;
  const abandon = new AbortController();
  const minuterie = setTimeout(() => abandon.abort(), DELAI_MAX);

  try {
    const reponse = await fetch(url, { signal: abandon.signal });
    if (reponse.status === 503) {
      // Le serveur répond, mais il ne moissonne pas la série : distinct d'une panne, et ce n'est
      // pas la peine de le redemander dans six heures.
      ecrireControle(maintenant, true);
      return { etat: "indisponible", quand: maintenant, raison: "ce serveur ne sert pas le catalogue" };
    }
    if (!reponse.ok) {
      ecrireControle(maintenant, true);
      return { etat: "indisponible", quand: maintenant, raison: `HTTP ${reponse.status}` };
    }

    const lot = (await reponse.json()) as LotCatalogue;
    ecrireControle(maintenant, false);
    if (!Array.isArray(lot.episodes) || lot.episodes.length === 0) {
      return { etat: "a-jour", quand: maintenant };
    }

    const { episodes } = await animeDb.fusionner(cheminDb, lot);
    return { etat: "maj", quand: maintenant, ajoutes: episodes };
  } catch {
    // Hors ligne, DNS muet, délai dépassé : on ne mémorise PAS de refus — la connexion peut
    // revenir dans dix minutes, et bloquer le contrôle vingt-quatre heures serait absurde.
    return { etat: "hors-ligne", quand: maintenant };
  } finally {
    clearTimeout(minuterie);
  }
}
