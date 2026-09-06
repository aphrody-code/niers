import { readFileSync } from "node:fs";
import * as path from "node:path";
import { createClient } from "@supabase/supabase-js";
import type { Command } from "commander";
import jwt from "jsonwebtoken";
import {
	CATEGORY_NAMES,
	RARITY_NAMES as CHAR_RARITY_NAMES,
	createInagleService,
	ELEMENT_NAMES,
	PASSIVE_BOOST_NAMES,
	PASSIVE_SCOPE_NAMES,
	PositionNames,
} from "./index.js";
import { loadTextFile } from "./parsers/text-parser.js";
// Pushers de catégories câblées (source unique sous src/, réutilisée par les
// scripts standalone scripts/push-<cat>.ts).
import {
	importBoostGroups,
	importConstellations,
	importEmblems,
	importFormations,
	importMissions,
	importShops,
	importSkillTechnic,
	importSpecialTactics,
	importStarSigns,
	importSuperTactics,
	importTeamBuild,
	importTelopWaza,
	importTricks,
	importTrophies,
	importUniforms,
	importVideoWaza,
} from "./push-categories.js";
import { type DataAdapter, dedup, PostgresAdapter, SupabaseAdapter } from "./push-adapter.js";
import { type AuraHissatsu, buildAuraHashSet, loadAuraSkills } from "./skills/mapper-aura.js";
import { importLuaScripts } from "./lua/pusher.js";

// Même racine que `push-categories.ts` (DATA_PATH). `exp_table.json`/`growth_tables.json`
// vivaient sous un chemin relatif au SOURCE de ce module (`../data/all-gamedata/`, jamais
// peuplé) au lieu du dump niers — les deux imports échouaient `ENOENT` à chaque `push` et
// arrêtaient le reste des catégories (exception non rattrapée), silencieusement chaque nuit.
const DATA_ROOT = process.env.DATA_PATH || "/home/ubuntu/niers/data";

// Remplace `dotenv`, qui n'était déclaré nulle part : la CLI mourait en « Cannot find package
// 'dotenv' » dès qu'on la lançait hors d'un contexte où le paquet traînait par hoisting. Bun lit
// `.env`/`.env.local` tout seul ; il ne manquait que le cas d'un chemin donné par `--env`.
// Une dépendance de moins au lockfile partagé par les 18 services de production.
function chargerEnv(fichier: string): void {
	let texte: string;
	try {
		texte = readFileSync(fichier, "utf8");
	} catch {
		console.warn(`[push] fichier d'environnement introuvable : ${fichier}`);
		return;
	}
	for (const ligne of texte.split("\n")) {
		const nette = ligne.trim();
		if (!nette || nette.startsWith("#")) continue;
		const coupe = nette.indexOf("=");
		if (coupe < 1) continue;
		const cle = nette.slice(0, coupe).trim();
		let valeur = nette.slice(coupe + 1).trim();
		if (
			(valeur.startsWith('"') && valeur.endsWith('"')) ||
			(valeur.startsWith("'") && valeur.endsWith("'"))
		) {
			valeur = valeur.slice(1, -1);
		}
		// `dotenv` n'écrase pas une variable déjà posée : garder ce comportement, sinon un
		// `SUPABASE_URL=…` de la ligne de commande serait silencieusement remplacé.
		if (process.env[cle] === undefined) process.env[cle] = valeur;
	}
}

// === Helpers ===

const getLoc = (obj: any, lang: string) => {
	const l = lang.toLowerCase();
	const U = lang.toUpperCase();
	if (obj.names?.[l]) return obj.names[l];
	if (obj[`name_${U}`]) return obj[`name_${U}`];
	return obj.name || obj.displayName || null;
};

const getDesc = (obj: any, lang: string) => {
	const l = lang.toLowerCase();
	const U = lang.toUpperCase();
	if (obj.descriptions?.[l]) return obj.descriptions[l];
	if (obj[`desc_${U}`]) return obj[`desc_${U}`];
	return obj.description || obj.desc || null;
};

const getPositionFr = (posRaw: number) => {
	return (PositionNames as any)[posRaw]?.fr || "Inconnu";
};

// === Command ===

