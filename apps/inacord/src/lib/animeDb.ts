// Catalogue des épisodes de la série — le quatrième gisement (`anime`, cf. `docs/FUSION.md`),
// lu depuis `data/anime/episodes.db` par `tauri-plugin-sql`, exactement comme le miroir du wiki
// (`wikiDb.ts`) et la base de reverse (`reDb.ts`).
//
// ## D'où vient cette base
//
// `packages/ietv` (`@aphrody/ietv`) recense les épisodes publiés par la chaîne officielle et les
// écrit dans ce SQLite ; la tâche `packages/cron/src/tasks/ietv-cache.ts` le rafraîchit. Le
// paquet lui-même n'est PAS importé ici : c'est un scraper Node qui parle à YouTube, il n'a rien
// à faire dans une webview. Ce qui voyage jusqu'à l'application, c'est son résultat — la base,
// embarquée dans l'installeur au même titre que les deux autres (`installer_bases_embarquees`).
//
// `@aphrody/ietv-client` (le client REST) vise un serveur `/api/ietv` : il reste la bonne porte
// pour un bot ou un site, pas pour une application qui doit fonctionner hors ligne. Le schéma lu
// ici est celui qu'écrit `IETVCache`, donc les deux chemins servent les mêmes données.
import Database from "@tauri-apps/plugin-sql";

import { api } from "./api";

/** Un épisode de la série. */
export interface EpisodeAnime {
  id: number;
  /** Numéro de saison tel que le porte la base (1…10). */
  saison: number;
  /** Numéro d'épisode dans la saison, `null` pour un film ou un hors-série. */
  episode: number | null;
  /**
   * Identifiant de la vidéo. **Pas toujours un identifiant YouTube** : 143 épisodes sur 355
   * portent un jeton propre à la base (`off-galaxy-1`) — cf. `plateformeDe`.
   */
  videoId: string;
  /** Page publique de l'épisode — YouTube, ou la plateforme officielle pour les 143 autres. */
  url: string;
  titre: string;
  /** Titre original japonais — renseigné pour 330 des 355 épisodes. */
  titreJp: string | null;
  /** Transcription latine du titre japonais — 327 des 355. */
  romaji: string | null;
  description: string | null;
  vignette: string | null;
  /** Date de première diffusion (`2008-10-05`) — 330 des 355. */
  publie: string | null;
  langue: string | null;
  /**
   * Durée en secondes. **Vide sur tout le corpus actuel** (0 épisode sur 355), comme `viewCount`
   * et `quality` : la colonne existe au schéma de `IETVCache`, la source ne la remplit pas. Elle
   * est lue quand même — le jour où elle l'est, rien n'aura à changer ici — mais l'interface ne
   * réserve aucune place à un chiffre qui n'arrive jamais.
   */
  duree: number | null;
}

/**
 * Une façon concrète de regarder un épisode — une ligne d'`episode_sources`.
 *
 * **Pourquoi une table à part.** `episodes.videoId` ne pouvait porter qu'UNE vidéo, et il portait
 * en réalité trois choses : 212 identifiants YouTube, et 143 jetons locaux (`off-galaxy-1`)
 * qu'aucun lecteur n'ouvre. Un épisode a maintenant en moyenne cinq sources (min 4, max 8) —
 * plusieurs langues, plusieurs plateformes — et le lecteur choisit.
 *
 * `confiance` n'est pas décoratif : `verifiee` signifie qu'on a obtenu une réponse de la
 * plateforme, `declaree` que la source l'annonce sans qu'on l'ait rejouée. Une source non
 * vérifiée ne doit pas se présenter comme un fait.
 */
