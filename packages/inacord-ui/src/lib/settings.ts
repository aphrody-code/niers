// Paramètres persistés (localStorage — pas de plugin-store nécessaire pour ces quelques valeurs).
import { useSyncExternalStore } from "react";

export type Locale = "fr" | "en" | "ja";

/** Variante de palette sombre — mêmes noms et mêmes valeurs que les thèmes de
 * `var/spaceui/packages/tokens/src/css/themes/*.css`. `spacedrive` = la palette de base
 * (`theme.css`, hue 235) ; les autres sont ses variantes officielles. Ignorée en thème clair
 * (`.light` de spaceui est la seule palette claire fournie), cf. `lib/appearance.ts`. */
export type AccentTheme = "spacedrive" | "midnight" | "noir" | "slate" | "nord" | "mocha";

/** Ordre d'affichage dans Paramètres (libellés côté UI). */
export const ACCENT_THEMES: readonly AccentTheme[] = [
  "spacedrive",
  "midnight",
  "noir",
  "slate",
  "nord",
  "mocha",
];

export interface Settings {
  gameDir: string;
  wikiDb: string;
  blenderExe: string;
  azaleeUrl: string;
  /** Origine de `nie-model-serve`/CDN : avatar assemblé et rendu de menus réels. */
  modelServiceUrl: string;
  /** Langue de l'interface. */
  locale: Locale;
  /** Échelle de la taille de police de base (agit sur `html { font-size }`, tout le reste est en rem). */
  fontScale: number;
  /** Zoom global de l'interface (CSS `zoom`, WebView2/Chromium). */
  uiZoom: number;
  /** Variante de palette sombre — cf. [`AccentTheme`]. */
  accentTheme: AccentTheme;
  /**
   * Autorise le serveur MCP `nie-mcp` à piloter cette fenêtre (naviguer, ouvrir un asset,
   * changer d'onglet) via le pont local `@niers/bridge`. Désactivé, aucun socket n'est ouvert.
   */
  bridgeEnabled: boolean;
  /**
   * Affiche les outils de spécialiste dans la barre latérale : RE, Viola, Live mod, Lua
   * (les vues marquées `avancee` dans `lib/vues.ts`).
   *
   * **Faux par défaut**, et c'est le point : l'application s'ouvre sur une médiathèque, or la
   * barre latérale offrait d'emblée le reverse-engineering, la lecture de la mémoire du jeu et
   * le désassemblage de scripts. Ces quatre entrées ne servent qu'à une personne ; les laisser
   * en permanence noyait les cinq qui comptent pour tout le monde.
   *
   * Rien n'est retiré : les vues restent atteignables par la palette de commandes (Ctrl+K) même
   * quand ce réglage est faux.
   */
  outilsAvances: boolean;
}

// Le thème clair/sombre/système est géré par next-themes (sa propre clé localStorage
// "theme") — pas dupliqué ici pour éviter deux sources de vérité qui divergent.
const KEY = "nie-explorer:settings";
const DEFAULTS: Settings = {
  gameDir: "",
  wikiDb: "",
  blenderExe: "",
  azaleeUrl: "",
  modelServiceUrl: "",
  locale: "fr",
  fontScale: 1,
  uiZoom: 1,
  accentTheme: "spacedrive",
  bridgeEnabled: true,
  outilsAvances: false,
};

function load(): Settings {
  try {
    const raw = localStorage.getItem(KEY);
    const merged: Settings = raw ? { ...DEFAULTS, ...JSON.parse(raw) } : DEFAULTS;
    // Migration : `accentTheme: "azalee"` (ancienne palette MD3, retirée avec le portage complet
    // des tokens spaceui) n'existe plus — sans ce garde, une valeur persistée pointerait vers une
    // classe CSS inexistante et l'app resterait sur la palette de base sans jamais s'en expliquer.
    if (!ACCENT_THEMES.includes(merged.accentTheme)) merged.accentTheme = DEFAULTS.accentTheme;
    return merged;
  } catch {
    return DEFAULTS;
  }
}

let state: Settings = load();
const listeners = new Set<() => void>();

export function getSettings(): Settings {
  return state;
}

export function setSettings(patch: Partial<Settings>): void {
  state = { ...state, ...patch };
  localStorage.setItem(KEY, JSON.stringify(state));
  listeners.forEach((l) => l());
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** Hook réactif : re-render au moindre `setSettings`, sans provider/context. */
export function useSettings(): Settings {
  return useSyncExternalStore(subscribe, getSettings);
}
