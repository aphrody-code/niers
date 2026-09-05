// Barre latérale « emplacements » — inspirée de cosmic-files (favoris épinglés) et yazi
// (historique par fréquence de visite, dit « frecency »). Le VFS a ~254 800 fichiers sous
// une poignée de racines profondes : sans raccourcis, chaque saut de contexte coûte 4-6 clics.
import { useSyncExternalStore } from "react";

export interface Place {
  label: string;
  prefix: string;
  icon: string;
}

/** Racines curées, vérifiées contre le VFS réel (cf. `crates/engine/nie-explore/src/folder_roles.rs`). */
export const PINNED_PLACES: Place[] = [
  { label: "Racine", prefix: "", icon: "hard_drive" },
  { label: "Personnages (visages)", prefix: "data/common/chr/_face", icon: "person" },
  { label: "Personnages (données)", prefix: "data/common/gamedata/character", icon: "database" },
  { label: "Techniques (bannières)", prefix: "data/dx11/menu/220_img/telop_waza", icon: "swords" },
  { label: "Icônes de menu", prefix: "data/dx11/menu/200_icon", icon: "grid_view" },
  { label: "Sons", prefix: "data/common/sound_asset", icon: "volume_up" },
  { label: "Textures (dx11)", prefix: "data/dx11", icon: "image" },
  { label: "Modèles/anim (common)", prefix: "data/common", icon: "view_in_ar" },
];

const RECENTS_KEY = "nie-explorer:recents";
const RECENTS_MAX = 12;

interface RecentEntry {
  prefix: string;
  visits: number;
  lastVisit: number;
}

function loadRecents(): RecentEntry[] {
  try {
    const raw = localStorage.getItem(RECENTS_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

let recents: RecentEntry[] = loadRecents();
const listeners = new Set<() => void>();

function persist() {
  localStorage.setItem(RECENTS_KEY, JSON.stringify(recents));
  listeners.forEach((l) => l());
}

/** Enregistre une visite (navigation `goto`) — alimente le tri par fréquence+récence. */
export function recordVisit(prefix: string): void {
  if (!prefix) return; // la racine ne mérite pas d'entrée « récents »
  const existing = recents.find((r) => r.prefix === prefix);
  if (existing) {
    existing.visits += 1;
    existing.lastVisit = Date.now();
  } else {
    recents.push({ prefix, visits: 1, lastVisit: Date.now() });
  }
  // Score « frecency » à la yazi : visites * décroissance temporelle (demi-vie ~7 jours).
  const now = Date.now();
  const score = (r: RecentEntry) => r.visits * Math.exp(-(now - r.lastVisit) / (7 * 86_400_000));
  recents = recents.sort((a, b) => score(b) - score(a)).slice(0, RECENTS_MAX);
  persist();
}

/** Retire UN emplacement de l'historique (clic droit → « Retirer des récents »). Sans ça, une
 * entrée récente ne pouvait que sortir d'elle-même en tombant hors du top 12 — l'utilisatrice
 * n'avait aucun moyen d'effacer une visite ponctuelle. */
export function forgetRecent(prefix: string): void {
  const before = recents.length;
  recents = recents.filter((r) => r.prefix !== prefix);
  if (recents.length !== before) persist();
}

/** Vide tout l'historique des emplacements récents. */
export function clearRecents(): void {
  if (recents.length === 0) return;
  recents = [];
  persist();
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** Emplacements récents triés par fréquence (hook réactif, sans provider). */
export function useRecentPlaces(): RecentEntry[] {
  return useSyncExternalStore(subscribe, () => recents);
}

// ── Dossiers épinglés par l'utilisatrice — Ctrl+D, comme « Add to sidebar » de cosmic-files ──

const PINS_KEY = "nie-explorer:pinned";
let pinned: string[] = (() => {
  try {
    const raw = localStorage.getItem(PINS_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
})();
const pinListeners = new Set<() => void>();

function persistPins() {
  localStorage.setItem(PINS_KEY, JSON.stringify(pinned));
  pinListeners.forEach((l) => l());
}

/** Épingle/désépingle `prefix` (Ctrl+D dans l'Explorateur, comme cosmic-files). */
export function togglePin(prefix: string): void {
  if (!prefix) return;
  pinned = pinned.includes(prefix) ? pinned.filter((p) => p !== prefix) : [...pinned, prefix];
  persistPins();
}

export function isPinned(prefix: string): boolean {
  return pinned.includes(prefix);
}

function subscribePins(cb: () => void): () => void {
  pinListeners.add(cb);
  return () => pinListeners.delete(cb);
}

/** Dossiers épinglés (hook réactif). */
export function usePinnedPlaces(): string[] {
  return useSyncExternalStore(subscribePins, () => pinned);
}