export interface SourceEpisode {
  episodeId: number;
  /**
   * Saison et numéro de l'épisode servi — la clé par laquelle le lecteur retrouve ses sources.
   *
   * Et pas `episodeId` : plusieurs lignes d'`episodes` décrivent le MÊME épisode (une par
   * langue et par chaîne), chacune avec son propre `id`. Indexer par `id` ne rendrait donc que
   * les sources de la variante affichée, en perdant les autres langues — exactement celles que
   * le sélecteur doit proposer.
   */
  saison: number;
  episode: number | null;
  /** `page` désigne une page à ouvrir dans le navigateur, pas un lecteur intégrable. */
  plateforme: "youtube" | "dailymotion" | "page";
  sourceId: string;
  url: string;
  /** `vo` | `vf` | `vostfr` | `en` | `es` | `unknown` — familles de `lib/sources.ts`. */
  langue: string;
  /** Renseignée par la plateforme quand elle la donne ; souvent absente. */
  qualite: string | null;
  officielle: number;
  confiance: "verifiee" | "declaree" | "deduite";
  /**
   * D'où vient la source, en clair — « inazuma-eleven.fr (official) », « Inazuma TV FR
   * (Dailymotion) ». C'est une PROVENANCE lisible, **pas** un identifiant technique : ne jamais
   * la mettre dans une URL. L'adresse du lecteur, quand la vidéo y est restreinte, est déjà
   * dans `url`.
   */
  origine: string | null;
  vignette: string | null;
}

/** Une saison, telle que nommée par la source (« GO », « Chrono Stones », « Films »…). */
export interface SaisonAnime {
  saison: number;
  nom: string;
  total: number;
}

/**
 * Un lot de catalogue venu du VPS (`GET /api/ietv`), tel que `fusionner` l'attend.
 *
 * Les champs portent les noms de la ROUTE, pas ceux des colonnes : c'est un contrat entre deux
 * machines, et le renommer côté serveur casserait le client silencieusement si les deux
 * partageaient les mêmes noms par coïncidence.
 */
export interface LotCatalogue {
  genere: number;
  chaines: { id: number; channel: string; title: string | null }[];
  saisons: { chaineId: number; saison: number; nom: string; total: number }[];
  episodes: {
    saison: number;
    episode: number | null;
    videoId: string;
    titre: string;
    url: string;
    description: string | null;
    titreJp: string | null;
    romaji: string | null;
    vignette: string | null;
    publie: string | null;
    langue: string | null;
    duree: number | null;
    /** Nom de la chaîne (`inazuma-eleven.fr (official)`) — la seule clé stable entre les deux bases. */
    chaine: string;
    /** `createdAt` du serveur : conservé tel quel, c'est le `since` de la prochaine requête. */
    moissonne: number;
  }[];
}

let promesseDb: Promise<Database> | null = null;
let cheminOuvert: string | null = null;

/** sqlx veut des `/`, pas des `\` — même conversion que `wikiDb`/`reDb`. */
function uriSqlite(chemin: string): string {
  return `sqlite:${chemin.replace(/\\/g, "/")}`;
}

function connect(chemin: string): Promise<Database> {
  if (promesseDb && cheminOuvert === chemin) return promesseDb;
  cheminOuvert = chemin;
  promesseDb = Database.load(uriSqlite(chemin));
  return promesseDb;
}

/** Chemin résolu de la base (commande Rust `default_anime_db`), ou `null` si aucune n'existe. */
export function defaultAnimeDbPath(gameDir?: string): Promise<string | null> {
  return api.defaultAnimeDb(gameDir);
}