export function registerPushCommand(program: Command) {
	program
		.command("push")
		.description("Push Inagle data to Supabase (via API or Direct DB)")
		.option("--env <path>", "Path to .env file", ".env")
		.option("--url <url>", "Supabase URL (for API mode)")
		.option("--jwt-secret <secret>", "Supabase JWT Secret (to generate service token)")
		.option("--key <key>", "Direct Service Role Key")
		.option("--db-url <url>", "Postgres Connection URL (Direct DB mode)")
		.action(async (options) => {
			if (options.env) {
				chargerEnv(path.resolve(process.cwd(), options.env));
			}

			let adapter: DataAdapter;

			if (options.dbUrl) {
				console.log(`[Push] Using Direct DB connection: ${options.dbUrl}`);
				adapter = new PostgresAdapter(options.dbUrl);
			} else {
				let SUPABASE_URL =
					options.url || process.env.SUPABASE_URL || "http://127.0.0.1:8811";
				let SUPABASE_KEY = options.key || process.env.SUPABASE_SERVICE_ROLE_KEY;
				const JWT_SECRET =
					options.jwtSecret || process.env.JWT_SECRET || process.env.SUPABASE_JWT_SECRET;

				const isValidKey = (val: string | undefined | null) =>
					typeof val === "string" &&
					val.length > 0 &&
					!val.startsWith("eyJ2Ijo") &&
					val !== "undefined" &&
					val !== "null";

				const isValidUrl = (val: string | undefined | null) =>
					typeof val === "string" &&
					val.length > 0 &&
					val.startsWith("http") &&
					!val.startsWith("eyJ2Ijo");

				if (!isValidUrl(SUPABASE_URL)) {
					SUPABASE_URL = "http://127.0.0.1:8811";
				}

				if (!isValidKey(SUPABASE_KEY)) {
					if (isValidKey(JWT_SECRET)) {
						console.log("[Push] Generating Service Role Token from Secret...");
						const payload = {
							role: "service_role",
							iss: "supabase",
							iat: Math.floor(Date.now() / 1000),
							exp: Math.floor(Date.now() / 1000) + 3600,
						};
						SUPABASE_KEY = jwt.sign(payload, JWT_SECRET!);
					} else {
						console.log("[Push] Using local dev Supabase fallback service role key");
						SUPABASE_KEY =
							"***REDACTED-SERVICE-ROLE***";
					}
				}

				console.log(`[Push] Connecting to Supabase API at: ${SUPABASE_URL}`);
				const supabase = createClient(SUPABASE_URL, SUPABASE_KEY!, {
					auth: { persistSession: false },
				});
				adapter = new SupabaseAdapter(supabase);
			}

			try {
				console.log("🚀 Starting Inagle Data Push...");
				const service = await createInagleService();

				await importCharacters(service, adapter);
				await importSkills(service, adapter);
				await importItems(service, adapter);
				await importPassives(service, adapter);
				await importAuras(service, adapter);
				await importTeams(service, adapter);
				await importFormations(service, adapter);
				await importQuests(service, adapter);
				await importDrops(service, adapter);

				// New parser imports
				await importGallery(service, adapter);
				await importCostumes(service, adapter);
				await importOpponentTeams(service, adapter);
				await importCapsules(service, adapter);

				// Catégories câblées depuis les pushers standalone (octets réels du dump).
				await importUniforms(service, adapter);
				await importShops(service, adapter);
				await importTricks(service, adapter);
				await importSpecialTactics(service, adapter);
				await importTelopWaza(service, adapter);
				await importVideoWaza(service, adapter);
				await importEmblems(service, adapter);
				await importSuperTactics(service, adapter);
				await importSkillTechnic(service, adapter);
				await importTeamBuild(service, adapter);
				await importBoostGroups(service, adapter);
				await importConstellations(service, adapter);
				await importStarSigns(service, adapter);
				await importTrophies(service, adapter);
				await importMissions(service, adapter);
				await importLuaScripts(service, adapter);


				// Static reference tables
				await importExpTable(adapter);
				await importGrowthTables(adapter);

				// Story text static export
				await exportStoryTextDatabase();

				console.log("🎉 Data push completed successfully!");
			} catch (error) {
				console.error("Fatal error during push:", error);
			} finally {
				await adapter.close();
			}
		});
}

// === Importers (Generic) ===

function slugify(name: string): string {
	return name
		.toLowerCase()
		.normalize("NFD")
		.replace(/[\u0300-\u036f]/g, "")
		.replace(/[^a-z0-9\s-]/g, "")
		.trim()
		.replace(/\s+/g, "-");
}

