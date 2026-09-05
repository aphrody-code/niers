/**
 * Types + helpers PURS de la couche de résolution de texte de jeu côté wiki,
 * client-safe (aucun `bun:sqlite`, aucun accès disque).
 *
 * Importable depuis un composant `"use client"`. La couche data serveur (qui
 * ouvre l'index SQLite + lit le mapping code-modèle) vit dans
 * `wiki/game-text.ts` (sous-chemin serveur de la lib).
 *
 * On réexpose les types/normalisation de `lib/game-text/shared.ts` pour donner
 * aux pages wiki un seul point d'entrée « game-text » côté wiki, et on ajoute le
 * type du mapping code-modèle → nom (waza/item/animal) produit à l'étape index.
 */

import type { GameTextLocale } from "../game-text/shared";

export type { GameTextLocale, GameTextLocalized } from "../game-text/shared";
export { normalizeHashId, pickLocale } from "../game-text/shared";

/**
 * Forme de l'artefact `data/chr-model-names.json` : résolution code de modèle 3D
 * (`waza`/`item`/`animal` : `i*`, `kt*`, `an*`, `b*`, `d*`, `ev*`) → nom réel.
 *
 * - `names` : codes joignant une entrée `item_text`/`coa_animal` (nom décodé réel).
 * - `decorative` : codes 3D SANS entrée texte (props, parts, avatars de démo) —
 *   honnêtement non nommables ; on garde le code en libellé.
 */
export interface ChrModelNames {
	names: Record<string, string>;
	decorative: string[];
}

/** Une entrée de galerie de modèle 3D, nom résolu + code interne conservé. */
export interface ResolvedModelEntry {
	code: string;
	name: string;
	glbUrl: string;
	/** `true` si `name` est un vrai nom décodé ; `false` si repli = code. */
	resolved: boolean;
}

/**
 * Résout un code de modèle vers son nom réel via un mapping déjà chargé.
 * PUR (pas d'I/O) — la lecture du JSON se fait côté serveur dans `game-text.ts`.
 * Renvoie le nom décodé si connu, sinon `code` (repli honnête).
 */
export function resolveModelName(
	mapping: ChrModelNames,
	code: string | undefined | null,
): { name: string; resolved: boolean } {
	if (!code) return { name: "", resolved: false };
	const real = mapping.names[code];
	if (real) return { name: real, resolved: true };
	return { name: code, resolved: false };
}

/** Libellé court d'une locale (pour les sélecteurs UI éventuels). */
export const LOCALE_LABELS: Record<GameTextLocale, string> = {
	en: "English",
	fr: "Français",
	ja: "日本語",
};
