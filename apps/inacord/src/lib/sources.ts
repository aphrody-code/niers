// Les langues et les SOURCES d'un titre — de quoi choisir en quelle langue, depuis quel montage
// et avec quelle bande-son on regarde.
//
// ## Ce que le corpus contient réellement (mesuré le 2026-09-03, sur cette installation)
//
// * **196 `.usm` dans le VFS pour 97 films** : chacun existe sous `data/common/movie` ET sous
//   `data/dx11/movie`. Le catalogue Rust n'expose que `common` (`video.rs`, filtre
//   `data/common/movie`) — la seconde source existait donc sans que rien ne la propose.
// * **4 titres seulement portent des variantes de langue** : `Chronicle_Title_*_01` en 9 langues
//   (les 9 codes de `nie_formats::usm::LANGUES`) et `NIE_Title_*_01/02/03` en 6. Les 93 autres
//   films n'ont pas de code de langue dans leur nom — ils n'ont qu'une version.
// * **Série : 355 épisodes sur 355 en `vf`**. Le schéma de `episodes.db` prévoit pourtant
//   `language IN ('vf','vostfr','unknown')` et une contrainte `UNIQUE(channel_id, season,
//   episode, language)` : plusieurs sources par épisode sont possibles, il n'y en a aucune
//   aujourd'hui. Le sélecteur doit donc dire ce qui manque, pas proposer un choix vide.
//
// ## Trois axes, et pas un seul « choix de langue »
//
// Les confondre donnerait un menu où « dx11 » et « japonais » seraient au même niveau alors
// qu'ils ne répondent pas à la même question :
//
//  1. **la langue** — une AUTRE version du film (`NIE_Title_JP_01` vs `NIE_Title_fr_01`) ;
//  2. **le montage** — le MÊME film, en définition supérieure (`dx11`) ou standard (`common`) ;
//  3. **la bande-son** — dans le conteneur (2 films sur 97) ou dans la banque `anime_stream`.
//
// Les sous-titres sont un quatrième axe, et le seul qui ne soit pas encore jouable : le jeu ne
// les met pas dans le conteneur mais dans un `.cfg.bin` de texte (`FilmDto.sous_titres`). Ce
// module les EXPOSE (on sait quels films en ont un) sans prétendre les afficher.
import type { EpisodeAnime } from "./animeDb";
import type { FilmDto } from "./bindings";
import type { ElementCinema } from "./cinema";

/** Les neuf codes de langue des noms de films — copie de `nie_formats::usm::LANGUES`. */
export const CODES_JEU: readonly string[] = ["JP", "EN", "CN", "TW", "fr", "de", "es", "it", "pt"];

/**
 * Une langue, telle qu'on la CHOISIT — pas telle qu'elle est codée.
 *
 * `vo` et `vf` sont des familles : côté jeu la VO se code `JP` et la VF `fr`, côté série la VF se
 * code `vf`. Un sélecteur qui afficherait `JP` et `vf` côte à côte demanderait à l'utilisateur de
 * connaître deux conventions de nommage pour poser une seule question.
 */
export interface Langue {
  cle: string;
  /** Ce qui s'affiche dans le menu. */
  libelle: string;
  /** Deux ou trois lettres, pour un badge de carte. */
  court: string;
  /** Codes acceptés côté films du jeu. */
  jeu: readonly string[];
  /** Codes acceptés côté épisodes de la série. */
  serie: readonly string[];
}

/** L'ordre est celui du menu : les trois questions qu'on se pose d'abord, puis le reste. */
export const LANGUES: readonly Langue[] = [
  { cle: "vo", libelle: "VO — japonais", court: "VO", jeu: ["JP"], serie: ["vo", "jp"] },
  { cle: "vf", libelle: "VF — français", court: "VF", jeu: ["fr"], serie: ["vf"] },
  { cle: "vostfr", libelle: "VOSTFR — japonais sous-titré", court: "VOSTFR", jeu: [], serie: ["vostfr"] },
  { cle: "en", libelle: "Anglais", court: "EN", jeu: ["EN"], serie: ["en"] },
  { cle: "de", libelle: "Allemand", court: "DE", jeu: ["de"], serie: [] },
  // `serie: ["es"]` et non `[]` : la plateforme officielle sert bien l'espagnol
  // (mesuré le 2026-09-03 — 355 épisodes sous `?lang=es`, avec des identifiants
  // de vidéo DIFFÉRENTS du français). Tant que la liste était vide, ces
  // épisodes entraient en base sans qu'aucune famille de langue ne les
  // reconnaisse : `langueDEpisode` rendait `null`, le sélecteur ne les
  // proposait pas, et ils s'affichaient « Langue non renseignée ».
  // L'allemand et l'italien restent vides — la plateforme répond 200 sur
  // `?lang=de` et `?lang=it` mais y sert la page française à l'octet près.
  { cle: "es", libelle: "Espagnol", court: "ES", jeu: ["es"], serie: ["es"] },
  { cle: "it", libelle: "Italien", court: "IT", jeu: ["it"], serie: [] },
  { cle: "pt", libelle: "Portugais", court: "PT", jeu: ["pt"], serie: [] },
  { cle: "cn", libelle: "Chinois simplifié", court: "CN", jeu: ["CN"], serie: [] },
  { cle: "tw", libelle: "Chinois traditionnel", court: "TW", jeu: ["TW"], serie: [] },
];

