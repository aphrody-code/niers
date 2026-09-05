// Lecteur vidéo natif des cinématiques du jeu.
//
// Il joue le flux produit à la volée par `nievideo://` (remux pur Rust, cf. `src-tauri/video.rs`) —
// un MP4 si le film est en H.264, un WebM s'il est en VP9 — et, quand le film en a une, une piste
// sonore WAV SÉPARÉE. La séparation n'est pas un choix esthétique : la bande-son d'un USM est du
// HCA Criware, qu'aucun conteneur MP4 ne transporte. Les deux éléments sont resynchronisés ici.
//
// Mesuré sur le corpus : **2 films sur 97 seulement portent une piste sonore** dans leur
// conteneur (les deux logos). Pour les autres, elle vit dans la banque Criware `anime_stream`, à
// côté — le `bgmName` du `gamedata` n'est pas « le nom d'une musique » mais le CRC32 du nom du
// film, qui y désigne une cue. Le backend résout ce lien et sert la piste sur `?track=audio` ;
// pour le lecteur, les deux cas sont identiques.
//
// ## Resynchronisation
//
// `<audio>` suit `<video>`, jamais l'inverse : la vidéo porte l'horloge de référence (c'est elle
// que le compositeur cadence sur le rafraîchissement de l'écran). À chaque `timeupdate` — soit
// ~4 fois par seconde — on mesure la dérive ; au-delà de DERIVE_MAX on recale l'audio d'un coup.
// Un recalage permanent produirait un hoquet audible à chaque mesure ; ne jamais recaler laisse
// la dérive s'installer sur un film de vingt minutes.
//
// ## Ce que le lecteur ne fait pas
//
// Il n'y a pas de piste de sous-titres : le jeu ne les stocke pas dans le conteneur mais dans un
// `.cfg.bin` séparé (`subtitleTextPath`), indexé par un hash de menu. Le chemin est affiché dans
// la fiche du film, sa résolution reste à faire.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";

import { Icon } from "@/components/ui/Icon";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { api } from "@/lib/api";
import type { FilmDto } from "@/lib/bindings";
import { cn } from "@/lib/utils";

/** Dérive tolérée entre l'image et le son, en secondes, avant recalage. */
const DERIVE_MAX = 0.25;

/** Délai d'inactivité avant escamotage des contrôles, en millisecondes. */
const DELAI_MASQUAGE = 2600;

/** Vitesses de lecture proposées. */
const VITESSES = [0.25, 0.5, 1, 1.25, 1.5, 2];

/** Les raccourcis, écrits UNE fois : le panneau d'aide les affiche, la boucle clavier les
 * applique. Une aide qui se maintient à la main finit par décrire un lecteur qui n'existe plus. */
const RACCOURCIS: readonly (readonly [string, string])[] = [
  ["K / Espace", "Lecture ou pause"],
  ["J / L", "Reculer ou avancer de 10 s"],
  ["← / →", "Reculer ou avancer de 5 s (Maj : 1 s)"],
  [", / .", "Image précédente ou suivante"],
  ["B / N", "Film précédent ou suivant"],
  ["↑ / ↓", "Volume"],
  ["M", "Couper le son"],
  ["]", "Poser A, puis B, puis effacer la boucle"],
  ["C", "Capturer l'image affichée en PNG"],
  ["< / >", "Ralentir ou accélérer"],
  ["0 – 9", "Sauter à 0 %, 10 %… de la durée"],
  ["F", "Plein écran"],
  ["P", "Incrustation (PiP)"],
  ["I", "Fiche technique"],
  ["?", "Cette aide"],
  ["Échap", "Fermer"],
];

/** Encode un chemin VFS en URL servie par le protocole `nievideo`.
 *
 * **`convertFileSrc` et pas une chaîne bâtie à la main** : la forme de l'URL dépend de la
 * plateforme. Sous Windows et Android, Tauri sert les protocoles personnalisés en
 * `http://<protocole>.localhost/<chemin>` ; ailleurs en `<protocole>://localhost/<chemin>`.
 * Écrire `nievideo://localhost/…` en dur ne chargerait donc rien sur cette machine.
 *
 * Hors runtime Tauri (un navigateur, pour déboguer une mise en page), il n'y a pas de protocole
 * du tout : la fonction rend une chaîne vide, et le `<video>` reste sans source plutôt que de
 * réclamer une URL qui n'existe pas. */
export function urlVideo(chemin: string, piste?: "audio"): string {
  let base: string;
  try {
    base = convertFileSrc(chemin, "nievideo");
  } catch {
    return "";
  }
  return `${base}${piste ? `?track=${piste}` : ""}`;
}

/** Deux chiffres, zéro devant. */
const deux = (n: number) => String(n).padStart(2, "0");

