// La recherche de la médiathèque — une requête, deux sources, un classement.
//
// ## Ce qu'elle remplace
//
// Il y avait deux filtres sans rapport l'un avec l'autre : les films testaient trois champs par
// `includes`, les épisodes quatre, et le résultat n'était pas classé — un épisode dont le titre
// EST la requête arrivait derrière un autre dont le résumé la mentionne. Chercher « ballon » ne
// disait rien de plus que « ces 60 titres contiennent le mot ».
//
// ## Ce qu'elle sait faire
//
//  * **Plusieurs mots** : tous doivent correspondre (ET), dans n'importe quel champ et n'importe
//    quel ordre. « raimon final » trouve ce que « final raimon » trouve.
//  * **Des filtres nommés**, parce que la moitié des questions qu'on pose à un catalogue de 355
//    épisodes portent sur un numéro : `s:3`, `e:12`, `s3e12`, `3x12`, `lang:vf`, `type:jeu`,
//    `chapitre:5`, `st:oui` (a des sous-titres), `vu:non`.
//  * **Un classement** : le titre avant la transcription, la transcription avant le résumé, et
//    une correspondance en début de champ avant une correspondance au milieu.
//  * **Un repli approché**, et seulement en repli : `ressemble` (le `fuzzyScore` porté de
//    `ietv/src/video-search.ts`) ne se déclenche que si la frappe exacte ne rend rien. Une
//    correspondance exacte ne doit jamais se faire diluer par des à-peu-près.
//
// Ce qu'elle NE fait pas : chercher dans le contenu des fichiers. La question « quel épisode
// montre tel but » demanderait un index que rien ne construit ici.
import type { ElementCinema } from "./cinema";
import { ressemble } from "./serie";
import { langueDe } from "./sources";

/** Une requête analysée. `termes` est ce qui reste une fois les filtres retirés. */
export interface Requete {
  brut: string;
  termes: string[];
  saison?: number;
  episode?: number;
  /** Clé de langue (`vf`, `vo`, `vostfr`…), cf. `lib/sources.ts`. */
  langue?: string;
  type?: "jeu" | "serie";
  chapitre?: number;
  sousTitres?: boolean;
  vu?: boolean;
  /** Vrai quand la requête ne porte QUE des filtres — « s:3 » doit rendre toute la saison 3. */
  sansTexte: boolean;
}

/** Les filtres reconnus, et leurs synonymes. Documenté ici, affiché par l'aide de la barre. */
const ALIAS: Record<string, keyof Requete> = {
  s: "saison",
  saison: "saison",
  season: "saison",
  e: "episode",
  ep: "episode",
  episode: "episode",
  lang: "langue",
  langue: "langue",
  vo: "langue",
  vf: "langue",
  type: "type",
  source: "type",
  chapitre: "chapitre",
  ch: "chapitre",
  st: "sousTitres",
  soustitres: "sousTitres",
  vu: "vu",
};

const OUI = new Set(["oui", "1", "true", "vrai", "o", "y", "yes"]);
const NON = new Set(["non", "0", "false", "faux", "n", "no"]);

/** `vf` / `français` / `fr` → la clé de langue du modèle (`lib/sources.ts`). */
function cleLangue(valeur: string): string | undefined {
  const v = valeur.toLowerCase();
  const table: Record<string, string> = {
    vo: "vo",
    jp: "vo",
    japonais: "vo",
    vf: "vf",
    fr: "vf",
    francais: "vf",
    français: "vf",
    vostfr: "vostfr",
    sous: "vostfr",
    en: "en",
    anglais: "en",
    de: "de",
    allemand: "de",
    es: "es",
    espagnol: "es",
    it: "it",
    italien: "it",
    pt: "pt",
    portugais: "pt",
    cn: "cn",
    tw: "tw",
  };
  return table[v];
}

/**
 * Analyse une requête.
 *
 * Les formes compactes (`s3e12`, `3x12`) sont reconnues AVANT les filtres nommés : ce sont celles
 * qu'on tape naturellement, et les traiter comme du texte ferait chercher « s3e12 » dans des
 * titres qui ne le contiennent jamais — donc zéro résultat sur la requête la plus évidente.
 */
export function analyser(brut: string): Requete {
  const r: Requete = { brut, termes: [], sansTexte: true };

  for (const mot of brut.trim().split(/\s+/).filter(Boolean)) {
    const bas = mot.toLowerCase();

    // `s3e12`, `s03e12`, `3x12`
    const compact = /^s?(\d{1,2})[ex](\d{1,3})$/.exec(bas);
    if (compact) {
      r.saison = Number(compact[1]);
      r.episode = Number(compact[2]);
      continue;
    }
    // `s3` seul
    const saisonSeule = /^s(\d{1,2})$/.exec(bas);
    if (saisonSeule) {
      r.saison = Number(saisonSeule[1]);
      continue;
    }

    // `clé:valeur`
    const deuxPoints = mot.indexOf(":");
    if (deuxPoints > 0) {
      const cle = bas.slice(0, deuxPoints).replaceAll("-", "");
      const valeur = mot.slice(deuxPoints + 1);
      const champ = ALIAS[cle];
      if (champ && valeur) {
        if (champ === "saison" || champ === "episode" || champ === "chapitre") {
          const n = Number(valeur);
          if (Number.isFinite(n)) r[champ] = n;
          continue;
        }
        if (champ === "langue") {
          const l = cleLangue(valeur);
          if (l) r.langue = l;
          continue;
        }
        if (champ === "type") {
          const v = valeur.toLowerCase();
          if (v.startsWith("jeu") || v.startsWith("vr")) r.type = "jeu";
          else if (v.startsWith("ser") || v.startsWith("ani")) r.type = "serie";
          continue;
        }
        if (champ === "sousTitres" || champ === "vu") {
          const v = valeur.toLowerCase();
          if (OUI.has(v)) r[champ] = true;
          else if (NON.has(v)) r[champ] = false;
          continue;
        }
      }
    }

    // `vo` / `vf` employés seuls : ce sont des filtres, pas des mots à chercher — aucun titre du
    // corpus ne contient « vf ».
    if ((bas === "vo" || bas === "vf" || bas === "vostfr") && !r.langue) {
      r.langue = cleLangue(bas);
      continue;
    }

    r.termes.push(bas);
  }

  r.sansTexte = r.termes.length === 0;
  return r;
}