const PAR_CLE = new Map(LANGUES.map((l) => [l.cle, l]));

/** La langue d'un film, d'après le code que porte son nom. `null` : le film n'a qu'une version. */
export function langueDeFilm(film: FilmDto): Langue | null {
  if (!film.langue) return null;
  return LANGUES.find((l) => l.jeu.includes(film.langue!)) ?? null;
}

/** La langue d'un épisode. `unknown` en base vaut « non renseigné », pas une langue. */
export function langueDEpisode(ep: EpisodeAnime): Langue | null {
  const code = ep.langue?.toLowerCase();
  if (!code || code === "unknown") return null;
  return LANGUES.find((l) => l.serie.includes(code)) ?? null;
}

/** La langue d'un élément du catalogue, quelle que soit sa source. */
export function langueDe(el: ElementCinema): Langue | null {
  if (el.film) return langueDeFilm(el.film);
  if (el.episode) return langueDEpisode(el.episode);
  return null;
}

export function langueParCle(cle: string): Langue | undefined {
  return PAR_CLE.get(cle);
}

/**
 * Le radical d'un film SANS son code de langue — la clé qui regroupe ses variantes.
 *
 * `NIE_Title_fr_01` et `NIE_Title_JP_01` rendent tous deux `NIE_Title_@_01`. Le motif exigé est
 * `_<code>_`, exactement celui que `nie_formats::usm::langue_de` reconnaît : sans les deux
 * séparateurs, le `it` de `Recruit_01` passerait pour de l'italien.
 */
export function radicalSansLangue(nom: string): string {
  for (const code of CODES_JEU) {
    const motif = `_${code}_`;
    if (nom.includes(motif)) return nom.replace(motif, "_@_");
  }
  return nom;
}

// ── Sources de lecture ────────────────────────────────────────────────────────

/** D'où vient ce qu'on va lire. */
export type TypeSource = "jeu" | "youtube";

/** Une façon concrète de regarder un titre. */
export interface SourceLecture {
  /** Identité stable : chemin VFS, ou identifiant YouTube. */
  id: string;
  /** Ce qui s'affiche dans le sélecteur (« dx11 · définition supérieure »). */
  libelle: string;
  /** Une ligne de précision, quand il y en a une à donner. */
  detail: string | null;
  type: TypeSource;
  langue: Langue | null;
  /** Le film à lire — absent pour une source YouTube. */
  film?: FilmDto;
  /** L'épisode à lire — absent pour une source du jeu. */
  episode?: EpisodeAnime;
  /** Source retenue par défaut : la meilleure définition dans la langue courante. */
  defaut: boolean;
  /** Lisible dans cette fenêtre ? `false` = codec que la webview ne décode pas. */
  lisible: boolean;
}

/** Chemin du même film sur l'autre montage. `null` si le chemin n'est pas un chemin de film. */
export function cheminAutreMontage(chemin: string): string | null {
  if (chemin.startsWith("data/common/movie/")) return chemin.replace("data/common/movie/", "data/dx11/movie/");
  if (chemin.startsWith("data/dx11/movie/")) return chemin.replace("data/dx11/movie/", "data/common/movie/");
  return null;
}

/** Le montage d'un chemin, tel qu'il se nomme dans l'interface. */
export function montageDe(chemin: string): "dx11" | "common" | null {
  if (chemin.includes("/dx11/")) return "dx11";
  if (chemin.includes("/common/")) return "common";
  return null;
}

export interface ContexteSources {
  /** Tous les films du catalogue, pour retrouver les variantes de langue. */
  films: readonly FilmDto[];
  /** Tous les épisodes, pour retrouver les autres langues d'un même numéro. */
  episodes: readonly EpisodeAnime[];
  /**
   * Chemins réellement présents sous `data/dx11/movie` — mesurés une fois au montage
   * (`api.ls`), jamais devinés : proposer un montage absent produirait une lecture qui échoue.
   */
  dx11: ReadonlySet<string>;
}

/**
 * Toutes les façons de regarder ce titre, la meilleure d'abord.
 *
 * L'ordre est celui du sélecteur : la langue demandée avant les autres, la définition supérieure
 * avant la standard. Le premier élément porte `defaut: true` — c'est ce que « Lecture » lance
 * quand personne n'a choisi.
 */
