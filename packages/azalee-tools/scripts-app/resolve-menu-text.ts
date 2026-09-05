#!/usr/bin/env bun
/**
 * Post-process DURABLE des layouts de menu (`app/menu/_layouts/*.json`).
 *
 * Problème : chaque node texte exporté par `MenuLayoutExporter` (iecode) porte un
 * `value` = la CLÉ brute du slot (`_text_btn01`, `_text_crossplay01`, …) parce que
 * l'exporteur tourne sans `TextResolver`. Le renderer WebGPU affiche donc la clé.
 *
 * Réalité (vérifiée sur le vrai dump) :
 *   - `node.keyHash` est un hash d'objet/layer du squelette OBJB, PAS un hash de clé
 *     texte → il ne résout PAS dans `common/text/<lg>/menu_text.cfg.bin.json`
 *     (0 / 340 keyHash distincts matchent ; CRC32 du nom de slot non plus).
 *   - Les noms de slot (`_text_list01_on`, `_text_name01`, `_text_btn01`, …) sont des
 *     TEMPLATES de liste/bouton remplis au RUNTIME par le moteur (noms de joueurs,
 *     objets, lignes de liste…). Il n'existe AUCUN pont statique hash→libellé.
 *
 * Donc la résolution honnête est double :
 *   1. CURATION : une petite table `SLOT_LABELS` (nom de slot → libellé FR) pour les
 *      rares slots VRAIMENT statiques. Chaque libellé FR est ré-cherché dans
 *      `menu_text` → le triplet `{fr,en,ja}` vient des VRAIES données (zéro
 *      traduction inventée ; un libellé absent de menu_text est ignoré + warn).
 *   2. FALLBACK : pour tout slot non curé (= dynamique runtime), on « humanise » le
 *      nom de slot (`_text_team_name01` → « Team name ») au lieu de montrer la clé
 *      brute. Le fallback est marqué identique sur les 3 locales (placeholder, pas
 *      une fausse traduction).
 *
 * Sortie : réécrit les 32 `_layouts/*.json` en place avec, sur chaque node texte :
 *   - `value`     = libellé FR résolu (curé) ou humanisé (fallback)
 *   - `localized` = { fr, en, ja } (triplet réel si curé, humanisé sinon)
 *
 * Idempotent : relisible/relançable (réécrit toujours depuis le `key`, pas depuis le
 * `value` déjà résolu). À relancer après chaque ré-export iecode des layouts.
 *
 * Run : `bun scripts/resolve-menu-text.ts`
 */

import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const TEXT_ROOT = process.env.IEVR_TEXT_ROOT
	?? `${process.env.DATA_PATH ?? "/home/ubuntu/niers/data"}/common/text`;
const LAYOUTS_DIR = join(import.meta.dir, "..", "app", "menu", "_layouts");
const LOCALES = ["fr", "en", "ja"] as const;
type Locale = (typeof LOCALES)[number];

/* ── Structure menu_text.cfg.bin.json ──────────────────────────────────────────
 * { entries: [ { name, variables, children: [ { name: "TEXT_INFO_N",
 *   variables: [ {type:"Int", value:<hash signé>}, {type:"Int"}, {type:"String", value:<label>} ] } ] } ] }
 * Le hash (Int signé) est le CRC32 d'une clé texte INCONNUE (autre namespace que les
 * slots OBJB) ; on l'indexe quand même mais on cure par LIBELLÉ FR (cf. SLOT_LABELS).
 */
interface CfgVar {
	type: string;
	value: string;
}
interface CfgNode {
	name: string;
	variables?: CfgVar[];
	children?: CfgNode[];
}
interface CfgRoot {
	entries: CfgNode[];
}

/** Triplet localisé d'un libellé. */
interface LocalizedLabel {
	fr: string;
	en: string;
	ja: string;
}

/**
 * Index d'une locale : `hash(u32) → label`, et `label → hash(u32)`. On garde les deux
 * pour pouvoir aligner un libellé FR sur le même hash dans en/ja (alignement par hash
 * = garanti par la structure : la N-ième entrée a le même hash dans les 3 locales).
 */
