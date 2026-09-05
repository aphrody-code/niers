#!/usr/bin/env bun
/**
 * Génère l'index de texte de jeu IEVR consommé par azalee pour afficher du VRAI
 * texte du jeu (noms perso/objet/technique, descriptions, dialogues…) au lieu
 * des hashId bruts.
 *
 * Source de vérité = les dictionnaires `*.cfg.bin.json` décodés du dump
 * (`<DATA_ROOT>/common/text/<lang>/`), parsés via la machinerie EXISTANTE
 * `@rosegriffon/inagle` (`loadTextFileRaw` — gère TEXT_INFO et NOUN_INFO, toutes
 * les locales). Le hashId (`crc32` hex d'une clé de texte) est la clé de jointure
 * réutilisée partout dans les configs gamedata.
 *
 * STRATÉGIE D'ARTEFACT (même pattern que `build-cpk-index.ts`) :
 *  - `data/game-text-names.ndjson.gz`  — TRACKÉ git. Les 43 dicts racine (noms,
 *    descriptions, menus…) dans les 3 langues décodées (fr/en/ja). Compact, c'est
 *    ce que les pages wiki résolvent.
 *  - `data/game-text-dialogue.ndjson.gz` — POTENTIELLEMENT gitignoré si gros. Les
 *    ~11 700 fichiers `event/` + `map/`/`phase/`/`purpose/` (dialogues histoire,
 *    fr/en/ja). C'est « tout le dossier text ».
 *
 * La lib runtime (`lib/game-text/index.ts`) MATÉRIALISE ces NDJSON en une table
 * SQLite `text(hashId, locale, category, value)` sur /tmp au 1er accès (singleton
 * process-local). Robuste au build standalone fresh-checkout (pas de gros binaire
 * en git, pas de symlink — piège Turbopack hors-racine).
 *
 * Run : `bun apps/azalee/scripts/build-game-text-index.ts`
 *   --data=<path>   racine du dump (défaut: $DATA_PATH ou /home/ubuntu/niers/data)
 *   --out-dir=<dir> dossier de sortie (défaut: apps/azalee/data)
 *   --names-only    n'écrit que l'artefact noms (saute event/map/phase/purpose)
 *   --sqlite        écrit AUSSI un .sqlite plein (debug)
 */
import { Database } from "bun:sqlite";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { gzipSync } from "node:zlib";
import { loadTextFileRaw } from "@rosegriffon/inagle/parsers/universal-text";
import { formatGameText } from "../lib/game-text/format";
import { buildSqliteFromEntries, type TextEntry } from "@rosegriffon/azalee/game-text/materialize";

// --- Catégorisation des dicts racine ---------------------------------------
//
// `category` = domaine fonctionnel dérivé du nom du dict (sans `_text`). Sert de
// facette de filtrage côté wiki. Les `_roma` (romaji) gardent un suffixe distinct.
const ROOT_CATEGORY: Record<string, string> = {
	ai_text: "ai",
	chara_add_info_text: "chara_add_info",
	chara_description_text: "chara_description",
	chara_text: "chara",
	chara_text_roma: "chara_roma",
	chat_text: "chat",
	craft_text: "craft",
	data_file_text: "data_file",
	extend_story_text: "extend_story",
	help_list_text: "help",
	inacode_text: "inacode",
	item_text: "item",
	map_text: "map_name",
	map_text_roma: "map_name_roma",
	medal_text: "medal",
	menu_text: "menu",
	mission_text: "mission",
	music_name_text: "music",
	players_universe_text: "players_universe",
	post_text: "post",
	quest_purpose_text: "quest_purpose",
	quest_title_text: "quest_title",
	rpg_battle_cmd_text: "rpg_battle_cmd",
	rpg_battle_message_text: "rpg_battle_message",
	rpg_battle_text: "rpg_battle",
	scene_archive_text: "scene_archive",
	scout_phase_text: "scout_phase",
	search_word_text: "search",
	setting_text: "setting",
	shop_text: "shop",
	skill_text: "skill",
	soccer_common_text: "soccer_common",
	soccer_game_title: "soccer_game_title",
	soccer_history_check_text: "soccer_history_check",
	soccer_quick_action_text: "soccer_quick_action",
	soccer_suggest_text: "soccer_suggest",
	soccer_team_passive_text: "soccer_team_passive",
	soccer_technic_text: "technic",
	staffroll_text: "staffroll",
	system_text: "system",
	team_text: "team",
	theater_text: "theater",
	trophy_text: "trophy",
	// Présents uniquement en `ja/` (5-7 dicts supplémentaires) — indexés quand même.
	battle_skill_text: "battle_skill",
	battle_text: "battle",
	chara_nickname_text: "chara_nickname",
	common_talk_text: "common_talk",
	dictionary_text: "dictionary",
	ng_word_text: "ng_word",
	power_site_text: "power_site",
};