async function importCharacters(service: any, db: DataAdapter) {
	console.log("🔄 Importing Characters...");

	// Clean existing characters in database first to remove leftover duplicates/mismatch entries
	// Préserve les colonnes CURATÉES hors-pipeline (alimentées par d'autres
	// process : sheet_data = données communautaires, zukan_order = ordre officiel
	// zukan.inazuma.jp). Le delete+reinsert les écraserait sinon. On les snapshot
	// puis on les réinjecte par id dans les records.
	console.log("  Préservation de sheet_data / zukan_order avant nettoyage...");
	const preserved = new Map<string, { sheet_data: any; zukan_order: any }>();
	for (const r of await db.fetchAll("inagle_characters", "id, sheet_data, zukan_order")) {
		preserved.set(r.id, { sheet_data: r.sheet_data ?? null, zukan_order: r.zukan_order ?? null });
	}
	console.log(`  ${preserved.size} lignes curatées snapshotées.`);

	console.log("  Cleaning existing characters in database...");
	await db.deleteAllExcept("inagle_characters", "id", "DUMMY_NONE");

	let chars = service.characters.all();
	chars = chars.filter((c: any) => c.charaParamId && c.charaParamId !== "null");

	// Group by character name to find the primary (youngest) version and set slugs
	const charsByName = new Map<string, any[]>();
	for (const c of chars) {
		const nameEn = getLoc(c, "en") || c.name || "";
		const key = nameEn.toLowerCase().trim() || c.charaId;
		if (!charsByName.has(key)) {
			charsByName.set(key, []);
		}
		charsByName.get(key)!.push(c);
	}

	// Align images/codes and assign is_primary and slugs
	const processedChars: any[] = [];
	for (const [, variants] of charsByName.entries()) {
		// Sort variants: normal first, prefer zukanHash, then sorted by charaParamId ASC
		variants.sort((a: any, b: any) => {
			const aHero = a.rarity === "Héros" || a.rarityCode === 10 ? 1 : 0;
			const bHero = b.rarity === "Héros" || b.rarityCode === 10 ? 1 : 0;
			if (aHero !== bHero) return aHero - bHero;

			const aBasara = a.rarity === "BASARA" || a.rarityCode >= 20 ? 1 : 0;
			const bBasara = b.rarity === "BASARA" || b.rarityCode >= 20 ? 1 : 0;
			if (aBasara !== bBasara) return aBasara - bBasara;

			const aHash = a.zukanHash ? 1 : 0;
			const bHash = b.zukanHash ? 1 : 0;
			if (aHash !== bHash) return bHash - aHash;

			return String(a.internalCode || "").localeCompare(String(b.internalCode || ""));
		});

		const primary = variants[0];
		const nameEn = getLoc(primary, "en") || primary.name || "unknown";
		const baseSlug = slugify(nameEn);

		for (let idx = 0; idx < variants.length; idx++) {
			const v = variants[idx];
			v.is_primary = idx === 0;
			v.base_slug = baseSlug;
			v.slug = `${baseSlug}-${v.charaParamId}`;

			// Mismatch correction: Hero and Basara variants get the same image, zukanHash, and zukanOrder as the normal version
			const isH = v.rarity === "Héros" || v.rarityCode === 10;
			const isB = v.rarity === "BASARA" || v.rarityCode >= 20;
			if ((isH || isB) && primary) {
				v.internalCode = primary.internalCode;
				v.zukanHash = primary.zukanHash;
				if (primary.zukanOrder) {
					v.zukanOrder = primary.zukanOrder;
				}
			}
			processedChars.push(v);
		}
	}

	// Build aura hash set to separate auras from skills in chara_param slots
	const auraHashes = buildAuraHashSet();
	console.log(`  Loaded ${auraHashes.size} aura hashes for skill/aura separation`);

	// Postgres safe batch size
	const BATCH_SIZE = 100;
	let successCount = 0;

	for (let i = 0; i < processedChars.length; i += BATCH_SIZE) {
		const batch = processedChars.slice(i, i + BATCH_SIZE);
		let records = batch.map((c: any) => {
			const stats = c.stats?.lv99 || {};

			// Separate skills from auras — chara_param mixes both in the same slots
			const rawSkills: { learnLevel: number; skillId: string }[] = c.skills || [];
			const skills = rawSkills.filter(
				(s) => s.skillId !== "0x00000002" && !auraHashes.has(s.skillId)
			);
			const auras = rawSkills.filter((s) => auraHashes.has(s.skillId));

			return {
				id: c.charaParamId,
				chara_id: c.charaId,
				name_fr: getLoc(c, "fr"),
				name_en: getLoc(c, "en"),
				name_ja: getLoc(c, "ja"),
				description_fr: getDesc(c, "fr"),
				description_en: getDesc(c, "en"),
				description_ja: getDesc(c, "ja"),
				rarity: c.rarityCode || 0,
				rarity_label: c.rarity || CHAR_RARITY_NAMES[c.rarityCode] || null,
				position: getPositionFr(c.positionRaw),
				element: ELEMENT_NAMES[c.elementRaw]?.fr || null,
				gender: c.gender === 1 ? "F" : "M",
				series: c.series || null,
				team_id: c.teamId || c.teams?.[0]?.id || null,
				stat_frappe: stats.kick || 0,
				stat_physique: stats.physical || 0,
				stat_controle: stats.control || 0,
				stat_pression: stats.pressure || 0,
				stat_agilite: stats.agility || 0,
				stat_technique: stats.technique || 0,
				stat_intelligence: stats.intelligence || 0,
				zukan_hash: c.zukanHash || null,
				// zukan_order : conserve l'ordre officiel curaté s'il existe, sinon parser.
				zukan_order: preserved.get(c.charaParamId)?.zukan_order ?? c.zukanOrder ?? null,
				// sheet_data : 100% curaté hors-pipeline → restaure le snapshot (jamais le parser), mais intègre les nouveaux movesets.
				sheet_data: (() => {
					const jsonSd = c.sheetData || {};
					const dbSd = preserved.get(c.charaParamId)?.sheet_data || {};
					const merged = { ...jsonSd, ...dbSd };
					if (!merged.moveset && jsonSd.moveset) {
						merged.moveset = jsonSd.moveset;
					}
					if (!merged.altMoveset && jsonSd.altMoveset) {
						merged.altMoveset = jsonSd.altMoveset;
					}
					return Object.keys(merged).length > 0 ? merged : null;
				})(),
				internal_code: c.internalCode || null,
				constellation: c.constellation?.names?.fr || null,
				constellation_index: c.constellation?.index ?? null,
				is_controllable: c.isControllable || false,
				control_type: c.controlType ?? null,
				stat_lv1_frappe: c.stats?.lv1?.kick || null,
				stat_lv1_controle: c.stats?.lv1?.control || null,
				stat_lv1_technique: c.stats?.lv1?.technique || null,
				stat_lv1_physique: c.stats?.lv1?.physical || null,
				stat_lv1_pression: c.stats?.lv1?.pressure || null,
				stat_lv1_agilite: c.stats?.lv1?.agility || null,
				stat_lv1_intelligence: c.stats?.lv1?.intelligence || null,
				image_url: service.images.faceUrl(c.internalCode || c.charaId),
				skills,
				data: { ...c, skills, auras },
				updated_at: new Date().toISOString(),
				is_primary: c.is_primary ?? false,
				hero_type: (c.sheetData && c.sheetData.heroType) || null,
				slug: c.slug || null,
				base_slug: c.base_slug || null,
			};
		});

		records = dedup(records, "id");
		const { error } = await db.upsert("inagle_characters", records, "id");
		if (error) console.error("❌ Error batch char:", error.message || error);
		else successCount += records.length;
	}
	console.log(`✅ Characters imported (${successCount}/${processedChars.length}).`);
}

/** Ce qu'une ligne `inagle_skills` sait d'elle-même et que le dump du jeu ignore. */
interface PreservedSkill {
	video_url: string | null;
	poster_url: string | null;
	thumbnail_url: string | null;
	/** Date de première apparition de la technique — voir `importSkills`. */
	created_at: string | null;
}

