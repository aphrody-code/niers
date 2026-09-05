// Images des composants du wiki, résolues **dans le VFS local** — remplace
// `@rosegriffon/azalee/images` pour les composants portés dans `components/wiki/`.
//
// ## Pourquoi ne pas importer le module du wiki
//
// Deux raisons, l'une de fond, l'autre mesurée :
//
//  1. le module du wiki rend des URL du CDN `azalee` : l'application de bureau doit fonctionner
//     **hors ligne**, sur les fichiers du jeu qu'elle a déjà montés. C'est toute sa raison d'être ;
//  2. `packages/azalee/src/images/utils.ts` importe sept manifestes JSON (`../data/
//     character-face-manifest.json`, …) qui sont **générés** par `packages/azalee/scripts/build-*`
//     et absents du dépôt : l'importer ici casse `tsc` (`Cannot find module`) et le build Vite.
//
// ## Ce qui est résolu, et ce qui ne peut pas l'être
//
// Nommage RELEVÉ sur l'installation Steam (`niers vfs ls`, 2026-09-03), pas supposé :
//
// | Famille    | Chemin VFS                                                     | Forme         |
// |------------|----------------------------------------------------------------|---------------|
// | visages    | `…/200_icon/10_icon_chr/face/<code>_l.g4tx`                     | un par code   |
// | entraîneurs| `…/200_icon/10_icon_chr/coach/coach<NN>_l.g4tx`                 | un par numéro |
// | écussons   | `…/200_icon/01_icon_emblem/<em####>.g4tx` (+ `_s` en petit)     | un par code   |
//
// Les objets, les techniques et les tactiques n'ont **pas** d'icône par entité : ce sont des
// ATLAS (`02_icon_item/icon_item01.g4tx` — 3 Mo, plusieurs centaines d'icônes ; `13_icon_tactics/
// icon_tactics.g4tx`). Sans index de découpe, une icône par objet n'est pas résolvable — ces
// fonctions rendent donc une chaîne vide, et les cartes affichent leur repli. Inventer un chemin
// « probable » produirait des images muettes que rien ne signalerait.

/** Racine des icônes de menu (rendu DX11) — commune à toutes les familles ci-dessous. */
const ICONES = "data/dx11/menu/200_icon";

/** Chemin VFS d'un visage de personnage. `code` = code interne (`c01000010`). */
export function getCharacterFaceUrl(code: string | undefined | null): string {
  if (!code) return "";
  // Les variantes de tenue (`c01000010_5000`) n'ont pas de visage propre : seul le code de base
  // en a un — même règle que le module du wiki.
  const base = code.replace(/_\d{4}$/, "");
  return `${ICONES}/10_icon_chr/face/${base}_l.g4tx`;
}

/** Chemin VFS d'un portrait d'entraîneur (`coach01`…). */
export function getCoachFaceUrl(code: string | undefined | null): string {
  if (!code) return "";
  const n = /^\d+$/.test(code) ? code.padStart(2, "0") : code.replace(/^coach/, "");
  return `${ICONES}/10_icon_chr/coach/coach${n}_l.g4tx`;
}

/** Chemin VFS d'un écusson d'équipe. `petit` rend la variante `_s` (65 ko au lieu de 262 ko). */
export function getEmblemUrl(code: string | undefined | null, petit = false): string {
  if (!code) return "";
  return `${ICONES}/01_icon_emblem/${code}${petit ? "_s" : ""}.g4tx`;
}

/** Atlas non indexé : aucune icône par objet n'est résolvable (cf. l'en-tête). */
export function getItemIconUrl(_id?: string | null): string {
  return "";
}

/** Atlas non indexé, cf. [`getItemIconUrl`]. */
export function getSkillIconUrl(_element?: string | null): string {
  return "";
}

/** Atlas non indexé, cf. [`getItemIconUrl`]. */
export function getSkillImageUrl(_skillId?: string | null): string {
  return "";
}

/** Les icônes d'aura sont indexées par un numéro de famille (`aura_fs/k000010_l.g4tx`) qu'aucune
 * table lue ici ne relie à un `asset_code` : non résolvable sans cet index. */
export function getAuraImageUrl(_assetCode?: string | null, _subType?: string | null): string {
  return "";
}

/** Compat de source avec le module du wiki : ici un chemin VFS est déjà « résolu ». */
export function resolveAssetUrl(path: string | null | undefined): string | null {
  return path ? path : null;
}

/** Repli des cartes — vide : l'application n'embarque pas les visuels du site. */
export const PLACEHOLDERS = {
  character: "",
  item: "",
  skill: "",
} as const;

/** `true` si `src` désigne un fichier du VFS que `ui/image.tsx` doit décoder lui-même. */
export function estCheminVfs(src: string): boolean {
  return src.startsWith("data/") && src.includes(".");
}