export const animeDb = {
  /**
   * Les saisons, dans l'ordre de diffusion.
   *
   * Le total vient d'un décompte des épisodes RÉELLEMENT présents, pas de la colonne
   * `seasons.totalEpisodes` : celle-ci porte ce que la chaîne annonce, et une saison partiellement
   * moissonnée afficherait un nombre d'épisodes qu'on ne saurait pas ouvrir.
   */
  async saisons(chemin: string): Promise<SaisonAnime[]> {
    const d = await connect(chemin);
    return d.select<SaisonAnime[]>(
      `SELECT s.season AS saison,
              COALESCE(s.name, 'Saison ' || s.season) AS nom,
              (SELECT count(*) FROM episodes e WHERE e.season = s.season) AS total
         FROM seasons s
        GROUP BY s.season
       HAVING total > 0
        ORDER BY s.season`,
    );
  },

  /** Les épisodes d'une saison, dans l'ordre. */
  async episodes(chemin: string, saison: number): Promise<EpisodeAnime[]> {
    const d = await connect(chemin);
    return d.select<EpisodeAnime[]>(
      `SELECT id, season AS saison, episode, videoId, url, title AS titre, description,
              titleJp AS titreJp, romaji,
              thumbnail AS vignette, publishDate AS publie, language AS langue, duration AS duree
         FROM episodes WHERE season = $1
        ORDER BY COALESCE(episode, 9999), id`,
      [saison],
    );
  },

  /**
   * Tous les épisodes, saison puis numéro — **un par épisode réel**, pas un par variante.
   *
   * `episodes` porte une ligne par (chaîne, saison, numéro, langue) : depuis que le catalogue est
   * multilingue, elle compte **931 lignes pour 355 épisodes** (390 en `vf`, 355 en `es`, 131 en
   * `en`, 42 en `vostfr`, 13 en `vo`). Rendre ces lignes telles quelles afficherait cinq fois le
   * même épisode dans la même rangée.
   *
   * On garde donc UNE ligne par (saison, numéro) — la VF d'abord, parce que le catalogue est
   * francophone et que c'est la seule langue complète sur les dix saisons. Les autres langues ne
   * sont pas perdues : elles sont devenues des entrées d'`episode_sources`, et c'est le sélecteur
   * du lecteur qui les propose.
   */
  async tous(chemin: string): Promise<EpisodeAnime[]> {
    const d = await connect(chemin);
    return d.select<EpisodeAnime[]>(
      `SELECT id, season AS saison, episode, videoId, url, title AS titre, description,
              titleJp AS titreJp, romaji,
              thumbnail AS vignette, publishDate AS publie, language AS langue, duration AS duree
         FROM episodes e
        WHERE e.id = (
                SELECT e2.id FROM episodes e2
                 WHERE e2.season = e.season
                   AND COALESCE(e2.episode, -1) = COALESCE(e.episode, -1)
                 ORDER BY CASE e2.language
                            WHEN 'vf' THEN 0 WHEN 'vostfr' THEN 1 WHEN 'vo' THEN 2 ELSE 3 END,
                          e2.id
                 LIMIT 1)
        ORDER BY season, COALESCE(episode, 9999), id`,
    );
  },

  /**
   * TOUTES les sources, en une requête — 1 770 lignes pour 355 épisodes.
   *
   * En une fois et pas par épisode : la vue tient déjà les 355 épisodes en mémoire, et cinq
   * sources chacun tiennent dans la même page. Une requête par épisode ouvert produirait 355
   * allers-retours pour la même information, et un temps d'attente à chaque ouverture de fiche.
   *
   * Le classement porte la préférence par défaut : une source vérifiée avant une source
   * seulement déclarée, une source officielle avant le reste, et YouTube avant Dailymotion
   * (l'intégration Dailymotion dépend d'un lecteur propriétaire, cf. `origine`).
   */
  async sources(chemin: string): Promise<SourceEpisode[]> {
    const d = await connect(chemin);
    return d.select<SourceEpisode[]>(
      `SELECT s.episode_id AS episodeId, e.season AS saison, e.episode AS episode,
              s.plateforme, s.sourceId, s.url, s.langue, s.qualite,
              s.officielle, s.confiance, s.origine, s.vignette
         FROM episode_sources s
         JOIN episodes e ON e.id = s.episode_id
        WHERE s.langue IN ('vo', 'vf', 'vostfr')
        ORDER BY e.season, COALESCE(e.episode, 9999),
                 CASE s.langue WHEN 'vo' THEN 0 WHEN 'vf' THEN 1 ELSE 2 END,
                 CASE s.confiance WHEN 'verifiee' THEN 0 WHEN 'declaree' THEN 1 ELSE 2 END,
                 s.officielle DESC,
                 CASE s.plateforme WHEN 'youtube' THEN 0 WHEN 'dailymotion' THEN 1 ELSE 2 END`,
    );
  },

  /**
   * Recherche plein texte, sur les quatre champs qui peuvent porter le nom d'un épisode.
   *
   * La vue Cinéma filtre en mémoire (elle a déjà les 355 épisodes) ; cette requête sert aux
   * appelants qui n'ont pas le catalogue sous la main — l'équivalent de `IETVCache.search` côté
   * bot, avec la même portée de champs que celle qu'`IETVCache` couvre désormais.
   */
  async chercher(chemin: string, q: string, limite = 200): Promise<EpisodeAnime[]> {
    const terme = q.trim().replace(/[%_]/g, "");
    if (terme.length < 2) return [];
    const d = await connect(chemin);
    return d.select<EpisodeAnime[]>(
      `SELECT id, season AS saison, episode, videoId, url, title AS titre, description,
              titleJp AS titreJp, romaji,
              thumbnail AS vignette, publishDate AS publie, language AS langue, duration AS duree
         FROM episodes
        WHERE title LIKE $1 OR titleJp LIKE $1 OR romaji LIKE $1 OR description LIKE $1
        ORDER BY season, COALESCE(episode, 9999)
        LIMIT $2`,
      [`%${terme}%`, limite],
    );
  },

  /**
   * Date de la dernière moisson connue localement (`createdAt` le plus récent), en ms.
   *
   * C'est le `since` envoyé au VPS : le serveur ne renvoie alors que ce qui a été moissonné
   * après, donc quelques centaines d'octets pour un client à jour.
   */
  async derniereMoisson(chemin: string): Promise<number> {
    const d = await connect(chemin);
    const [r] = await d.select<{ v: number | null }[]>("SELECT max(createdAt) AS v FROM episodes");
    return r?.v ?? 0;
  },

  /**
   * Fusionne un lot venu du VPS dans la base locale.
   *
   * `INSERT OR REPLACE` et non un `ON CONFLICT` ciblé : la table porte DEUX contraintes uniques
   * (`videoId` seul, et `(channel_id, season, episode, language)`), et un épisode republié sous
   * un autre identifiant violerait la seconde pendant qu'on résout la première. Rien ne référence
   * `episodes.id`, donc remplacer la ligne n'a aucun effet de bord.
   *
   * `createdAt` conserve la valeur du SERVEUR : elle est ce que la prochaine requête enverra en
   * `since`. L'écraser par l'heure locale ferait redemander éternellement le même lot.
   *
   * Les écritures sont **séquentielles à dessein** (le lint le signale) : elles partent toutes
   * vers la même connexion SQLite, que `sqlx` sérialise de toute façon. Les lancer par
   * `Promise.all` n'accélérerait rien et ferait perdre l'ordre chaînes → saisons → épisodes, dont
   * dépendent les clés étrangères.
   */
  async fusionner(chemin: string, lot: LotCatalogue): Promise<{ chaines: number; episodes: number }> {
    const d = await connect(chemin);
    /** Identifiant DISTANT → identifiant local, et nom de chaîne → identifiant local. */
    const idParChaine = new Map<number, number>();
    const idParNom = new Map<string, number>();

    for (const c of lot.chaines) {
      await d.execute(
        `INSERT INTO channels (channel, title) VALUES ($1, $2)
         ON CONFLICT(channel) DO UPDATE SET title = excluded.title,
                                            updatedAt = cast(unixepoch() * 1000 as integer)`,
        [c.channel, c.title],
      );
      const [r] = await d.select<{ id: number }[]>("SELECT id FROM channels WHERE channel = $1", [
        c.channel,
      ]);
      // Les identifiants du VPS ne sont PAS ceux d'ici (deux AUTOINCREMENT indépendants) : la
      // correspondance se fait par le nom de chaîne, seule clé stable des deux côtés.
      if (r) {
        idParChaine.set(c.id, r.id);
        idParNom.set(c.channel, r.id);
      }
    }

    for (const s of lot.saisons) {
      const local = idParChaine.get(s.chaineId);
      if (local === undefined) continue;
      await d.execute(
        `INSERT INTO seasons (channel_id, season, name, totalEpisodes) VALUES ($1, $2, $3, $4)
         ON CONFLICT(channel_id, season) DO UPDATE SET name = excluded.name,
                                                       totalEpisodes = excluded.totalEpisodes`,
        [local, s.saison, s.nom, s.total],
      );
    }

    let episodes = 0;
    for (const e of lot.episodes) {
      // Un épisode dont la chaîne n'a pas été fusionnée est ignoré plutôt que rattaché au
      // hasard : `channel_id` porte une clé étrangère, et le rattacher à la mauvaise chaîne
      // ferait apparaître l'épisode sous une source qui ne l'a jamais publié.
      const local = idParNom.get(e.chaine);
      if (local === undefined) continue;
      await d.execute(
        `INSERT OR REPLACE INTO episodes
           (channel_id, season, episode, videoId, title, url, description, titleJp, romaji,
            thumbnail, publishDate, language, duration, createdAt)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)`,
        [
          local,
          e.saison,
          e.episode,
          e.videoId,
          e.titre,
          e.url,
          e.description,
          e.titreJp,
          e.romaji,
          e.vignette,
          e.publie,
          e.langue,
          e.duree,
          e.moissonne,
        ],
      );
      episodes += 1;
    }
    return { chaines: idParChaine.size, episodes };
  },

  /** Volumétrie, pour le tableau de bord et l'état de la vue Cinéma. */
  async stats(chemin: string): Promise<{ saisons: number; episodes: number }> {
    const d = await connect(chemin);
    const [r] = await d.select<{ saisons: number; episodes: number }[]>(
      `SELECT (SELECT count(DISTINCT season) FROM episodes) AS saisons,
              (SELECT count(*) FROM episodes) AS episodes`,
    );
    return r ?? { saisons: 0, episodes: 0 };
  },
};