async function importSkills(service: any, db: DataAdapter) {
	console.log("🔄 Importing Skills...");

	// Préserve l'enrichissement zukan (vidéo, poster, vignette) AVANT le delete+reinsert,
	// sinon chaque push le wipe — même régression que `sheet_data` sur les items.
	//
	// CE QUI S'EST PASSÉ EN PRODUCTION LE 17/8/2026. `deleteAllExcept` vide la table :
	//  1. `video_url` / `poster_url` / `thumbnail_url` repartaient à NULL (le dump du jeu
	//     ne les contient pas — elles viennent de `zukan/sync-skill-videos.ts`) ;
	//  2. la clé étrangère `inagle_skill_videos.skill_id … on delete cascade` emportait
	//     avec elle les 1211 variantes, dont les deux vidéos des 236 techniques qui en ont.
	// Résultat : plus une seule vidéo ni vignette sur /skill après le push de 2h00, et
	// rien pour les remettre — la tâche `zukan:videos` n'était pas planifiée.
	//
	// Le nom du fichier `data/zukan/skill-videos.json` que ce code lisait auparavant
	// n'existe nulle part dans le dépôt : la lecture retombait toujours sur `null`,
	// donc le push écrasait activement la donnée au lieu de simplement la perdre.
	// La base fait désormais foi, et `zukan:videos` la rafraîchit juste après (cron 2h00).
	//
	// `created_at` EST AUSSI PRÉSERVÉE, ET C'EST DU SEO. Le plan de site
	// (`apps/azalee/app/sitemap.ts`) l'annonce en `lastModified` pour les 993
	// techniques. Réinsérées chaque nuit, elles prenaient toutes le `default now()`
	// du jour : le plan jurait à Google que 993 pages inchangées venaient d'être
	// modifiées, toutes les nuits. Un plan de site qui ment sur ses dates finit par
	// ne plus être cru — et gaspille le budget d'exploration en attendant.
	const preserved = new Map<string, PreservedSkill>();
	for (const r of await db.fetchAll(
		"inagle_skills",
		"id, video_url, poster_url, thumbnail_url, created_at"
	)) {
		preserved.set(String(r.id), {
			created_at: r.created_at ?? null,
			poster_url: r.poster_url ?? null,
			thumbnail_url: r.thumbnail_url ?? null,
			video_url: r.video_url ?? null,
		});
	}
	const preservedVideos = await db.fetchAll(
		"inagle_skill_videos",
		"skill_id, position, label, video_url, poster_url, source"
	);
	const avecZukan = [...preserved.values()].filter((p) => p.video_url || p.thumbnail_url).length;
	console.log(
		`  ${preserved.size} techniques préservées (${avecZukan} enrichies zukan, ${preservedVideos.length} variantes vidéo).`
	);

	// Clean existing skills in database first to remove leftover passive/stat boost and duplicate entries
	console.log("  Cleaning existing skills in database...");
	await db.deleteAllExcept("inagle_skills", "id", "DUMMY_NONE");

	let skills = service.skills.all();
	skills = skills.filter(
		(s: any) =>
			(s.internalCode || s.skillIDStr || s.skillID || s.id) &&
			s.category !== 6 &&
			s.category !== "Passive" &&
			s.category !== 9 &&
			s.category !== "Stat Boost" &&
			s.category !== "Boost Stats"
	);

	// Map to database records
	const maintenant = new Date().toISOString();
	let records = skills.map((s: any) => {
		const id = s.internalCode || s.skillIDStr || s.skillID || s.id;
		const zukan = preserved.get(String(id));

		return {
			// Une technique déjà connue garde sa date de première apparition ; une
			// technique réellement nouvelle (mise à jour du jeu) est datée d'aujourd'hui,
			// et c'est alors une vraie information pour le plan de site.
			created_at: zukan?.created_at ?? maintenant,
			id: String(id),
			name_fr: getLoc(s, "fr"),
			name_en: getLoc(s, "en"),
			name_ja: getLoc(s, "ja"),
			description_fr: getDesc(s, "fr"),
			description_en: getDesc(s, "en"),
			description_ja: getDesc(s, "ja"),
			element: ELEMENT_NAMES[s.element]?.fr || null,
			category: CATEGORY_NAMES[s.category]?.fr || null,
			tp_cost: s.consumeTp || s.tensionCost || 0,
			power_min: s.power_min || s.powerMin || 0,
			power_max: s.power_max || s.powerMax || 0,
			foul_rate: s.foulRate || 0,
			growth_type: s.growthType,
			internal_code: s.internalCode || null,
			hash_id: s.hashId || null,
			video_url: zukan?.video_url ?? null,
			poster_url: zukan?.poster_url ?? null,
			thumbnail_url: zukan?.thumbnail_url ?? null,
			recast_time: s.recastTime || 0,
			is_eldorado: s.eldorado || false,
			partner_count: s.partnerCount || 0,
			evolution_type: s.growthSpeed || null,
			skill_effect_bit_flag: s.skillEffectBitFlag || 0,
			tags: s.tags
				? `{${s.tags.map((t: string) => `"${t.replace(/"/g, '\\"')}"`).join(",")}}`
				: "{}",
			image_url: service.images.skillUrl(id),
			data: s,
			updated_at: new Date().toISOString(),
		};
	});

	// Deduplicate the entire set of records before chunking
	records = dedup(records);

	const BATCH_SIZE = 100;
	let successCount = 0;

	for (let i = 0; i < records.length; i += BATCH_SIZE) {
		const batch = records.slice(i, i + BATCH_SIZE);
		const { error } = await db.upsert("inagle_skills", batch);
		if (error) console.error("❌ Error batch skills:", error.message || error);
		else successCount += batch.length;
	}
	console.log(`✅ Skills imported (${successCount}/${records.length}).`);

	// Restauration des variantes vidéo emportées par la cascade. On ne réinsère que
	// celles dont la technique existe encore : une technique disparue du dump ne doit
	// pas ressusciter par sa clé étrangère (l'insert échouerait, et à raison).
	const keptIds = new Set(records.map((r: { id: string }) => r.id));
	const restorable = preservedVideos.filter((v: any) => keptIds.has(String(v.skill_id)));
	if (restorable.length > 0) {
		const { error } = await db.upsert("inagle_skill_videos", restorable, "skill_id,position");
		if (error) console.error("❌ Error restoring skill videos:", error.message || error);
		else console.log(`✅ Skill videos restored (${restorable.length}/${preservedVideos.length}).`);
	}
}

