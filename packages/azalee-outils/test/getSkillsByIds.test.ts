/**
 * Non-régression de `wikiService.getSkillsByIds` — le batch qui remplace la
 * boucle `skillList.map((s) => wikiService.getSkill(s.skillId))` de
 * `chara/[id]/page.tsx` (le N+1 mesuré à 599 requêtes `inagle_skills` pour
 * une seule fiche, en repli Postgres — cause du timeout de build Vercel).
 *
 * Le risque n'est PAS la performance, c'est la sémantique : `getSkill` a deux
 * chemins de résolution (hex `0x…` vs code interne/nom) avec un
 * post-traitement conséquent (fusion `data`+`sheet_data`, `realSkillName`,
 * `elementMap`/`categoryMap`). Un batch qui ne les préserve pas EXACTEMENT
 * rendrait une fiche personnage fausse en silence — aucun test existant ne
 * l'aurait attrapé.
 *
 * La preuve retenue : comparer, sur des ids RÉELS (les 3 boucles de
 * `chara/[id]/page.tsx`, pour un échantillon de fiches incluant les 5 qui
 * échouaient en prod), le résultat du batch à celui de `getSkill` appelé un
 * par un — id par id, `toEqual` strict. Rejoué sur les DEUX moteurs qui
 * servent réellement `apps/azalee` : le miroir SQLite (chemin par défaut) et
 * Postgres via `rg-postgrest` en local (le chemin qu'emprunte le repli
 * quand `SQLITE_DB_PATH` est introuvable, exactement comme sur Vercel).
 */

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { createClient as createSupabaseClient, type SupabaseClient } from "@supabase/supabase-js";

import { resolveMirrorPath } from "@rosegriffon/azalee/config";
import { setDatabaseProvider } from "@rosegriffon/azalee/db/provider";
import { createSqliteClient } from "@rosegriffon/azalee/db/sqlite-client";
import { wikiService } from "@rosegriffon/azalee/wiki/service";

/** 5 fiches qui échouaient réellement au build (`Failed to build … after 3
 * attempts`, log `/tmp/gate-vercel.log`, 2026-09-05) + 20 fiches choisies au
 * hasard parmi celles qui portent des techniques, pour varier rareté/poste/
 * élément. */
const SLUGS_FICHE = [
	"astro-lor",
	"fei",
	"kevin",
	"maxwell-carson",
	"nathan-swift",
	"shilo-placidus",
	"dougal-axe",
	"beta",
	"rocky-mccanns",
	"carlos-lagarto",
	"pip-skinner",
	"erik-pentona",
	"silke-coombe",
	"fae-grimm",
	"ruth-karnes",
	"soji-okita",
	"bourne-birch",
	"cinnamon-dougherty",
	"gideon-poe",
	"dariusius",
	"dante-diavolo",
	"nazim-nizar",
	"janet-hollilocks",
	"janus",
	"randolf-finn",
] as const;

/** Même filtre que `chara/[id]/page.tsx` (slot spécial dépendant d'une aura). */
const PHANTOM_IDS = new Set(["0xDBEDB6B8"]);

type Loose = Record<string, any>;

/**
 * Reproduit EXACTEMENT les deux jeux d'ids que construit `chara/[id]/page.tsx` :
 * - `variantSkillIds` : l'union des `skillId` de toutes les variantes (pour la
 *   comparaison de variantes) ;
 * - `skillListsByVariant` : le moveset binaire de secours, par variante (le
 *   chemin réellement emprunté sur 100 % des fiches — `sheet_data.moveset`
 *   n'est peuplé sur aucune ligne du miroir, vérifié 2026-09-05).
 */
async function idsPourFiche(
	baseSlug: string
): Promise<{ variantSkillIds: string[]; skillListsByVariant: string[][] }> {
	const baseChar = (await wikiService.getCharacterByBaseSlug(baseSlug)) as Loose | undefined;
	if (!baseChar) {
		return { variantSkillIds: [], skillListsByVariant: [] };
	}

	const variantSkillIds = new Set<string>();
	const skillListsByVariant: string[][] = [];

	for (const v of baseChar.variants as Loose[]) {
		for (const sk of v.skills || []) {
			if (sk.skillId) variantSkillIds.add(sk.skillId);
		}

		const rawSkills: Array<{ learnLevel: number; skillId: string }> | string[] =
			v.skills || v.moves || [];
		const skillList = rawSkills
			.map((s) => (typeof s === "string" ? { skillId: s, learnLevel: 0 } : s))
			.filter((s) => !PHANTOM_IDS.has(s.skillId))
			.map((s) => s.skillId);
		skillListsByVariant.push(skillList);
	}

	return { variantSkillIds: Array.from(variantSkillIds), skillListsByVariant };
}

/**
 * Le cœur de la preuve : pour chaque id, `getSkillsByIds([...ids]).get(id)`
 * doit rendre EXACTEMENT ce que `getSkill(id)` rend seul — même valeur
 * `undefined` incluse.
 */
async function compareUnitaireVsBatch(ids: string[]): Promise<void> {
	if (ids.length === 0) return;
	const unitaires = await Promise.all(ids.map((id) => wikiService.getSkill(id)));
	const parLot = await wikiService.getSkillsByIds(ids);
	ids.forEach((id, index) => {
		expect(parLot.get(id)).toEqual(unitaires[index]);
	});
}

/** Fabrique de client qui compte ses appels `.from("inagle_skills")` — sert à
 * chiffrer le gain (nombre de requêtes) sans dépendre d'une métrique du
 * driver SQL, qui diffère entre SQLite et PostgREST. */