// ── D'où vient réellement la vidéo d'un épisode ───────────────────────────────
//
// **Tous les épisodes ne sont PAS sur YouTube**, et le supposer cassait deux saisons entières.
// Mesuré le 2026-09-03 sur les 355 épisodes de la base :
//
// | Saison        | Épisodes | Sur YouTube |
// |---------------|---------:|------------:|
// | 1, 2, GO, Outer Code, Ares, Orion, Films | 201 | 201 |
// | 3             |       60 |          11 |
// | Chrono Stones |       51 |           0 |
// | Galaxy        |       43 |           0 |
//
// Soit **143 épisodes (40 %)** dont le `videoId` n'est pas un identifiant YouTube mais un jeton
// de la base (`off-galaxy-1`), dont l'`url` pointe la plateforme officielle
// (`inazuma-eleven.fr/tv/watch/...`) et dont la vignette est servie par Dailymotion. Les 143
// portent une vignette Dailymotion : l'identifiant de lecture y est lisible, donc ces épisodes
// sont parfaitement jouables — ils ne l'étaient pas parce qu'on les envoyait tous à YouTube.

/** Un identifiant YouTube fait onze caractères de l'alphabet base64url. */
const RE_YOUTUBE = /^[A-Za-z0-9_-]{11}$/;

