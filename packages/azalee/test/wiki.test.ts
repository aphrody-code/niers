/**
 * API wiki sur données RÉELLES (miroir SQLite embarqué).
 *
 * Aucun mock : `wikiService` et les modules de section lisent le miroir via le
 * provider par défaut. Les assertions portent sur des invariants structurels
 * (formes, cohérence des filtres, pagination) et des planchers de volume — pas
 * sur des chaînes exactes qui bougent à chaque resync du miroir.
 */

import { describe, expect, test } from "bun:test";

import { resolveMirrorPath } from "@niers/azalee-tools/config";
import { wikiService } from "../src/wiki/service";
import { getShop, getShopsList } from "../src/wiki/shops";
import { getTeamDetail, getTeamsList } from "../src/wiki/teams";

const hasMirror = resolveMirrorPath() !== null;
const suite = describe.skipIf(!hasMirror);

/** Accès souple aux entités du wiki (typage inagle non exhaustif côté test). */
type Loose = Record<string, any>;

suite("wikiService.getCharactersList", () => {
	test("liste paginée cohérente + forme BaseCharacter", async () => {
		const res = await wikiService.getCharactersList({ page: 1, limit: 5 });
		expect(res.page).toBe(1);
		expect(res.limit).toBe(5);
		expect(res.data.length).toBeLessThanOrEqual(5);
		expect(res.data.length).toBeGreaterThan(0);
		// Le miroir contient ~6100 lignes `inagle_characters`, ~5100 après filtres.
		expect(res.total).toBeGreaterThan(1000);

		for (const chara of res.data as Loose[]) {
			expect(chara.charaId).toBeString();
			expect(chara.internalCode).toBeString();
			expect(chara.names).toBeObject();
			expect(chara.baseSlug).toBeString();
			expect(chara.slug).toBeString();
			expect(Array.isArray(chara.variants)).toBe(true);
			expect(chara.variants.length).toBeGreaterThan(0);
		}
	});

	test("le tri par défaut démarre au #0 du zukan officiel", async () => {
		const res = await wikiService.getCharactersList({ page: 1, limit: 1 });
		const first = res.data[0] as Loose;
		// Ordre `zukan_order ASC NULLS LAST` : la 1re carte est le #0 du zukan.
		expect(first.internalCode).toBe("c01000010");
	});

	test("deux pages consécutives ne se recouvrent pas", async () => {
		const p1 = await wikiService.getCharactersList({ page: 1, limit: 10 });
		const p2 = await wikiService.getCharactersList({ page: 2, limit: 10 });
		expect(p2.page).toBe(2);
		const ids = new Set((p1.data as Loose[]).map((c) => c.charaId));
		expect((p2.data as Loose[]).every((c) => !ids.has(c.charaId))).toBe(true);
		expect(p1.total).toBe(p2.total);
	});

	test("une page hors range renvoie une liste vide sans erreur", async () => {
		const res = await wikiService.getCharactersList({ page: 99_999, limit: 5 });
		expect(res.data).toHaveLength(0);
		expect(res.total).toBeGreaterThan(0);
	});

	test("filtre q : chaque résultat matche le terme (fr ou en)", async () => {
		const res = await wikiService.getCharactersList({ q: "Mark", limit: 20 });
		expect(res.data.length).toBeGreaterThan(0);
		for (const chara of res.data as Loose[]) {
			const fr = String(chara.names?.fr ?? "").toLowerCase();
			const en = String(chara.names?.en ?? "").toLowerCase();
			expect(fr.includes("mark") || en.includes("mark")).toBe(true);
		}
	});

	test("filtre element : le code EN est traduit vers la colonne FR de la DB", async () => {
		const res = await wikiService.getCharactersList({ element: "Fire", limit: 10 });
		expect(res.data.length).toBeGreaterThan(0);
		for (const chara of res.data as Loose[]) {
			expect(chara.variants[0].element).toBe("Feu");
		}
	});

	test("filtre position : GK → Gardien", async () => {
		const res = await wikiService.getCharactersList({ limit: 10, position: "GK" });
		expect(res.data.length).toBeGreaterThan(0);
		for (const chara of res.data as Loose[]) {
			expect(chara.variants[0].position).toBe("Gardien");
		}
	});

	test("position COACH est redirigé vers la liste des coordinateurs", async () => {
		const coachs = await wikiService.getCharactersList({ limit: 5, position: "COACH" });
		const direct = await wikiService.getCoordinatorsList({ limit: 5 });
		expect(coachs.total).toBe(direct.total);
	});
});