/** `93.55` → `1:33`. Rend `--:--` pour une durée inconnue. */
export function formaterDuree(secondes: number | null | undefined): string {
  if (secondes == null || !Number.isFinite(secondes) || secondes < 0) return "--:--";
  const s = Math.floor(secondes % 60);
  const m = Math.floor((secondes / 60) % 60);
  const h = Math.floor(secondes / 3600);
  return h > 0 ? `${h}:${deux(m)}:${deux(s)}` : `${m}:${deux(s)}`;
}

export interface VideoPlayerProps {
  /** Chemin VFS du `.usm`. */
  chemin: string;
  /** Titre affiché en surimpression. */
  titre: string;
  /** Sous-titre (rubrique, définition, codec…). */
  detail?: string;
  /** Le film a-t-il une piste sonore ? Sans elle, aucun `<audio>` n'est monté. */
  avecAudio?: boolean;
  /** Lecture immédiate. */
  autoPlay?: boolean;
  /** Fermeture (croix, Échap). */
  onClose?: () => void;
  /** Progression rapportée en continu — sert au « Reprendre la lecture ». */
  onProgression?: (secondes: number, duree: number) => void;
  /** Position de reprise, en secondes. */
  depart?: number;
  /**
   * Fiche technique du film — alimente le panneau d'informations (touche `I`) et, surtout, donne
   * la CADENCE : sans elle, l'avance image par image devrait deviner la durée d'une image.
   */
  film?: FilmDto | null;
  /** Film suivant de la file courante — bouton, touche `N`, et enchaînement en fin de lecture. */
  onSuivant?: () => void;
  /** Film précédent de la file courante — bouton, touche `B`. */
  onPrecedent?: () => void;
  /** Rang dans la file, affiché en clair (« 12 / 97 ») : sans lui, « suivant » ne veut rien dire. */
  file?: { index: number; total: number } | null;
  className?: string;
}

/** Une image, en secondes, pour un film de cadence `c`. 30 i/s par défaut : la cadence n'est
 * connue qu'une fois le film inspecté, et une avance « image par image » qui ne bouge pas tant
 * que l'inspection n'a pas rendu serait pire qu'une avance approchée. */
function pasImage(cadence: number | null | undefined): number {
  return 1 / (cadence && cadence > 0 ? cadence : 30);
}