/** Identifiant Dailymotion, tel que la vignette de la base le porte. */
const RE_DAILYMOTION = /dailymotion\.com\/thumbnail\/video\/([A-Za-z0-9]+)/;

export type Plateforme = "youtube" | "dailymotion" | "inconnue";

/** Où vit la vidéo de cet épisode. */
export function plateformeDe(ep: EpisodeAnime): Plateforme {
  if (RE_YOUTUBE.test(ep.videoId)) return "youtube";
  if (ep.vignette && RE_DAILYMOTION.test(ep.vignette)) return "dailymotion";
  return "inconnue";
}

/** L'identifiant de lecture Dailymotion, extrait de la vignette. */
export function idDailymotion(ep: EpisodeAnime): string | null {
  return ep.vignette ? (RE_DAILYMOTION.exec(ep.vignette)?.[1] ?? null) : null;
}

/**
 * URL d'intégration d'un épisode, quelle que soit sa plateforme.
 *
 * `youtube-nocookie` plutôt que `youtube` : le domaine sans cookie ne dépose rien tant que la
 * lecture n'a pas commencé — la vue Cinéma affiche des dizaines de vignettes, elle n'a aucune
 * raison d'ouvrir autant de traceurs. Dailymotion a son équivalent avec `sharing-enable=false`
 * et le suivi publicitaire désactivé.
 *
 * Rend `null` quand aucune intégration n'est possible : l'appelant propose alors d'ouvrir la
 * page officielle plutôt que d'afficher un cadre vide.
 */
