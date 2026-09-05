/**
 * Couche d'accès données : `createSqliteClient` + `SqliteQueryBuilder` sur le
 * VRAI miroir SQLite des tables `inagle_*`, et point d'injection `provider.ts`.
 *
 * Aucun stub : toutes les assertions portent sur des lignes réellement présentes
 * dans `apps/azalee/data/backups/mirror.sqlite`. Les volumes sont asserés par
 * ordre de grandeur (`toBeGreaterThan`) — pas d'égalité fragile sur un snapshot
 * qui est resynchronisé quotidiennement (`nie-miroir.timer`).
 */

import { afterEach, describe, expect, test } from "bun:test";
import type { SupabaseClient } from "@supabase/supabase-js";

import { resolveMirrorPath } from "../src/config";
import { createSqliteClient } from "../src/db/sqlite-client";
import { createClient, hasDatabaseProvider, setDatabaseProvider } from "@rosegriffon/azalee/db";

const hasMirror = resolveMirrorPath() !== null;

/** Ligne générique : le miroir n'a pas de types générés côté test. */
type Row = Record<string, unknown>;

/**
 * Exécute la requête et remonte les lignes (échoue si `error`).
 *
 * `SqliteQueryBuilder` est « thenable » avec une signature générique propre :
 * on l'awaite derrière un cast plutôt que d'imposer un `PromiseLike` exact.
 */
async function rows(builder: unknown): Promise<Row[]> {
	const res = (await (builder as Promise<{ data: unknown; error: Error | null }>));
	expect(res.error).toBeNull();
	expect(Array.isArray(res.data)).toBe(true);
	return res.data as Row[];
}

describe.skipIf(!hasMirror)("SqliteQueryBuilder — lectures réelles du miroir", () => {
	const db = createSqliteClient();

	test("select + limit renvoie des lignes typées objet", async () => {
		const data = await rows(db.from("inagle_teams").select("id, name_fr").limit(3));
		expect(data).toHaveLength(3);
		for (const row of data) {
			// Projection respectée : uniquement les colonnes demandées.
			expect(Object.keys(row).sort()).toEqual(["id", "name_fr"]);
			expect(row.id).toBeString();
		}
	});

	test("count:'exact' compte la table entière, indépendamment de limit", async () => {
		const res = await db.from("inagle_teams").select("id", { count: "exact" }).limit(2);
		expect(res.error).toBeNull();
		expect((res.data as Row[]).length).toBe(2);
		// 208 équipes dans le miroir courant — on asserte un plancher stable.
		expect(res.count).toBeGreaterThanOrEqual(200);
	});

	test("eq filtre sur une valeur exacte", async () => {
		const data = await rows(
			db.from("inagle_characters").select("internal_code, name_fr").eq("internal_code", "c01000010"),
		);
		expect(data.length).toBeGreaterThan(0);
		for (const row of data) {
			expect(row.internal_code).toBe("c01000010");
		}
		// Mark Evans = perso #0 du zukan, présent dans tous les snapshots.
		expect(data.some((r) => typeof r.name_fr === "string" && r.name_fr.length > 0)).toBe(true);
	});

	test("eq(col, null) émet IS NULL (et non `= NULL`)", async () => {
		const res = await db
			.from("inagle_characters")
			.select("internal_code", { count: "exact" })
			.eq("zukan_order", null)
			.limit(1);
		expect(res.error).toBeNull();
		expect(res.count).toBeGreaterThan(0);
	});

	test("ilike applique un LIKE (insensible à la casse ASCII)", async () => {
		const data = await rows(db.from("inagle_skills").select("name_fr").ilike("name_fr", "%feu%").limit(10));
		expect(data.length).toBeGreaterThan(0);
		for (const row of data) {
			expect(String(row.name_fr).toLowerCase()).toContain("feu");
		}
	});

	test("or(...) combine plusieurs colonnes en OR", async () => {
		const data = await rows(
			db
				.from("inagle_skills")
				.select("id, name_fr, name_en")
				.or("name_fr.ilike.%tornade%,name_en.ilike.%tornado%")
				.limit(20),
		);
		expect(data.length).toBeGreaterThan(0);
		for (const row of data) {
			const fr = String(row.name_fr ?? "").toLowerCase();
			const en = String(row.name_en ?? "").toLowerCase();
			expect(fr.includes("tornade") || en.includes("tornado")).toBe(true);
		}
	});

	test("or(...) préserve les valeurs multi-mots (pas de troncature à l'espace)", async () => {
		// Vérité terrain : on récupère un nom contenant un espace puis on le
		// recherche via `.or(col.eq.<valeur avec espaces>)`.
		const [sample] = await rows(
			db.from("inagle_skills").select("name_fr").ilike("name_fr", "% %").limit(1),
		);
		const name = String(sample?.name_fr);
		expect(name).toContain(" ");
		const data = await rows(db.from("inagle_skills").select("name_fr").or(`name_fr.eq.${name}`).limit(5));
		expect(data.length).toBeGreaterThan(0);
		expect(data.every((r) => r.name_fr === name)).toBe(true);
	});

	test("or(...) malformé matche RIEN (jamais toute la table)", async () => {
		const data = await rows(db.from("inagle_teams").select("id").or("").limit(50));
		expect(data).toHaveLength(0);
	});

	test("in([]) vide matche RIEN, in([...]) filtre", async () => {
		expect(await rows(db.from("inagle_items").select("id").in("category", []).limit(5))).toHaveLength(0);
		const data = await rows(db.from("inagle_items").select("id, category").in("category", ["shoes"]).limit(5));
		expect(data.length).toBeGreaterThan(0);
		expect(data.every((r) => r.category === "shoes")).toBe(true);
	});

	test("not(col,'like',...) exclut le motif", async () => {
		const data = await rows(
			db.from("inagle_characters").select("internal_code").not("internal_code", "like", "c%").limit(20),
		);
		expect(data.length).toBeGreaterThan(0);
		expect(data.every((r) => !String(r.internal_code).startsWith("c"))).toBe(true);
	});

	test("range(from,to) pagine (LIMIT/OFFSET) sans altérer le count", async () => {
		const ordered = db.from("inagle_items").select("id").order("id", { ascending: true });
		const page1 = await rows(ordered.range(0, 4));
		const other = await db
			.from("inagle_items")
			.select("id", { count: "exact" })
			.order("id", { ascending: true })
			.range(5, 9);
		expect(page1).toHaveLength(5);
		expect((other.data as Row[]).length).toBe(5);
		// Pages disjointes.
		const ids = new Set(page1.map((r) => r.id));
		expect((other.data as Row[]).some((r) => ids.has(r.id))).toBe(false);
		expect(other.count).toBeGreaterThan(1000);
	});

	test("order ascending/descending inverse bien le tri", async () => {
		const asc = await rows(db.from("inagle_items").select("id").order("id", { ascending: true }).limit(5));
		const desc = await rows(db.from("inagle_items").select("id").order("id", { ascending: false }).limit(5));
		const ascIds = asc.map((r) => String(r.id));
		expect([...ascIds].sort()).toEqual(ascIds);
		expect(String(desc[0]?.id) > String(asc[0]?.id)).toBe(true);
	});

	test("order(nullsFirst:false) renvoie les NULL en dernier (sémantique PostgREST)", async () => {
		const data = await rows(
			db
				.from("inagle_characters")
				.select("zukan_order")
				.order("zukan_order", { ascending: true, nullsFirst: false })
				.limit(20),
		);
		// SQLite trie les NULL en tête par défaut : sans NULLS LAST, la 1re ligne
		// serait un NULL et l'ordre du zukan serait cassé.
		expect(data[0]?.zukan_order).not.toBeNull();
		expect(data[0]?.zukan_order).toBe(0);
	});

	test("maybeSingle renvoie la ligne ou null (jamais d'exception)", async () => {
		const found = await db.from("inagle_teams").select("*").eq("name_fr", "Raimon").maybeSingle();
		expect(found.error).toBeNull();
		expect((found.data as Row | null)?.name_fr).toBe("Raimon");

		const missing = await db.from("inagle_teams").select("*").eq("id", "id-inexistant").maybeSingle();
		expect(missing.error).toBeNull();
		expect(missing.data).toBeNull();
	});

	test("single sur 0 ligne remonte une erreur (pas une ligne au hasard)", async () => {
		const res = await db.from("inagle_teams").select("*").eq("id", "id-inexistant").single();
		expect(res.data).toBeNull();
		expect(res.error).toBeInstanceOf(Error);
	});

	test("les colonnes JSON sont désérialisées automatiquement", async () => {
		const res = await db.from("inagle_teams").select("*").eq("name_fr", "Raimon").maybeSingle();
		const team = res.data as Row;
		// `emblems`/`kits` sont stockés en TEXT JSON dans le miroir.
		expect(typeof team.emblems).toBe("object");
		expect(team.emblems).not.toBeNull();
	});

	test("une table inexistante remonte une erreur au lieu de planter", async () => {
		const res = await db.from("table_qui_nexiste_pas").select("*").limit(1);
		expect(res.data).toBeNull();
		expect(res.error).toBeInstanceOf(Error);
	});
});