suite("wikiService.getCharacterBySlug", () => {
	test("résout le slug renvoyé par la liste", async () => {
		const list = await wikiService.getCharactersList({ page: 1, limit: 3 });
		for (const item of list.data as Loose[]) {
			const found = (await wikiService.getCharacterBySlug(item.slug)) as Loose | undefined;
			expect(found).toBeDefined();
			expect(found?.charaId).toBe(item.charaId);
			expect(found?.names?.fr).toBe(item.names.fr);
		}
	});

	test("getCharacterByBaseSlug résout l'URL canonique (sans variante)", async () => {
		const list = await wikiService.getCharactersList({ page: 1, limit: 1 });
		const item = list.data[0] as Loose;
		// `getCharacterBySlug` ne matche QUE la colonne `slug` (slug de variante) :
		// la forme canonique passe par `getCharacterByBaseSlug`.
		expect(await wikiService.getCharacterBySlug(item.baseSlug)).toBeUndefined();
		const found = (await wikiService.getCharacterByBaseSlug(item.baseSlug)) as Loose | undefined;
		expect(found).toBeDefined();
		expect(found?.baseSlug).toBe(item.baseSlug);
		expect(found?.charaId).toBe(item.charaId);
	});

	test("slug inconnu → undefined (404 côté appelant, pas une ligne au hasard)", async () => {
		expect(await wikiService.getCharacterBySlug("slug-qui-nexiste-pas-xyz")).toBeUndefined();
	});
});

suite("wikiService.getSkillsList", () => {
	test("liste + forme Skill", async () => {
		const res = await wikiService.getSkillsList({ page: 1, limit: 10 });
		expect(res.data.length).toBeGreaterThan(0);
		// ~1000 techniques dans `inagle_skills`.
		expect(res.total).toBeGreaterThan(500);
		for (const skill of res.data as Loose[]) {
			expect(skill.skillId).toBeString();
			expect(skill.names).toBeObject();
			expect(typeof skill.displayName).toBe("string");
		}
	});

	test("filtre q sur le nom (fr/en)", async () => {
		const res = await wikiService.getSkillsList({ limit: 20, q: "tornade" });
		expect(res.data.length).toBeGreaterThan(0);
		for (const skill of res.data as Loose[]) {
			const fr = String(skill.names?.fr ?? "").toLowerCase();
			const en = String(skill.names?.en ?? "").toLowerCase();
			expect(fr.includes("tornade") || en.includes("tornado") || en.includes("tornade")).toBe(true);
		}
	});

	test("getSkill résout un id issu de la liste", async () => {
		const res = await wikiService.getSkillsList({ limit: 1 });
		const id = (res.data[0] as Loose).skillId as string;
		const skill = (await wikiService.getSkill(id)) as Loose | undefined;
		expect(skill).toBeDefined();
		expect(skill?.skillId ?? skill?.skillID).toBe(id);
	});

	test("getSkill sur un id inconnu → undefined", async () => {
		expect(await wikiService.getSkill("skill-inexistant-xyz")).toBeUndefined();
	});
});

suite("wikiService.getItemsList", () => {
	test("liste + forme Item", async () => {
		const res = await wikiService.getItemsList({ page: 1, limit: 10 });
		expect(res.data.length).toBe(10);
		// 1668 objets dans `inagle_items`.
		expect(res.total).toBeGreaterThan(1000);
		for (const item of res.data as Loose[]) {
			expect(item.itemId).toBeString();
			expect(item.names).toBeObject();
		}
	});

	test("filtre category homogène", async () => {
		const res = await wikiService.getItemsList({ category: "shoes", limit: 10 });
		expect(res.data.length).toBeGreaterThan(0);
		expect(res.total).toBeGreaterThan(0);
		for (const item of res.data as Loose[]) {
			expect(item.category).toBe("shoes");
		}
	});

	test("getItem résout un id issu de la liste, id inconnu → undefined", async () => {
		const res = await wikiService.getItemsList({ limit: 5 });
		const id = (res.data[4] as Loose).itemId as string;
		const item = (await wikiService.getItem(id)) as Loose | undefined;
		expect(item).toBeDefined();
		expect(item?.itemId).toBe(id);
		expect(await wikiService.getItem("0xFFFFFFFFF-inexistant")).toBeUndefined();
	});
});