function fabriqueComptee(
	base: () => SupabaseClient | Promise<SupabaseClient>,
	compteur: { n: number }
): () => Promise<SupabaseClient> {
	return async () => {
		const client = await base();
		return new Proxy(client, {
			get(target, prop, receiver) {
				if (prop === "from") {
					return (table: string) => {
						if (table === "inagle_skills") compteur.n++;
						return (target as Loose).from(table);
					};
				}
				return Reflect.get(target, prop, receiver);
			},
		}) as SupabaseClient;
	};
}

const hasMirror = resolveMirrorPath() !== null;

// Le repli Postgres qui échoue réellement sur Vercel : `rg-postgrest` derrière
// nginx en local (`SUPABASE_INTERNAL_URL`), avec la clé anon PUBLIQUE
// (`NEXT_PUBLIC_...`, déjà exposée au navigateur — pas un secret). Absent →
// on saute en l'annonçant, jamais en silence (cf. doctrine golden tests).
const PG_URL = process.env.SUPABASE_INTERNAL_URL;
const PG_ANON_KEY = process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY;
const hasPostgres = Boolean(PG_URL && PG_ANON_KEY);

interface Backend {
	nom: string;
	actif: boolean;
	/** Fabrique EXPLICITE du client — reproduit le comportement par défaut
	 * (SQLite : `createSqliteClient()`, c'est ce que `createClient()` appelle
	 * en l'absence de provider) pour pouvoir aussi le compter. */
	baseFactory: () => SupabaseClient | Promise<SupabaseClient>;
}

const BACKENDS: Backend[] = [
	{
		nom: "sqlite (miroir local, défaut)",
		actif: hasMirror,
		baseFactory: () => createSqliteClient() as unknown as SupabaseClient,
	},
	{
		nom: "postgres (rg-postgrest local — chemin du repli Vercel)",
		actif: hasPostgres,
		baseFactory: () => createSupabaseClient(PG_URL as string, PG_ANON_KEY as string),
	},
];

for (const backend of BACKENDS) {
	const suite = describe.skipIf(!backend.actif);

	suite(`getSkillsByIds — non-régression (${backend.nom})`, () => {
		// Provider = singleton de module partagé par TOUT le process `bun test`
		// (cf. `db.test.ts`) : posé avant chaque test (un des tests l'écrase
		// temporairement pour compter les requêtes), et TOUJOURS restauré à
		// `null` après — jamais à `backend.baseFactory`, qui fuiterait sur les
		// fichiers de test suivants (vécu : `wiki.test.ts` a viré au rouge en
		// lisant le Postgres local au lieu du miroir, provider non nettoyé).
		beforeEach(() => {
			setDatabaseProvider(backend.baseFactory);
		});
		afterEach(() => {
			setDatabaseProvider(null);
		});

		test("techniques distinctes entre variantes (boucle skillPromises)", async () => {
			for (const slug of SLUGS_FICHE) {
				const { variantSkillIds } = await idsPourFiche(slug);
				await compareUnitaireVsBatch(variantSkillIds);
			}
		});

		test("moveset binaire de secours, par variante (boucle fetchedSkills)", async () => {
			for (const slug of SLUGS_FICHE) {
				const { skillListsByVariant } = await idsPourFiche(slug);
				for (const skillList of skillListsByVariant) {
					await compareUnitaireVsBatch(skillList);
				}
			}
		});

		test("chemin nom/code interne (resolveSkillNames), y compris les noms en double", async () => {
			// `name_fr`/`name_en` ne sont PAS uniques dans `inagle_skills` (constaté
			// sur le miroir) : "Attaque cosmique" désigne 3 lignes
			// (whs02980/whs02980_or/whs02980_or_02), "Black Hole" en désigne 2
			// (ock5005/whk00390). `getSkill` ne garantit déjà aucun ordre dans ce
			// cas (pas d'ORDER BY, LIMIT 1) — cette suite vérifie que le batch
			// choisit LA MÊME ligne que l'appel unitaire, pas qu'il choisit "la
			// bonne" au sens absolu (ambiguïté préexistante, hors périmètre).
			const ids = [
				"whd00010",
				"whd00011",
				"whd00020",
				"Attaque cosmique",
				"Black Hole",
				"Cosmic Blaster",
				"whd00030",
			];
			await compareUnitaireVsBatch(ids);
		});

		test("id introuvable (des deux côtés) → undefined pour les deux chemins", async () => {
			await compareUnitaireVsBatch(["0xDEADBEEF", "ce-code-nexiste-pas"]);
		});

		test("gain mesuré : requêtes inagle_skills, boucle vs batch (fiche réelle)", async () => {
			const { skillListsByVariant } = await idsPourFiche("fei");
			const skillList = skillListsByVariant.find((l) => l.length > 0) ?? [];
			expect(skillList.length).toBeGreaterThan(0);

			const compteurBoucle = { n: 0 };
			setDatabaseProvider(fabriqueComptee(backend.baseFactory, compteurBoucle));
			// Boucle historique (avant ce correctif) : un `getSkill` par technique.
			await Promise.all(skillList.map((id) => wikiService.getSkill(id)));

			const compteurBatch = { n: 0 };
			setDatabaseProvider(fabriqueComptee(backend.baseFactory, compteurBatch));
			await wikiService.getSkillsByIds(skillList);

			// eslint-disable-next-line no-console -- gain chiffré, utile en sortie de CI
			console.log(
				`[gain ${backend.nom}] fiche "fei" (${skillList.length} techniques) : ` +
					`${compteurBoucle.n} requêtes (boucle) → ${compteurBatch.n} requête(s) (batch)`
			);
			expect(compteurBoucle.n).toBe(skillList.length);
			expect(compteurBatch.n).toBeLessThan(compteurBoucle.n);
			expect(compteurBatch.n).toBeLessThanOrEqual(2);
		});
	});
}
