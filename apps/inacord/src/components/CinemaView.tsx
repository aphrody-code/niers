// Cinéma — la médiathèque Inazuma Eleven : les dix saisons de la série ET les cinématiques du
// jeu, dans un seul catalogue à saisons.
//
// ## Deux sources, un seul catalogue
//
// La série vient de `data/anime/episodes.db` (355 épisodes, dix saisons nommées — Saison 1 à 3,
// GO, Chrono Stones, Galaxy, Outer Code, Ares, Orion, Films), que `packages/ietv` recense et que
// l'installeur embarque (cf. `lib/animeDb.ts`). Les cinématiques du jeu viennent du VFS.
//
// **`Victory Road` est présentée comme la saison qui suit les autres** : c'est ce qu'elle est
// pour qui regarde la série — la suite de l'histoire, dans un autre média. La ranger dans un
// onglet séparé aurait demandé de savoir, avant de chercher, si un passage est un épisode ou une
// cinématique ; ici la question ne se pose plus.
//
// Les deux sources ne se lisent pas de la même façon : un épisode est une vidéo YouTube (cadre
// d'intégration), une cinématique est un `.usm` démultiplexé par le lecteur natif. C'est la seule
// asymétrie visible, et elle l'est jusque dans les cartes (une vignette distante contre une
// affiche capturée à la volée).
//
// ## La forme : ce que font Netflix et Disney+, et ce qu'on leur emprunte
//
// * **« Qui regarde ? »** (`cinema/ChoixProfil`) — la progression, les reprises et « ma liste »
//   n'ont de sens que rapportées à quelqu'un. Une seule série de clés `localStorage` faisait que
//   deux personnes devant la même fenêtre s'effaçaient l'une l'autre.
// * **Un bandeau de tête qui tourne** (`cinema/HerosCarrousel`) — sans jamais ouvrir un
//   conteneur : il n'affiche que des vignettes distantes ou des affiches déjà capturées.
// * **Une fiche avant la lecture** (`cinema/FicheDetail`) — durée, résumé, position de reprise et
//   épisodes voisins, AVANT de démultiplexer jusqu'à 300 Mo pour s'apercevoir qu'on s'est trompé.
//   Le bouton ▶ des cartes court-circuite la fiche quand on sait déjà ce qu'on veut.
// * **Une navigation en pilules**, pas une barre de saisons. La rangée de pastilles qui listait
//   les onze saisons débordait, se tassait et ne disait plus rien ; les saisons s'atteignent
//   désormais par « tout voir » sur leur rangée, et se choisissent au sélecteur DANS la fiche —
//   comme sur les deux plateformes de référence.
//
// ## Coût, et ce qui en découle
//
// Une vignette n'est pas gratuite : l'obtenir demande de démultiplexer le conteneur et de le
// remuxer en MP4 (jusqu'à 300 Mo pour un chapitre). D'où trois décisions :
//
// * le catalogue s'ouvre **sans** lire un octet des conteneurs (`videoCatalog`) ;
// * durée, définition et codec arrivent film par film (`videoInfo`), pour les cartes visibles
//   seulement, via un `IntersectionObserver` et une file à un seul travailleur ;
// * la prévisualisation animée ne démarre qu'au **survol soutenu** — c'est le seul moment où
//   l'on sait que l'utilisateur veut vraiment voir ce film-là. La première image capturée reste
//   ensuite affichée comme affiche.
import {
  memo,
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

import { AvatarProfil, ChoixProfil } from "@/components/cinema/ChoixProfil";
import { FicheDetail } from "@/components/cinema/FicheDetail";
import { HerosCarrousel } from "@/components/cinema/HerosCarrousel";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/Icon";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { VideoPlayer, formaterDuree, urlVideo } from "@/components/VideoPlayer";
import { api } from "@/lib/api";
import {
  animeDb,
  defaultAnimeDbPath,
  plateformeDe,
  urlExterne,
  urlIntegrationEpisode,
  urlIntegrationSource,
  type EpisodeAnime,
  type SaisonAnime,
  type SourceEpisode,
} from "@/lib/animeDb";
import {
  CLE_VICTORY_ROAD,
  ecrireReprises,
  formaterOctets,
  INSTANT_AFFICHE,
  lireReprises,
  REPRISE_MIN,
  vignetteDe,
  type ElementCinema,
  type Reprises,
  type SaisonCinema,
} from "@/lib/cinema";
import { afficheConnue, demanderAffiche, poserAffiche, surAffiche, viderFile } from "@/lib/affiches";
import { showFilmContextMenu } from "@/lib/contextMenu";
import { verifier, type EtatMaj } from "@/lib/majCatalogue";
import { analyser, chercher, requeteVide } from "@/lib/recherche";
import {
  LANGUES,
  LANGUES_PROPOSEES,
  langueParCle,
  languesDisponibles,
  passeLangue,
  sourcesDe,
  type ContexteSources,
  type SourceLecture,
} from "@/lib/sources";
import {
  ecrireListe,
  ecrireProfilActif,
  lireListe,
  lireProfilActif,
  lireProfils,
  PROFIL_PRINCIPAL,
  type Profil,
} from "@/lib/profils";
import {
  decrireLacune,
  ecrireVus,
  empreinte,
  lacunesDeSaison,
  lireVus,
  prochainNonVu,
  titreCourt,
  voisins,
  type LacuneSaison,
} from "@/lib/serie";
import { useSettings } from "@/lib/settings";
import { cn } from "@/lib/utils";
import type { FilmDto } from "@/lib/bindings";

/** Vues de la navigation principale. Une saison ouverte porte sa propre clé (`s3`). */
const VUE_ACCUEIL = "__accueil__";
const VUE_SERIE = "__serie__";
const VUE_LISTE = "__liste__";

/** Valeur du filtre « toutes langues » — `base-ui` réserve la chaîne vide à l'absence de
 * sélection et refuse un `SelectItem value=""`, d'où ce jeton, traduit en `""` à la sortie. */
const TOUTES_LANGUES = "__toutes__";

/**
 * Range les sources par épisode, en conservant l'ordre de préférence rendu par SQL.
 *
 * L'ordre compte : la première source d'un épisode est celle que le lecteur ouvre par défaut,
 * la mieux établie (vérifiée avant déclarée, officielle avant le reste).
 */
/** Identité stable d'une source — ce que le sélecteur met en `value`. */
const cleDe = (s: SourceEpisode) => `${s.plateforme}:${s.sourceId}:${s.langue}`;

function indexerSources(sources: readonly SourceEpisode[]): Map<string, SourceEpisode[]> {
  const parEpisode = new Map<string, SourceEpisode[]>();
  for (const s of sources) {
    const cle = empreinte(s.saison, s.episode ?? -1);
    const liste = parEpisode.get(cle);
    if (liste) liste.push(s);
    else parEpisode.set(cle, [s]);
  }
  return parEpisode;
}

/** Délai de survol avant de lancer une prévisualisation, en millisecondes. */
const DELAI_APERCU = 550;

/** Délai de survol avant de PRÉCHARGER, en millisecondes — plus court : rien ne s'affiche. */
const DELAI_PRECHARGE = 150;

/** Nombre de titres mis en avant par le bandeau de tête. */
const MAX_HEROS = 7;

export function CinemaView({ onOpenFile }: { onOpenFile?: (path: string) => void }) {
  const [films, setFilms] = useState<FilmDto[]>([]);
  const [chargement, setChargement] = useState(true);
  const [erreur, setErreur] = useState<string | null>(null);
  const [recherche, setRecherche] = useState("");
  const [langue, setLangue] = useState<string>("");
  const [enLecture, setEnLecture] = useState<FilmDto | null>(null);
  const parametres = useSettings();

  // ── Profils ─────────────────────────────────────────────────────────────────
  const [profils, setProfils] = useState<Profil[]>(() => lireProfils());
  const [profilActif, setProfilActif] = useState<string | null>(() => lireProfilActif());
  const [changerProfil, setChangerProfil] = useState(false);
  const profil = profils.find((p) => p.id === profilActif) ?? null;
  const idProfil = profil?.id ?? PROFIL_PRINCIPAL;

  const [reprises, setReprises] = useState<Reprises>(() => lireReprises(lireProfilActif() ?? PROFIL_PRINCIPAL));
  /** Épisodes marqués vus, en empreintes `saison:episode` — même règle que le bot Discord. */
  const [vus, setVus] = useState<Set<string>>(() => lireVus(lireProfilActif() ?? PROFIL_PRINCIPAL));
  /** Clés des titres mis de côté — le `+` des deux plateformes de référence. */
  const [liste, setListe] = useState<Set<string>>(() => lireListe(lireProfilActif() ?? PROFIL_PRINCIPAL));

  // Changer de profil recharge les trois jeux de données d'un coup : rien n'est fusionné, rien
  // n'est écrit à la bascule — ce qui appartient à l'autre profil reste dans ses propres clés.
  useEffect(() => {
    setReprises(lireReprises(idProfil));
    setVus(lireVus(idProfil));
    setListe(lireListe(idProfil));
  }, [idProfil]);

  /** Catalogue de la série — vide tant que la base n'est pas résolue, ou si elle est absente. */
  const [episodes, setEpisodes] = useState<EpisodeAnime[]>([]);
  const [saisonsAnime, setSaisonsAnime] = useState<SaisonAnime[]>([]);
  /**
   * Les sources de lecture, indexées par épisode — 1 770 lignes pour 355 épisodes.
   *
   * Chargées avec le catalogue et gardées en mémoire : une requête par fiche ouverte ferait
   * attendre à chaque clic pour une information qui tient entière dans une page.
   */
  const [sourcesParEpisode, setSourcesParEpisode] = useState<Map<string, SourceEpisode[]>>(new Map());
  /**
   * La source que l'utilisateur a choisie dans le lecteur — `null` = celle par défaut.
   *
   * Une CLÉ (`plateforme:sourceId:langue`) et pas un rang : le catalogue peut se compléter
   * pendant la lecture, et un rang désignerait alors une autre vidéo sans prévenir.
   */
  const [cleSourceChoisie, setCleSourceChoisie] = useState<string | null>(null);
  /** La langue voulue dans le lecteur, retenue d'un épisode au suivant — `null` = indifférent. */
  const [languePreferee, setLanguePreferee] = useState<string | null>(null);
  // Échap referme le lecteur d'épisode. Le lecteur de cinématiques a déjà le sien
  // (`VideoPlayer`, où Échap est documenté dans le panneau d'aide) ; l'intégration n'en avait
  // aucun, et une iframe qui a le focus ne rend pas la touche à la fenêtre.
  useEffect(() => {
    const surTouche = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") setEnLectureEpisode(null);
    };
    window.addEventListener("keydown", surTouche);
    return () => window.removeEventListener("keydown", surTouche);
  }, []);
  /** Chemin résolu de la base des épisodes — ce que la mise à jour depuis le VPS fusionne. */
  const [cheminAnimeDb, setCheminAnimeDb] = useState<string | null>(null);
  /** Dernier verdict de la mise à jour : à jour, N épisodes ajoutés, hors ligne, indisponible. */
  const [maj, setMaj] = useState<EtatMaj | null>(null);
  /** Épisode ouvert dans le cadre d'intégration — l'équivalent série de `enLecture`. */
  const [enLectureEpisode, setEnLectureEpisode] = useState<EpisodeAnime | null>(null);
  /** Vue affichée : accueil, série, Victory Road, ma liste, ou une saison précise. */
  const [vue, setVue] = useState<string>(VUE_ACCUEIL);
  /** Le titre dont la fiche est ouverte, et la saison que cette fiche liste. */
  const [fiche, setFiche] = useState<ElementCinema | null>(null);
  const [saisonFiche, setSaisonFiche] = useState<string>("");

  /** Bascule « vu » d'un épisode, et le persiste aussitôt. */
  const basculerVu = useCallback(
    (saison: number, episode: number | null) => {
      if (episode === null) return;
      setVus((prec) => {
        const suivant = new Set(prec);
        const cle = empreinte(saison, episode);
        if (suivant.has(cle)) suivant.delete(cle);
        else suivant.add(cle);
        ecrireVus(suivant, idProfil);
        return suivant;
      });
    },
    [idProfil],
  );

  /** Ajoute ou retire un titre de « ma liste ». La clé est celle du catalogue unifié : chemin VFS
   * pour une cinématique, identifiant YouTube pour un épisode. */
  const basculerListe = useCallback(
    (cle: string) => {
      setListe((prec) => {
        const suivant = new Set(prec);
        if (suivant.has(cle)) suivant.delete(cle);
        else suivant.add(cle);
        ecrireListe(idProfil, suivant);
        return suivant;
      });
    },
    [idProfil],
  );

  // Le catalogue de la série se charge en parallèle de celui du jeu, et son absence n'est pas une
  // erreur : sans le jeu on garde les épisodes, sans la base d'épisodes on garde les cinématiques.
  // C'est ce qui permet à la vue de rester utile sur une machine qui n'a que l'un des deux.
  useEffect(() => {
    let vivant = true;
    defaultAnimeDbPath(parametres.gameDir)
      .then(async (chemin) => {
        if (!chemin || !vivant) return;
        const [tousEpisodes, saisonsLues, toutesSources] = await Promise.all([
          animeDb.tous(chemin),
          animeDb.saisons(chemin),
          // `catch` et pas `await` nu : une base antérieure à la table `episode_sources` doit
          // continuer d'ouvrir le catalogue, sans sélecteur, plutôt que de rester noire.
          animeDb.sources(chemin).catch(() => [] as SourceEpisode[]),
        ]);
        if (!vivant) return;
        setEpisodes(tousEpisodes);
        setSaisonsAnime(saisonsLues);
        setSourcesParEpisode(indexerSources(toutesSources));
        setCheminAnimeDb(chemin);

        // ── Mise à jour depuis le VPS ────────────────────────────────────────
        //
        // APRÈS l'affichage, jamais avant : le catalogue embarqué s'ouvre tout de suite, et se
        // complète si le serveur a du neuf. Un VPS injoignable ne retarde donc rien et ne
        // produit aucune erreur visible (`lib/majCatalogue.ts` rend un état, pas une exception).
        const etat = await verifier(chemin);
        if (!vivant || !etat) return;
        setMaj(etat);
        if (etat.etat === "maj") {
          const [apres, saisonsApres, sourcesApres] = await Promise.all([
            animeDb.tous(chemin),
            animeDb.saisons(chemin),
            animeDb.sources(chemin).catch(() => [] as SourceEpisode[]),
          ]);
          if (!vivant) return;
          setEpisodes(apres);
          setSaisonsAnime(saisonsApres);
          setSourcesParEpisode(indexerSources(sourcesApres));
        }
      })
      .catch(() => {});
    return () => {
      vivant = false;
    };
  }, [parametres.gameDir]);

  // File d'enrichissement : un seul film inspecté à la fois. Démultiplexer en parallèle ferait
  // lire plusieurs centaines de mégaoctets simultanément pour aucun gain d'affichage.
  const file = useRef<string[]>([]);
  const occupe = useRef(false);
  const demandes = useRef(new Set<string>());

  useEffect(() => {
    let vivant = true;
    api
      .videoCatalog()
      .then((c) => {
        if (vivant) setFilms(c.films);
        return c;
      })
      .catch((e: unknown) => vivant && setErreur(String(e)))
      .finally(() => vivant && setChargement(false));
    return () => {
      vivant = false;
    };
  }, []);

  /**
   * Les chemins réellement présents sous `data/dx11/movie`.
   *
   * Mesuré le 2026-09-03 : le VFS porte 196 `.usm` pour 97 films — chacun existe sous `common`
   * ET sous `dx11`, et le catalogue Rust ne retient que `common` (`video.rs`). La seconde source
   * existait donc sans que rien ne la propose. On la MESURE plutôt que de la deviner : offrir un
   * montage absent produirait une lecture qui échoue.
   */
  const [dx11, setDx11] = useState<ReadonlySet<string>>(() => new Set<string>());

  useEffect(() => {
    let vivant = true;
    api
      .find("dx11/movie", "usm", 500)
      .then((entrees) => {
        if (vivant) setDx11(new Set(entrees.map((e) => e.path)));
        return entrees;
      })
      // Un VFS sans montage `dx11` n'est pas une erreur : le sélecteur de source n'aura
      // simplement qu'une entrée par langue.
      .catch(() => {});
    return () => {
      vivant = false;
    };
  }, []);

  /** « Vérifier maintenant » : le même chemin que le contrôle automatique, sans son espacement. */
  const verifierMaintenant = useCallback(() => {
    const chemin = cheminAnimeDb;
    if (!chemin) return;
    setMaj(null);
    void verifier(chemin, true).then(async (etat) => {
      if (!etat) return;
      setMaj(etat);
      if (etat.etat === "maj") {
        const [apres, saisonsApres, sourcesApres] = await Promise.all([
          animeDb.tous(chemin),
          animeDb.saisons(chemin),
          animeDb.sources(chemin).catch(() => [] as SourceEpisode[]),
        ]);
        setEpisodes(apres);
        setSaisonsAnime(saisonsApres);
        setSourcesParEpisode(indexerSources(sourcesApres));
      }
      return etat;
    });
  }, [cheminAnimeDb]);

  // Changer de vue vide la file des vignettes en attente : ce qui n'est plus à l'écran n'a plus
  // de raison d'être démultiplexé, et laisser la file finir ferait attendre les cartes de la
  // vue qu'on vient d'ouvrir derrière celles qu'on vient de quitter.
  useEffect(() => {
    viderFile();
  }, [vue]);

  const contexteSources = useMemo<ContexteSources>(
    () => ({ films, episodes, dx11 }),
    [films, episodes, dx11],
  );

  const traiterFile = useCallback(() => {
    if (occupe.current) return;
    const chemin = file.current.shift();
    if (!chemin) return;
    occupe.current = true;
    api
      .videoInfo(chemin)
      .then((inspecte) => {
        setFilms((prec) => prec.map((f) => (f.chemin === chemin ? inspecte : f)));
        return inspecte;
      })
      .catch(() => {})
      .finally(() => {
        occupe.current = false;
        traiterFile();
      });
  }, []);

  // ── Préchargement ───────────────────────────────────────────────────────────
  //
  // Précharger, c'est produire le conteneur web et le garder côté Rust : au clic suivant, la
  // lecture démarre sans attendre le démultiplexage. Un seul film à la fois, et seulement celui
  // que le curseur désigne — le cache ne tient que quatre entrées et 768 Mo, précharger « tout »
  // se contredirait lui-même en évinçant ce qu'on vient de préparer.
  const prechargeEnCours = useRef<string | null>(null);
  const dejaPrecharge = useRef(new Set<string>());

  const precharger = useCallback((chemin: string) => {
    if (dejaPrecharge.current.has(chemin) || prechargeEnCours.current) return;
    prechargeEnCours.current = chemin;
    api
      .videoPrecharger(chemin)
      .then((o) => {
        dejaPrecharge.current.add(chemin);
        return o;
      })
      // Un échec est normal ici (MPEG-2 sans conteneur web) : la carte le dit déjà, inutile
      // d'ajouter un toast pour un geste que l'utilisateur n'a pas explicitement demandé.
      .catch(() => {})
      .finally(() => {
        prechargeEnCours.current = null;
      });
  }, []);

  const enrichir = useCallback(
    (chemin: string) => {
      if (demandes.current.has(chemin)) return;
      demandes.current.add(chemin);
      file.current.push(chemin);
      traiterFile();
    },
    [traiterFile],
  );

  const menuFilm = useCallback(
    (f: FilmDto) => {
      void showFilmContextMenu({
        path: f.chemin,
        nom: f.nom,
        octets: f.octets,
        codec: f.codec,
        avecAudio: f.audio.length > 0,
        onLire: () => setEnLecture(f),
        onReveler: onOpenFile ? () => onOpenFile(f.chemin) : undefined,
      });
    },
    [onOpenFile],
  );

  const noterProgression = useCallback(
    (chemin: string, position: number, duree: number) => {
      if (position < REPRISE_MIN || duree <= 0) return;
      setReprises((prec) => {
        const suivant = { ...prec };
        // Un film vu à plus de 95 % n'est plus « en cours » : le proposer serait absurde.
        if (position / duree > 0.95) delete suivant[chemin];
        else suivant[chemin] = { position, duree };
        ecrireReprises(suivant, idProfil);
        return suivant;
      });
    },
    [idProfil],
  );

  // ── Filtrage et regroupement ────────────────────────────────────────────────

  /**
   * La requête analysée : filtres nommés (`s3e12`, `lang:vf`, `type:jeu`) et termes libres.
   *
   * Sur la valeur DIFFÉRÉE, pas sur la frappe : filtrer et classer 452 titres à chaque touche
   * faisait sauter le curseur du champ. `useDeferredValue` garde la saisie réactive et laisse
   * React re-classer quand il a le temps — la liste a un rendu de retard, la frappe aucune.
   */
  const rechercheDifferee = useDeferredValue(recherche);
  const requete = useMemo(() => analyser(rechercheDifferee), [rechercheDifferee]);
  const enRecherche = !requeteVide(requete);

  const estVu = useCallback(
    (el: ElementCinema) =>
      el.episode?.episode != null && vus.has(empreinte(el.episode.saison, el.episode.episode)),
    [vus],
  );

  /**
   * Le catalogue entier, les deux sources sur le même pied — AVANT toute recherche.
   *
   * Les films et les épisodes étaient filtrés séparément, par deux codes qui ne se ressemblaient
   * pas : trois champs d'un côté, quatre de l'autre, et aucun classement. Une seule liste, un
   * seul moteur (`lib/recherche.ts`), et la pertinence vaut pour les deux.
   */
  const tous = useMemo<ElementCinema[]>(() => {
    const sortie: ElementCinema[] = [];
    for (const e of episodes) {
      sortie.push({
        cle: e.videoId,
        // `titreCourt` et pas `e.titre` : la base préfixe chaque titre de « <saison> — Épisode
        // <n> - », en numérotation CONTINUE, quand le badge et `sousTitre` comptent par saison.
        titre: titreCourt(e.titre),
        sousTitre: e.episode ? `Épisode ${e.episode}` : null,
        source: "anime",
        saison: `s${e.saison}`,
        vignette: e.vignette,
        episode: e,
      });
    }
    for (const f of films) {
      sortie.push({
        cle: f.chemin,
        titre: f.nom,
        sousTitre: f.rubrique,
        source: "jeu",
        saison: CLE_VICTORY_ROAD,
        vignette: null,
        film: f,
      });
    }
    return sortie;
  }, [episodes, films]);

  /**
   * Les langues d'un épisode, d'après ses SOURCES — pas d'après sa ligne de catalogue.
   *
   * `episodes.language` ne peut porter qu'une valeur, et la déduplication retient la VF : les
   * 355 lignes affichées sont donc **toutes en `vf`**. Un filtre bâti dessus n'aurait qu'une
   * entrée. La vérité est dans `episode_sources`, où le même épisode existe en plusieurs
   * langues — mesuré : VF 355 épisodes, VOSTFR 42, VO 13.
   */
  const languesParEpisode = useMemo(() => {
    const par = new Map<string, Set<string>>();
    for (const [cle, sources] of sourcesParEpisode) {
      par.set(cle, new Set(sources.map((s) => s.langue)));
    }
    return par;
  }, [sourcesParEpisode]);

  /** Ce que la langue choisie et la recherche laissent passer, classé par pertinence. */
  const retenus = useMemo(() => {
    // Un ÉPISODE passe si l'une de ses sources est dans la langue voulue ; `passeLangue` ne
    // regarde que `episodes.language`, qui vaut `vf` pour les 355 lignes affichées et ferait
    // donc disparaître le catalogue entier dès qu'on choisit VO ou VOSTFR. Les FILMS du jeu
    // n'ont pas de sources : pour eux, `passeLangue` reste la bonne réponse.
    const garde = (el: ElementCinema) => {
      if (!el.episode) return passeLangue(el, langue);
      const langues = languesParEpisode.get(empreinte(el.episode.saison, el.episode.episode ?? -1));
      // Aucune source connue : on ne masque pas l'épisode sur une information qu'on n'a pas.
      return langues === undefined || langues.size === 0 ? true : langues.has(langue);
    };
    const parLangue = langue ? tous.filter(garde) : tous;
    return chercher(parLangue, requete, estVu);
  }, [tous, langue, requete, estVu, languesParEpisode]);

  /** Les films retenus, dans l'ordre — c'est la file de lecture du lecteur. */
  const filtres = useMemo(
    () => retenus.map((el) => el.film).filter((f): f is FilmDto => f !== undefined),
    [retenus],
  );

  const rangees = useMemo(() => {
    const par = new Map<string, FilmDto[]>();
    for (const f of filtres) {
      const groupe = par.get(f.rubrique);
      if (groupe) groupe.push(f);
      else par.set(f.rubrique, [f]);
    }
    // Les rubriques nommées d'abord, puis les chapitres dans leur ordre numérique.
    // `sort` porte sur la copie que `[...entries()]` vient de créer — rien de partagé n'est muté.
    // (`toSorted` n'existe pas ici : cette application cible ES2022.)
    const ordre = (r: string) => (r.startsWith("Chapitre ") ? 2 : r === "Logos et intros" ? 0 : 1);
    return [...par.entries()].sort(([a], [b]) => ordre(a) - ordre(b) || a.localeCompare(b, "fr"));
  }, [filtres]);

  /**
   * Les langues RÉELLEMENT proposables, avec leur compte.
   *
   * Deux corpus se rejoignent ici et ne se comptent pas pareil : les films du jeu portent leur
   * code dans leur nom (`langueDisponibles`), les épisodes le portent dans leurs sources. On
   * additionne les deux, et `languesDisponibles` borne déjà l'ensemble à VO / VF / VOSTFR.
   */
  const languesDispo = useMemo(() => {
    const desFilms = languesDisponibles(films, []);
    const compte = new Map<string, number>();
    for (const langues of languesParEpisode.values()) {
      for (const l of langues) compte.set(l, (compte.get(l) ?? 0) + 1);
    }
    const cles = new Set<string>([...desFilms.map((d) => d.langue.cle), ...compte.keys()]);
    return LANGUES.filter((l) => LANGUES_PROPOSEES.includes(l.cle) && cles.has(l.cle)).map((langue) => ({
      langue,
      films: desFilms.find((d) => d.langue.cle === langue.cle)?.films ?? 0,
      episodes: compte.get(langue.cle) ?? 0,
    }));
  }, [films, languesParEpisode]);

  // ── Le catalogue unifié ─────────────────────────────────────────────────────
  //
  // Les saisons de la série d'abord, dans leur ordre de diffusion, puis Victory Road. L'ordre
  // n'est pas cosmétique : c'est la chronologie de la franchise, et la place du jeu à la fin est
  // exactement ce que la vue veut dire.
  const saisons = useMemo<SaisonCinema[]>(() => {
    const parSaison = new Map<string, ElementCinema[]>();
    for (const el of retenus) {
      const groupe = parSaison.get(el.saison);
      if (groupe) groupe.push(el);
      else parSaison.set(el.saison, [el]);
    }

    const sortie: SaisonCinema[] = saisonsAnime
      .map((s) => ({
        cle: `s${s.saison}`,
        titre: s.nom,
        source: "anime" as const,
        elements: parSaison.get(`s${s.saison}`) ?? [],
      }))
      .filter((s) => s.elements.length > 0);

    const vr = parSaison.get(CLE_VICTORY_ROAD);
    if (vr && vr.length > 0) {
      sortie.push({ cle: CLE_VICTORY_ROAD, titre: "Victory Road", source: "jeu", elements: vr });
    }
    return sortie;
  }, [retenus, saisonsAnime]);

  /** Tous les éléments du catalogue, par clé — la table que « ma liste » et la fiche consultent. */
  const parCle = useMemo(() => {
    const m = new Map<string, ElementCinema>();
    for (const s of saisons) for (const el of s.elements) m.set(el.cle, el);
    return m;
  }, [saisons]);

  const saisonsSerie = useMemo(() => saisons.filter((s) => s.source === "anime"), [saisons]);
  const saisonVR = useMemo(() => saisons.find((s) => s.cle === CLE_VICTORY_ROAD) ?? null, [saisons]);

  /** Les titres mis de côté, dans l'ordre du catalogue. */
  const maListe = useMemo(
    () => [...liste].map((c) => parCle.get(c)).filter((el): el is ElementCinema => el !== undefined),
    [liste, parCle],
  );

  const saisonCourante = useMemo(
    () => saisons.find((s) => s.cle === vue) ?? null,
    [saisons, vue],
  );

  /**
   * Les trous du catalogue, saison par saison — calculés sur le catalogue COMPLET (`episodes`) et
   * non sur le résultat filtré : une recherche qui laisse trois épisodes ne crée pas trente-huit
   * lacunes. C'est la règle du bot Discord, portée telle quelle (`lib/serie.ts`).
   */
  const lacunes = useMemo(() => {
    const parSaison = new Map<number, (number | null)[]>();
    for (const e of episodes) {
      const numeros = parSaison.get(e.saison);
      if (numeros) numeros.push(e.episode);
      else parSaison.set(e.saison, [e.episode]);
    }
    const sortie = new Map<number, LacuneSaison>();
    for (const [saison, numeros] of parSaison) {
      const l = lacunesDeSaison(saison, numeros);
      if (l) sortie.set(saison, l);
    }
    return sortie;
  }, [episodes]);

  /**
   * Le premier épisode de chaque saison — les portes d'entrée du catalogue.
   *
   * Remplace la rangée « les plus récents (diffusion) », qui classait 355 épisodes par date de
   * première diffusion : elle mettait donc toujours en avant la FIN de la série, ce qui est
   * exactement ce qu'on ne veut montrer à personne. Dix ouvertures de saison disent au moins par
   * où commencer.
   */
  const ouverturesSaisons = useMemo<ElementCinema[]>(() => {
    const sortie: ElementCinema[] = [];
    for (const s of saisonsAnime) {
      const premier = episodes.find((e) => e.saison === s.saison);
      if (!premier) continue;
      sortie.push({
        cle: premier.videoId,
        titre: titreCourt(premier.titre),
        sousTitre: s.nom,
        source: "anime",
        saison: `s${s.saison}`,
        vignette: premier.vignette,
        episode: premier,
      });
    }
    return sortie;
  }, [episodes, saisonsAnime]);

  /**
   * Le prochain épisode à regarder : le premier non vu de la première saison qui en a un. C'est
   * `prochainNonVu` du bot, appliqué saison par saison — donc jamais un numéro absent du
   * catalogue, et jamais « celui qui suit le dernier vu » quand un trou a été sauté.
   */
  const aReprendreSerie = useMemo(() => {
    for (const s of saisonsAnime) {
      const dansSaison = episodes.filter((e) => e.saison === s.saison);
      const numeros = dansSaison.map((e) => e.episode).filter((n): n is number => n !== null);
      const vusSaison = new Set(numeros.filter((n) => vus.has(empreinte(s.saison, n))));
      const prochain = prochainNonVu(numeros, vusSaison);
      if (prochain === null) continue;
      const ep = dansSaison.find((e) => e.episode === prochain);
      if (ep) return { episode: ep, saison: s };
    }
    return null;
  }, [saisonsAnime, episodes, vus]);

  /** Ouvre un élément, quelle que soit sa source — le seul endroit qui connaît les deux lecteurs. */
  const lire = useCallback((el: ElementCinema, source?: SourceLecture) => {
    setFiche(null);
    // La source choisie prime sur l'élément : c'est elle qui porte la langue et le montage
    // retenus (`dx11` plutôt que `common`, `JP` plutôt que `fr`). Sans source, on lit ce que la
    // carte désigne — le comportement d'avant le sélecteur.
    if (source?.film) {
      setEnLecture(source.film);
      return;
    }
    if (source?.episode) {
      setEnLectureEpisode(source.episode);
      return;
    }
    if (el.film) setEnLecture(el.film);
    else if (el.episode) setEnLectureEpisode(el.episode);
  }, []);

  /**
   * Ouvre la FICHE d'un titre — le geste par défaut d'un clic sur une carte.
   *
   * La saison que la fiche listera n'est pas toujours celle de l'élément : pour une cinématique,
   * la fratrie utile est sa RUBRIQUE (« Chapitre 3 »), pas les 97 films du jeu d'un bloc.
   */
  const ouvrirFiche = useCallback((el: ElementCinema) => {
    setSaisonFiche(el.film ? `rubrique:${el.film.rubrique}` : el.saison);
    setFiche(el);
  }, []);

  /**
   * Les titres du bandeau de tête.
   *
   * L'ordre dit quelque chose : ce qui reste à voir de la série, puis le plus gros morceau du
   * jeu, puis une porte d'entrée par saison. Un doublon serait un titre qui revient deux fois
   * dans le même carrousel — la table de clés l'empêche.
   *
   * La reprise de lecture n'y figure plus : elle ne concernait que les cinématiques du jeu, seule
   * source dont le lecteur natif rapporte une position (une intégration YouTube ou Dailymotion
   * n'en rapporte aucune). Le bandeau mettait donc systématiquement en avant un `.usm`.
   */
  const misEnAvant = useMemo<ElementCinema[]>(() => {
    const sortie: ElementCinema[] = [];
    const vues = new Set<string>();
    const pousser = (el: ElementCinema | undefined | null) => {
      if (!el || vues.has(el.cle) || sortie.length >= MAX_HEROS) return;
      vues.add(el.cle);
      sortie.push(el);
    };

    if (aReprendreSerie) pousser(parCle.get(aReprendreSerie.episode.videoId));
    // Le plus gros conteneur du jeu : c'est presque toujours une cinématique de chapitre, donc
    // ce que le catalogue a de plus proche d'un « film ».
    const vedette = films.length > 0 ? films.reduce((m, f) => (f.octets > m.octets ? f : m), films[0]!) : null;
    if (vedette) pousser(parCle.get(vedette.chemin));
    for (const el of ouverturesSaisons) pousser(el);
    return sortie;
  }, [aReprendreSerie, films, parCle, ouverturesSaisons]);

  useEffect(() => {
    // Le bandeau affiche la durée et la définition d'une cinématique : elles n'existent qu'après
    // inspection, et il n'est pas dans le champ d'un `IntersectionObserver` de carte.
    for (const el of misEnAvant) if (el.film && el.film.duree == null) enrichir(el.film.chemin);
  }, [misEnAvant, enrichir]);

  // ── Ce que la fiche affiche ─────────────────────────────────────────────────

  /** Les saisons proposées par le sélecteur de la fiche : les saisons de la série pour un
   * épisode, les rubriques du jeu pour une cinématique. */
  const saisonsFiche = useMemo<SaisonCinema[]>(() => {
    if (!fiche) return [];
    if (fiche.source === "anime") return saisonsSerie;
    return rangees.map(([rubrique, filmsRubrique]) => ({
      cle: `rubrique:${rubrique}`,
      titre: rubrique,
      source: "jeu" as const,
      elements: filmsRubrique.map((f) => ({
        cle: f.chemin,
        titre: f.nom,
        sousTitre: f.rubrique,
        source: "jeu" as const,
        saison: CLE_VICTORY_ROAD,
        vignette: null,
        film: f,
      })),
    }));
  }, [fiche, saisonsSerie, rangees]);

  /**
   * Les façons de regarder le titre ouvert : variantes de langue, montages `dx11`/`common`, et
   * — pour un épisode — les autres entrées du même numéro. La langue choisie dans la barre passe
   * en tête, c'est elle que « Lecture » lancera.
   */
  const sourcesFiche = useMemo(
    () => (fiche ? sourcesDe(fiche, contexteSources, langue || undefined) : []),
    [fiche, contexteSources, langue],
  );

  const fratrieFiche = useMemo(
    () => saisonsFiche.find((s) => s.cle === saisonFiche)?.elements ?? [],
    [saisonsFiche, saisonFiche],
  );

  // ── Rendu ───────────────────────────────────────────────────────────────────

  // « Qui regarde ? » : au premier passage, ou sur demande explicite. Le choix est mémorisé, donc
  // l'écran ne réapparaît pas à chaque ouverture de la vue.
  if (!profil || changerProfil) {
    return (
      <ChoixProfil
        profils={profils}
        onProfils={setProfils}
        onChoisir={(id) => {
          ecrireProfilActif(id);
          setProfilActif(id);
          setChangerProfil(false);
        }}
        onAnnuler={profil ? () => setChangerProfil(false) : undefined}
      />
    );
  }

  // Un épisode de la série se lit dans un cadre d'intégration : la vidéo est hébergée par la
  // chaîne officielle, l'application n'en détient ni le flux ni le droit de le redistribuer. Le
  // lecteur natif reste pour ce que le jeu contient, et lui seul.
  if (enLectureEpisode) {
    const e = enLectureEpisode;
    const dansSaison = episodes.filter((x) => x.saison === e.saison);
    const numeros = dansSaison.map((x) => x.episode).filter((n): n is number => n !== null);
    // `voisins` du bot, et non un `index ± 1` : il encadre correctement un épisode retiré du
    // catalogue entre-temps, et ne propose jamais un numéro qui n'existe pas.
    const { precedent, suivant } = e.episode !== null ? voisins(numeros, e.episode) : { precedent: null, suivant: null };
    const parNumero = (n: number | null) => (n === null ? null : (dansSaison.find((x) => x.episode === n) ?? null));
    const nomSaison = saisonsAnime.find((s) => s.saison === e.saison)?.nom ?? `Saison ${e.saison}`;
    const vu = e.episode !== null && vus.has(empreinte(e.saison, e.episode));

    // ── Les sources de CET épisode ────────────────────────────────────────────
    //
    // Cinq en moyenne (min 4, max 8), en plusieurs langues et sur trois plateformes. L'ordre
    // vient de SQL et porte la préférence : vérifiée avant déclarée, officielle avant le reste.
    const sourcesEp = sourcesParEpisode.get(empreinte(e.saison, e.episode ?? -1)) ?? [];
    /** Les langues réellement disponibles POUR CET ÉPISODE — pas la liste théorique. */
    const languesEp = [...new Set(sourcesEp.map((s) => s.langue))];
    /**
     * La langue retenue : celle qu'on préfère si cet épisode l'a, sinon la VF, sinon ce qu'il a.
     *
     * Le repli sur `vf` est explicite et ne doit pas devenir « la première de la liste » : le
     * sélecteur les range vo, vf, vostfr, or la VO ne couvre que 13 épisodes quand la VF les
     * couvre tous. Prendre la première ouvrirait donc en VO les rares épisodes qui en ont une,
     * et en VF tous les autres — la langue changerait d'un épisode à l'autre sans qu'on l'ait
     * demandé.
     */
    const langueEp =
      languePreferee && languesEp.includes(languePreferee)
        ? languePreferee
        : (languesEp.find((l) => l === "vf") ?? languesEp[0] ?? null);
    const sourcesLangue = sourcesEp.filter((s) => s.langue === langueEp);
    /** Le choix explicite s'il vaut encore pour cet épisode, sinon la première — la mieux établie. */
    const sourceActive =
      sourcesLangue.find((s) => cleDe(s) === cleSourceChoisie) ?? sourcesLangue[0] ?? null;

    return (
      <div className="flex h-full min-h-0 flex-col bg-black">
        <div className="flex items-center gap-2 border-b border-white/10 px-4 py-2">
          {/* Le retour est le premier élément du bandeau, à gauche du titre : c'est là qu'on le
              cherche, et c'est le geste le plus fréquent une fois l'épisode fini. */}
          <Button
            variant="outline"
            size="sm"
            className="shrink-0 border-white/20 bg-white/5 text-white hover:bg-white/10"
            onClick={() => setEnLectureEpisode(null)}
            title="Retour au catalogue (Échap)"
          >
            <Icon name="arrow_back" size={16} />
            Retour
          </Button>
          <div className="min-w-0 flex-1">
            {/* `titreCourt` : la ligne du dessous donne déjà la saison et le numéro. Sans lui, le
                bandeau affichait « Chrono Stones — Épisode 48 - … » AU-DESSUS de « Chrono Stones ·
                épisode 1 » — deux numérotations du même épisode, à deux lignes d'écart. */}
            <div className="truncate text-sm font-semibold text-white">{titreCourt(e.titre)}</div>
            <div className="truncate text-xs text-white/50">
              {nomSaison}
              {e.episode ? ` · épisode ${e.episode}` : ""}
              {e.publie ? ` · ${new Date(e.publie).toLocaleDateString("fr-FR")}` : ""}
            </div>
          </div>

          {/* ── Langue et source ──────────────────────────────────────────────
              Les deux sélecteurs n'apparaissent QUE s'il y a un choix à faire : un épisode qui
              n'existe qu'en une langue afficherait sinon un menu à une entrée, qui promet un
              choix inexistant. Le second est étiqueté par la plateforme, parce que c'est ce qui
              change réellement d'une source à l'autre — la qualité, elle, n'est presque jamais
              renseignée par les plateformes, et l'annoncer serait inventer. */}
          {languesEp.length > 1 && (
            <Select
              value={langueEp ?? undefined}
              onValueChange={(v) => {
                setLanguePreferee(v);
                setCleSourceChoisie(null);
              }}
            >
              <SelectTrigger className="h-7 w-auto min-w-[5.5rem] border-white/15 bg-white/5 text-xs text-white/85">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {languesEp.map((l) => (
                  <SelectItem key={l} value={l}>
                    {langueParCle(l)?.libelle ?? l.toUpperCase()}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}

          {sourcesLangue.length > 1 && sourceActive && (
            <Select value={cleDe(sourceActive)} onValueChange={(v) => setCleSourceChoisie(v)}>
              <SelectTrigger className="h-7 w-auto min-w-[7rem] border-white/15 bg-white/5 text-xs text-white/85">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {sourcesLangue.map((s) => (
                  <SelectItem key={cleDe(s)} value={cleDe(s)}>
                    {s.plateforme === "page" ? "Page officielle" : s.plateforme}
                    {s.qualite ? ` · ${s.qualite}` : ""}
                    {s.confiance === "verifiee" ? " ✓" : ""}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}

          <Button
            variant="ghost"
            size="sm"
            className={vu ? "text-emerald-400" : "text-white/80 hover:text-white"}
            onClick={() => basculerVu(e.saison, e.episode)}
            disabled={e.episode === null}
            title={vu ? "Marquer comme non vu" : "Marquer comme vu"}
          >
            <Icon name={vu ? "check_circle" : "radio_button_unchecked"} size={16} />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="text-white/80 hover:text-white"
            disabled={precedent === null}
            onClick={() => {
              const p = parNumero(precedent);
              if (p) setEnLectureEpisode(p);
            }}
            title="Épisode précédent"
          >
            <Icon name="skip_previous" size={16} />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="text-white/80 hover:text-white"
            disabled={suivant === null}
            onClick={() => {
              const s = parNumero(suivant);
              if (s) setEnLectureEpisode(s);
            }}
            title="Épisode suivant"
          >
            <Icon name="skip_next" size={16} />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="text-white/80 hover:text-white"
            onClick={() => void openUrl(urlExterne(e))}
            title={
              plateformeDe(e) === "youtube"
                ? "Ouvrir sur YouTube"
                : "Ouvrir sur la plateforme officielle"
            }
          >
            <Icon name="open_in_new" size={16} />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="text-white/80 hover:text-white"
            onClick={() => setEnLectureEpisode(null)}
            title="Fermer"
          >
            <Icon name="close" size={16} />
          </Button>
        </div>
        {/* L'intégration dépend de la PLATEFORME, et pas toujours de YouTube : 143 épisodes sur
            355 (Chrono Stones et Galaxy en entier, 49 de la saison 3) vivent sur la plateforme
            officielle et se lisent par Dailymotion. Les envoyer tous à `youtube-nocookie` donnait
            un cadre vide — c'est ce qui faisait paraître ces deux saisons en panne. */}
        {(() => {
          // La SOURCE choisie d'abord ; `urlIntegrationEpisode` ne sert plus que de repli pour
          // une base antérieure à `episode_sources`, où l'épisode n'a que son `videoId`.
          const integration = sourceActive
            ? urlIntegrationSource(sourceActive)
            : urlIntegrationEpisode(e);
          if (!integration) {
            return (
              <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 bg-black px-6 text-center">
                <Icon name="movie" size={40} className="text-white/30" />
                <p className="max-w-md text-sm text-white/70">
                  {sourceActive
                    ? "Cette source n'a pas de lecteur intégrable : elle désigne une page à ouvrir."
                    : "Cet épisode n'a aucune source intégrable dans la base."}
                </p>
                <Button
                  variant="outline"
                  onClick={() => void openUrl(sourceActive?.url ?? urlExterne(e))}
                >
                  <Icon name="open_in_new" size={16} />
                  Ouvrir la page officielle
                </Button>
              </div>
            );
          }
          return (
            <iframe
              // La clé porte la SOURCE : changer de langue ou de plateforme doit remonter un
              // nouveau lecteur. Avec `e.videoId`, React gardait l'iframe et sa vidéo d'origine.
              key={sourceActive ? `${sourceActive.plateforme}:${sourceActive.sourceId}` : e.videoId}
              src={integration}
              title={e.titre}
              allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; fullscreen"
              allowFullScreen
              className="min-h-0 flex-1 border-0 bg-black"
            />
          );
        })()}
        {/* La fiche sous la vidéo : titre original, transcription et résumé. Trois colonnes de la
            base que rien n'affichait — 330 titres japonais et 355 résumés dormaient dans le
            fichier. */}
        {(e.titreJp || e.romaji || e.description) && (
          <div className="max-h-40 shrink-0 overflow-y-auto border-t border-white/10 px-4 py-2">
            {(e.titreJp || e.romaji) && (
              <div className="mb-1 flex flex-wrap items-baseline gap-2 text-xs">
                {e.titreJp && <span className="text-white/85">{e.titreJp}</span>}
                {e.romaji && <span className="italic text-white/45">{e.romaji}</span>}
              </div>
            )}
            {e.description && <p className="text-xs leading-relaxed text-white/60">{e.description}</p>}
          </div>
        )}
      </div>
    );
  }

  if (enLecture) {
    const f = enLecture;
    const detail = [
      f.rubrique,
      f.largeur ? `${f.largeur}×${f.hauteur}` : null,
      f.codec?.toUpperCase(),
      f.cadence ? `${f.cadence.toFixed(3)} i/s` : null,
    ]
      .filter(Boolean)
      .join(" · ");
    // La file de lecture est la LISTE AFFICHÉE (`filtres`), pas le catalogue entier : « suivant »
    // doit désigner ce que l'utilisatrice voit à l'écran, filtres de recherche et de langue
    // compris. Un film joué puis exclu par un changement de filtre sort de la file (`-1`) — les
    // boutons disparaissent alors plutôt que de sauter à un film sans rapport.
    const rang = filtres.findIndex((x) => x.chemin === f.chemin);
    const versRang = (delta: number) => {
      const cible = filtres[rang + delta];
      if (cible) setEnLecture(cible);
    };
    return (
      <VideoPlayer
        chemin={f.chemin}
        titre={f.nom}
        detail={detail}
        film={f}
        avecAudio={f.audio.length > 0}
        depart={reprises[f.chemin]?.position}
        onProgression={(p, d) => noterProgression(f.chemin, p, d)}
        onClose={() => setEnLecture(null)}
        file={rang >= 0 ? { index: rang, total: filtres.length } : null}
        onPrecedent={rang > 0 ? () => versRang(-1) : undefined}
        onSuivant={rang >= 0 && rang < filtres.length - 1 ? () => versRang(1) : undefined}
      />
    );
  }

  const technique = !profil.jeunesse;
  const accueil = vue === VUE_ACCUEIL;

  /** Les rangées de la vue courante — le seul endroit qui décide de ce qui est affiché. */
  const rangeesElements: SaisonCinema[] =
    vue === VUE_SERIE ? saisonsSerie : accueil ? saisons : [];

  return (
    // `overflow-x-hidden` + `min-w-0` : sans eux, ce conteneur `flex-col` prend la largeur de son
    // enfant le plus LARGE — une rangée de cartes déborde toujours — et la barre collante s'étire
    // d'autant. Tout ce qu'elle aligne à droite (la recherche, le sélecteur de langue) partait
    // alors hors du viewport : les pilules restaient visibles à gauche, et on croyait ces deux
    // contrôles absents alors qu'ils étaient rendus, hors champ. Chaque rangée garde son propre
    // défilement horizontal, qui n'est pas affecté.
    <div className="flex h-full min-h-0 w-full min-w-0 flex-col overflow-y-auto overflow-x-hidden bg-app">
      {/* ── La barre ────────────────────────────────────────────────────────────
          Ni titre ni compteur : ce qui compte est la navigation et la recherche. Elle est
          translucide et floute ce qui défile dessous, comme celle de Netflix. */}
      <div className="sticky top-0 z-20 flex items-center gap-2 border-b border-app-line bg-app/80 px-4 py-2 backdrop-blur">
        {/* Les pilules de Disney+ : quatre destinations, pas onze saisons. */}
        <nav className="flex items-center gap-0.5 rounded-full bg-app-box/80 p-0.5">
          <Pilule titre="Accueil" actif={accueil} onClick={() => setVue(VUE_ACCUEIL)} />
          {saisonsSerie.length > 0 && (
            <Pilule
              titre="Série"
              actif={vue === VUE_SERIE || vue.startsWith("s")}
              onClick={() => setVue(VUE_SERIE)}
            />
          )}
          {saisonVR && (
            <Pilule
              titre="Victory Road"
              actif={vue === CLE_VICTORY_ROAD}
              onClick={() => setVue(CLE_VICTORY_ROAD)}
            />
          )}
          {maListe.length > 0 && (
            <Pilule titre="Ma liste" actif={vue === VUE_LISTE} onClick={() => setVue(VUE_LISTE)} />
          )}
        </nav>

        <div className="flex-1" />

        <IndicateurMaj etat={maj} onVerifier={verifierMaintenant} />

        <button
          type="button"
          onClick={() => setChangerProfil(true)}
          title={`${profil.nom} — changer de profil`}
          aria-label={`Profil ${profil.nom} — changer de profil`}
          className="rounded-lg outline-none transition-transform hover:scale-105 focus-visible:ring-2 focus-visible:ring-accent"
        >
          <AvatarProfil profil={profil} taille={26} />
        </button>
      </div>

      {/* ── Les filtres, sur la page ────────────────────────────────────────────
          Ils vivaient dans la barre au-dessus, poussés à droite par un `flex-1`. Dans ce
          conteneur, tout ce qui est aligné à droite finissait hors champ : la recherche, le
          sélecteur de langue, et jusqu'à l'indicateur de mise à jour n'étaient jamais visibles.
          On les croyait absents ; ils étaient rendus, ailleurs.

          Ici ils sont dans le FLUX, sur toute la largeur, alignés à GAUCHE : rien ne peut les
          repousser, et ils ne dépendent plus de la largeur de la fenêtre. C'est aussi la place
          qu'ils méritent — filtrer un catalogue n'est pas une action secondaire. */}
      <div className="flex w-full min-w-0 flex-wrap items-center gap-2 border-b border-app-line bg-app-dark-box px-4 py-2">
        <div className="relative min-w-0 flex-1 sm:max-w-md">
          <Icon
            name="search"
            size={14}
            className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-ink-faint"
          />
          <Input
            value={recherche}
            onChange={(e) => setRecherche(e.target.value)}
            placeholder="Rechercher un épisode, un film, une technique…"
            className="h-8 w-full border-app-line bg-app-box pl-7 pr-7 text-xs"
            title={
              "Filtres reconnus : s3e12 · s:3 · e:12 · lang:vf|vo|vostfr · type:jeu|serie · " +
              "chapitre:5 · st:oui · vu:non. Tout le reste est cherché dans le titre, le titre " +
              "japonais, la transcription et le résumé."
            }
          />
          {recherche && (
            <button
              type="button"
              onClick={() => setRecherche("")}
              aria-label="Effacer la recherche"
              className="absolute right-1 top-1/2 -translate-y-1/2 rounded p-0.5 text-ink-faint hover:text-ink"
            >
              <Icon name="close" size={13} />
            </button>
          )}
        </div>

        {/* VO / VF / VOSTFR, et seulement ce que le catalogue contient vraiment, avec son
            compte : annoncer une langue qui ne filtrerait rien est une promesse vide. */}
        {languesDispo.length > 0 && (
          <Select
            value={langue || TOUTES_LANGUES}
            onValueChange={(v) => setLangue(v === TOUTES_LANGUES ? "" : (v ?? ""))}
          >
            <SelectTrigger size="sm" className="h-8 w-44 text-xs" aria-label="Choisir la langue">
              <SelectValue placeholder="Toutes les langues" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={TOUTES_LANGUES}>Toutes les langues</SelectItem>
              {languesDispo.map(({ langue: l, films: nf, episodes: ne }) => (
                <SelectItem key={l.cle} value={l.cle}>
                  {l.libelle}
                  <span className="ml-2 text-ink-faint">
                    {[nf > 0 ? `${nf} film${nf > 1 ? "s" : ""}` : null, ne > 0 ? `${ne} ép.` : null]
                      .filter(Boolean)
                      .join(" · ")}
                  </span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}

        {/* Le compte de ce que les filtres laissent passer : sans lui, une recherche qui ne rend
            rien ressemble à une vue qui s'est vidée toute seule. */}
        {(enRecherche || langue) && (
          <span className="text-tiny text-ink-faint">
            {filtres.length} résultat{filtres.length > 1 ? "s" : ""}
          </span>
        )}
      </div>

      {chargement && (
        <div className="flex flex-1 items-center justify-center text-sm text-ink-faint">
          Chargement du catalogue…
        </div>
      )}

      {/* `Alert` plutôt qu'un cadre rouge écrit à la main : c'est le composant que toutes les
          autres vues emploient pour dire qu'une source a échoué. */}
      {erreur && (
        <div className="m-4">
          <Alert variant="destructive">
            <AlertTitle>Catalogue indisponible</AlertTitle>
            <AlertDescription>{erreur}</AlertDescription>
          </Alert>
        </div>
      )}

      {!chargement && !erreur && films.length === 0 && episodes.length === 0 && (
        <div className="m-4">
          <Alert>
            <AlertTitle>Médiathèque vide</AlertTitle>
            <AlertDescription>
              Ni cinématique ni épisode : le VFS du jeu n'est pas monté (aucun fichier USM) et la
              base des épisodes est absente.
            </AlertDescription>
          </Alert>
        </div>
      )}

      {enRecherche && saisons.length === 0 && !chargement && (
        <div className="m-4">
          <Alert>
            <AlertTitle>Aucun résultat</AlertTitle>
            <AlertDescription>
              Rien ne correspond à « {recherche} », ni dans les titres du jeu ni dans les quatre
              champs qui nomment un épisode.
            </AlertDescription>
          </Alert>
        </div>
      )}

      {/* ── Accueil et Série : le bandeau, puis des rangées ──────────────────── */}
      {(accueil || vue === VUE_SERIE) && (
        <>
          {accueil && !enRecherche && misEnAvant.length > 0 && (
            <HerosCarrousel
              elements={misEnAvant}
              liste={liste}
              technique={technique}
              onLire={lire}
              onOuvrir={ouvrirFiche}
              onBasculerListe={basculerListe}
            />
          )}

          {/* Le prochain épisode non vu, en tête : c'est la question qu'on se pose devant une
              série de 355 épisodes, et la seule réponse que le catalogue seul ne donne pas. */}
          {aReprendreSerie && !enRecherche && (
            <div className="mx-4 mt-3 flex items-center gap-3 rounded-lg border border-app-line bg-app-box px-3 py-2">
              <Icon name="play_circle" size={20} className="text-accent" />
              <div className="min-w-0 flex-1">
                <div className="text-tiny uppercase tracking-wider text-ink-faint">Reprendre la série</div>
                <div className="truncate text-sm text-ink">
                  {aReprendreSerie.saison.nom}
                  {aReprendreSerie.episode.episode ? ` · épisode ${aReprendreSerie.episode.episode}` : ""} —{" "}
                  {titreCourt(aReprendreSerie.episode.titre)}
                </div>
              </div>
              <Button size="sm" onClick={() => setEnLectureEpisode(aReprendreSerie.episode)}>
                Lire
              </Button>
            </div>
          )}

          {accueil && maListe.length > 0 && !enRecherche && (
            <RangeeElements
              titre="Ma liste"
              elements={maListe.slice(0, 30)}
              total={maListe.length}
              reprises={reprises}
              onOuvrir={ouvrirFiche}
              onLire={lire}
              onVisible={enrichir}
              onPrecharger={precharger}
              onMenu={menuFilm}
              onTout={() => setVue(VUE_LISTE)}
              vus={vus}
            />
          )}


          {rangeesElements.map((s) => (
            <RangeeElements
              key={s.cle}
              titre={s.titre}
              elements={s.elements.slice(0, 30)}
              total={s.elements.length}
              reprises={reprises}
              onOuvrir={ouvrirFiche}
              onLire={lire}
              onVisible={enrichir}
              onPrecharger={precharger}
              onMenu={menuFilm}
              onTout={() => setVue(s.cle)}
              vus={vus}
            />
          ))}
        </>
      )}

      {/* ── Victory Road : ses rubriques, en rangées ─────────────────────────────
          « Chapitre 3 » et « Logos et intros » ne sont pas du même ordre, et les aplatir en une
          grille de 97 vignettes reviendrait à l'état que cette vue a justement remplacé. */}
      {vue === CLE_VICTORY_ROAD && (
        <>
          {rangees.map(([rubrique, listeFilms]) => (
            <RangeeElements
              key={rubrique}
              titre={rubrique}
              elements={listeFilms.map((f) => ({
                cle: f.chemin,
                titre: f.nom,
                sousTitre: f.rubrique,
                source: "jeu" as const,
                saison: CLE_VICTORY_ROAD,
                vignette: null,
                film: f,
              }))}
              total={listeFilms.length}
              reprises={reprises}
              onOuvrir={ouvrirFiche}
              onLire={lire}
              onVisible={enrichir}
              onPrecharger={precharger}
              onMenu={menuFilm}
              vus={vus}
            />
          ))}
        </>
      )}

      {/* ── Ma liste ─────────────────────────────────────────────────────────── */}
      {vue === VUE_LISTE && (
        <Grille
          titre="Ma liste"
          sousTitre={`${maListe.length} titre${maListe.length > 1 ? "s" : ""}`}
          elements={maListe}
          vus={vus}
          reprises={reprises}
          onOuvrir={ouvrirFiche}
          onLire={lire}
          onRetour={() => setVue(VUE_ACCUEIL)}
        />
      )}

      {/* ── Une saison ouverte ───────────────────────────────────────────────── */}
      {saisonCourante && saisonCourante.source === "anime" && (
        <Grille
          titre={saisonCourante.titre}
          sousTitre={`${saisonCourante.elements.length} épisodes`}
          elements={saisonCourante.elements}
          vus={vus}
          reprises={reprises}
          onOuvrir={ouvrirFiche}
          onLire={lire}
          onRetour={() => setVue(VUE_SERIE)}
        >
          {/* Ce que le catalogue N'A PAS. Le bot Discord le dit depuis toujours ; l'application le
              taisait, et une saison trouée y ressemblait à une saison courte. */}
          {(() => {
            const numSaison = saisonCourante.elements[0]?.episode?.saison;
            const lacune = numSaison === undefined ? undefined : lacunes.get(numSaison);
            if (!lacune) return null;
            return (
              <Alert className="mb-3">
                <AlertTitle>
                  {lacune.manquants.length} épisode{lacune.manquants.length > 1 ? "s" : ""} absent
                  {lacune.manquants.length > 1 ? "s" : ""} du catalogue
                </AlertTitle>
                <AlertDescription>
                  Entre les épisodes {lacune.borne.debut} et {lacune.borne.fin} : {decrireLacune(lacune)}. La
                  source ne les publie pas — ce n'est pas un défaut de lecture.
                </AlertDescription>
              </Alert>
            );
          })()}
        </Grille>
      )}

      <div className="h-6" />

      {fiche && (
        <FicheDetail
          element={fiche}
          fratrie={fratrieFiche}
          sources={sourcesFiche}
          saisons={saisonsFiche}
          saisonAffichee={saisonFiche}
          vus={vus}
          liste={liste}
          reprises={reprises}
          technique={technique}
          onLire={lire}
          onBasculerVu={(el) => el.episode && basculerVu(el.episode.saison, el.episode.episode)}
          onBasculerListe={basculerListe}
          onChoisirSaison={setSaisonFiche}
          onChoisirElement={setFiche}
          onPrecharger={precharger}
          onReveler={onOpenFile}
          onFermer={() => setFiche(null)}
        />
      )}
    </div>
  );
}

// ── Navigation ────────────────────────────────────────────────────────────────

/** Une destination de la barre — la pilule de Disney+, pas la pastille de saison qu'elle
 * remplace : quatre entrées tiennent sur une ligne, onze débordaient et se tassaient. */
function Pilule({ titre, actif, onClick }: { titre: string; actif: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={actif}
      className={cn(
        "rounded-full px-3 py-1 text-xs font-medium transition-colors",
        actif ? "bg-ink text-app" : "text-ink-dull hover:bg-app-hover hover:text-ink",
      )}
    >
      {titre}
    </button>
  );
}

/**
 * L'état de la mise à jour du catalogue, en un point et une info-bulle.
 *
 * Volontairement minuscule : une mise à jour réussie n'est pas une nouvelle, c'est le
 * fonctionnement normal. Seul le cas « N épisodes ajoutés » mérite d'être lisible sans survoler,
 * parce qu'il explique pourquoi le catalogue vient de changer sous les yeux.
 */
function IndicateurMaj({ etat, onVerifier }: { etat: EtatMaj | null; onVerifier: () => void }) {
  const couleur =
    etat === null
      ? "bg-ink-faint/40"
      : etat.etat === "maj"
        ? "bg-accent"
        : etat.etat === "a-jour"
          ? "bg-emerald-500/70"
          : "bg-status-warning/70";
  const titre =
    etat === null
      ? "Catalogue : contrôle non effectué. Cliquer pour vérifier maintenant."
      : etat.etat === "maj"
        ? `${etat.ajoutes} épisode${etat.ajoutes > 1 ? "s" : ""} ajouté${etat.ajoutes > 1 ? "s" : ""} depuis le serveur.`
        : etat.etat === "a-jour"
          ? "Catalogue à jour."
          : etat.etat === "hors-ligne"
            ? "Serveur injoignable — le catalogue embarqué reste utilisable."
            : `Mise à jour indisponible : ${etat.raison}`;

  return (
    <button
      type="button"
      onClick={onVerifier}
      title={titre}
      aria-label={titre}
      className="flex items-center gap-1.5 rounded px-1.5 py-1 text-tiny text-ink-faint transition-colors hover:bg-app-hover hover:text-ink"
    >
      <span className={cn("size-1.5 rounded-full", couleur)} />
      {etat?.etat === "maj" && <span className="text-accent">+{etat.ajoutes}</span>}
    </button>
  );
}

// ── Grille d'une vue ouverte ──────────────────────────────────────────────────

function Grille({
  titre,
  sousTitre,
  elements,
  vus,
  reprises,
  onOuvrir,
  onLire,
  onRetour,
  children,
}: {
  titre: string;
  sousTitre: string;
  elements: ElementCinema[];
  vus: ReadonlySet<string>;
  reprises: Reprises;
  onOuvrir: (el: ElementCinema) => void;
  onLire: (el: ElementCinema) => void;
  onRetour: () => void;
  children?: ReactNode;
}) {
  return (
    <section className="px-4 py-3">
      <div className="mb-3 flex items-center gap-2">
        {/* Libellé et pas seulement une flèche : une icône nue de 16 px, en variante fantôme,
            ne se distingue pas du titre qu'elle jouxte — on ne la voyait pas. */}
        <Button variant="outline" size="sm" onClick={onRetour} title="Revenir (Échap)">
          <Icon name="arrow_back" size={16} />
          Retour
        </Button>
        <h3 className="text-sm font-semibold text-ink">{titre}</h3>
        <span className="text-tiny text-ink-faint">{sousTitre}</span>
      </div>
      {children}
      <div className="grid grid-cols-[repeat(auto-fill,minmax(224px,1fr))] gap-3">
        {elements.map((el) => (
          <CarteTitre
            key={el.cle}
            element={el}
            reprise={reprises[el.cle]}
            vu={el.episode?.episode != null && vus.has(empreinte(el.episode.saison, el.episode.episode))}
            onOuvrir={() => onOuvrir(el)}
            onLire={() => onLire(el)}
          />
        ))}
      </div>
    </section>
  );
}

// ── Rangée horizontale ────────────────────────────────────────────────────────

function RangeeElements({
  titre,
  elements,
  total,
  reprises,
  onOuvrir,
  onLire,
  onVisible,
  onPrecharger,
  onMenu,
  onTout,
  vus,
}: {
  titre: string;
  elements: ElementCinema[];
  total: number;
  reprises: Reprises;
  onOuvrir: (el: ElementCinema) => void;
  onLire: (el: ElementCinema) => void;
  onVisible: (chemin: string) => void;
  onPrecharger: (chemin: string) => void;
  onMenu: (f: FilmDto) => void;
  /** Absent quand la rangée n'a pas de vue « tout voir » propre (reprise, rubriques du jeu). */
  onTout?: () => void;
  vus: ReadonlySet<string>;
}) {
  const pisteRef = useRef<HTMLDivElement | null>(null);

  const defiler = (sens: 1 | -1) => {
    const piste = pisteRef.current;
    if (!piste) return;
    piste.scrollBy({ left: sens * piste.clientWidth * 0.8, behavior: "smooth" });
  };

  return (
    <section className="group/rangee relative px-4 py-2">
      <div className="mb-1.5 flex items-baseline gap-2">
        <h3 className="text-sm font-semibold text-ink">
          {titre} <span className="ml-1 text-tiny font-normal text-ink-faint">{total}</span>
        </h3>
        {onTout && (
          <button
            type="button"
            onClick={onTout}
            title="Voir tous les épisodes de cette saison"
            className="flex items-center gap-1 rounded-full border border-app-line bg-app-box px-2.5 py-0.5 text-tiny font-medium text-ink transition-colors hover:border-accent hover:text-accent"
          >
            Tous les épisodes
            <Icon name="chevron_right" size={12} />
          </button>
        )}
      </div>
      <div className="relative">
        <button
          type="button"
          onClick={() => defiler(-1)}
          aria-label="Défiler vers la gauche"
          className="absolute left-0 top-0 z-10 hidden h-full w-8 items-center justify-center rounded-l-md bg-gradient-to-r from-app to-transparent text-ink-dull opacity-0 transition-opacity hover:text-ink group-hover/rangee:opacity-100 md:flex"
        >
          <Icon name="chevron_left" size={20} />
        </button>
        <div ref={pisteRef} className="no-scrollbar flex gap-2 overflow-x-auto scroll-smooth pb-1">
          {elements.map((el) => (
            <div key={el.cle} className="w-56 shrink-0">
              <CarteTitre
                element={el}
                reprise={reprises[el.cle]}
                vu={el.episode?.episode != null && vus.has(empreinte(el.episode.saison, el.episode.episode))}
                onOuvrir={() => onOuvrir(el)}
                onLire={() => onLire(el)}
                onVisible={onVisible}
                onPrecharger={onPrecharger}
                onMenu={el.film ? () => el.film && onMenu(el.film) : undefined}
              />
            </div>
          ))}
        </div>
        <button
          type="button"
          onClick={() => defiler(1)}
          aria-label="Défiler vers la droite"
          className="absolute right-0 top-0 z-10 hidden h-full w-8 items-center justify-center rounded-r-md bg-gradient-to-l from-app to-transparent text-ink-dull opacity-0 transition-opacity hover:text-ink group-hover/rangee:opacity-100 md:flex"
        >
          <Icon name="chevron_right" size={20} />
        </button>
      </div>
    </section>
  );
}

// ── La carte, une seule pour les deux sources ─────────────────────────────────

/**
 * Une carte de titre.
 *
 * Elle était en deux exemplaires — `Carte` pour le jeu, `CarteEpisode` pour la série — qui
 * divergeaient sur tout ce qui n'était pas l'image : le survol, la progression, le menu
 * contextuel, le geste de clic. Une seule carte porte désormais les deux sources, et la seule
 * différence qui subsiste est celle qui existe vraiment : d'où vient l'image, et si un aperçu
 * animé est possible.
 *
 * **Le clic ouvre la FICHE**, le bouton ▶ lance la lecture. C'est le geste des deux plateformes
 * de référence, et il évite de démultiplexer 300 Mo pour vérifier qu'on ne s'est pas trompé.
 */
const CarteTitre = memo(function CarteTitre({
  element,
  reprise,
  vu,
  onOuvrir,
  onLire,
  onVisible,
  onPrecharger,
  onMenu,
}: {
  element: ElementCinema;
  reprise?: { position: number; duree: number };
  vu?: boolean;
  onOuvrir: () => void;
  onLire: () => void;
  onVisible?: (chemin: string) => void;
  onPrecharger?: (chemin: string) => void;
  onMenu?: () => void;
}) {
  const film = element.film;
  const episode = element.episode;
  const hoteRef = useRef<HTMLDivElement | null>(null);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const minuterie = useRef<number | null>(null);
  const minuteriePrecharge = useRef<number | null>(null);
  const [apercu, setApercu] = useState(false);
  const [imageKo, setImageKo] = useState(false);
  const [affiche, setAffiche] = useState<string | null>(() =>
    film ? afficheConnue(film.chemin) : null,
  );

  /**
   * À l'entrée dans le champ de vision : la fiche technique ET la vignette.
   *
   * Ni au montage (97 requêtes d'un coup) ni au survol — la vignette d'un film doit être là
   * AVANT qu'on pointe dessus, sinon elle ne sert à rien pour choisir. La capture passe par la
   * file à un travailleur de `lib/affiches`, qui la persiste : le second passage dans ce dossier
   * est instantané.
   */
  useEffect(() => {
    const hote = hoteRef.current;
    if (!hote || !film) return;
    const obs = new IntersectionObserver(
      (entrees) => {
        if (!entrees.some((e) => e.isIntersecting)) return;
        if (film.duree == null) onVisible?.(film.chemin);
        if (!afficheConnue(film.chemin) && film.lisible !== false) demanderAffiche(film.chemin);
        obs.disconnect();
      },
      { rootMargin: "200px" },
    );
    obs.observe(hote);
    return () => obs.disconnect();
  }, [film, onVisible]);

  // La capture est asynchrone et vient d'ailleurs (la file) : la carte s'abonne plutôt que
  // d'attendre. Une carte sans affiche montre son fond typographique et se met à jour seule.
  useEffect(() => {
    if (!film) return;
    return surAffiche((chemin, url) => {
      if (chemin === film.chemin) setAffiche(url);
    });
  }, [film]);

  /** Capture opportuniste : l'aperçu au survol joue déjà, autant en garder l'image. */
  const capturer = useCallback(() => {
    const v = videoRef.current;
    if (!v || !film || afficheConnue(film.chemin) || v.videoWidth === 0) return;
    const canvas = document.createElement("canvas");
    // Largeur d'affiche fixe : une carte fait 224 px, inutile de garder du 1920.
    canvas.width = 320;
    canvas.height = Math.round((320 * v.videoHeight) / v.videoWidth);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.drawImage(v, 0, 0, canvas.width, canvas.height);
    try {
      const url = canvas.toDataURL("image/jpeg", 0.7);
      poserAffiche(film.chemin, url);
      setAffiche(url);
    } catch {
      // Canvas « teinté » : sans affiche, la carte garde son fond typographique.
    }
  }, [film]);

  const entrer = () => {
    if (!film || film.lisible === false) return;
    // Deux temporisations distinctes, et c'est voulu : le PRÉCHARGEMENT part vite (150 ms), parce
    // qu'il ne fait que préparer des octets côté Rust ; la PRÉVISUALISATION attend plus longtemps
    // (550 ms), parce qu'elle démarre un décodage vidéo visible. Traverser une rangée à la souris
    // ne doit lancer ni l'un ni l'autre.
    if (onPrecharger) {
      if (minuteriePrecharge.current) window.clearTimeout(minuteriePrecharge.current);
      minuteriePrecharge.current = window.setTimeout(() => onPrecharger(film.chemin), DELAI_PRECHARGE);
    }
    if (minuterie.current) window.clearTimeout(minuterie.current);
    minuterie.current = window.setTimeout(() => setApercu(true), DELAI_APERCU);
  };

  const sortir = () => {
    if (minuterie.current) window.clearTimeout(minuterie.current);
    if (minuteriePrecharge.current) window.clearTimeout(minuteriePrecharge.current);
    setApercu(false);
  };

  useEffect(
    () => () => {
      if (minuterie.current) window.clearTimeout(minuterie.current);
      if (minuteriePrecharge.current) window.clearTimeout(minuteriePrecharge.current);
    },
    [],
  );

  const progression = reprise && reprise.duree > 0 ? (reprise.position / reprise.duree) * 100 : 0;
  const vignette = episode ? (imageKo ? element.vignette : vignetteDe(episode)) : affiche;

  return (
    <div
      ref={hoteRef}
      // `anneau-focus` : la carte est déjà atteignable au clavier (`tabIndex={0}`,
      // `role="button"`, Entrée/Espace gérés juste en dessous) mais RIEN ne montrait laquelle
      // avait le focus — on tabulait à l'aveugle dans une rangée de trente vignettes. La classe
      // n'agit qu'en `:focus-visible`, donc un clic à la souris ne laisse aucun halo.
      className="group/carte anneau-focus relative w-full cursor-pointer select-none"
      onMouseEnter={entrer}
      onMouseLeave={sortir}
      onClick={onOuvrir}
      onContextMenu={
        onMenu
          ? (e) => {
              // `preventDefault` : sans lui, la webview ouvre SON menu (« Recharger »,
              // « Inspecter ») par-dessus le menu natif — deux menus, dont un hors de propos.
              e.preventDefault();
              sortir();
              onMenu();
            }
          : undefined
      }
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOuvrir();
        }
      }}
      role="button"
      tabIndex={0}
      title={film?.chemin ?? element.titre}
    >
      <div
        className={cn(
          // Ombre et durée viennent des jetons du thème (`styles.css`, § Médiathèque) et non
          // d'un `duration-150` écrit ici : chaque carte avait sinon son propre rythme, et une
          // rangée survolée rapidement donnait l'impression que certaines cartes traînaient.
          // `transform` + `box-shadow` seulement — composés par le GPU, aucun recalcul de mise
          // en page quand on balaie une rangée de trente cartes.
          "relative aspect-video overflow-hidden rounded-md border bg-app-dark-box",
          "shadow-[var(--ombre-carte)] transition-[transform,box-shadow] duration-[var(--duree-vignette)] ease-[var(--courbe-sortie)]",
          "group-hover/carte:scale-[1.04] group-hover/carte:border-accent/60 group-hover/carte:shadow-[var(--ombre-carte-survol)]",
          vu ? "border-emerald-500/40" : "border-app-line",
        )}
      >
        {vignette && !apercu && (
          <img
            src={vignette}
            alt=""
            loading="lazy"
            onError={() => setImageKo(true)}
            className="h-full w-full object-cover"
            draggable={false}
          />
        )}
        {!vignette && !apercu && (
          <div className="flex h-full w-full flex-col items-center justify-center gap-1 bg-gradient-to-br from-app-box to-app-dark-box">
            <Icon name="movie" size={22} className="text-ink-faint/50" />
            <span className="px-2 text-center font-mono text-[10px] text-ink-faint">{element.titre}</span>
          </div>
        )}
        {apercu && film && (
          // eslint-disable-next-line jsx-a11y/media-has-caption -- aperçu muet, sans dialogue.
          <video
            ref={videoRef}
            src={urlVideo(film.chemin)}
            // `crossOrigin` : le protocole `nievideo` a sa propre origine sous Windows, et
            // sans requête CORS le `canvas` de `capturer` serait teinté — donc aucune affiche.
            crossOrigin="anonymous"
            muted
            autoPlay
            loop
            playsInline
            className="h-full w-full object-cover"
            onLoadedMetadata={(e) => {
              const v = e.currentTarget;
              // On saute le tout début : les cinématiques ouvrent souvent sur un fondu au noir.
              if (v.duration > 2) v.currentTime = v.duration * INSTANT_AFFICHE;
            }}
            onSeeked={capturer}
          />
        )}

        {film?.lisible === false && (
          <div className="absolute inset-x-0 bottom-0 bg-status-warning/85 px-1.5 py-0.5 text-center text-[10px] font-medium text-black">
            {film.codec?.toUpperCase()} — non lisible ici
          </div>
        )}

        {film?.duree != null && film.lisible !== false && (
          <div className="absolute bottom-1 right-1 rounded bg-black/70 px-1 py-0.5 font-mono text-[10px] text-white/90">
            {formaterDuree(film.duree)}
          </div>
        )}

        {episode?.episode ? (
          <span className="absolute left-1 top-1 rounded bg-black/70 px-1.5 py-0.5 text-tiny font-medium text-white">
            É{episode.episode}
          </span>
        ) : null}

        {(film?.langue || episode?.langue) && (
          <div className="absolute right-1 top-1 rounded bg-black/60 px-1 py-0.5 text-[10px] font-medium uppercase text-white/80">
            {film?.langue ?? episode?.langue}
          </div>
        )}

        {vu && (
          <span className="absolute bottom-1 left-1 rounded-full bg-black/70 p-0.5 text-emerald-400">
            <Icon name="check_circle" size={14} />
          </span>
        )}

        {progression > 0 && (
          <div className="absolute inset-x-0 bottom-0 h-0.5 bg-white/20">
            <div className="h-full bg-accent" style={{ width: `${Math.min(100, progression)}%` }} />
          </div>
        )}

        {/* Le seul bouton de la carte : il LIT. Le reste de la surface ouvre la fiche. */}
        <button
          type="button"
          aria-label={`Lire ${element.titre}`}
          title="Lire"
          onClick={(e) => {
            e.stopPropagation();
            onLire();
          }}
          className="absolute inset-0 flex items-center justify-center opacity-0 transition-opacity group-hover/carte:opacity-100"
        >
          <span className="rounded-full bg-black/55 p-2 backdrop-blur-sm">
            <Icon name="play_arrow" size={18} className="text-white" />
          </span>
        </button>
      </div>

      <div className="mt-1 truncate text-xs text-ink" title={element.titre}>
        {element.titre}
      </div>
      <div className="truncate text-[10px] text-ink-faint">
        {episode?.romaji ? (
          <span className="italic">{episode.romaji}</span>
        ) : film ? (
          <>
            {film.largeur ? `${film.largeur}×${film.hauteur} · ` : ""}
            {formaterOctets(film.octets)}
          </>
        ) : (
          (element.sousTitre ?? "")
        )}
      </div>
    </div>
  );
});
