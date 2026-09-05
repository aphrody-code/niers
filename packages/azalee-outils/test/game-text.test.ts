/**
 * Index de texte de jeu IEVR (fr/en/ja) — résolution par hashId, recherche,
 * catégories.
 *
 * Source réelle : `game-text-names.ndjson.gz` + `game-text-dialogue.ndjson.gz`,
 * matérialisés en SQLite de cache au premier accès.
 */

import { describe, expect, test } from "bun:test";

import {
	categoryStats,
	getText,
	getTextLocale,
	getTexts,
	listCategory,
	normalizeHashId,
	pickLocale,
	searchText,
} from "../src/game-text/index";
import { resolveDataFile } from "../src/config";

const hasIndex =
	resolveDataFile("game-text-names.ndjson.gz") !== null ||
	resolveDataFile("game-text-dialogue.ndjson.gz") !== null ||
	Boolean(process.env.GAME_TEXT_DATA_DIR);
const suite = describe.skipIf(!hasIndex);

suite("categoryStats — répartition par catégorie (locale fr)", () => {
	test("plusieurs dizaines de catégories, comptes décroissants", () => {
		const stats = categoryStats();
		expect(stats.length).toBeGreaterThan(10);
		for (const stat of stats) {
			expect(stat.category).toBeString();
			expect(stat.count).toBeGreaterThan(0);
		}
		const counts = stats.map((s) => s.count);
		expect([...counts].sort((a, b) => b - a)).toEqual(counts);
		// L'index couvre l'intégralité du texte de jeu décodé (~250k lignes,
		// toutes langues confondues) → largement plus de 10 000 en fr.
		expect(counts.reduce((a, b) => a + b, 0)).toBeGreaterThan(10_000);
		// Catégories sans doublon.
		expect(new Set(stats.map((s) => s.category)).size).toBe(stats.length);
	});
});

suite("searchText — recherche plein-texte naïve", () => {
	test("chaque ligne contient le terme et respecte la locale demandée", () => {
		const res = searchText("Mark", "fr", 20);
		expect(res.length).toBeGreaterThan(0);
		expect(res.length).toBeLessThanOrEqual(20);
		for (const ligne of res) {
			expect(ligne.locale).toBe("fr");
			// LIKE SQLite = insensible à la casse ASCII (« BISMARK » matche « Mark »).
			expect(ligne.value.toLowerCase()).toContain("mark");
			expect(ligne.hashId).toMatch(/^0x[0-9A-F]{8}$/);
			expect(ligne.category).toBeString();
		}
	});

	test("la locale sélectionne bien un autre corpus", () => {
		const ja = searchText("円堂", "ja", 5);
		expect(ja.length).toBeGreaterThan(0);
		for (const ligne of ja) {
			expect(ligne.locale).toBe("ja");
			expect(ligne.value).toContain("円堂");
		}
	});

	test("un terme absent renvoie une liste vide", () => {
		expect(searchText("zzz-terme-absent-du-jeu-zzz", "fr", 10)).toHaveLength(0);
	});

	test("la limite est respectée", () => {
		expect(searchText("a", "fr", 3)).toHaveLength(3);
	});
});

suite("getText / getTextLocale — résolution d'un hashId", () => {
	/** Un hashId réel, découvert par recherche (pas de constante figée). */
	const echantillon = searchText("Mark", "fr", 1)[0];

	test("résout les 3 langues décodées + la catégorie", () => {
		expect(echantillon).toBeDefined();
		const texte = getText(echantillon!.hashId);
		expect(texte).toBeDefined();
		expect(texte?.hashId).toBe(echantillon!.hashId);
		expect(texte?.category).toBe(echantillon!.category);
		expect(texte?.fr).toBe(echantillon!.value);
		// Au moins une autre langue existe pour une entrée de nom.
		expect(Boolean(texte?.en || texte?.ja)).toBe(true);
		expect(pickLocale(texte!)).toBe(echantillon!.value);
	});

	test("accepte les formes hex/décimale/majuscule du même hashId", () => {
		const key = echantillon!.hashId;
		const decimal = Number.parseInt(key.slice(2), 16);
		expect(getText(key.toLowerCase())?.hashId).toBe(key);
		expect(getText(key.slice(2))?.hashId).toBe(key);
		expect(getText(decimal)?.hashId).toBe(key);
	});

	test("getTextLocale applique le repli fr → en → ja", () => {
		const key = echantillon!.hashId;
		const texte = getText(key)!;
		expect(getTextLocale(key, "fr")).toBe(texte.fr ?? texte.en ?? texte.ja!);
		expect(getTextLocale(key, "ja")).toBe(texte.ja ?? texte.fr ?? texte.en!);
		// Sans locale explicite → fr.
		expect(getTextLocale(key)).toBe(getTextLocale(key, "fr"));
	});

	test("un hashId inconnu → undefined (jamais une entrée voisine)", () => {
		expect(getText("0xFFFFFFFF")).toBeUndefined();
		expect(getTextLocale("0xFFFFFFFF")).toBeUndefined();
	});

	test("getTexts résout un lot en une passe", () => {
		const lot = searchText("Mark", "fr", 5).map((l) => l.hashId);
		const map = getTexts([...lot, "0xFFFFFFFF", lot[0]!]);
		expect(map.size).toBe(new Set(lot).size);
		for (const key of lot) {
			expect(map.get(key)?.fr).toBeString();
		}
		expect(map.has("0xFFFFFFFF")).toBe(false);
	});
});

suite("listCategory — pagination par catégorie", () => {
	test("pages disjointes et catégorie/locale homogènes", () => {
		const categorie = categoryStats()[0]!.category;
		const p1 = listCategory(categorie, "fr", 10, 0);
		const p2 = listCategory(categorie, "fr", 10, 10);
		expect(p1).toHaveLength(10);
		expect(p2).toHaveLength(10);
		for (const ligne of [...p1, ...p2]) {
			expect(ligne.category).toBe(categorie);
			expect(ligne.locale).toBe("fr");
		}
		const vus = new Set(p1.map((l) => l.hashId));
		expect(p2.some((l) => vus.has(l.hashId))).toBe(false);
		// Tri par hashId croissant.
		const ids = p1.map((l) => l.hashId);
		expect([...ids].sort()).toEqual(ids);
	});

	test("une catégorie inconnue renvoie une liste vide", () => {
		expect(listCategory("categorie-inexistante", "fr", 10, 0)).toHaveLength(0);
	});
});

describe("game-text/shared — normalizeHashId (pur)", () => {
	test("normalise vers `0x` + 8 hex MAJUSCULES", () => {
		expect(normalizeHashId(0x99a1c150)).toBe("0x99A1C150");
		expect(normalizeHashId("0x99a1c150")).toBe("0x99A1C150");
		expect(normalizeHashId("99A1C150")).toBe("0x99A1C150");
		expect(normalizeHashId("  0X99a1c150  ")).toBe("0x99A1C150");
	});

	test("les entiers décimaux (signés) deviennent des hex non signés", () => {
		expect(normalizeHashId(0)).toBe("0x00000000");
		expect(normalizeHashId("255")).toBe("0x000000FF");
		expect(normalizeHashId(-1)).toBe("0xFFFFFFFF");
		expect(normalizeHashId("-1")).toBe("0xFFFFFFFF");
	});

	test("les valeurs courtes sont zéro-paddées", () => {
		expect(normalizeHashId("0xFF")).toBe("0x000000FF");
	});
});