/** Locales décodées disponibles dans le dump (les autres n'ont pas de `.json`). */
const LOCALES = ["fr", "en", "ja"] as const;

interface Args {
	dataRoot: string;
	outDir: string;
	namesOnly: boolean;
	sqlite: boolean;
}

function parseArgs(argv: string[]): Args {
	const get = (flag: string): string | undefined => {
		const hit = argv.find((a) => a.startsWith(`${flag}=`));
		return hit?.slice(flag.length + 1);
	};
	const scriptDir = path.dirname(new URL(import.meta.url).pathname);
	const appRoot = path.resolve(scriptDir, "..");
	return {
		dataRoot: path.resolve(get("--data") ?? process.env.DATA_PATH ?? "/home/ubuntu/niers/data"),
		outDir: path.resolve(get("--out-dir") ?? path.join(appRoot, "data")),
		namesOnly: argv.includes("--names-only"),
		sqlite: argv.includes("--sqlite"),
	};
}

/**
 * Priorité de résolution en cas de collision de hashId entre dicts (la PK
 * `(hashId, locale)` ne garde qu'une valeur ; on insère les dicts prioritaires
 * en PREMIER → `INSERT OR IGNORE` les conserve). Un même hashId apparaît p.ex.
 * dans `chara` (« Mark ») ET `chara_roma` (« Mamoru ») : on veut le nom localisé,
 * pas le romaji. Plus le poids est BAS, plus le dict est prioritaire.
 */
function rootPriority(dict: string): number {
	if (dict.endsWith("_roma")) return 90; // romaji = repli, jamais prioritaire
	if (dict === "chara_text") return 0;
	if (dict === "item_text") return 1;
	if (dict === "skill_text") return 1;
	if (dict === "soccer_technic_text") return 1;
	if (dict === "team_text") return 1;
	if (dict === "menu_text") return 2;
	return 10;
}

/** Catégorie d'un fichier de sous-dossier dialogue (event/map/phase/purpose). */
function dialogueCategory(sub: string): string {
	switch (sub) {
		case "event":
			return "event-dialogue";
		case "map":
			return "map-npc";
		case "phase":
			return "phase";
		case "purpose":
			return "purpose";
		default:
			return sub;
	}
}

/**
 * Collecte toutes les entrées texte d'un ensemble de dicts racine pour toutes les
 * locales décodées. `dictToCategory` mappe le nom de fichier (sans extension) sur
 * sa catégorie.
 */
function collectRoot(dataRoot: string): { entries: TextEntry[]; perCat: Map<string, number> } {
	const textDir = path.join(dataRoot, "common", "text");
	const entries: TextEntry[] = [];
	const perCat = new Map<string, number>();

	// Union des dicts présents sur toutes les locales (ja en a 7 de plus).
	const dictNames = new Set<string>();
	for (const loc of LOCALES) {
		const dir = path.join(textDir, loc);
		if (!existsSync(dir)) continue;
		for (const f of readdirSync(dir)) {
			if (f.endsWith(".cfg.bin.json")) dictNames.add(f.replace(".cfg.bin.json", ""));
		}
	}

	// Ordre prioritaire : les dicts sémantiques (chara/item/skill…) avant `_roma`
	// et les génériques, pour que `INSERT OR IGNORE` conserve le bon texte.
	const orderedDicts = [...dictNames].sort((a, b) => rootPriority(a) - rootPriority(b) || a.localeCompare(b));

	for (const dict of orderedDicts) {
		const category = ROOT_CATEGORY[dict] ?? dict.replace(/_text$/, "");
		for (const loc of LOCALES) {
			const fp = path.join(textDir, loc, `${dict}.cfg.bin.json`);
			if (!existsSync(fp)) continue;
			const map = loadTextFileRaw(fp);
			for (const [hashId, value] of map) {
				entries.push([hashId, loc, category, formatGameText(value)]);
				perCat.set(category, (perCat.get(category) ?? 0) + 1);
			}
		}
	}

	return { entries, perCat };
}

/**
 * Collecte les dialogues des sous-dossiers (`event`, `map`, `phase`, `purpose`)
 * pour toutes les locales décodées. Volumineux (~11 700 fichiers).
 */
