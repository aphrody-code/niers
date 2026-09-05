/**
 * Recherche floue multilingue (`src/search`) — module PUR : il reçoit les
 * résultats bruts et les enrichit (langue détectée, priorisation, suggestion).
 *
 * Les jeux de données d'entrée sont construits à partir des VRAIS personnages du
 * miroir SQLite quand il est disponible, sinon à partir de noms canoniques du
 * jeu — dans les deux cas ce sont des noms réels, jamais des `foo/bar`.
 */

import { afterEach, beforeEach, describe, expect, test } from "bun:test";

import { resolveMirrorPath } from "@niers/azalee-outils/config";
import {
	clearSearchCache,
	detectMatchedLanguage,
	findClosestMatch,
	getCacheStats,
	getNameVariations,
	highlightMatches,
	isSimilar,
	levenshteinDistance,
	normalizeText,
	similarityScore,
	smartSearch,
	type SmartSearchResult,
} from "../src/search/index";
import { wikiService } from "../src/wiki/service";

const hasMirror = resolveMirrorPath() !== null;

describe("fuzzy-match — distance et normalisation", () => {
	test("levenshteinDistance : identité, insertion, substitution", () => {
		expect(levenshteinDistance("mark", "mark")).toBe(0);
		expect(levenshteinDistance("mark", "marc")).toBe(1);
		expect(levenshteinDistance("mark", "marks")).toBe(1);
		expect(levenshteinDistance("", "mark")).toBe(4);
		expect(levenshteinDistance("mark", "")).toBe(4);
		// Symétrique.
		expect(levenshteinDistance("gouenji", "goenji")).toBe(levenshteinDistance("goenji", "gouenji"));
	});

	test("similarityScore borné [0,1] et insensible à la casse", () => {
		expect(similarityScore("Mark", "mark")).toBe(1);
		expect(similarityScore("", "")).toBe(1);
		const score = similarityScore("Gouenji", "Goenji");
		expect(score).toBeGreaterThan(0.8);
		expect(score).toBeLessThan(1);
	});

	test("isSimilar applique le seuil demandé", () => {
		expect(isSimilar("Gouenji", "Goenji")).toBe(true);
		expect(isSimilar("Mark Evans", "Axel Blaze")).toBe(false);
		expect(isSimilar("Mark", "Marc", 0.99)).toBe(false);
	});

	test("normalizeText retire accents et casse", () => {
		expect(normalizeText("L'Étoffe des Héros")).toBe("l'etoffe des heros");
		expect(normalizeText("  Forêt  ")).toBe("foret");
	});

	test("findClosestMatch renvoie le meilleur candidat au-dessus du seuil", () => {
		const candidats = ["Mark Evans", "Axel Blaze", "Jude Sharp", "Nathan Swift"];
		const trouve = findClosestMatch("Mark Evan", candidats);
		expect(trouve?.match).toBe("Mark Evans");
		expect(trouve?.score).toBeGreaterThan(0.6);
		// Rien d'assez proche → null (pas de suggestion absurde).
		expect(findClosestMatch("zzzzzzzzzz", candidats)).toBeNull();
	});

	test("getNameVariations normalise et écarte les langues absentes", () => {
		expect(getNameVariations({ en: "Mark Evans", fr: "Mark Evans", ja: null })).toEqual([
			"mark evans",
			"mark evans",
		]);
		expect(getNameVariations({})).toEqual([]);
	});

	test("detectMatchedLanguage identifie la langue du match", () => {
		const names = { en: "Mark Evans", fr: "Mark Evans", ja: "円堂 守" };
		expect(detectMatchedLanguage("Mark", names)).toBe("FR");
		expect(detectMatchedLanguage("円堂", names)).toBe("JP");
		expect(detectMatchedLanguage("Evans", { en: "Mark Evans", fr: null, ja: null })).toBe("EN");
		expect(detectMatchedLanguage("zzzzzzzzz", names)).toBeNull();
	});

	test("highlightMatches découpe le texte autour du terme", () => {
		expect(highlightMatches("Mark Evans", "evans")).toEqual([
			{ highlight: false, text: "Mark " },
			{ highlight: true, text: "Evans" },
		]);
		// Terme absent ou vide → un seul segment non surligné.
		expect(highlightMatches("Mark Evans", "")).toEqual([{ highlight: false, text: "Mark Evans" }]);
		expect(highlightMatches("Mark Evans", "zzz")).toEqual([
			{ highlight: false, text: "Mark Evans" },
		]);
	});
});

