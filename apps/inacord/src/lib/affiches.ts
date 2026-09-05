// Les affiches des cinématiques — la première image de chaque film, capturée une fois et gardée.
//
// ## Pourquoi ce n'est pas fait côté Rust
//
// Ce serait la bonne place, et ce n'est pas possible ici : le backend REMUXE (`video_mp4_from_bytes`
// réempaquette le H.264 du conteneur USM dans un MP4), il ne DÉCODE pas. Aucun décodeur H.264 ne
// figure dans les dépendances du dépôt. Le seul décodeur disponible est celui de la webview,
// donc la capture se fait ici, avec un `<video>` hors écran et un `canvas`.
//
// ## Ce qui rend la chose supportable
//
// Un remux coûte jusqu'à 300 Mo de lecture. Capturer les 97 films à l'ouverture saturerait le
// disque pendant plusieurs minutes pour des vignettes que personne ne regardera. Trois garde-fous :
//
//  * **à la demande** — seules les cartes réellement visibles déclenchent une capture ;
//  * **un seul travailleur** — la file est stricte, comme celle de `videoInfo` ; deux remux
//    simultanés se disputent le même disque sans rien afficher plus tôt ;
//  * **persistant** — l'affiche est écrite dans `localStorage` et survit au redémarrage. C'est ce
//    qui fait qu'un dossier déjà parcouru s'affiche instantanément la fois suivante, et que le
//    coût est payé une fois pour toutes.
//
// ## La première image, et quand elle ne vaut rien
//
// La demande est « la première frame ». Beaucoup de cinématiques ouvrent sur un fondu au noir :
// une vignette noire ne distingue rien d'une autre vignette noire. On capture donc à `t=0`, on
// MESURE la luminance du résultat, et on ne retente à 12 % de la durée que si l'image est
// effectivement vide. Le compromis est vérifiable, pas supposé.
import { urlVideo } from "@/components/VideoPlayer";

/** Largeur d'affiche. Une carte fait 224 px ; 320 couvre les écrans à deux pixels par point. */
const LARGEUR = 320;

/** Qualité JPEG — au-delà, on gagne des kilo-octets sans rien voir de plus. */
const QUALITE = 0.72;

/** Au-delà, on abandonne ce film : un conteneur de 300 Mo ne doit pas bloquer la file. */
const DELAI_MAX = 25_000;

/** Instant de repli quand la première image est noire. */
const INSTANT_REPLI = 0.12;

/** En deçà de cette luminance moyenne (0–255), l'image est considérée comme vide. */
const SEUIL_NOIR = 12;

/** Nombre d'affiches gardées sur le disque. 90 × ~14 Ko ≈ 1,2 Mo — le quota `localStorage` en
 * tient bien davantage, mais il porte aussi la progression et « ma liste » : les évincer pour des
 * vignettes serait un mauvais échange. */
const MAX_PERSISTEES = 90;

const CLE = "nie-explorer:cinema:affiches";

/** Cache mémoire — la source de vérité pendant la session. */
const memoire = new Map<string, string>();

/** Chemins dont la capture a échoué : on ne réessaie pas en boucle. */
const echecs = new Set<string>();

let chargeDuDisque = false;

function chargerDisque(): void {
  if (chargeDuDisque) return;
  chargeDuDisque = true;
  try {
    const brut = localStorage.getItem(CLE);
    if (!brut) return;
    const table = JSON.parse(brut) as Record<string, string>;
    for (const [chemin, url] of Object.entries(table)) {
      if (typeof url === "string") memoire.set(chemin, url);
    }
  } catch {
    // Table illisible (quota, JSON tronqué) : on repart d'un cache vide plutôt que de planter.
  }
}

function persister(): void {
  try {
    // Les plus récentes d'abord : `Map` conserve l'ordre d'insertion, donc la fin est ce qui a
    // été capturé en dernier — c'est ce qu'on garde.
    const entrees = [...memoire.entries()].slice(-MAX_PERSISTEES);
    localStorage.setItem(CLE, JSON.stringify(Object.fromEntries(entrees)));
  } catch {
    // Quota plein : les affiches restent en mémoire pour la session. Rien d'autre à faire — les
    // sacrifier est exactement le bon arbitrage, elles sont reconstructibles.
  }
}

/** L'affiche déjà connue d'un film, ou `null`. Aucun décodage n'est lancé. */
export function afficheConnue(chemin: string): string | null {
  chargerDisque();
  return memoire.get(chemin) ?? null;
}

/** Enregistre une affiche capturée ailleurs (l'aperçu au survol, l'en-tête de la fiche). */
export function poserAffiche(chemin: string, url: string): void {
  chargerDisque();
  memoire.set(chemin, url);
  persister();
}

// ── La file ───────────────────────────────────────────────────────────────────