/** Vrai si la requête ne contraint rien du tout. */
export function requeteVide(r: Requete): boolean {
  return (
    r.sansTexte &&
    r.saison === undefined &&
    r.episode === undefined &&
    r.langue === undefined &&
    r.type === undefined &&
    r.chapitre === undefined &&
    r.sousTitres === undefined &&
    r.vu === undefined
  );
}

/** Les champs d'un élément, du plus significatif au moins — l'ordre EST le classement. */
function champs(el: ElementCinema): { texte: string; poids: number }[] {
  const sortie: { texte: string; poids: number }[] = [{ texte: el.titre, poids: 100 }];
  const e = el.episode;
  if (e) {
    if (e.titreJp) sortie.push({ texte: e.titreJp, poids: 80 });
    if (e.romaji) sortie.push({ texte: e.romaji, poids: 70 });
    if (e.description) sortie.push({ texte: e.description, poids: 30 });
  }
  const f = el.film;
  if (f) {
    sortie.push({ texte: f.rubrique, poids: 60 });
    if (f.nom_origine) sortie.push({ texte: f.nom_origine, poids: 50 });
    sortie.push({ texte: f.chemin, poids: 20 });
  }
  return sortie;
}

/** Les filtres nommés passent-ils ? Ils sont stricts : un `s:3` ne « ressemble » à rien. */
function passeFiltres(el: ElementCinema, r: Requete, vu: boolean): boolean {
  if (r.type === "jeu" && el.source !== "jeu") return false;
  if (r.type === "serie" && el.source !== "anime") return false;

  if (r.saison !== undefined) {
    if (!el.episode || el.episode.saison !== r.saison) return false;
  }
  if (r.episode !== undefined) {
    if (!el.episode || el.episode.episode !== r.episode) return false;
  }
  if (r.chapitre !== undefined) {
    if (!el.film || el.film.rubrique !== `Chapitre ${r.chapitre}`) return false;
  }
  if (r.sousTitres !== undefined) {
    const a = el.film ? el.film.sous_titres !== null : false;
    if (a !== r.sousTitres) return false;
  }
  if (r.vu !== undefined && vu !== r.vu) return false;
  if (r.langue !== undefined) {
    const l = langueDe(el);
    // Contrairement au filtre de la barre (`passeLangue`), un `lang:` explicite est STRICT : qui
    // le tape demande cette version-là, pas « celle-là ou celles qui n'en déclarent aucune ».
    if (l?.cle !== r.langue) return false;
  }
  return true;
}

/**
 * Score d'un élément pour une requête. `-1` = exclu.
 *
 * Un score, et non un booléen, parce que le classement est la moitié de la réponse : sur
 * « raimon », le titre qui commence par le mot doit passer devant les quarante résumés qui le
 * citent.
 */
export function scorer(el: ElementCinema, r: Requete, vu = false, approche = false): number {
  if (!passeFiltres(el, r, vu)) return -1;
  if (r.sansTexte) return 1;

  let total = 0;
  const liste = champs(el);

  for (const terme of r.termes) {
    let meilleur = -1;
    for (const { texte, poids } of liste) {
      const bas = texte.toLowerCase();
      const i = bas.indexOf(terme);
      if (i === 0) meilleur = Math.max(meilleur, poids + 20);
      else if (i > 0) {
        // En début de MOT vaut mieux qu'au milieu d'un mot : « eleven » dans « Inazuma Eleven »
        // est une correspondance, dans « seleven » c'en est une par accident.
        const debutMot = bas[i - 1] === " " || bas[i - 1] === "-" || bas[i - 1] === "'";
        meilleur = Math.max(meilleur, poids + (debutMot ? 10 : 0));
      } else if (approche && ressemble(terme, texte)) {
        // Le repli vaut toujours moins que la moins bonne correspondance exacte.
        meilleur = Math.max(meilleur, 1);
      }
    }
    // ET : un terme sans correspondance disqualifie l'élément entier.
    if (meilleur < 0) return -1;
    total += meilleur;
  }
  return total;
}

/**
 * Applique une requête à une liste, classée.
 *
 * Le repli approché est une SECONDE passe, lancée seulement si la première ne rend rien : c'est
 * ce qui garantit qu'une faute de frappe soit rattrapée sans que les résultats exacts d'une
 * requête correcte soient noyés.
 */
export function chercher(
  elements: readonly ElementCinema[],
  r: Requete,
  estVu: (el: ElementCinema) => boolean = () => false,
): ElementCinema[] {
  if (requeteVide(r)) return [...elements];

  const passe = (approche: boolean) => {
    const notes: { el: ElementCinema; note: number }[] = [];
    for (const el of elements) {
      const note = scorer(el, r, estVu(el), approche);
      if (note >= 0) notes.push({ el, note });
    }
    notes.sort((a, b) => b.note - a.note);
    return notes.map((n) => n.el);
  };

  const exact = passe(false);
  if (exact.length > 0 || r.sansTexte) return exact;
  // Une seule lettre ne « ressemble » à rien d'utile : `ressemble` exige déjà trois caractères.
  return r.termes.some((t) => t.length >= 3) ? passe(true) : exact;
}
