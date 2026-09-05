/**
 * Types + helpers PURS de l'index de texte de jeu, client-safe (pas de `bun:sqlite`).
 *
 * Importable depuis un composant `"use client"`. La couche data serveur vit dans
 * `game-text/index.ts` (sous-chemin serveur de la lib).
 */

/** Locales réellement décodées dans le dump (les autres n'ont pas de `.json`). */
export type GameTextLocale = "fr" | "en" | "ja";

/** Ligne de l'index : un texte localisé pour un hashId + sa catégorie. */
export interface GameText {
	hashId: string;
	locale: GameTextLocale;
	category: string;
	value: string;
}

/** Texte résolu dans les 3 langues pour un hashId. */
export interface GameTextLocalized {
	hashId: string;
	category?: string;
	fr?: string;
	en?: string;
	ja?: string;
}

/** Normalise un hashId entrant (`0xXXXX` MAJUSCULE 8 chiffres) pour le lookup. */
export function normalizeHashId(raw: string | number): string {
	if (typeof raw === "number") {
		return `0x${(raw >>> 0).toString(16).toUpperCase().padStart(8, "0")}`;
	}
	const s = raw.trim();
	if (/^-?\d+$/.test(s)) {
		// Entier décimal (signé) → hex non signé.
		return `0x${(Number.parseInt(s, 10) >>> 0).toString(16).toUpperCase().padStart(8, "0")}`;
	}
	const hex = s.replace(/^0x/i, "").toUpperCase().padStart(8, "0");
	return `0x${hex}`;
}

/** Choisit la meilleure langue disponible (fr → en → ja). */
export function pickLocale(t: GameTextLocalized): string | undefined {
	return t.fr ?? t.en ?? t.ja;
}