async function importItems(service: any, db: DataAdapter) {
	console.log("🔄 Importing Items...");

	// Préserve le sheet_data curaté/riche (shops, price, attributes) AVANT le delete+reinsert,
	// sinon chaque push wipe la donnée enrichie des items (régression connue).
	const preserved = new Map<string, any>();
	for (const r of await db.fetchAll("inagle_items", "id, sheet_data")) {
		if (r.sheet_data) preserved.set(String(r.id), r.sheet_data);
	}
	console.log(`  ${preserved.size} sheet_data items préservés.`);

	// Clean existing items first to remove leftover fake/null entries from previous syncs
	console.log("  Cleaning existing items in database...");
	await db.deleteAllExcept("inagle_items", "id", "0x00000000");

	let items = service.items.allItems();
	items = items.filter((item: any) => {
		const id = item.itemId || item.id;
		if (!id || id === "0x00000000" || id === "0x00000002" || id === "null") return false;

		const name = item.name || "";
		if (
			name.includes("_0x") ||
			name.startsWith("consume_") ||
			name.startsWith("shoes_") ||
			name.startsWith("misanga_") ||
			name.startsWith("accessory_") ||
			name.startsWith("special_") ||
			name.startsWith("fashion_") ||
			name.startsWith("badge_") ||
			name.startsWith("coin_") ||
			name.startsWith("gacha_ticket_")
		) {
			return false;
		}

		// Must have at least one name in FR or EN
		const hasName = getLoc(item, "fr") || getLoc(item, "en");
		if (!hasName) return false;

		return true;
	});

	// Deduplicate items by category + normalized name (preferring records with internalCode / descriptions)
	const uniqueMap = new Map<string, any>();
	for (const item of items) {
		const name = (getLoc(item, "fr") || getLoc(item, "en") || "").toLowerCase().trim();
		const key = `${item.category}_${name}`;
		const existing = uniqueMap.get(key);
		if (!existing) {
			uniqueMap.set(key, item);
		} else {
			const existingHasCode = !!existing.internalCode;
			const itemHasCode = !!item.internalCode;
			const existingHasDesc = !!getDesc(existing, "fr");
			const itemHasDesc = !!getDesc(item, "fr");

			if (
				(!existingHasCode && itemHasCode) ||
				(existingHasCode === itemHasCode && !existingHasDesc && itemHasDesc)
			) {
				uniqueMap.set(key, item);
			}
		}
	}
	items = Array.from(uniqueMap.values());

	const BATCH_SIZE = 100;
	let successCount = 0;

	for (let i = 0; i < items.length; i += BATCH_SIZE) {
		const batch = items.slice(i, i + BATCH_SIZE);
		let records = batch.map((item: any) => {
			const id = item.itemId || item.id;
			return {
				id: String(id),
				name_fr: getLoc(item, "fr"),
				name_en: getLoc(item, "en"),
				name_ja: getLoc(item, "ja"),
				description_fr: getDesc(item, "fr"),
				description_en: getDesc(item, "en"),
				description_ja: getDesc(item, "ja"),
				category: item.category,
				rarity: item.rarity || 0,
				price: item.price || 0,
				sell_price: item.sellPrice || 0,
				internal_code: item.internalCode || null,
				stat_boost_1: item.stats?.stat1 || null,
				stat_boost_2: item.stats?.stat2 || null,
				image_url: item.imageUrl || null,
				data: item,
				// NATIF d'abord : l'objet parsé porte shops/price/attributes directement de
				// shop_config/item_config (cfg.bin) — plus de dépendance au Google Sheet
				// communautaire. Garde-fou : si ce run n'a pas chargé les shops (DATA_ROOT
				// absent), retombe sur le snapshot préservé pour ne pas appauvrir la donnée.
				sheet_data: (item?.shops ? item : (preserved.get(String(id)) ?? item)) ?? null,
				updated_at: new Date().toISOString(),
			};
		});

		records = dedup(records);
		const { error } = await db.upsert("inagle_items", records);
		if (error) console.error("❌ Error batch item:", error.message || error);
		else successCount += records.length;
	}
	console.log(`✅ Items imported (${successCount}/${items.length}).`);
}

async function importPassives(service: any, db: DataAdapter) {
	console.log("🔄 Importing Passives...");
	const passives = service.skills.passiveAll();
	const BATCH_SIZE = 100;
	let successCount = 0;

	for (let i = 0; i < passives.length; i += BATCH_SIZE) {
		const batch = passives.slice(i, i + BATCH_SIZE);
		let records = batch.map((p: any) => ({
			id: p.passiveIdStr || p.passiveId,
			name_fr: getLoc(p, "fr"),
			name_en: getLoc(p, "en"),
			name_ja: getLoc(p, "ja"),
			description_fr: getDesc(p, "fr"),
			description_en: getDesc(p, "en"),
			description_ja: getDesc(p, "ja"),
			category: (PASSIVE_SCOPE_NAMES as any)[p.scope]?.fr || p.scope || "Inconnu",
			boost_type: (PASSIVE_BOOST_NAMES as any)[p.boostType]?.fr || p.boostType || "Inconnu",
			effect_value: p.effectValue || 0,
			data: p,
			updated_at: new Date().toISOString(),
		}));

		records = dedup(records);
		const { error } = await db.upsert("inagle_passives", records);
		if (error) console.error("❌ Error batch passive:", error.message || error);
		else successCount += records.length;
	}
	console.log(`✅ Passives imported (${successCount}/${passives.length}).`);
}