suite("wiki/teams — section Équipes", () => {
	test("getTeamsList renvoie les 208 équipes du miroir", async () => {
		const teams = await getTeamsList();
		expect(teams.length).toBeGreaterThanOrEqual(200);
		for (const team of teams) {
			expect(team.id).toBeString();
			expect(team.name).toBeString();
			expect(team.name.length).toBeGreaterThan(0);
			expect(typeof team.rosterCount).toBe("number");
			expect(team.rosterCount).toBeGreaterThanOrEqual(0);
		}
		// Les ids sont uniques (1 ligne = 1 équipe).
		expect(new Set(teams.map((t) => t.id)).size).toBe(teams.length);
	});

	test("getTeamDetail hydrate l'effectif et les emblèmes", async () => {
		const teams = await getTeamsList();
		const raimon = teams.find((t) => t.name === "Raimon") ?? teams[0]!;
		const detail = await getTeamDetail(raimon.id);
		expect(detail).not.toBeNull();
		expect(detail?.id).toBe(raimon.id);
		expect(detail?.name).toBe(raimon.name);
		expect(Array.isArray(detail?.roster)).toBe(true);
		expect(detail?.roster.length).toBe(raimon.rosterCount);
		// Un effectif dédupliqué par `chara_id`.
		const charaIds = (detail?.roster ?? []).map((m) => m.charaId);
		expect(new Set(charaIds).size).toBe(charaIds.length);
	});

	test("getTeamDetail sur un id inconnu → null", async () => {
		expect(await getTeamDetail("0xDEADBEEF-inexistant")).toBeNull();
	});
});

suite("wiki/shops — section Boutiques", () => {
	test("getShopsList agrège les 2331 lignes en boutiques distinctes", async () => {
		const shops = await getShopsList();
		expect(shops.length).toBeGreaterThan(5);
		let totalItems = 0;
		for (const shop of shops) {
			expect(typeof shop.shopId).toBe("number");
			expect(shop.name).toBeString();
			expect(shop.itemCount).toBeGreaterThan(0);
			expect(Array.isArray(shop.categories)).toBe(true);
			// La répartition ne couvre que les lignes dont `item_db_id` résout vers
			// `inagle_items` (~1217/2331) : la somme est donc bornée par itemCount,
			// jamais au-delà (sinon = double comptage).
			const parCategorie = shop.categories.reduce((acc, c) => acc + c.count, 0);
			expect(parCategorie).toBeLessThanOrEqual(shop.itemCount);
			// Tri décroissant sur les catégories.
			const counts = shop.categories.map((c) => c.count);
			expect([...counts].sort((a, b) => b - a)).toEqual(counts);
			totalItems += shop.itemCount;
		}
		expect(totalItems).toBeGreaterThan(2000);
		expect(new Set(shops.map((s) => s.shopId)).size).toBe(shops.length);
		// La jointure vers `inagle_items` résout la catégorie d'une bonne part des
		// lignes : au moins une boutique doit être ventilée.
		expect(shops.some((s) => s.categories.length > 0)).toBe(true);
		// Tri décroissant du plus gros catalogue au plus petit.
		const tailles = shops.map((s) => s.itemCount);
		expect([...tailles].sort((a, b) => b - a)).toEqual(tailles);
	});

	test("getShop hydrate les objets vendus, shopId inconnu → null", async () => {
		const shops = await getShopsList();
		const first = shops[0]!;
		const detail = await getShop(first.shopId);
		expect(detail).not.toBeNull();
		expect(detail?.shopId).toBe(first.shopId);
		expect(detail?.items.length).toBe(first.itemCount);
		expect(await getShop(-1)).toBeNull();
	});
});