interface LocaleIndex {
	byHash: Map<number, string>;
	byLabel: Map<string, number>;
}

async function loadLocale(locale: Locale): Promise<LocaleIndex> {
	const path = join(TEXT_ROOT, locale, "menu_text.cfg.bin.json");
	const raw = JSON.parse(await readFile(path, "utf8")) as CfgRoot;
	const byHash = new Map<number, string>();
	const byLabel = new Map<string, number>();
	for (const top of raw.entries ?? []) {
		for (const kid of top.children ?? []) {
			const vars = kid.variables ?? [];
			const first = vars[0];
			if (!first || first.type !== "Int") continue;
			// hash signé Int32 → u32
			const hash = Number.parseInt(first.value, 10) >>> 0;
			const label = vars.find((v) => v.type === "String" && v.value)?.value;
			if (!label) continue;
			byHash.set(hash, label);
			if (!byLabel.has(label)) byLabel.set(label, hash);
		}
	}
	return { byHash, byLabel };
}

/**
 * Table de CURATION : nom de slot OBJB → libellé FR (qui DOIT exister dans menu_text).
 *
 * Conservatrice par design : on ne cure QUE les slots dont le libellé est statique et
 * non ambigu (guides de boutons, commandes communes, en-têtes d'écran fixes). Tous les
 * `_text_list*`, `_text_name*`, `_text_item*`, `_text_*_on/_off` (templates de liste
 * remplis au runtime) sont laissés au fallback humanisé — les curer reviendrait à
 * halluciner un contenu dynamique.
 *
 * Le libellé FR est ré-résolu dans menu_text à l'exécution → en/ja viennent du même
 * hash (vraies données). Si un libellé n'existe pas dans menu_text, l'entrée est
 * ignorée (warn) plutôt que d'injecter une traduction inventée.
 */
const SLOT_LABELS: Readonly<Record<string, string>> = {
	// Commandes / guides de boutons communs (présents sur de nombreux écrans).
	_text_crossplay01: "Multijoueur",
	_text_cmd_back01: "Retour",
	_text_cmd_back01_on: "Retour",
	_text_cmd_cancel01: "Annuler",
	_text_cmd_decide01: "Confirmer",
	_text_cmd_select01: "Sélectionner",
	_text_btn_back01: "Retour",
	_text_btn_cancel01: "Annuler",
	// En-têtes d'écran fixes (le menu principal expose ces sections statiques).
	_text_team01: "Équipe",
	_text_item01_title: "Objets",
	_text_shop_title01: "Boutique",
	_text_option_title01: "Options",
	_text_map_title01: "Carte",
	_text_story_title01: "Histoire",
	_text_quest_title01: "Quête",
	_text_save_title01: "Sauvegarder",
	_text_formation_title01: "Formation",
	// Détails / pagination.
	_text_detail01: "Détails",
	_text_detail_title01: "Détails",
	_text_next01: "Suivant",
};

/* ── Humanisation des slots dynamiques (fallback propre, jamais la clé brute) ──── */

/**
 * Transforme un nom de slot en libellé lisible :
 *   `_text_team_name01`     → « Team name »
 *   `_text_option_list01_on`→ « Option list »
 *   `_text_btn01_l`         → « Btn »
 * Strip : préfixe `_text_`, suffixes d'état (`_on/_off/_l/_r/_u/_d`), digits, `_`.
 */
function humanizeSlot(slot: string): string {
	let s = slot.replace(/^_?text_/i, "");
	// retire les suffixes d'état/position en fin (répétables : `01_on`, `_l`, …)
	s = s.replace(/(_(on|off|l|r|u|d|active|select|sel))+$/gi, "");
	// retire les groupes de chiffres (indices de slot)
	s = s.replace(/\d+/g, " ");
	// underscores → espaces, compacte
	s = s.replace(/_/g, " ").replace(/\s+/g, " ").trim();
	if (!s) return "";
	return s.charAt(0).toUpperCase() + s.slice(1);
}

/* ── Nodes de layout ───────────────────────────────────────────────────────────── */

interface MenuTextNode {
	key: string;
	keyHash: number;
	value: string;
	localized?: LocalizedLabel;
	[k: string]: unknown;
}