async function importAuras(service: any, db: DataAdapter) {
	console.log("🔄 Importing Auras...");
	const serviceAuras = service.skills.auraAll();

	// HISSATSU NATIF : on résout les auras + leur hissatsu directement depuis le dump
	// (loadAuraSkills → config.skillId1 → skill_config). Le service peut renvoyer un
	// snapshot prépackagé (entries/auras.json) incomplet et sans hissatsu ; si le parse
	// natif est plus riche (dump présent), il devient la source de vérité. Sinon on
	// retombe sur le service et on fusionne le hissatsu natif par auraId / assetCode.
	const nativeHissatsu = new Map<string, AuraHissatsu>();
	let nativeAuras: any[] = [];
	try {
		const fr = loadTextFile("skill", "fr");
		const en = loadTextFile("skill", "en");
		const ja = loadTextFile("skill", "ja");
		nativeAuras = loadAuraSkills("", "", fr, en, ja);
		for (const a of nativeAuras) {
			if (!a.hissatsu) continue;
			if (a.auraId) nativeHissatsu.set(a.auraId, a.hissatsu);
			if (a.auraIdStr) nativeHissatsu.set(a.auraIdStr, a.hissatsu);
			if (a.assetCode) nativeHissatsu.set(a.assetCode, a.hissatsu);
		}
		console.log(
			`   ↳ ${nativeAuras.length} auras natives, ${nativeAuras.filter((x) => x.hissatsu).length} avec hissatsu natif.`
		);
	} catch (e) {
		console.warn("   ⚠️ Résolution hissatsu native indisponible:", (e as Error)?.message);
	}

	// Source de vérité : le dump natif s'il est plus complet que le snapshot du service.
	const auras = nativeAuras.length >= serviceAuras.length ? nativeAuras : serviceAuras;

	const resolveHissatsu = (a: any): AuraHissatsu | undefined =>
		a.hissatsu ||
		nativeHissatsu.get(a.auraId) ||
		nativeHissatsu.get(a.auraIdStr) ||
		(a.assetCode ? nativeHissatsu.get(a.assetCode) : undefined);
	const keshins: any[] = [];
	const souls: any[] = [];
	const awakenings: any[] = [];
	const modeChanges: any[] = [];
	const miximax: any[] = [];
	const aurasList: any[] = [];

	for (const a of auras) {
		const name = getLoc(a, "fr");
		const isInvalid = !name || name.includes("(was") || name.startsWith("Aura ");
		if (isInvalid) continue;

		const baseRecord = {
			id: a.auraIdStr || a.auraId,
			name_fr: name,
			name_en: getLoc(a, "en"),
			name_ja: getLoc(a, "ja"),
			description_fr: getDesc(a, "fr"),
			description_en: getDesc(a, "en"),
			description_ja: getDesc(a, "ja"),
			element_id: a.element ?? null,
			asset_code: a.assetCode || null,
			image_url: a.image || null,
			// Injecte TOUJOURS le lien aura→technique (config.skillId1/skillId2) dans
			// sheet_data — c'est ce que getAura consomme côté azalee. (Le `data` complet
			// porte aussi a.config, mais on garantit la présence dans sheet_data.)
			//
			// HISSATSU NATIF : on écrit le hissatsu résolu depuis les fichiers du jeu
			// (a.hissatsu = {name,type,element,power} via config.skillId1 → skill_config),
			// qui REMPLACE le hissatsu du Google Sheet communautaire. Le `name` du sheet
			// (sheet_data.name / sheetName) reste seulement comme alias d'affichage.
			sheet_data: (() => {
				const base = (a.sheetData as any) ?? {};
				const out: Record<string, unknown> = { ...base };
				if (a.config) out.config = a.config;
				// Le hissatsu natif prime sur celui du sheet (s'il y en avait un).
				const hissatsu = resolveHissatsu(a);
				if (hissatsu) out.hissatsu = hissatsu;
				return Object.keys(out).length > 0 ? out : null;
			})(),
			updated_at: new Date().toISOString(),
		};

		const subType = a.subType || "Aura";
		if (subType === "Keshin") keshins.push({ ...baseRecord, type: "Esprit Guerrier", data: a });
		else if (subType === "Soul") souls.push({ ...baseRecord, type: "Totem", data: a });
		else if (subType === "Awakening") awakenings.push({ ...baseRecord, type: "Éveil", data: a });
		else if (subType === "ModeChange") modeChanges.push({ ...baseRecord, type: "Changement de mode", data: a });
		else if (subType === "Aura") {
			if (name.includes("Miximax")) miximax.push({ ...baseRecord, type: "Miximax", data: a });
			else {
				aurasList.push({ ...baseRecord, sub_type: "Aura" });
			}
		} else miximax.push({ ...baseRecord, type: "Miximax", data: a });
	}

	if (keshins.length > 0) await db.upsert("inagle_keshins", dedup(keshins));
	if (souls.length > 0) await db.upsert("inagle_souls", dedup(souls));
	if (awakenings.length > 0) await db.upsert("inagle_awakenings", dedup(awakenings));
	if (modeChanges.length > 0) await db.upsert("inagle_mode_changes", dedup(modeChanges));
	if (miximax.length > 0) await db.upsert("inagle_miximax", dedup(miximax));
	if (aurasList.length > 0) await db.upsert("inagle_auras", dedup(aurasList));
	console.log(`✅ Auras imported (${auras.length}) — ${aurasList.length} dans inagle_auras.`);
}

