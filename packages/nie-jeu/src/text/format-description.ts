/**
 * Description tag resolver and formatter for IEVR game descriptions
 *
 * Resolves name tags like <FST:TENMA>, <FLC:GOENJI>, <MNT:NAGUMOHARA>
 * and removes furigana notation like [漢字/かんじ]
 */

/**
 * Character name mappings for description tags
 * Based on official game data from players.json
 *
 * Tag types:
 * - FST: First name (Japanese romanized) → Use firstJp/firstEn
 * - FLC: Family localized → Use familyEn (English version)
 * - FLA: Family Japanese → Use familyJp (Japanese romanized)
 * - FUL: Full name → Use nameEn/nameJp
 * - MNT: Place/Team name → Custom mapping
 */
const CHARACTER_NAME_TAGS: Record<string, { en?: string; fr?: string; jp?: string }> = {
	// Main protagonists - Inazuma Eleven GO series
	TENMA: { en: "Arion", fr: "Arion", jp: "Tenma" },
	TSURUGI: { en: "Victor", fr: "Victor", jp: "Kyosuke" },
	SHINDOU: { en: "Riccardo", fr: "Riccardo", jp: "Takuto" },
	KIRINO: { en: "Jean-Pierre", fr: "Jean-Pierre", jp: "Ranmaru" },

	// Main protagonists - Original series
	ENDO: { en: "Evans", fr: "Evans", jp: "Endo" },
	GOENJI: { en: "Axel", fr: "Axel", jp: "Shuya" },
	KIDOU: { en: "Jude", fr: "Jude", jp: "Yuuto" },
	KAZEMARU: { en: "Nathan", fr: "Nathan", jp: "Ichirouta" },
	FUBUKI: { en: "Fubuki", fr: "Shawn", jp: "Shirou" },
	GOUENJI: { en: "Axel", fr: "Axel", jp: "Shuya" }, // Alternate spelling

	// Full names for FUL tags
	SYU: { en: "Shuu", fr: "Shuu", jp: "Shuu" },
	ARTHUR: { en: "King Arthur", fr: "Roi Arthur", jp: "Arthur" },
	BETA: { en: "Beta", fr: "Beta", jp: "Beta" },
	RYUBI: { en: "Liu Bei", fr: "Liu Bei", jp: "Ryubi" },
	HAKURYU: { en: "Hakuryuu", fr: "Hakuryuu", jp: "Hakuryuu" },
	OZROCK: { en: "Ozrock", fr: "Ozrock", jp: "Ozrock" },

	// Chrono Stone characters
	FEYZA: { en: "Fey", fr: "Fey", jp: "Fei" },
	KINAKO: { en: "Goldie", fr: "Goldie", jp: "Kinako" },

	// Galaxy characters
	TETSUKADO: { en: "Iron", fr: "Iron", jp: "Tetsukado" },
	MATATAGI: { en: "Matatagi", fr: "Matatagi", jp: "Hayato" },
	IBUKI: { en: "Ibuki", fr: "Ibuki", jp: "Munemasa" },
	MINAHO: { en: "Manabe", fr: "Manabe", jp: "Minaho" },
	KONOHA: { en: "Konoha", fr: "Konoha", jp: "Konoha" },

	// Ares/Orion
	ASUTO: { en: "Asuto", fr: "Asuto", jp: "Inamori" },
	NOSAKA: { en: "Nosaka", fr: "Nosaka", jp: "Yuuma" },
	HAIZAKI: { en: "Haizaki", fr: "Haizaki", jp: "Ryouhei" },
	KIRA: { en: "Kira", fr: "Kira", jp: "Hiroto" },
};

/**
 * Place/Team name mappings for MNT tags
 */
