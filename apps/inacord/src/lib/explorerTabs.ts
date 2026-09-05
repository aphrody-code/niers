// Onglets de l'Explorateur — même patron que `lib/places.ts` : un état module-level, un jeu de
// réducteurs PURS, et `useSyncExternalStore` pour l'abonnement React. Aucun React dans la logique
// ci-dessous : les réducteurs sont testables et réutilisables tels quels.
//
// L'Explorateur n'avait qu'UN état de navigation (`{prefix, selected}` dans `App.tsx`) : ouvrir un
// dossier depuis la barre latérale écrasait le contexte de travail précédent, et tout l'état de
// présentation (filtre, tri, vue, taille de vignette) repartait de zéro à chaque aller-retour.
// Chaque onglet porte donc son propre contexte complet ET son propre historique arrière/avant.
import { useSyncExternalStore } from "react";

export type ExplorerSortKey = "name" | "size";
export type ExplorerViewMode = "list" | "grid";

export interface ExplorerTab {
  /** Identité stable de l'onglet — clé React et cible des actions, jamais réutilisée. */
  id: string;
  prefix: string;
  selected: string | null;
  /** Recherche VFS en cours dans cet onglet. */
  query?: string;
  /** Filtre d'extension du champ dédié. */
  ext?: string;
  sortKey?: ExplorerSortKey;
  viewMode?: ExplorerViewMode;
  gridSize?: number;
  /** Préfixes visités, du plus ancien au plus récent — pile arrière/avant de CET onglet. */
  history: string[];
  /** Position courante dans `history` ; tout ce qui suit est la pile « avant ». */
  historyIndex: number;
}

export interface ExplorerTabsState {
  tabs: ExplorerTab[];
  activeId: string;
}

/** Champs modifiables d'un onglet — `id`/`history` appartiennent au réducteur, pas à l'appelant. */
export type ExplorerTabPatch = Partial<Omit<ExplorerTab, "id" | "history" | "historyIndex">>;

const STORAGE_KEY = "nie-explorer:tabs";
/** Au-delà, l'historique d'un onglet est tronqué par la tête — une pile infinie ne sert personne
 * et gonflerait le `localStorage` à chaque navigation. */
const HISTORY_MAX = 64;
/** Préfixe du premier onglet — l'Explorateur s'ouvrait déjà là avant les onglets. */
const DEFAULT_PREFIX = "data";

// ── Réducteurs purs ──────────────────────────────────────────────────────────────────────────

/** Onglet neuf, historique amorcé sur son préfixe initial. */
export function makeTab(id: string, prefix: string, selected: string | null = null): ExplorerTab {
  return { id, prefix, selected, history: [prefix], historyIndex: 0 };
}

/** Ajoute un onglet après l'onglet actif (comme un navigateur), activé sauf `activate: false`. */
export function openTab(
  state: ExplorerTabsState,
  id: string,
  prefix: string,
  selected: string | null = null,
  activate = true,
): ExplorerTabsState {
  const tab = makeTab(id, prefix, selected);
  const at = state.tabs.findIndex((t) => t.id === state.activeId);
  const tabs = [...state.tabs];
  tabs.splice(at === -1 ? tabs.length : at + 1, 0, tab);
  return { tabs, activeId: activate ? id : state.activeId };
}

/** Ferme un onglet. Le DERNIER ne se ferme jamais : sans onglet, l'Explorateur n'a plus rien à
 * afficher — le fermer laisserait une vue vide sans moyen d'en rouvrir un. */
export function closeTab(state: ExplorerTabsState, id: string): ExplorerTabsState {
  if (state.tabs.length <= 1) return state;
  const at = state.tabs.findIndex((t) => t.id === id);
  if (at === -1) return state;
  const tabs = state.tabs.filter((t) => t.id !== id);
  if (id !== state.activeId) return { tabs, activeId: state.activeId };
  const neighbour = tabs[Math.min(at, tabs.length - 1)];
  return { tabs, activeId: neighbour ? neighbour.id : tabs[0]!.id };
}