async function importTeams(service: any, db: DataAdapter) {
	console.log("🔄 Importing Teams...");
	const teams = service.teams.allTeams();
	const BATCH_SIZE = 50;
	let successCount = 0;

	for (let i = 0; i < teams.length; i += BATCH_SIZE) {
		const batch = teams.slice(i, i + BATCH_SIZE);
		let records = batch.map((t: any) => ({
			id: t.teamId || t.id,
			name_fr: getLoc(t, "fr"),
			name_en: getLoc(t, "en"),
			name_ja: getLoc(t, "ja"),
			description_fr: getDesc(t, "fr"),
			description_en: getDesc(t, "en"),
			description_ja: getDesc(t, "ja"),
			country_code: t.countryCode || null,
			series: t.series?.name || t.series || null,
			emblem_url: service.images.emblemUrl(t.teamId || t.id),
			data: t,
			updated_at: new Date().toISOString(),
		}));
		records = dedup(records);
		const { error } = await db.upsert("inagle_teams", records);
		if (error) console.error("❌ Error batch teams:", error.message || error);
		else successCount += records.length;
	}
	console.log(`✅ Teams imported (${successCount}/${teams.length}).`);
}

// `importFormations` n'est PAS redéfinie ici : elle est importée de
// `./push-categories.js` (voir l'en-tête du fichier).
//
// Une copie locale vivait à cet endroit et lisait `service.teams.allFormations()`
// — l'ancien chemin, sans les noms ni les onze placements. Elle entrait en
// conflit avec l'import du même nom (TS2440), ce qui cassait `inagle#build`
// depuis le 5/6/2026 : `dist/` n'a plus été régénéré, et le fichier lui-même
// était de toute façon inexécutable (redéclaration en module ES).
// La version retenue est celle de `push-categories.ts`, qui construit la base
// depuis `formation/formation_config` + `formation_text` et journalise combien
// de formations sont réellement nommées.

async function importQuests(service: any, db: DataAdapter) {
	console.log("🔄 Importing Quests...");
	const quests = service.quests.all();
	const BATCH_SIZE = 50;
	let successCount = 0;

	for (let i = 0; i < quests.length; i += BATCH_SIZE) {
		const batch = quests.slice(i, i + BATCH_SIZE);
		let records = batch.map((q: any) => ({
			id: q.id,
			display_text: q.displayText,
			phase: q.phase,
			data: q,
			updated_at: new Date().toISOString(),
		}));
		records = dedup(records);
		const { error } = await db.upsert("inagle_quests", records);
		if (error) console.error("❌ Error batch quests:", error.message || error);
		else successCount += records.length;
	}
	console.log(`✅ Quests imported (${successCount}/${quests.length}).`);
}

async function importDrops(service: any, db: DataAdapter) {
	console.log("🔄 Importing Drops...");

	const tables = service.drops.getAllTables();
	const tableEntries = Array.from(tables.entries());
	const BATCH_SIZE = 50;

	for (let i = 0; i < tableEntries.length; i += BATCH_SIZE) {
		const batch = tableEntries.slice(i, i + BATCH_SIZE);
		const records = batch.map((entry: any) => {
			const [id, entries] = entry;
			return {
				table_id: id,
				entries: entries,
				updated_at: new Date().toISOString(),
			};
		});
		await db.upsert("inagle_drops_tables", records, "table_id");
	}

	const battles = service.drops.getAllBattles();
	const battleEntries = Array.from(battles.entries());
	for (let i = 0; i < battleEntries.length; i += BATCH_SIZE) {
		const batch = battleEntries.slice(i, i + BATCH_SIZE);
		const records = batch.map((entry: any) => {
			const [id, info] = entry;
			return {
				battle_group_id: id,
				item_table_id: (info as any).itemTableId,
				data: info,
				updated_at: new Date().toISOString(),
			};
		});
		await db.upsert("inagle_drops_battles", records, "battle_group_id");
	}

	const treasures = service.drops.getAllTreasures();
	await db.deleteAllExcept("inagle_drops_treasures", "map_id", "xx");

	for (let i = 0; i < treasures.length; i += BATCH_SIZE) {
		const batch = treasures.slice(i, i + BATCH_SIZE);
		const records = batch.map((t: any) => ({
			map_id: t.mapId,
			pos: t.pos,
			items: t.items,
			data: t,
			updated_at: new Date().toISOString(),
		}));
		await db.insert("inagle_drops_treasures", records);
	}
	console.log("✅ Drops imported.");
}

// === New Parser Importers ===

async function importGallery(_service: any, db: DataAdapter) {
	console.log("🔄 Importing Gallery...");
	const { buildGalleryDatabase } = await import("./parsers/gallery-config.js");
	const gallery = await buildGalleryDatabase();

	const records = gallery.galleries.map((g: any) => ({
		id: g.galleryId,
		img_path: g.imgPath,
		thumb_path: g.thumbPath,
		need_token_num: g.needTokenNum,
		flg_no: g.flgNo,
		data: g,
		updated_at: new Date().toISOString(),
	}));

	if (records.length > 0) {
		const { error } = await db.upsert("inagle_gallery", dedup(records));
		if (error) console.error("❌ Error gallery:", error.message || error);
	}
	console.log(`✅ Gallery imported (${records.length}).`);
}

async function importCostumes(_service: any, db: DataAdapter) {
	console.log("🔄 Importing Costumes...");
	const { buildCharaCostumeDatabase } = await import("./parsers/chara-costume-config.js");
	const costumes = await buildCharaCostumeDatabase();

	const records = costumes.costumes.map((c: any) => ({
		id: `costume_${c.index}`,
		costume_index: c.index,
		type: c.type,
		model_ref: c.modelRef,
		flag1: c.flag1,
		flag2: c.flag2,
		data: c,
		updated_at: new Date().toISOString(),
	}));

	const BATCH_SIZE = 100;
	let successCount = 0;
	for (let i = 0; i < records.length; i += BATCH_SIZE) {
		const batch = records.slice(i, i + BATCH_SIZE);
		const { error } = await db.upsert("inagle_costumes", batch);
		if (error) console.error("❌ Error costumes:", error.message || error);
		else successCount += batch.length;
	}
	console.log(`✅ Costumes imported (${successCount}).`);
}