const PLACE_NAME_TAGS: Record<string, { en?: string; fr?: string; jp?: string }> = {
	ALIEA: { en: "Aliea Academy", fr: "Académie Aliea", jp: "エイリア学園" },
	INAZUMAJAPAN: {
		en: "Inazuma Japan",
		fr: "Inazuma Japon",
		jp: "イナズマジャパン",
	},
	LITTLEGIGANTS: {
		en: "Little Gigant",
		fr: "Little Gigant",
		jp: "リトルギガント",
	},
	NAGUMOHARA: { en: "Nagumohara", fr: "Nagumohara", jp: "凪村原" },
	ORPHEUS: { en: "Orpheus", fr: "Orpheus", jp: "オルフェウス" },
	RAIMON: { en: "Raimon", fr: "Raimon", jp: "雷門" },
	SHIROSHIKA: { en: "White Deer", fr: "Cerf Blanc", jp: "白鹿" },
	TEAMK: { en: "Team K", fr: "Team K", jp: "チームK" },
	TEIKOKU: { en: "Royal Academy", fr: "Académie Royale", jp: "帝国" },
	THEKINGDOM: { en: "The Kingdom", fr: "The Kingdom", jp: "ザ・キングダム" },
	UNICORN: { en: "Unicorn", fr: "Unicorn", jp: "ユニコーン" },
	ZEUS: { en: "Zeus", fr: "Zeus", jp: "ゼウス" },
};

/**
 * Resolve a single tag to its text value
 */
function resolveTag(
	_tagType: string,
	tagName: string,
	lang: "en" | "fr" | "jp" | "ja" = "fr"
): string {
	const upperName = tagName.toUpperCase();
	const normalizedLang = lang === "ja" ? "jp" : lang;

	// Check character names first
	const charMapping = CHARACTER_NAME_TAGS[upperName];
	if (charMapping) {
		return charMapping[normalizedLang] || charMapping.en || tagName;
	}

	// Check place names
	const placeMapping = PLACE_NAME_TAGS[upperName];
	if (placeMapping) {
		return placeMapping[normalizedLang] || placeMapping.en || tagName;
	}

	// For unknown tags, return the name part only (without the tag prefix)
	// This makes "Nagumohara" instead of "<MNT:NAGUMOHARA>"
	return tagName.charAt(0) + tagName.slice(1).toLowerCase();
}

/**
 * Format a description by:
 * 1. Resolving name tags (<FST:...>, <FLC:...>, <FLA:...>, <FUL:...>, <MNT:...>)
 * 2. Removing furigana notation [kanji/reading] → keeping just kanji
 * 3. Preserving line breaks
 */
export function formatDescription(
	description: string | undefined | null,
	lang: "en" | "fr" | "jp" | "ja" = "fr"
): string {
	if (!description) {
		return "";
	}

	let result = description;

	// Step 1: Resolve all name tags
	// Pattern: <TYPE:NAME> where TYPE is FST/FLC/FLA/FUL/MNT
	const tagPattern = /<(FST|FLC|FLA|FUL|MNT):([A-Z0-9_]+)>/gi;
	result = result.replace(tagPattern, (_, tagType, tagName) => resolveTag(tagType, tagName, lang));

	// Step 2: Remove furigana notation [kanji/reading] → keep kanji only
	// Pattern: [漢字/ふりがな] → 漢字
	const furiganaPattern = /\[([^\]/]+)\/[^\]]+\]/g;
	result = result.replace(furiganaPattern, "$1");

	return result;
}

/**
 * Format Japanese name by removing furigana notation
 * [漢字/ふりがな] → 漢字
 */
export function formatJapaneseName(name: string | undefined | null): string {
	if (!name) {
		return "";
	}

	// Remove furigana notation [kanji/reading] → keep kanji only
	const furiganaPattern = /\[([^\]/]+)\/[^\]]+\]/g;
	return name.replace(furiganaPattern, "$1");
}

/**
 * Convert Japanese name (katakana/hiragana) to romaji.
 * Splits on middle dots (・) and capitalizes each part.
 * Returns null if input is empty or has no kana.
 */
export { japaneseToRomaji } from "./japanese-romaji";

/**
 * Check if a description contains unresolved tags
 * Useful for debugging and identifying missing mappings
 */
export function hasUnresolvedTags(description: string): boolean {
	const tagPattern = /<(FST|FLC|FLA|FUL|MNT):([A-Z0-9_]+)>/gi;
	return tagPattern.test(description);
}