describe("smart-search — priorisation, cache et suggestions", () => {
	beforeEach(() => clearSearchCache());
	afterEach(() => clearSearchCache());

	/** Résultats bruts, forme réellement produite par le wiki. */
	const bruts = [
		{
			data: { names: { en: "Inazuma Japan", fr: "Inazuma Japon", ja: "イナズマジャパン" } },
			id: "0xF01BB293",
			name: "Inazuma Japon",
			score: 0.7,
			type: "team" as const,
		},
		{
			data: { names: { en: "Mark Evans", fr: "Mark Evans", ja: "円堂 守" } },
			id: "0x99A1C150",
			name: "Mark Evans",
			score: 0.9,
			type: "character" as const,
		},
		{
			data: { name_EN: "Fire Tornado", name_FR: "Tornade de feu", name_JA: "ファイアトルネード" },
			id: "whs00010",
			name: "Tornade de feu",
			score: 0.8,
			type: "skill" as const,
		},
	];

	test("contexte 'chara' remonte les personnages en tête", async () => {
		const res = await smartSearch("Mark", bruts, { context: "chara" });
		expect(res[0]?.type).toBe("character");
	});

	test("contexte 'skill' remonte les techniques en tête", async () => {
		const res = await smartSearch("Mark", bruts, { context: "skill" });
		expect(res[0]?.type).toBe("skill");
	});

	test("la langue détectée et le nom correspondant sont annotés", async () => {
		const res = await smartSearch("円堂", bruts, { context: "global" });
		const perso = res.find((r) => r.type === "character") as SmartSearchResult;
		expect(perso.matchedLanguage).toBe("JP");
		expect(perso.matchedName).toBe("円堂 守");
	});

	test("minScore filtre les résultats faibles", async () => {
		const res = await smartSearch("Mark", bruts, { minScore: 0.85 });
		expect(res).toHaveLength(1);
		expect(res[0]?.id).toBe("0x99A1C150");
	});

	test("limit borne la sortie", async () => {
		expect(await smartSearch("Mark", bruts, { limit: 2 })).toHaveLength(2);
	});

	test("aucun résultat → suggestion « vouliez-vous dire »", async () => {
		expect(await smartSearch("Mark Evan", [], { enableSuggestions: true })).toHaveLength(0);
		// `minScore` inatteignable → tout est filtré, la branche suggestion s'active.
		const avecCandidats = await smartSearch("Mark Evan", bruts, { minScore: 2 });
		expect(avecCandidats[0]?.suggestion).toBe("Mark Evans");
	});

	test("la suggestion conserve casse et accents du nom réel", async () => {
		const accentues = [
			{
				data: { names: { en: "Legendary Heroes", fr: "L'Étoffe des Héros", ja: "英雄の証" } },
				id: "x",
				name: "L'Étoffe des Héros",
				score: 1,
				type: "team" as const,
			},
		];
		const res = await smartSearch("L'Étoffe des Hero", accentues, { minScore: 2 });
		// Régression : la suggestion sortait normalisée (« l'etoffe des heros »).
		expect(res[0]?.suggestion).toBe("L'Étoffe des Héros");
	});

	test("le cache distingue contexte, limite, minScore et suggestions", async () => {
		expect(getCacheStats().size).toBe(0);
		const premier = await smartSearch("Mark", bruts, { context: "chara", limit: 5 });
		expect(getCacheStats().size).toBe(1);
		const second = await smartSearch("Mark", bruts, { context: "chara", limit: 5 });
		expect(second).toEqual(premier);
		// Chaque option qui change la sortie doit produire une entrée distincte.
		await smartSearch("Mark", bruts, { context: "skill", limit: 5 });
		await smartSearch("Mark", bruts, { context: "chara", limit: 10 });
		await smartSearch("Mark", bruts, { context: "chara", limit: 5, minScore: 0.85 });
		await smartSearch("Mark", bruts, { context: "chara", enableSuggestions: false, limit: 5 });
		expect(getCacheStats().size).toBe(5);
		clearSearchCache();
		expect(getCacheStats().size).toBe(0);
	});

	test("un minScore différent ne récupère PAS l'entrée de cache voisine", async () => {
		// Régression : `minScore` était absent de la clé de cache → le 2e appel
		// renvoyait les résultats filtrés du 1er.
		const filtre = await smartSearch("Mark", bruts, { minScore: 2 });
		expect(filtre.every((r) => r.id === "")).toBe(true);
		const complet = await smartSearch("Mark", bruts, { minScore: 0 });
		expect(complet).toHaveLength(bruts.length);
	});
});

describe.skipIf(!hasMirror)("smart-search sur de VRAIS personnages du miroir", () => {
	beforeEach(() => clearSearchCache());

	test("les noms réels du wiki sont annotés dans la bonne langue", async () => {
		const liste = await wikiService.getCharactersList({ limit: 10, q: "Mark" });
		const bruts = (liste.data as Array<Record<string, any>>).map((chara) => ({
			data: { names: chara.names },
			id: String(chara.charaId),
			name: String(chara.names?.fr ?? ""),
			score: 1,
			type: "character" as const,
		}));
		expect(bruts.length).toBeGreaterThan(0);

		const res = await smartSearch("Mark", bruts, { context: "chara", limit: 50 });
		expect(res.length).toBe(Math.min(bruts.length, 50));
		for (const item of res) {
			expect(item.type).toBe("character");
			// Le terme vient des colonnes FR/EN → jamais détecté comme japonais.
			expect(item.matchedLanguage).not.toBe("JP");
		}
	});

	test("une faute de frappe sur un vrai nom produit la bonne suggestion", async () => {
		const liste = await wikiService.getCharactersList({ limit: 1 });
		const nomReel = String((liste.data[0] as Record<string, any>).names.fr);
		const bruts = [
			{
				data: { names: (liste.data[0] as Record<string, any>).names },
				id: "x",
				name: nomReel,
				score: 1,
				type: "character" as const,
			},
		];
		// `minScore` inatteignable → la branche suggestion est empruntée.
		const res = await smartSearch(`${nomReel.slice(0, -1)}z`, bruts, { minScore: 2 });
		expect(res[0]?.suggestion).toBe(nomReel);
	});
});
