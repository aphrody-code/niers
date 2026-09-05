// Vignettes de textures pour les grilles de fichiers — source UNIQUE, partagée par la vue grille
// de l'Explorateur et le navigateur de contenu de l'éditeur (qui en avaient chacun une copie).
//
// Ce que ce module corrige, mesuré sur cette installation :
//
//   * **résolution** — les deux grilles appelaient `api.texturePngB64`, qui décode la texture
//     PLEINE RÉSOLUTION. Une texture de personnage fait 2048×2048 : 16 Mio de bitmap dans le
//     processus de rendu, pour une vignette de 88 px. Afficher une seule page d'un dossier de
//     textures faisait passer le renderer WebView2 de 453 à 704 Mio ; le VFS contient un dossier
//     de 12 560 `.g4tx` (`data/dx11/menu/200_icon/10_icon_chr/uniform`), et le parcourir tuait la
//     fenêtre. On demande désormais une vignette bornée à 128 px, réduite CÔTÉ RUST
//     (`api.textureThumbB64`) : quelques kio au lieu de plusieurs Mio, avant même l'IPC.
//
//   * **cache non borné** — le cache module-level n'oubliait jamais rien : en défilant un gros
//     dossier, il accumulait une data-URL par fichier vu, sans limite. Il est maintenant borné
//     (LRU) : au-delà de `MAX_ENTREES`, les plus anciennes sont oubliées.
//
//   * **concurrence** — chaque vignette qui devenait visible lançait sa commande immédiatement.
//     Un défilement rapide en déclenchait des centaines en parallèle, qui se disputaient le VFS
//     et noyaient l'IPC. Une file limite à `MAX_PARALLELE` décodages simultanés.
//
// L'entrée `null` en cache signifie « décodage tenté et échoué » (texture factice, format non
// reconnu) : on ne réessaie pas indéfiniment.
import { useEffect, useRef, useState } from "react";

import { api } from "@/lib/api";

/** Extensions dont on sait produire une vignette. */
export const THUMB_EXTS = new Set(["g4tx"]);

/** Plus grand côté demandé au backend. 128 : les grilles affichent ~88 px, le double couvre les
 * écrans à 2 dpr. */
const COTE = 128;

/** Nombre de vignettes gardées en mémoire. 400 × ~8 kio ≈ 3 Mio — assez pour un aller-retour
 * dans un dossier, sans le comportement « le cache grossit jusqu'à ce que ça tombe ». */
const MAX_ENTREES = 400;

/** Décodages simultanés. Au-delà, la file attend : le goulot est le VFS, pas le client. */
const MAX_PARALLELE = 4;

/** `Map` = ordre d'insertion → LRU par simple `delete`/`set` à chaque lecture. */
const cache = new Map<string, string | null>();

function cacheGet(path: string): string | null | undefined {
  if (!cache.has(path)) return undefined;
  const v = cache.get(path)!;
  cache.delete(path);
  cache.set(path, v); // remonte en tête de fraîcheur
  return v;
}

function cacheSet(path: string, valeur: string | null) {
  cache.delete(path);
  cache.set(path, valeur);
  while (cache.size > MAX_ENTREES) {
    const plusAncien = cache.keys().next();
    if (plusAncien.done) break;
    cache.delete(plusAncien.value);
  }
}

let enCours = 0;
const attente: (() => void)[] = [];

/** Attend un jeton de la file de décodage. */
function acquerir(): Promise<void> {
  if (enCours < MAX_PARALLELE) {
    enCours++;
    return Promise.resolve();
  }
  return new Promise((resolve) => attente.push(resolve));
}

function liberer() {
  const suivant = attente.shift();
  if (suivant) suivant();
  else enCours--;
}

/** Vide le cache — appelé au changement de dossier du jeu (les chemins ne désignent plus la même
 * chose). */
export function viderCacheVignettes() {
  cache.clear();
}

/**
 * Vignette d'un fichier, chargée seulement quand l'élément approche de l'écran.
 *
 * Rend `ref` (à poser sur le conteneur) et `src` (`null` tant qu'il n'y a rien à afficher).
 */
export function useThumbnail(path: string, ext: string, gameDir?: string) {
  const supporte = THUMB_EXTS.has(ext);
  const [src, setSrc] = useState<string | null>(() => (supporte ? cacheGet(path) ?? null : null));
  const [visible, setVisible] = useState(() => supporte && cacheGet(path) !== undefined);
  const ref = useRef<HTMLDivElement | null>(null);

  // Le même composant est réutilisé pour un autre chemin quand la liste change (React recycle par
  // position quand la clé bouge) : sans cette remise à zéro, une vignette resterait affichée sur
  // le mauvais fichier.
  useEffect(() => {
    if (!supporte) {
      setSrc(null);
      setVisible(false);
      return;
    }
    const connu = cacheGet(path);
    setSrc(connu ?? null);
    setVisible(connu !== undefined);
  }, [path, supporte]);

  useEffect(() => {
    if (visible || !supporte || !ref.current) return;
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          setVisible(true);
          obs.disconnect();
        }
      },
      { rootMargin: "150px" },
    );
    obs.observe(ref.current);
    return () => obs.disconnect();
  }, [visible, supporte]);

  useEffect(() => {
    if (!visible || !supporte || cacheGet(path) !== undefined) return;
    let annule = false;
    acquerir().then(() => {
      // Le composant a pu défiler hors champ / changer de chemin pendant l'attente du jeton :
      // décoder pour personne coûte le même prix que décoder pour quelqu'un.
      if (annule) {
        liberer();
        return;
      }
      api
        .textureThumbB64(path, COTE, gameDir)
        .then((b64) => {
          const url = `data:image/png;base64,${b64}`;
          cacheSet(path, url);
          if (!annule) setSrc(url);
        })
        .catch(() => {
          cacheSet(path, null);
        })
        .finally(liberer);
    });
    return () => {
      annule = true;
    };
  }, [visible, supporte, path, gameDir]);

  return { ref, src, supporte };
}