const attente: string[] = [];
const enFile = new Set<string>();
let occupe = false;
const abonnes = new Set<(chemin: string, url: string) => void>();

/** S'abonner aux affiches qui arrivent. Rend la fonction de désabonnement. */
export function surAffiche(f: (chemin: string, url: string) => void): () => void {
  abonnes.add(f);
  return () => abonnes.delete(f);
}

/**
 * Demande l'affiche d'un film.
 *
 * Sans effet si elle est déjà connue, déjà en file, ou déjà tentée sans succès. Le résultat
 * arrive par `surAffiche` — la carte n'attend pas, elle s'affiche avec son fond typographique et
 * se met à jour quand l'image est prête.
 */
export function demanderAffiche(chemin: string): void {
  chargerDisque();
  if (memoire.has(chemin) || enFile.has(chemin) || echecs.has(chemin)) return;
  enFile.add(chemin);
  attente.push(chemin);
  void traiter();
}

/** Vide la file d'attente (pas le cache) — au changement de vue, ce qui n'est plus à l'écran
 * n'a plus de raison d'être décodé. */
export function viderFile(): void {
  attente.length = 0;
  enFile.clear();
}

async function traiter(): Promise<void> {
  if (occupe) return;
  const chemin = attente.shift();
  if (!chemin) return;
  occupe = true;
  try {
    const url = await capturer(chemin);
    if (url) {
      memoire.set(chemin, url);
      persister();
      for (const f of abonnes) f(chemin, url);
    } else {
      echecs.add(chemin);
    }
  } catch {
    echecs.add(chemin);
  } finally {
    enFile.delete(chemin);
    occupe = false;
    if (attente.length > 0) void traiter();
  }
}

/** Luminance moyenne d'un canvas, sur un échantillon d'un pixel sur seize. */
function luminance(ctx: CanvasRenderingContext2D, l: number, h: number): number {
  const { data } = ctx.getImageData(0, 0, l, h);
  let somme = 0;
  let n = 0;
  for (let i = 0; i < data.length; i += 4 * 16) {
    somme += 0.2126 * (data[i] ?? 0) + 0.7152 * (data[i + 1] ?? 0) + 0.0722 * (data[i + 2] ?? 0);
    n += 1;
  }
  return n === 0 ? 0 : somme / n;
}

/**
 * Capture une image du film, dans un `<video>` hors écran.
 *
 * `crossOrigin` est indispensable : le protocole `nievideo` a sa propre origine sous Windows, et
 * sans requête CORS le `canvas` serait teinté — `toDataURL` lèverait, donc aucune affiche.
 */
function capturer(chemin: string): Promise<string | null> {
  return new Promise((resoudre) => {
    const src = urlVideo(chemin);
    if (!src) {
      resoudre(null);
      return;
    }

    const v = document.createElement("video");
    v.crossOrigin = "anonymous";
    v.muted = true;
    v.preload = "auto";
    v.playsInline = true;
    // Hors flux, mais dans le document : certains moteurs ne décodent pas un média détaché.
    v.style.cssText = "position:fixed;left:-9999px;top:0;width:2px;height:2px;opacity:0";
    let fini = false;
    let replTente = false;

    const terminer = (url: string | null) => {
      if (fini) return;
      fini = true;
      clearTimeout(minuterie);
      v.removeAttribute("src");
      v.load();
      v.remove();
      resoudre(url);
    };

    const minuterie = setTimeout(() => terminer(null), DELAI_MAX);

    const dessiner = (): string | null => {
      if (v.videoWidth === 0) return null;
      const canvas = document.createElement("canvas");
      canvas.width = LARGEUR;
      canvas.height = Math.max(1, Math.round((LARGEUR * v.videoHeight) / v.videoWidth));
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      if (!ctx) return null;
      ctx.drawImage(v, 0, 0, canvas.width, canvas.height);
      // Une image noire ne distingue rien : on ne la retient pas, on retente plus loin.
      if (!replTente && luminance(ctx, canvas.width, canvas.height) < SEUIL_NOIR) return null;
      try {
        return canvas.toDataURL("image/jpeg", QUALITE);
      } catch {
        return null;
      }
    };

    v.addEventListener("loadeddata", () => {
      // La première image est déjà décodée à `loadeddata` : pas besoin de chercher pour l'avoir.
      const url = dessiner();
      if (url) {
        terminer(url);
        return;
      }
      if (!replTente && v.duration > 2) {
        replTente = true;
        v.currentTime = v.duration * INSTANT_REPLI;
        return;
      }
      terminer(null);
    });

    v.addEventListener("seeked", () => terminer(dessiner()));
    v.addEventListener("error", () => terminer(null));

    document.body.append(v);
    v.src = src;
  });
}