function collectDialogue(dataRoot: string): { entries: TextEntry[]; perCat: Map<string, number>; files: number } {
	const textDir = path.join(dataRoot, "common", "text");
	const entries: TextEntry[] = [];
	const perCat = new Map<string, number>();
	const SUBS = ["event", "map", "phase", "purpose"];
	let files = 0;

	for (const loc of LOCALES) {
		for (const sub of SUBS) {
			const dir = path.join(textDir, loc, sub);
			if (!existsSync(dir)) continue;
			const category = dialogueCategory(sub);
			for (const f of readdirSync(dir)) {
				if (!f.endsWith(".cfg.bin.json")) continue;
				files++;
				const map = loadTextFileRaw(path.join(dir, f));
				for (const [hashId, value] of map) {
					entries.push([hashId, loc, category, formatGameText(value)]);
					perCat.set(category, (perCat.get(category) ?? 0) + 1);
				}
			}
		}
	}

	return { entries, perCat, files };
}

/** Écrit un artefact NDJSON gzippé (`[hashId, locale, category, value]` par ligne). */
function writeNdjsonGz(outPath: string, entries: TextEntry[]): number {
	const ndjson = entries.map((e) => JSON.stringify(e)).join("\n");
	const gz = gzipSync(Buffer.from(ndjson, "utf8"), { level: 9 });
	writeFileSync(outPath, gz);
	return gz.length;
}

function fmt(n: number): string {
	return n.toLocaleString("fr-FR");
}

async function main() {
	const args = parseArgs(process.argv.slice(2));
	if (!existsSync(path.join(args.dataRoot, "common", "text"))) {
		console.error(`[game-text] dump introuvable: ${args.dataRoot}/common/text`);
		process.exit(1);
	}
	mkdirSync(args.outDir, { recursive: true });

	console.log(`[game-text] dump = ${args.dataRoot}`);
	console.log(`[game-text] locales = ${LOCALES.join(", ")}`);

	// --- Artefact NOMS (43+ dicts racine) ---
	console.log("[game-text] collecte des dicts racine…");
	const root = collectRoot(args.dataRoot);
	const namesPath = path.join(args.outDir, "game-text-names.ndjson.gz");
	const namesBytes = writeNdjsonGz(namesPath, root.entries);
	console.log(
		`[game-text] NOMS → ${path.relative(process.cwd(), namesPath)} : ${fmt(root.entries.length)} entrées, ${fmt(namesBytes)} o`,
	);
	const sortedCats = [...root.perCat.entries()].sort((a, b) => b[1] - a[1]);
	for (const [cat, n] of sortedCats) console.log(`    ${cat.padEnd(22)} ${fmt(n)}`);

	// --- Artefact DIALOGUE (event/map/phase/purpose) ---
	let dialogue: { entries: TextEntry[]; perCat: Map<string, number>; files: number } | null = null;
	if (!args.namesOnly) {
		console.log("[game-text] collecte des dialogues (event/map/phase/purpose)…");
		dialogue = collectDialogue(args.dataRoot);
		const dialPath = path.join(args.outDir, "game-text-dialogue.ndjson.gz");
		const dialBytes = writeNdjsonGz(dialPath, dialogue.entries);
		console.log(
			`[game-text] DIALOGUE → ${path.relative(process.cwd(), dialPath)} : ${fmt(dialogue.entries.length)} entrées, ${fmt(dialogue.files)} fichiers, ${fmt(dialBytes)} o`,
		);
		for (const [cat, n] of [...dialogue.perCat.entries()].sort((a, b) => b[1] - a[1])) {
			console.log(`    ${cat.padEnd(22)} ${fmt(n)}`);
		}
	}

	// --- SQLite plein optionnel (debug) ---
	if (args.sqlite) {
		const all = [...root.entries, ...(dialogue?.entries ?? [])];
		const dbPath = path.join(args.outDir, "game-text.sqlite");
		const db = new Database(dbPath);
		const res = buildSqliteFromEntries(db, all);
		db.close();
		console.log(
			`[game-text] SQLite debug → ${path.relative(process.cwd(), dbPath)} : ${fmt(res.inserted)} insérées, ${fmt(res.ignored)} collisions ignorées`,
		);
	}

	// --- Stats globales (hashId uniques, langues) ---
	const allEntries = [...root.entries, ...(dialogue?.entries ?? [])];
	const uniqueHashes = new Set(allEntries.map((e) => e[0]));
	const perLocale = new Map<string, number>();
	for (const e of allEntries) perLocale.set(e[1], (perLocale.get(e[1]) ?? 0) + 1);
	console.log("[game-text] === STATS ===");
	console.log(`    entrées totales : ${fmt(allEntries.length)}`);
	console.log(`    hashId uniques  : ${fmt(uniqueHashes.size)}`);
	for (const [loc, n] of [...perLocale.entries()].sort()) console.log(`    locale ${loc.padEnd(4)}: ${fmt(n)}`);
}

await main();