describe("provider — injection et retour au défaut", () => {
	afterEach(() => {
		// Le provider est un singleton de module : toute fuite corromprait les
		// tests wiki/serve qui suivent dans le même process.
		setDatabaseProvider(null);
	});

	test("aucun provider injecté par défaut", () => {
		expect(hasDatabaseProvider()).toBe(false);
	});

	test("setDatabaseProvider bascule createClient sur la fabrique injectée", async () => {
		let appels = 0;
		const sentinelle = { marqueur: "client-injecte" };
		setDatabaseProvider(() => {
			appels++;
			return sentinelle as unknown as SupabaseClient;
		});
		expect(hasDatabaseProvider()).toBe(true);
		expect(await createClient()).toBe(sentinelle as unknown as SupabaseClient);
		expect(appels).toBe(1);
	});

	test("une fabrique asynchrone est attendue (await)", async () => {
		const sentinelle = { marqueur: "async" };
		setDatabaseProvider(async () => sentinelle as unknown as SupabaseClient);
		expect(await createClient()).toBe(sentinelle as unknown as SupabaseClient);
	});

	test("setDatabaseProvider(null) rend la main a la source de secours", async () => {
		setDatabaseProvider(() => ({}) as unknown as SupabaseClient);
		setDatabaseProvider(null);
		expect(hasDatabaseProvider()).toBe(false);
		// `null` retire MA fabrique, il ne supprime pas toute source : le defaut pose par
		// l'outillage (le miroir) reprend la main. Sans ce second niveau, cet appel levait et
		// condamnait au passage tous les fichiers de test suivants.
		const client = await createClient();
		expect(typeof client.from).toBe("function");
	});

	test.skipIf(!hasMirror)("le client injecte lit reellement le miroir", async () => {
		const client = await createClient();
		const res = await (
			client.from("inagle_teams") as unknown as PromiseLike<{ data: unknown; error: Error | null }> & {
				select: (f: string) => { limit: (n: number) => PromiseLike<{ data: unknown; error: Error | null }> };
			}
		)
			.select("id")
			.limit(1);
		expect(res.error).toBeNull();
		expect((res.data as Row[]).length).toBe(1);
	});
});