export function activateTab(state: ExplorerTabsState, id: string): ExplorerTabsState {
  if (id === state.activeId || !state.tabs.some((t) => t.id === id)) return state;
  return { tabs: state.tabs, activeId: id };
}

/** Active l'onglet à `delta` positions du courant, en boucle (Ctrl+Tab / Ctrl+Maj+Tab). */
export function cycleTab(state: ExplorerTabsState, delta: number): ExplorerTabsState {
  const at = state.tabs.findIndex((t) => t.id === state.activeId);
  if (at === -1 || state.tabs.length < 2) return state;
  const n = state.tabs.length;
  const next = state.tabs[(((at + delta) % n) + n) % n]!;
  return { tabs: state.tabs, activeId: next.id };
}

/** Applique un patch à un onglet. Un changement de `prefix` empile l'historique et purge la pile
 * « avant » — sémantique d'un navigateur : naviguer depuis un point de l'historique abandonne la
 * branche qui suivait. */
export function updateTab(state: ExplorerTabsState, id: string, patch: ExplorerTabPatch): ExplorerTabsState {
  const at = state.tabs.findIndex((t) => t.id === id);
  if (at === -1) return state;
  const prev = state.tabs[at]!;
  const next: ExplorerTab = { ...prev, ...patch };
  if (patch.prefix !== undefined && patch.prefix !== prev.prefix) {
    const kept = prev.history.slice(0, prev.historyIndex + 1);
    kept.push(patch.prefix);
    const trimmed = kept.length > HISTORY_MAX ? kept.slice(kept.length - HISTORY_MAX) : kept;
    next.history = trimmed;
    next.historyIndex = trimmed.length - 1;
  }
  const tabs = [...state.tabs];
  tabs[at] = next;
  return { tabs, activeId: state.activeId };
}

export function canGoBack(tab: ExplorerTab): boolean {
  return tab.historyIndex > 0;
}

export function canGoForward(tab: ExplorerTab): boolean {
  return tab.historyIndex < tab.history.length - 1;
}

/** Recule d'un cran — ne réempile RIEN (sinon l'arrière deviendrait un aller simple). */
export function goBack(state: ExplorerTabsState, id: string): ExplorerTabsState {
  return travel(state, id, -1);
}

export function goForward(state: ExplorerTabsState, id: string): ExplorerTabsState {
  return travel(state, id, +1);
}

function travel(state: ExplorerTabsState, id: string, delta: number): ExplorerTabsState {
  const at = state.tabs.findIndex((t) => t.id === id);
  if (at === -1) return state;
  const prev = state.tabs[at]!;
  const idx = prev.historyIndex + delta;
  if (idx < 0 || idx >= prev.history.length) return state;
  const tabs = [...state.tabs];
  tabs[at] = { ...prev, prefix: prev.history[idx]!, selected: null, historyIndex: idx };
  return { tabs, activeId: state.activeId };
}

// ── Store module-level ───────────────────────────────────────────────────────────────────────

let idSeq = 0;

/** Identifiant d'onglet unique pour la session — le compteur est réamorcé au-dessus des ids
 * restaurés pour qu'un onglet neuf n'entre jamais en collision avec un onglet persisté. */
export function newTabId(): string {
  idSeq += 1;
  return `tab-${idSeq}`;
}

function freshState(): ExplorerTabsState {
  const id = newTabId();
  return { tabs: [makeTab(id, DEFAULT_PREFIX)], activeId: id };
}

/** Restauration DÉFENSIVE : un `localStorage` absent, corrompu, d'une version antérieure du type
 * ou réduit à un tableau vide doit rendre l'Explorateur utilisable, pas le laisser sans onglet. */