export function sourcesDe(
  el: ElementCinema,
  ctx: ContexteSources,
  languePreferee?: string,
): SourceLecture[] {
  const sorties: SourceLecture[] = [];

  if (el.film) {
    // Les variantes de langue : le même titre, une autre version. Le film courant en fait partie.
    const cle = radicalSansLangue(el.film.nom);
    const variantes = ctx.films.filter((f) => radicalSansLangue(f.nom) === cle);

    for (const f of variantes) {
      const langue = langueDeFilm(f);
      // Deux montages pour chaque variante — quand le second est réellement là.
      const autre = cheminAutreMontage(f.chemin);
      const chemins = [f.chemin, autre && ctx.dx11.has(autre) ? autre : null].filter(
        (c): c is string => c !== null,
      );
      for (const chemin of chemins) {
        const montage = montageDe(chemin);
        sorties.push({
          id: chemin,
          libelle: langue ? langue.libelle : "Version unique",
          detail:
            montage === "dx11"
              ? "dx11 — définition supérieure"
              : montage === "common"
                ? "common — définition standard"
                : null,
          type: "jeu",
          langue,
          film: chemin === f.chemin ? f : { ...f, chemin },
          defaut: false,
          lisible: f.lisible !== false,
        });
      }
    }
  }

  if (el.episode) {
    const e = el.episode;
    // La contrainte `UNIQUE(channel_id, season, episode, language)` autorise plusieurs entrées
    // pour un même numéro : c'est là que vivraient une VOSTFR et une VF du même épisode.
    const memes = ctx.episodes.filter((x) => x.saison === e.saison && x.episode === e.episode);
    for (const x of memes) {
      const langue = langueDEpisode(x);
      sorties.push({
        id: x.videoId,
        libelle: langue ? langue.libelle : "Langue non renseignée",
        detail: "YouTube — chaîne officielle",
        type: "youtube",
        langue,
        episode: x,
        defaut: false,
        lisible: true,
      });
    }
  }

  // Tri : la langue préférée d'abord, puis `dx11` avant `common`, puis l'ordre des langues.
  const rangLangue = (s: SourceLecture) => {
    if (languePreferee && s.langue?.cle === languePreferee) return -1;
    return s.langue ? LANGUES.findIndex((l) => l.cle === s.langue!.cle) : LANGUES.length;
  };
  sorties.sort(
    (a, b) =>
      rangLangue(a) - rangLangue(b) ||
      (montageDe(b.id) === "dx11" ? 1 : 0) - (montageDe(a.id) === "dx11" ? 1 : 0) ||
      a.id.localeCompare(b.id),
  );

  const premier = sorties.find((s) => s.lisible) ?? sorties[0];
  if (premier) premier.defaut = true;
  return sorties;
}

/**
 * Les trois langues que le filtre du catalogue PROPOSE — et rien d'autre.
 *
 * `LANGUES` en décrit dix parce que les films du jeu portent un code de langue dans leur nom
 * (`JP`, `fr`, `de`, `es`, `it`, `pt`, `CN`, `TW`). Le sélecteur les reprenait toutes, si bien
 * qu'on y trouvait « Allemand » ou « Chinois traditionnel » à côté de « VF » : un résidu du
 * filtre des films du jeu, dans un sélecteur qu'on lit comme celui de la série.
 *
 * Restreindre ne perd rien du jeu : `vo` couvre déjà le code `JP` et `vf` le code `fr`
 * (cf. `LANGUES`), c'est-à-dire les deux seules versions que le corpus contient en nombre.
 */
export const LANGUES_PROPOSEES: readonly string[] = ["vo", "vf", "vostfr"];

/**
 * Les langues réellement présentes dans le catalogue, avec leur compte.
 *
 * Deux filtres, et les deux comptent : on ne propose que `LANGUES_PROPOSEES`, et parmi elles
 * seulement celles que le corpus contient VRAIMENT. Une entrée qui ne filtrerait rien est une
 * promesse vide, et « VOSTFR » sur un corpus qui n'en a pas désignerait un ensemble vide.
 */
export function languesDisponibles(
  films: readonly FilmDto[],
  episodes: readonly EpisodeAnime[],
): { langue: Langue; films: number; episodes: number }[] {
  const compte = new Map<string, { films: number; episodes: number }>();
  const ajouter = (cle: string, quoi: "films" | "episodes") => {
    const c = compte.get(cle) ?? { films: 0, episodes: 0 };
    c[quoi] += 1;
    compte.set(cle, c);
  };
  for (const f of films) {
    const l = langueDeFilm(f);
    if (l) ajouter(l.cle, "films");
  }
  for (const e of episodes) {
    const l = langueDEpisode(e);
    if (l) ajouter(l.cle, "episodes");
  }
  return LANGUES.filter((l) => LANGUES_PROPOSEES.includes(l.cle) && compte.has(l.cle)).map((langue) => ({
    langue,
    films: compte.get(langue.cle)?.films ?? 0,
    episodes: compte.get(langue.cle)?.episodes ?? 0,
  }));
}

/**
 * L'élément passe-t-il le filtre de langue ?
 *
 * Un titre SANS code de langue passe toujours : les 93 films qui n'ont qu'une version ne sont pas
 * « dans la mauvaise langue », ils n'en déclarent aucune. Les exclure viderait la médiathèque au
 * premier clic sur « VF ».
 */
export function passeLangue(el: ElementCinema, cle: string): boolean {
  if (!cle) return true;
  const l = langueDe(el);
  return l === null || l.cle === cle;
}