async function importOpponentTeams(_service: any, db: DataAdapter) {
	console.log("🔄 Importing Opponent Teams...");
	const { buildOpponentTeamDatabase } = await import("./parsers/opponent-team-config.js");
	const opponents = await buildOpponentTeamDatabase();

	const records = opponents.opponents.map((o: any) => ({
		id: o.opponentId,
		team_id: o.teamId,
		type: o.type,
		game_id: o.gameId,
		difficulty_type: o.difficultyType,
		bg_texture: o.bgTextureName,
		data: o,
		updated_at: new Date().toISOString(),
	}));

	if (records.length > 0) {
		const { error } = await db.upsert("inagle_opponent_teams", dedup(records));
		if (error) console.error("❌ Error opponent teams:", error.message || error);
	}
	console.log(`✅ Opponent Teams imported (${records.length}).`);
}

async function importExpTable(db: DataAdapter) {
	console.log("🔄 Importing Exp Table...");
	const dataPath = path.join(DATA_ROOT, "all-gamedata/exp_table.json");
	const rows: { level: number; need_exp: number }[] = await Bun.file(dataPath).json();
	const records = rows.map((r: any) => ({ level: r.level, need_exp: r.needExp ?? r.need_exp }));
	const { error } = await db.upsert("inagle_exp_table", records, "level");
	if (error) console.error("❌ Error exp_table:", error.message || error);
	else console.log(`✅ Exp Table imported (${records.length} levels).`);
}

async function importGrowthTables(db: DataAdapter) {
	console.log("🔄 Importing Growth Tables...");
	const dataPath = path.join(DATA_ROOT, "all-gamedata/growth_tables.json");
	const raw: Record<string, any[]> = await Bun.file(dataPath).json();

	const records: any[] = [];

	// lv1 : mainPosition, subPosition, playStyle
	for (const row of raw.lv1 ?? []) {
		records.push({
			section: "lv1",
			main_position: row.mainPosition ?? null,
			sub_position: row.subPosition ?? null,
			play_style: row.playStyle ?? null,
			growth_pattern: null,
			chara_rank: null,
			data: row,
		});
	}

	// lv30 : mainPosition, subPosition, growthPattern, charaRank
	for (const row of raw.lv30 ?? []) {
		records.push({
			section: "lv30",
			main_position: row.mainPosition ?? null,
			sub_position: row.subPosition ?? null,
			play_style: null,
			growth_pattern: row.growthPattern ?? null,
			chara_rank: row.charaRank ?? null,
			data: row,
		});
	}

	// main : mainPosition (pas de subPosition)
	for (const row of Object.values(raw.main ?? {})) {
		records.push({
			section: "main",
			main_position: row.mainPosition ?? null,
			sub_position: null,
			play_style: null,
			growth_pattern: row.growthPattern ?? null,
			chara_rank: row.charaRank ?? null,
			data: row,
		});
	}

	// sub : subPosition (pas de mainPosition)
	for (const row of Object.values(raw.sub ?? {})) {
		records.push({
			section: "sub",
			main_position: null,
			sub_position: row.subPosition ?? null,
			play_style: null,
			growth_pattern: row.growthPattern ?? null,
			chara_rank: row.charaRank ?? null,
			data: row,
		});
	}

	const { error } = await db.upsert(
		"inagle_growth_tables",
		records,
		"section,main_position,sub_position,play_style,growth_pattern,chara_rank"
	);
	if (error) console.error("❌ Error growth_tables:", error.message || error);
	else console.log(`✅ Growth Tables imported (${records.length} rows).`);
}

async function importCapsules(_service: any, db: DataAdapter) {
	console.log("🔄 Importing Capsules...");
	const { buildCapsuleDatabase } = await import("./parsers/capsule-config.js");
	const capsule = await buildCapsuleDatabase();

	const records = capsule.prizes.map((p: any) => ({
		id: p.id,
		prize_data: p.vars,
		data: p,
		updated_at: new Date().toISOString(),
	}));

	const BATCH_SIZE = 100;
	let successCount = 0;
	for (let i = 0; i < records.length; i += BATCH_SIZE) {
		const batch = records.slice(i, i + BATCH_SIZE);
		const { error } = await db.upsert("inagle_capsules", batch);
		if (error) console.error("❌ Error capsules:", error.message || error);
		else successCount += batch.length;
	}
	console.log(`✅ Capsules imported (${successCount}).`);
}

async function exportStoryTextDatabase() {
	console.log("🔄 Exporting Story Text Database...");
	const { buildStoryTextDatabase } = await import("./parsers/story-text-parser.js");
	const db = buildStoryTextDatabase();

	const { PATHS } = await import("./core/paths.js");
	const { writeFileSync } = await import("node:fs");

	const storyTextPath = path.join(PATHS.allGamedata, "story_text_database.json");
	try {
		writeFileSync(storyTextPath, JSON.stringify(db, null, 2), "utf-8");
		console.log(`✅ Story Text Database written to ${storyTextPath}`);
	} catch (error: any) {
		// Le dump du jeu est monté en lecture seule sur le VPS : cette écriture y échoue
		// en EROFS chaque nuit. C'est un export de confort (fichier statique dérivé), pas
		// une écriture en base — le faire remonter en « Fatal error during push » masquait
		// les vraies pannes du push dans le journal, et le shell rendait quand même 0.
		if (error?.code === "EROFS" || error?.code === "EACCES") {
			console.warn(
				`⚠️  Story Text Database non écrite (${error.code} sur ${storyTextPath}) — dump monté en lecture seule, export ignoré.`
			);
			return;
		}
		throw error;
	}
}