export function urlIntegrationEpisode(ep: EpisodeAnime, depart?: number): string | null {
  const plateforme = plateformeDe(ep);
  if (plateforme === "youtube") {
    const p = new URLSearchParams({ autoplay: "1", rel: "0", modestbranding: "1" });
    if (depart && depart > 0) p.set("start", String(Math.floor(depart)));
    return `https://www.youtube-nocookie.com/embed/${ep.videoId}?${p}`;
  }
  if (plateforme === "dailymotion") {
    const id = idDailymotion(ep);
    if (!id) return null;
    const p = new URLSearchParams({ autoplay: "1", "queue-enable": "false", "sharing-enable": "false" });
    if (depart && depart > 0) p.set("start", String(Math.floor(depart)));
    return `https://www.dailymotion.com/embed/video/${id}?${p}`;
  }
  return null;
}

/**
 * L'URL d'intégration d'une SOURCE choisie — la forme à préférer depuis que chaque épisode en a
 * plusieurs.
 *
 * Trois cas, et le troisième n'est pas un échec :
 *
 *  * **YouTube** — `youtube-nocookie`, `rel=0` pour que la fin de la vidéo ne propose pas le
 *    catalogue de la plateforme.
 *  * **Dailymotion** — les 143 épisodes hors YouTube sont **restreints au lecteur officiel** :
 *    leur identifiant renvoie 404 sur l'API publique. Quand la source porte une `origine`
 *    (la clé de ce lecteur), on passe par lui ; c'est la seule adresse où ils se lisent.
 *  * **`page`** — il n'existe pas de lecteur intégrable, seulement une page à ouvrir. La
 *    fonction rend `null` pour que l'appelant propose « ouvrir » au lieu d'afficher un cadre
 *    noir en prétendant lire.
 */
export function urlIntegrationSource(source: SourceEpisode, depart?: number): string | null {
  if (source.plateforme === "youtube") {
    const p = new URLSearchParams({ autoplay: "1", rel: "0", modestbranding: "1" });
    if (depart && depart > 0) p.set("start", String(Math.floor(depart)));
    return `https://www.youtube-nocookie.com/embed/${source.sourceId}?${p}`;
  }
  if (source.plateforme === "dailymotion") {
    const p = new URLSearchParams({ autoplay: "1", "queue-enable": "false", "sharing-enable": "false" });
    if (depart && depart > 0) p.set("start", String(Math.floor(depart)));

    // **`url` porte DÉJÀ l'adresse du lecteur** quand la vidéo est restreinte à celui de la
    // chaîne : `…/player/<clé>.html?video=<id>` (143 sources sur ce corpus). On la reprend telle
    // quelle, en n'ajoutant que nos paramètres.
    //
    // Ne PAS reconstruire cette adresse à partir d'`origine` : cette colonne contient le nom
    // lisible de la chaîne (« inazuma-eleven.fr (official) »), pas une clé de lecteur. L'y
    // employer produisait `…/player/inazuma-eleven.fr (official).html` et le lecteur répondait
    // « Not found » — vu à l'écran.
    if (source.url.includes("/player/")) {
      const base = source.url.split("?")[0];
      p.set("video", source.sourceId);
      return `${base}?${p}`;
    }
    return `https://www.dailymotion.com/embed/video/${source.sourceId}?${p}`;
  }
  return null;
}

/**
 * La page publique de l'épisode — pour « ouvrir dans le navigateur ».
 *
 * C'est `url` de la base, pas une URL YouTube reconstruite : pour les 143 épisodes hors YouTube
 * elle désigne la plateforme officielle, seul endroit où ils se regardent en entier.
 */
export function urlExterne(ep: EpisodeAnime): string {
  return ep.url || `https://www.youtube.com/watch?v=${ep.videoId}`;
}

