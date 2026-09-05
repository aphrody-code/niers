// i18n minimal, sans dépendance (react-i18next serait surdimensionné pour ~80 clés).
// Dictionnaires FR/EN/JA + hook `useT()` réactif sur `useSettings().locale`.
import { useSettings } from "@/lib/settings";
import type { Locale } from "@/lib/settings";

type Dict = Record<string, string>;

const fr: Dict = {
  "app.title": "niers",
  // Une clé PAR VUE, sans exception : six vues portaient leur libellé en dur dans `App.tsx`
  // (« Éditeur », « Cinéma », « Galerie », « Outils wiki », « Viola », « Live mod », « Lua »),
  // donc restaient en français dans les trois locales. Le registre `lib/vues.ts` lit ces clés.
  "tab.dashboard": "Tableau de bord",
  "tab.editor": "Éditeur 3D",
  "tab.explorer": "Explorateur",
  "tab.search": "Recherche",
  "tab.cinema": "Cinéma",
  "tab.data": "Encyclopédie",
  "tab.gallery": "Galerie",
  "tab.tools": "Boîte à outils",
  "tab.mods": "Mods",
  // Les quatre outils de spécialiste portaient des noms qui ne disent rien à qui ne les a pas
  // écrits : « RE », « Viola », « Live mod », « Lua ». Ils sont masqués par défaut, mais quand on
  // les affiche il faut pouvoir deviner ce qu'ils font sans ouvrir la vue.
  "tab.cpk": "Archives CPK",
  "tab.re": "Reverse-engineering",
  "tab.viola": "Archives Criware",
  "tab.livemod": "Mémoire du jeu",
  "tab.lua": "Scripts Lua",
  "tab.save": "Sauvegardes",
  "tab.settings": "Paramètres",

  "external.opened": "Fichier ouvert depuis l'explorateur Windows",
  "external.close": "✕ fermer",

  "explorer.root": "Racine",
  "explorer.parent": "Dossier parent",
  "explorer.search_placeholder": "Rechercher (sous-chaîne)…",
  "explorer.ext_placeholder": "ext",
  "explorer.empty": "Aucune entrée.",
  "explorer.loading": "chargement…",
  "explorer.count": "{dirs} dossier(s), {files} fichier(s)",
  "explorer.results": "{n} résultat(s)",
  "explorer.places": "Emplacements",
  "explorer.recents": "Récents",
  "explorer.sort_name": "Trier par nom",
  "explorer.sort_size": "Trier par taille",

  "settings.appearance": "Apparence",
  "settings.language": "Langue",
  "settings.theme": "Thème",
  "settings.theme.light": "Clair",
  "settings.theme.dark": "Sombre",
  "settings.theme.system": "Système",
  "settings.font_scale": "Taille de police",
  "settings.ui_zoom": "Zoom de l'interface",
  "settings.reset": "Réinitialiser",
};

const en: Dict = {
  "app.title": "niers",
  "tab.dashboard": "Dashboard",
  "tab.editor": "3D editor",
  "tab.explorer": "Explorer",
  "tab.search": "Search",
  "tab.cinema": "Cinema",
  "tab.data": "Encyclopedia",
  "tab.gallery": "Gallery",
  "tab.tools": "Toolbox",
  "tab.mods": "Mods",
  "tab.cpk": "CPK archives",
  "tab.re": "Reverse engineering",
  "tab.viola": "Criware archives",
  "tab.livemod": "Game memory",
  "tab.lua": "Lua scripts",
  "tab.save": "Saves",
  "tab.settings": "Settings",

  "external.opened": "File opened from Windows Explorer",
  "external.close": "✕ close",

  "explorer.root": "Root",
  "explorer.parent": "Parent folder",
  "explorer.search_placeholder": "Search (substring)…",
  "explorer.ext_placeholder": "ext",
  "explorer.empty": "No entries.",
  "explorer.loading": "loading…",
  "explorer.count": "{dirs} folder(s), {files} file(s)",
  "explorer.results": "{n} result(s)",
  "explorer.places": "Places",
  "explorer.recents": "Recent",
  "explorer.sort_name": "Sort by name",
  "explorer.sort_size": "Sort by size",

  "settings.appearance": "Appearance",
  "settings.language": "Language",
  "settings.theme": "Theme",
  "settings.theme.light": "Light",
  "settings.theme.dark": "Dark",
  "settings.theme.system": "System",
  "settings.font_scale": "Font size",
  "settings.ui_zoom": "Interface zoom",
  "settings.reset": "Reset",
};

const ja: Dict = {
  "app.title": "niers",
  "tab.dashboard": "ダッシュボード",
  "tab.editor": "3Dエディター",
  "tab.explorer": "エクスプローラー",
  "tab.search": "検索",
  "tab.cinema": "シネマ",
  "tab.data": "図鑑",
  "tab.gallery": "ギャラリー",
  "tab.tools": "ツールボックス",
  "tab.mods": "MOD",
  "tab.cpk": "CPKアーカイブ",
  "tab.re": "リバースエンジニアリング",
  "tab.viola": "Criwareアーカイブ",
  "tab.livemod": "ゲームメモリ",
  "tab.lua": "Luaスクリプト",
  "tab.save": "セーブデータ",
  "tab.settings": "設定",

  "external.opened": "Windows エクスプローラーから開いたファイル",
  "external.close": "✕ 閉じる",

  "explorer.root": "ルート",
  "explorer.parent": "親フォルダ",
  "explorer.search_placeholder": "検索(部分一致)…",
  "explorer.ext_placeholder": "拡張子",
  "explorer.empty": "項目がありません。",
  "explorer.loading": "読み込み中…",
  "explorer.count": "フォルダ {dirs} 件、ファイル {files} 件",
  "explorer.results": "{n} 件の結果",
  "explorer.places": "場所",
  "explorer.recents": "最近使った項目",
  "explorer.sort_name": "名前で並べ替え",
  "explorer.sort_size": "サイズで並べ替え",

  "settings.appearance": "外観",
  "settings.language": "言語",
  "settings.theme": "テーマ",
  "settings.theme.light": "ライト",
  "settings.theme.dark": "ダーク",
  "settings.theme.system": "システム",
  "settings.font_scale": "文字サイズ",
  "settings.ui_zoom": "画面ズーム",
  "settings.reset": "リセット",
};

const DICTS: Record<Locale, Dict> = { fr, en, ja };

export const LOCALE_LABELS: Record<Locale, string> = {
  fr: "Français",
  en: "English",
  ja: "日本語",
};

function interpolate(template: string, vars?: Record<string, string | number>): string {
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
}

/** Traduction réactive : re-render dès que `settings.locale` change. Repli FR -> clé brute. */
/** La fonction de traduction rendue par [`useT`] — nommée pour être passable en argument
 * (`lib/vues.ts` la reçoit : le registre des vues ne peut pas appeler un hook). */
export type TFn = (key: string, vars?: Record<string, string | number>) => string;

export function useT(): TFn {
  const { locale } = useSettings();
  const dict = DICTS[locale] ?? fr;
  return (key, vars) => interpolate(dict[key] ?? fr[key] ?? key, vars);
}
