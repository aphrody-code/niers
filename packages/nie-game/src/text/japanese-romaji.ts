import { isKana, toRomaji } from "wanakana";

/**
 * Convert Japanese name (katakana/hiragana) to romaji.
 * Splits on middle dots (・) and spaces, capitalizes each part.
 * Returns null if input is empty or contains no kana.
 */
export function japaneseToRomaji(name: string | undefined | null): string | null {
	if (!name) {
		return null;
	}

	const hasKana = [...name].some((ch) => isKana(ch));
	if (!hasKana) {
		return null;
	}

	const parts = name.split(/[・\s]+/).filter(Boolean);
	const romaji = parts
		.map((p) => {
			const r = toRomaji(p);
			return r.charAt(0).toUpperCase() + r.slice(1);
		})
		.join(" ");

	return romaji || null;
}