function load(): ExplorerTabsState {
  let raw: string | null = null;
  try {
    raw = localStorage.getItem(STORAGE_KEY);
  } catch {
    return freshState();
  }
  if (!raw) return freshState();
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return freshState();
    const src = (parsed as { tabs?: unknown }).tabs;
    if (!Array.isArray(src)) return freshState();
    const tabs: ExplorerTab[] = [];
    for (const t of src) {
      if (!t || typeof t !== "object") continue;
      const o = t as Record<string, unknown>;
      if (typeof o.prefix !== "string") continue;
      const id = typeof o.id === "string" && o.id ? o.id : newTabId();
      if (tabs.some((x) => x.id === id)) continue;
      const history = Array.isArray(o.history) && o.history.every((h) => typeof h === "string")
        ? (o.history as string[])
        : [o.prefix];
      const rawIndex = typeof o.historyIndex === "number" ? o.historyIndex : history.length - 1;
      const historyIndex = Math.min(Math.max(0, Math.trunc(rawIndex)), Math.max(0, history.length - 1));
      tabs.push({
        id,
        prefix: o.prefix,
        selected: typeof o.selected === "string" ? o.selected : null,
        ...(typeof o.query === "string" ? { query: o.query } : {}),
        ...(typeof o.ext === "string" ? { ext: o.ext } : {}),
        ...(o.sortKey === "name" || o.sortKey === "size" ? { sortKey: o.sortKey } : {}),
        ...(o.viewMode === "list" || o.viewMode === "grid" ? { viewMode: o.viewMode } : {}),
        ...(typeof o.gridSize === "number" && Number.isFinite(o.gridSize) ? { gridSize: o.gridSize } : {}),
        history: history.length > 0 ? history : [o.prefix],
        historyIndex,
      });
    }
    if (tabs.length === 0) return freshState();
    // Le compteur d'ids doit dépasser tout id restauré, sinon `newTabId` recréerait une clé déjà
    // présente (deux onglets indistinguables côté React et côté actions).
    for (const t of tabs) {
      const n = /^tab-(\d+)$/.exec(t.id);
      if (n) idSeq = Math.max(idSeq, Number(n[1]));
    }
    const wanted = (parsed as { activeId?: unknown }).activeId;
    const activeId = typeof wanted === "string" && tabs.some((t) => t.id === wanted) ? wanted : tabs[0]!.id;
    return { tabs, activeId };
  } catch {
    return freshState();
  }
}

let state: ExplorerTabsState = load();
const listeners = new Set<() => void>();

function persist(): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Quota plein ou stockage refusé : les onglets restent parfaitement utilisables en mémoire.
  }
  listeners.forEach((l) => l());
}

function apply(fn: (s: ExplorerTabsState) => ExplorerTabsState): void {
  const next = fn(state);
  if (next === state) return; // identité inchangée = aucun rendu inutile
  state = next;
  persist();
}

export function getExplorerTabs(): ExplorerTabsState {
  return state;
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

/** Onglets de l'Explorateur (hook réactif, sans provider — comme `usePinnedPlaces`). */
export function useExplorerTabs(): ExplorerTabsState {
  return useSyncExternalStore(subscribe, getExplorerTabs);
}

/** Actions liées au store — chacune délègue au réducteur pur correspondant. */
export const explorerTabs = {
  open(prefix: string, selected: string | null = null, activate = true): string {
    const id = newTabId();
    apply((s) => openTab(s, id, prefix, selected, activate));
    return id;
  },
  close(id: string): void {
    apply((s) => closeTab(s, id));
  },
  activate(id: string): void {
    apply((s) => activateTab(s, id));
  },
  cycle(delta: number): void {
    apply((s) => cycleTab(s, delta));
  },
  update(id: string, patch: ExplorerTabPatch): void {
    apply((s) => updateTab(s, id, patch));
  },
  back(id: string): void {
    apply((s) => goBack(s, id));
  },
  forward(id: string): void {
    apply((s) => goForward(s, id));
  },
};