export function VideoPlayer({
  chemin,
  titre,
  detail,
  avecAudio = true,
  autoPlay = true,
  onClose,
  onProgression,
  depart,
  film,
  onSuivant,
  onPrecedent,
  file,
  className,
}: VideoPlayerProps) {
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const hoteRef = useRef<HTMLDivElement | null>(null);
  const minuterieRef = useRef<number | null>(null);
  /** Second `<video>`, hors écran, dédié à la vignette d'aperçu : chercher dans l'élément
   * principal pour dessiner un survol interromprait la lecture en cours. */
  const apercuRef = useRef<HTMLVideoElement | null>(null);
  const barreRef = useRef<HTMLDivElement | null>(null);

  const [enLecture, setEnLecture] = useState(false);
  const [position, setPosition] = useState(0);
  const [duree, setDuree] = useState(0);
  const [tampon, setTampon] = useState(0);
  const [volume, setVolume] = useState(1);
  const [muet, setMuet] = useState(false);
  const [vitesse, setVitesse] = useState(1);
  const [pleinEcran, setPleinEcran] = useState(false);
  const [visible, setVisible] = useState(true);
  const [erreur, setErreur] = useState<string | null>(null);
  const [chargement, setChargement] = useState(true);
  /** Survol de la barre : l'instant visé et son abscisse, pour la vignette. */
  const [apercu, setApercu] = useState<{ t: number; x: number } | null>(null);
  /** Glissement en cours sur la barre — un vrai scrub, pas un clic ponctuel. */
  const [scrub, setScrub] = useState(false);
  /**
   * Boucle A–B. Deux bornes, posées à la volée : c'est l'outil qui permet de revoir vingt fois
   * les mêmes trente images d'une animation sans reprendre le film au début.
   */
  const [boucle, setBoucle] = useState<{ a: number | null; b: number | null }>({ a: null, b: null });
  /** Panneau latéral ouvert, s'il y en a un. */
  const [panneau, setPanneau] = useState<"aucun" | "infos" | "aide">("aucun");

  const src = useMemo(() => urlVideo(chemin), [chemin]);
  const srcAudio = useMemo(() => (avecAudio ? urlVideo(chemin, "audio") : null), [chemin, avecAudio]);

  // Deux valeurs lues DEPUIS les écouteurs du média, jamais dans leurs dépendances : les borner
  // par `useEffect` remonterait les huit `addEventListener` à chaque borne posée et à chaque
  // rendu du parent — un remontage en plein `timeupdate` fait sauter la lecture.
  const boucleRef = useRef(boucle);
  const suivantRef = useRef(onSuivant);
  useEffect(() => {
    boucleRef.current = boucle;
  }, [boucle]);
  useEffect(() => {
    suivantRef.current = onSuivant;
  }, [onSuivant]);

  // ── Contrôles ───────────────────────────────────────────────────────────────

  const reveiller = useCallback(() => {
    setVisible(true);
    if (minuterieRef.current) window.clearTimeout(minuterieRef.current);
    minuterieRef.current = window.setTimeout(() => setVisible(false), DELAI_MASQUAGE);
  }, []);

  const basculerLecture = useCallback(() => {
    const v = videoRef.current;
    if (!v) return;
    if (v.paused) void v.play().catch(() => {});
    else v.pause();
    reveiller();
  }, [reveiller]);

  const chercher = useCallback(
    (secondes: number) => {
      const v = videoRef.current;
      if (!v || !Number.isFinite(secondes)) return;
      const cible = Math.max(0, Math.min(secondes, v.duration || 0));
      v.currentTime = cible;
      // Recalage immédiat de l'audio : attendre le `timeupdate` laisserait entendre l'ancienne
      // position pendant un quart de seconde.
      if (audioRef.current) audioRef.current.currentTime = cible;
      setPosition(cible);
      reveiller();
    },
    [reveiller],
  );

  const decaler = useCallback(
    (delta: number) => chercher((videoRef.current?.currentTime ?? 0) + delta),
    [chercher],
  );

  /**
   * Avance ou recule d'une image. Met la lecture en pause d'abord : sans cela, la lecture
   * reprend la main au `timeupdate` suivant et le pas n'est jamais visible.
   */
  const parImage = useCallback(
    (sens: 1 | -1) => {
      const v = videoRef.current;
      if (!v) return;
      v.pause();
      chercher(v.currentTime + sens * pasImage(film?.cadence));
    },
    [chercher, film?.cadence],
  );

  /**
   * Enregistre l'image affichée en PNG, à la résolution réelle du film (pas celle de la fenêtre).
   *
   * `drawImage` sur un `<video>` dont la source vient d'un protocole personnalisé teinte le canvas
   * — l'appel à `toBlob` échouerait sur une `SecurityError` si le flux était considéré comme
   * d'une autre origine. `nievideo://` est servi par l'application elle-même, donc de même
   * origine ; l'échec éventuel est rapporté tel quel plutôt que masqué.
   */
  const capturer = useCallback(async () => {
    const v = videoRef.current;
    if (!v || !v.videoWidth) return;
    try {
      const canvas = document.createElement("canvas");
      canvas.width = v.videoWidth;
      canvas.height = v.videoHeight;
      const ctx = canvas.getContext("2d");
      if (!ctx) throw new Error("contexte 2D indisponible");
      ctx.drawImage(v, 0, 0, canvas.width, canvas.height);
      const url = canvas.toDataURL("image/png");
      const b64 = url.slice(url.indexOf(",") + 1);
      const image = Math.round(v.currentTime * (film?.cadence || 30));
      const dest = await save({
        defaultPath: `${film?.nom ?? "capture"}_${String(image).padStart(5, "0")}.png`,
        filters: [{ name: "Image PNG", extensions: ["png"] }],
      });
      if (!dest) return;
      const octets = await api.saveBytesB64(dest, b64);
      toast.success(`Image ${image} capturée — ${octets.toLocaleString("fr-FR")} octets`);
    } catch (e) {
      toast.error(`Capture impossible : ${e}`);
    }
  }, [film?.cadence, film?.nom]);

  /**
   * Pose la borne suivante de la boucle : A si aucune, B si A seule, remise à zéro si les deux
   * sont posées. Un seul geste pour les trois états — c'est la convention des lecteurs d'analyse.
   */
  const poserBoucle = useCallback(() => {
    const t = videoRef.current?.currentTime ?? 0;
    setBoucle((b) => {
      if (b.a === null) return { a: t, b: null };
      if (b.b === null) return t > b.a ? { a: b.a, b: t } : { a: t, b: b.a };
      return { a: null, b: null };
    });
    reveiller();
  }, [reveiller]);

  const basculerPleinEcran = useCallback(() => {
    const hote = hoteRef.current;
    if (!hote) return;
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
    else void hote.requestFullscreen().catch(() => {});
  }, []);

  const basculerPip = useCallback(() => {
    const v = videoRef.current;
    if (!v) return;
    if (document.pictureInPictureElement) void document.exitPictureInPicture().catch(() => {});
    else void v.requestPictureInPicture?.().catch(() => {});
  }, []);

  // ── Synchronisation image/son ───────────────────────────────────────────────

  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;

    const surTemps = () => {
      // La boucle A–B se referme ICI, pas dans un `setInterval` : `timeupdate` est l'horloge du
      // média lui-même, donc le rebouclage suit la vitesse de lecture sans dériver. Elle passe
      // par une ref pour ne pas remonter tout ce bloc de listeners à chaque borne posée.
      const bcl = boucleRef.current;
      if (bcl.a !== null && bcl.b !== null && v.currentTime >= bcl.b) {
        v.currentTime = bcl.a;
        if (audioRef.current) audioRef.current.currentTime = bcl.a;
      }
      setPosition(v.currentTime);
      onProgression?.(v.currentTime, v.duration || 0);
      const a = audioRef.current;
      if (a && !a.paused) {
        const derive = a.currentTime - v.currentTime;
        if (Math.abs(derive) > DERIVE_MAX) a.currentTime = v.currentTime;
      }
      if (v.buffered.length > 0) setTampon(v.buffered.end(v.buffered.length - 1));
    };
    const surMeta = () => {
      setDuree(v.duration || 0);
      setChargement(false);
      if (depart && depart > 0 && depart < (v.duration || 0)) {
        v.currentTime = depart;
        if (audioRef.current) audioRef.current.currentTime = depart;
      }
    };
    const surLecture = () => {
      setEnLecture(true);
      const a = audioRef.current;
      if (a) {
        a.currentTime = v.currentTime;
        void a.play().catch(() => {});
      }
    };
    const surPause = () => {
      setEnLecture(false);
      audioRef.current?.pause();
      setVisible(true);
    };
    const surFin = () => {
      setEnLecture(false);
      audioRef.current?.pause();
      setVisible(true);
      // Enchaînement sur le film suivant de la file — ce qui fait la différence entre « ouvrir un
      // fichier » et « regarder ». Une boucle A–B posée l'emporte : elle signifie qu'on travaille
      // sur CE passage, et enchaîner serait l'inverse de ce qui est demandé.
      const bcl = boucleRef.current;
      if (bcl.a === null || bcl.b === null) suivantRef.current?.();
    };
    const surErreur = () => {
      setChargement(false);
      setErreur(
        v.error?.message ||
          "cette vidéo n'a pas pu être décodée — son codec n'est peut-être pas du H.264",
      );
    };
    const surAttente = () => setChargement(true);
    const surPret = () => setChargement(false);

    v.addEventListener("timeupdate", surTemps);
    v.addEventListener("loadedmetadata", surMeta);
    v.addEventListener("play", surLecture);
    v.addEventListener("pause", surPause);
    v.addEventListener("ended", surFin);
    v.addEventListener("error", surErreur);
    v.addEventListener("waiting", surAttente);
    v.addEventListener("canplay", surPret);
    return () => {
      v.removeEventListener("timeupdate", surTemps);
      v.removeEventListener("loadedmetadata", surMeta);
      v.removeEventListener("play", surLecture);
      v.removeEventListener("pause", surPause);
      v.removeEventListener("ended", surFin);
      v.removeEventListener("error", surErreur);
      v.removeEventListener("waiting", surAttente);
      v.removeEventListener("canplay", surPret);
    };
  }, [depart, onProgression]);

  // La vitesse et le volume s'appliquent aux DEUX éléments, sinon le son dérive aussitôt.
  useEffect(() => {
    if (videoRef.current) videoRef.current.playbackRate = vitesse;
    if (audioRef.current) audioRef.current.playbackRate = vitesse;
  }, [vitesse]);

  useEffect(() => {
    // La piste vidéo est toujours muette : tout le son vient du `<audio>`. Un MP4 remuxé n'a
    // d'ailleurs aucune piste sonore — mais le rendre muet explicitement évite qu'un futur
    // conteneur avec son ne se superpose au WAV.
    if (videoRef.current) videoRef.current.muted = true;
    if (audioRef.current) {
      audioRef.current.volume = volume;
      audioRef.current.muted = muet;
    }
  }, [volume, muet]);

  // Changement de film : on repart de zéro plutôt que de garder l'erreur du précédent. La boucle
  // en fait partie — ses deux bornes sont des instants d'UN film, les reporter sur le suivant
  // ferait boucler un passage choisi ailleurs.
  useEffect(() => {
    setErreur(null);
    setChargement(true);
    setPosition(0);
    setDuree(0);
    setTampon(0);
    setBoucle({ a: null, b: null });
    setApercu(null);
  }, [chemin]);

  useEffect(() => {
    const surChangement = () => setPleinEcran(Boolean(document.fullscreenElement));
    document.addEventListener("fullscreenchange", surChangement);
    return () => document.removeEventListener("fullscreenchange", surChangement);
  }, []);

  // ── Clavier ─────────────────────────────────────────────────────────────────

  useEffect(() => {
    const surTouche = (e: KeyboardEvent) => {
      const cible = e.target as HTMLElement | null;
      // Un raccourci ne doit pas voler une frappe destinée à un champ de saisie.
      if (cible && (cible.tagName === "INPUT" || cible.tagName === "TEXTAREA" || cible.isContentEditable)) {
        return;
      }
      switch (e.key) {
        case " ":
        case "k":
          e.preventDefault();
          basculerLecture();
          break;
        case "ArrowLeft":
          e.preventDefault();
          decaler(e.shiftKey ? -1 : -5);
          break;
        case "ArrowRight":
          e.preventDefault();
          decaler(e.shiftKey ? 1 : 5);
          break;
        case "j":
          e.preventDefault();
          decaler(-10);
          break;
        case "l":
          e.preventDefault();
          decaler(10);
          break;
        case "ArrowUp":
          e.preventDefault();
          setVolume((v) => Math.min(1, v + 0.05));
          reveiller();
          break;
        case "ArrowDown":
          e.preventDefault();
          setVolume((v) => Math.max(0, v - 0.05));
          reveiller();
          break;
        case "m":
          setMuet((m) => !m);
          reveiller();
          break;
        case "f":
          basculerPleinEcran();
          break;
        case "p":
          basculerPip();
          break;
        // Pas à pas : les deux touches des tables de montage, `,` et `.`.
        case ",":
          e.preventDefault();
          parImage(-1);
          break;
        case ".":
          e.preventDefault();
          parImage(1);
          break;
        case "b":
          onPrecedent?.();
          break;
        case "n":
          onSuivant?.();
          break;
        case "]":
          poserBoucle();
          break;
        case "c":
          void capturer();
          break;
        case "i":
          setPanneau((p) => (p === "infos" ? "aucun" : "infos"));
          reveiller();
          break;
        case "?":
          setPanneau((p) => (p === "aide" ? "aucun" : "aide"));
          reveiller();
          break;
        case "<":
          setVitesse((v) => VITESSES[Math.max(0, VITESSES.indexOf(v) - 1)] ?? v);
          reveiller();
          break;
        case ">":
          setVitesse((v) => VITESSES[Math.min(VITESSES.length - 1, VITESSES.indexOf(v) + 1)] ?? v);
          reveiller();
          break;
        case "Escape":
          // Un panneau ouvert se ferme d'abord : sinon `Échap` referme le lecteur entier alors
          // qu'on voulait seulement replier la fiche technique.
          if (panneau !== "aucun") setPanneau("aucun");
          else if (!document.fullscreenElement) onClose?.();
          break;
        default:
          // 0–9 : saut au pourcentage correspondant, comme sur un lecteur web.
          if (/^[0-9]$/.test(e.key) && duree > 0) {
            e.preventDefault();
            chercher((Number(e.key) / 10) * duree);
          }
      }
    };
    window.addEventListener("keydown", surTouche);
    return () => window.removeEventListener("keydown", surTouche);
  }, [
    basculerLecture,
    basculerPip,
    basculerPleinEcran,
    capturer,
    chercher,
    decaler,
    duree,
    onClose,
    onPrecedent,
    onSuivant,
    panneau,
    parImage,
    poserBoucle,
    reveiller,
  ]);

  useEffect(
    () => () => {
      if (minuterieRef.current) window.clearTimeout(minuterieRef.current);
    },
    [],
  );

  // ── Rendu ───────────────────────────────────────────────────────────────────

  const pourcentage = duree > 0 ? (position / duree) * 100 : 0;
  const pourcentageTampon = duree > 0 ? (tampon / duree) * 100 : 0;

  /** L'instant visé par une abscisse écran, borné à la durée du film. */
  const instantSous = (clientX: number): number | null => {
    const rect = barreRef.current?.getBoundingClientRect();
    if (!rect || rect.width <= 0 || duree <= 0) return null;
    return Math.max(0, Math.min(1, (clientX - rect.left) / rect.width)) * duree;
  };

  /** Survol : vignette d'aperçu. Le `<video>` caché est cherché à la volée — un survol de la
   * barre ne doit jamais toucher l'élément qui joue. */
  const surSurvolBarre = (e: React.PointerEvent<HTMLDivElement>) => {
    const t = instantSous(e.clientX);
    if (t === null) return;
    const rect = barreRef.current?.getBoundingClientRect();
    setApercu({ t, x: e.clientX - (rect?.left ?? 0) });
    const a = apercuRef.current;
    if (a && Number.isFinite(t)) a.currentTime = t;
    if (scrub) chercher(t);
  };

  const surAppuiBarre = (e: React.PointerEvent<HTMLDivElement>) => {
    const t = instantSous(e.clientX);
    if (t === null) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setScrub(true);
    chercher(t);
  };

  const surRelacheBarre = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    setScrub(false);
  };

  return (
    <div
      ref={hoteRef}
      className={cn(
        "group relative flex h-full min-h-0 w-full items-center justify-center overflow-hidden bg-black",
        !visible && enLecture && "cursor-none",
        className,
      )}
      onMouseMove={reveiller}
      onDoubleClick={basculerPleinEcran}
    >
      {/* eslint-disable-next-line jsx-a11y/media-has-caption -- le jeu ne fournit aucune piste
          de sous-titres dans le conteneur (cf. l'en-tête de ce fichier). */}
      <video
        ref={videoRef}
        src={src}
        autoPlay={autoPlay}
        muted
        playsInline
        className="h-full w-full object-contain"
        onClick={basculerLecture}
      />
      {srcAudio && <audio ref={audioRef} src={srcAudio} preload="auto" />}

      {chargement && !erreur && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <div className="h-10 w-10 animate-spin rounded-full border-2 border-white/25 border-t-white/90" />
        </div>
      )}

      {erreur && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-black/85 p-6 text-center">
          <Icon name="error" size={28} className="text-status-error" />
          <div className="text-sm font-medium text-white">Lecture impossible</div>
          <div className="max-w-lg text-xs text-white/60">{erreur}</div>
        </div>
      )}

      {/* Bandeau haut : titre + fermeture. */}
      <div
        className={cn(
          "pointer-events-none absolute inset-x-0 top-0 flex items-start justify-between gap-3 bg-gradient-to-b from-black/80 to-transparent p-4 transition-opacity",
          visible || !enLecture ? "opacity-100" : "opacity-0",
        )}
      >
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold text-white">{titre}</div>
          {detail && <div className="truncate text-xs text-white/55">{detail}</div>}
        </div>
        {onClose && (
          <button
            type="button"
            onClick={onClose}
            title="Fermer (Échap)"
            aria-label="Fermer"
            className="pointer-events-auto rounded-md p-1.5 text-white/70 transition-colors hover:bg-white/15 hover:text-white"
          >
            <Icon name="close" size={18} />
          </button>
        )}
      </div>

      {/* Panneau latéral — fiche technique ou raccourcis. En surimpression et non dans une
          modale : on continue de voir le film pendant qu'on lit ce qu'il est. */}
      {panneau !== "aucun" && (
        <div className="absolute right-3 top-14 z-20 w-72 rounded-lg border border-white/10 bg-black/85 p-3 text-xs backdrop-blur">
          <div className="mb-2 flex items-center gap-2 text-white">
            <Icon name={panneau === "infos" ? "feed" : "lightbulb"} size={15} />
            <span className="flex-1 font-medium">
              {panneau === "infos" ? "Fiche technique" : "Raccourcis"}
            </span>
            <button
              type="button"
              onClick={() => setPanneau("aucun")}
              aria-label="Fermer le panneau"
              className="rounded p-0.5 text-white/60 hover:bg-white/15 hover:text-white"
            >
              <Icon name="close" size={14} />
            </button>
          </div>

          {panneau === "infos" && film && (
            <div className="space-y-0">
              <LigneFiche intitule="Fichier" valeur={film.nom} />
              <LigneFiche intitule="Rubrique" valeur={film.rubrique} />
              <LigneFiche intitule="Langue" valeur={film.langue} />
              <LigneFiche intitule="Codec" valeur={film.codec} />
              <LigneFiche
                intitule="Définition"
                valeur={film.largeur && film.hauteur ? `${film.largeur} × ${film.hauteur}` : null}
              />
              <LigneFiche intitule="Cadence" valeur={film.cadence ? `${film.cadence.toFixed(2)} i/s` : null} />
              <LigneFiche intitule="Images" valeur={film.images?.toLocaleString("fr-FR")} />
              <LigneFiche intitule="Durée" valeur={formaterDuree(film.duree ?? duree)} />
              <LigneFiche intitule="Taille" valeur={`${(film.octets / 1024 / 1024).toFixed(1)} Mo`} />
              <LigneFiche
                intitule="Bande-son"
                valeur={film.audio.length > 0 ? `${film.audio.length} piste(s)` : "aucune"}
              />
              <LigneFiche intitule="XOR CRI" valeur={film.chiffre === null ? null : film.chiffre ? "oui" : "non"} />
              <LigneFiche intitule="Nom d'origine" valeur={film.nom_origine} />
              {/* Le chemin en entier, sélectionnable : c'est ce qu'on recopie dans une commande
                  `niers vfs extract` ou dans un test. */}
              <div className="mt-2 select-text break-all border-t border-white/10 pt-2 font-mono text-[10px] text-white/45">
                {film.chemin}
              </div>
            </div>
          )}

          {panneau === "aide" && (
            <div className="space-y-0.5">
              {RACCOURCIS.map(([touche, quoi]) => (
                <div key={touche} className="flex items-baseline justify-between gap-4 py-0.5">
                  <kbd className="shrink-0 rounded border border-white/15 bg-white/10 px-1.5 font-mono text-[10px] text-white/80">
                    {touche}
                  </kbd>
                  <span className="truncate text-right text-white/70">{quoi}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Contrôles. */}
      <div
        className={cn(
          "absolute inset-x-0 bottom-0 flex flex-col gap-1 bg-gradient-to-t from-black/90 via-black/60 to-transparent px-4 pb-3 pt-10 transition-opacity",
          visible || !enLecture ? "opacity-100" : "pointer-events-none opacity-0",
        )}
      >
        {/* Barre de progression : le tampon en gris, la position en accent, l'intervalle de
            boucle en surbrillance. Elle se GLISSE (pointer capture) et non plus seulement se
            clique — un clic isolé ne permet pas de chercher une image précise. */}
        <div
          ref={barreRef}
          className="group/barre relative h-4 cursor-pointer touch-none"
          onPointerDown={surAppuiBarre}
          onPointerMove={surSurvolBarre}
          onPointerUp={surRelacheBarre}
          onPointerCancel={surRelacheBarre}
          onPointerLeave={() => setApercu(null)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") basculerLecture();
          }}
          role="slider"
          tabIndex={0}
          aria-label="Position dans la vidéo"
          aria-valuemin={0}
          aria-valuemax={Math.round(duree)}
          aria-valuenow={Math.round(position)}
        >
          <div className="absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 rounded-full bg-white/20 transition-[height] group-hover/barre:h-1.5">
            <div className="h-full rounded-full bg-white/30" style={{ width: `${pourcentageTampon}%` }} />
          </div>
          {boucle.a !== null && duree > 0 && (
            <div
              className="absolute top-1/2 h-1 -translate-y-1/2 rounded-full bg-amber-400/40 transition-[height] group-hover/barre:h-1.5"
              style={{
                left: `${(boucle.a / duree) * 100}%`,
                width: `${(((boucle.b ?? boucle.a) - boucle.a) / duree) * 100}%`,
              }}
            />
          )}
          <div
            className="absolute top-1/2 h-1 -translate-y-1/2 rounded-full bg-accent transition-[height] group-hover/barre:h-1.5"
            style={{ width: `${pourcentage}%` }}
          />
          {/* Les deux bornes restent visibles même hors survol : c'est un état du travail en
              cours, pas une décoration de la barre. */}
          {(["a", "b"] as const).map((borne) =>
            boucle[borne] !== null && duree > 0 ? (
              <div
                key={borne}
                className="absolute top-1/2 h-3 w-0.5 -translate-y-1/2 bg-amber-400"
                style={{ left: `${((boucle[borne] as number) / duree) * 100}%` }}
                title={`Boucle ${borne.toUpperCase()} — ${formaterDuree(boucle[borne] as number)}`}
              />
            ) : null,
          )}
          <div
            className={cn(
              "absolute top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-accent transition-opacity",
              scrub ? "opacity-100" : "opacity-0 group-hover/barre:opacity-100",
            )}
            style={{ left: `${pourcentage}%` }}
          />

          {/* Vignette d'aperçu — l'image du point visé, pas seulement son horodatage. */}
          {apercu && duree > 0 && (
            <div
              className="pointer-events-none absolute bottom-5 z-10 -translate-x-1/2 overflow-hidden rounded-md border border-white/15 bg-black/90 shadow-lg"
              style={{ left: `${Math.max(80, Math.min(apercu.x, (barreRef.current?.clientWidth ?? 0) - 80))}px` }}
            >
              <video
                ref={apercuRef}
                src={src}
                muted
                preload="metadata"
                className="h-[90px] w-40 bg-black object-contain"
              />
              <div className="border-t border-white/10 py-0.5 text-center font-mono text-[10px] tabular-nums text-white/80">
                {formaterDuree(apercu.t)}
              </div>
            </div>
          )}
        </div>

        <div className="flex items-center gap-1 text-white">
          {onPrecedent && <BoutonLecteur icone="skip_previous" titre="Film précédent (B)" onClick={onPrecedent} />}
          <BoutonLecteur icone={enLecture ? "pause" : "play_arrow"} titre={enLecture ? "Pause (K)" : "Lecture (K)"} onClick={basculerLecture} />
          {onSuivant && <BoutonLecteur icone="skip_next" titre="Film suivant (N)" onClick={onSuivant} />}
          <BoutonLecteur icone="fast_rewind" titre="Reculer de 10 s (J)" onClick={() => decaler(-10)} />
          <BoutonLecteur icone="fast_forward" titre="Avancer de 10 s (L)" onClick={() => decaler(10)} />

          {/* Pas à pas image par image — l'outil qui distingue un lecteur d'analyse d'un lecteur
              de salon. La cadence vient de la fiche du film, pas d'une constante. */}
          <BoutonLecteur icone="chevron_left" titre="Image précédente (,)" onClick={() => parImage(-1)} />
          <BoutonLecteur icone="chevron_right" titre="Image suivante (.)" onClick={() => parImage(1)} />

          <div className="group/volume flex items-center gap-1">
            <BoutonLecteur
              icone={muet || volume === 0 ? "volume_off" : "volume_up"}
              titre={muet ? "Rétablir le son (M)" : "Couper le son (M)"}
              onClick={() => setMuet((m) => !m)}
              desactive={!srcAudio}
            />
            {/* `Slider` du design system : la glissière était un `<input type=range>` nu, sans
                piste ni curseur communs au reste de l'application. */}
            <div className="w-0 overflow-hidden opacity-0 transition-all group-hover/volume:w-24 group-hover/volume:opacity-100">
              <Slider
                value={[muet ? 0 : volume]}
                min={0}
                max={1}
                step={0.01}
                disabled={!srcAudio}
                onValueChange={(v) => {
                  const n = Array.isArray(v) ? v[0] : v;
                  if (typeof n !== "number") return;
                  setVolume(n);
                  setMuet(false);
                }}
                aria-label="Volume"
                className="w-24"
              />
            </div>
          </div>

          <div className="ml-2 select-none font-mono text-xs tabular-nums text-white/80">
            {formaterDuree(position)} <span className="text-white/35">/ {formaterDuree(duree)}</span>
            {/* Le numéro d'image : sur un film à 30 i/s, « 2:14 » ne désigne pas une image, il en
                désigne trente. */}
            {film?.cadence ? (
              <span className="ml-2 text-white/35">img {Math.round(position * film.cadence).toLocaleString("fr-FR")}</span>
            ) : null}
          </div>

          <div className="flex-1" />

          {file && file.total > 1 && (
            <span className="mr-2 select-none font-mono text-[11px] tabular-nums text-white/45">
              {file.index + 1} / {file.total}
            </span>
          )}

          {!srcAudio && (
            <span
              className="mr-1 rounded bg-white/10 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-white/50"
              title="Aucune bande-son trouvée pour ce film — ni dans son conteneur, ni dans la banque anime_stream. Les écrans-titres et les logos n'en déclarent aucune."
            >
              sans bande-son
            </span>
          )}

          <BoutonLecteur
            icone="replay"
            titre={
              boucle.b !== null
                ? `Boucle ${formaterDuree(boucle.a)} → ${formaterDuree(boucle.b)} — effacer (])`
                : boucle.a !== null
                  ? `Borne A posée à ${formaterDuree(boucle.a)} — poser B (])`
                  : "Poser la borne A de la boucle (])"
            }
            onClick={poserBoucle}
            actif={boucle.a !== null}
          />
          <BoutonLecteur icone="image" titre="Capturer l'image affichée (C)" onClick={() => void capturer()} />

          <Select value={String(vitesse)} onValueChange={(v) => setVitesse(Number(v ?? 1))}>
            <SelectTrigger size="sm" className="mr-1 h-7 w-16 border-white/15 bg-white/10 text-xs text-white/85">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {VITESSES.map((v) => (
                <SelectItem key={v} value={String(v)}>
                  {v}×
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <BoutonLecteur
            icone="feed"
            titre="Fiche technique (I)"
            onClick={() => setPanneau((p) => (p === "infos" ? "aucun" : "infos"))}
            actif={panneau === "infos"}
            desactive={!film}
          />
          <BoutonLecteur
            icone="lightbulb"
            titre="Raccourcis clavier (?)"
            onClick={() => setPanneau((p) => (p === "aide" ? "aucun" : "aide"))}
            actif={panneau === "aide"}
          />
          <BoutonLecteur icone="picture_in_picture" titre="Incrustation (P)" onClick={basculerPip} />
          <BoutonLecteur
            icone={pleinEcran ? "fullscreen_exit" : "fullscreen"}
            titre={pleinEcran ? "Quitter le plein écran (F)" : "Plein écran (F)"}
            onClick={basculerPleinEcran}
          />
        </div>
      </div>
    </div>
  );
}

function BoutonLecteur({
  icone,
  titre,
  onClick,
  desactive,
  actif,
}: {
  icone: string;
  titre: string;
  onClick: () => void;
  desactive?: boolean;
  /** Bascule enfoncée (boucle armée, panneau ouvert) — `aria-pressed`, pas seulement une teinte. */
  actif?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={titre}
      aria-label={titre}
      aria-pressed={actif}
      disabled={desactive}
      className={cn(
        "rounded-md p-1.5 transition-colors hover:bg-white/15 hover:text-white disabled:cursor-not-allowed disabled:opacity-30",
        actif ? "bg-white/15 text-amber-300" : "text-white/85",
      )}
    >
      <Icon name={icone} size={18} />
    </button>
  );
}

/** Une ligne « intitulé → valeur » de la fiche technique. Rend `null` quand la valeur manque :
 * une fiche qui affiche « — » partout ne dit pas que le film n'a pas été inspecté, elle laisse
 * croire qu'il n'a pas ces propriétés. */
function LigneFiche({ intitule, valeur }: { intitule: string; valeur: React.ReactNode }) {
  if (valeur === null || valeur === undefined || valeur === "") return null;
  return (
    <div className="flex items-baseline justify-between gap-4 py-0.5">
      <span className="shrink-0 text-white/45">{intitule}</span>
      <span className="truncate text-right font-mono text-white/85">{valeur}</span>
    </div>
  );
}