/** Un node est un node texte s'il porte `key`, `keyHash` et `value` (string). */
function isTextNode(o: unknown): o is MenuTextNode {
	return (
		typeof o === "object"
		&& o !== null
		&& typeof (o as Record<string, unknown>).key === "string"
		&& typeof (o as Record<string, unknown>).keyHash === "number"
		&& typeof (o as Record<string, unknown>).value === "string"
	);
}

interface ResolveStats {
	total: number;
	curated: number;
	fallback: number;
	empty: number;
}

/**
 * Résout un node texte en place (toujours depuis `key`, pas depuis `value`).
 * Retourne la catégorie de résolution (pour les stats).
 */
function resolveNode(
	node: MenuTextNode,
	idx: Record<Locale, LocaleIndex>,
): "curated" | "fallback" | "empty" {
	const slot = node.key;
	const labelFr = SLOT_LABELS[slot];

	if (labelFr) {
		const hash = idx.fr.byLabel.get(labelFr);
		if (hash !== undefined) {
			// Triplet réel : même hash dans en/ja (repli FR si une locale manque l'entrée).
			const localized: LocalizedLabel = {
				fr: labelFr,
				en: idx.en.byHash.get(hash) ?? labelFr,
				ja: idx.ja.byHash.get(hash) ?? labelFr,
			};
			node.value = localized.fr;
			node.localized = localized;
			return "curated";
		}
		// Libellé curé mais absent de menu_text → on ne fabrique pas de traduction.
		console.warn(`[resolve-menu-text] libellé curé absent de menu_text: ${slot} → ${labelFr}`);
	}

	// Fallback humanisé (slot dynamique runtime) — jamais la clé brute.
	const human = humanizeSlot(slot);
	node.value = human;
	node.localized = { fr: human, en: human, ja: human };
	return human ? "fallback" : "empty";
}

/** Parcourt récursivement un layout et résout tous les nodes texte. */
function walkAndResolve(
	o: unknown,
	idx: Record<Locale, LocaleIndex>,
	stats: ResolveStats,
): void {
	if (Array.isArray(o)) {
		for (const x of o) walkAndResolve(x, idx, stats);
		return;
	}
	if (typeof o !== "object" || o === null) return;
	if (isTextNode(o)) {
		stats.total += 1;
		const cat = resolveNode(o, idx);
		if (cat === "curated") stats.curated += 1;
		else if (cat === "fallback") stats.fallback += 1;
		else stats.empty += 1;
	}
	for (const v of Object.values(o as Record<string, unknown>)) {
		walkAndResolve(v, idx, stats);
	}
}

async function main(): Promise<void> {
	const idx = {
		fr: await loadLocale("fr"),
		en: await loadLocale("en"),
		ja: await loadLocale("ja"),
	} as const;
	console.log(
		`menu_text chargé: fr=${idx.fr.byHash.size} en=${idx.en.byHash.size} ja=${idx.ja.byHash.size} entrées`,
	);

	const files = (await readdir(LAYOUTS_DIR)).filter((f) => f.endsWith(".json"));
	const totals: ResolveStats = { total: 0, curated: 0, fallback: 0, empty: 0 };

	for (const file of files.sort()) {
		const path = join(LAYOUTS_DIR, file);
		const layout = JSON.parse(await readFile(path, "utf8"));
		const stats: ResolveStats = { total: 0, curated: 0, fallback: 0, empty: 0 };
		walkAndResolve(layout, idx, stats);
		await writeFile(path, `${JSON.stringify(layout, null, "\t")}\n`, "utf8");
		totals.total += stats.total;
		totals.curated += stats.curated;
		totals.fallback += stats.fallback;
		totals.empty += stats.empty;
		console.log(
			`${file.padEnd(20)} texte=${stats.total} curé=${stats.curated} humanisé=${stats.fallback} vide=${stats.empty}`,
		);
	}

	console.log(
		`\nTOTAL: ${totals.total} nodes texte — curé=${totals.curated} humanisé=${totals.fallback} vide=${totals.empty}`,
	);
}

await main();
