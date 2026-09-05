/**
 * Détection et nettoyage de texte japonais brut issu du jeu.
 *
 * Helpers purs (aucun I/O, aucune dépendance) partagés par le glossaire du
 * wiki et par le CLI (`translate`, `glossary-rebuild`).
 */

/**
 * Kana (U+3040-30FF), kanji étendus (U+3400-4DBF), kanji usuels (U+4E00-9FFF),
 * formes de compatibilité (U+F900-FAFF) et katakana demi-largeur (U+FF66-FF9F).
 */
const JAPANESE_RANGES = /[\u3040-\u30ff\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\uff66-\uff9f]/;

/** Vrai si le texte contient au moins un caractère japonais. */
export function containsJapanese(text: string): boolean {
	return JAPANESE_RANGES.test(text);
}

/**
 * Retire les annotations furigana `[base/lecture]` en ne gardant que la base,
 * puis supprime les espaces de bord. Les textes du jeu utilisent cette forme
 * pour la ruby ; le glossaire ne doit indexer que la base.
 */
export function stripRubyAnnotations(text: string): string {
	return text.replace(/\[([^/\]]+)\/[^\]]+\]/g, "$1").trim();
}

/** Échappe les métacaractères d'une expression régulière. */
export function escapeRegExp(str: string): string {
	return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
