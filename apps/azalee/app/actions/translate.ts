"use server";

import { isKana, toKatakana, toHiragana } from "wanakana";
import { japaneseToRomaji } from "@rosegriffon/azalee/text/japanese-romaji";
import { createClient } from "@/lib/supabase/server";
import { TACTIC_FR } from "@rosegriffon/azalee/text/translations";

export interface TranslationResult {
	id: string;
	type: string;
	typeLabel: string;
	name_fr: string | null;
	name_en: string | null;
	name_ja: string | null;
	name_roma: string | null;
	url: string;
	/** Score de pertinence interne (0-1). Optionnel : ajouté pour le ranking. */
	score?: number;
	/** true si ce résultat provient d'une passe fuzzy (approximative). */
	fuzzy?: boolean;
}

const ENTITY_TYPE_CONFIG: Record<string, { label: string; urlPrefix: string }> = {
	character: { label: "Personnage", urlPrefix: "/chara" },
	item: { label: "Objet", urlPrefix: "/item" },
	keshin: { label: "Esprit Guerrier", urlPrefix: "/aura/esprits-guerriers" },
	skill: { label: "Technique", urlPrefix: "/skill" },
	soul: { label: "Totem", urlPrefix: "/aura/totems" },
	tactic: { label: "Tactique", urlPrefix: "/tactic" },
	team: { label: "Équipe", urlPrefix: "/team" },
};

// ──────────────────────────────────────────────────────────────────────────
// Normalisation & similarité — logique pure (testable hors Next)
// ──────────────────────────────────────────────────────────────────────────

/**
 * Normalise une chaîne pour le matching tolérant :
 * minuscule, suppression des accents/diacritiques (NFD + strip combining),
 * suppression de la ponctuation, espaces multiples → un espace, trim.
 */
function normalizeForSearch(input: string): string {
	return (
		input
			.normalize("NFD")
			// Supprime les marques diacritiques combinantes (accents).
			.replaceAll(/[̀-ͯ]/g, "")
			.toLowerCase()
			// Ponctuation / symboles → espace (garde lettres, chiffres, kana, kanji).
			.replaceAll(/[^\p{L}\p{N}]+/gu, " ")
			.replaceAll(/\s+/g, " ")
			.trim()
	);
}

/** Découpe une requête normalisée en tokens non vides. */
function tokenize(input: string): string[] {
	const norm = normalizeForSearch(input);
	if (!norm) return [];
	return norm.split(" ").filter(Boolean);
}

/**
 * Distance de Levenshtein (édition) entre deux chaînes — implémentation O(n·m)
 * à deux lignes, sans dépendance.
 */
function levenshtein(a: string, b: string): number {
	if (a === b) return 0;
	if (a.length === 0) return b.length;
	if (b.length === 0) return a.length;

	let prev = new Array<number>(b.length + 1);
	let curr = new Array<number>(b.length + 1);
	for (let j = 0; j <= b.length; j++) prev[j] = j;

	for (let i = 1; i <= a.length; i++) {
		curr[0] = i;
		const ca = a.charCodeAt(i - 1);
		for (let j = 1; j <= b.length; j++) {
			const cost = ca === b.charCodeAt(j - 1) ? 0 : 1;
			const del = prev[j]! + 1;
			const ins = curr[j - 1]! + 1;
			const sub = prev[j - 1]! + cost;
			curr[j] = del < ins ? (del < sub ? del : sub) : ins < sub ? ins : sub;
		}
		const tmp = prev;
		prev = curr;
		curr = tmp;
	}
	return prev[b.length]!;
}

/** Similarité normalisée 0-1 dérivée de la distance de Levenshtein. */
function similarity(a: string, b: string): number {
	if (!a && !b) return 1;
	const maxLen = Math.max(a.length, b.length);
	if (maxLen === 0) return 1;
	return 1 - levenshtein(a, b) / maxLen;
}

/**
 * Score de pertinence d'un nom candidat vis-à-vis d'une requête, tous deux
 * déjà normalisés. Combine match exact / préfixe / inclusion / tokens (AND
 * dans le désordre) / similarité fuzzy par fenêtre.
 * Retourne une valeur dans [0, 1].
 */
function nameScore(nameNorm: string, queryNorm: string, queryTokens: string[]): number {
	if (!nameNorm) return 0;
	if (nameNorm === queryNorm) return 1;
	if (nameNorm.startsWith(queryNorm)) return 0.92;
	if (nameNorm.includes(queryNorm)) return 0.82;

	const nameTokens = nameNorm.split(" ").filter(Boolean);

	// Multi-mots : tous les tokens de la requête présents (AND, ordre libre),
	// en match exact OU préfixe d'un token du nom.
	if (queryTokens.length > 0) {
		let matchedAll = true;
		let prefixOnly = false;
		for (const qt of queryTokens) {
			const exact = nameTokens.includes(qt);
			const asPrefix = !exact && nameTokens.some((nt) => nt.startsWith(qt) || qt.startsWith(nt));
			if (!exact && !asPrefix) {
				matchedAll = false;
				break;
			}
			if (!exact) prefixOnly = true;
		}
		if (matchedAll) return prefixOnly ? 0.72 : 0.78;
	}

	// Fuzzy : meilleure similarité de la requête entière contre le nom complet
	// et contre chaque token du nom (gère "endo" vs "endou mamoru").
	let best = similarity(queryNorm, nameNorm);
	for (const nt of nameTokens) {
		const s = similarity(queryNorm, nt);
		if (s > best) best = s;
	}
	// Bonus si chaque token de requête a un token de nom proche (fautes de frappe).
	if (queryTokens.length > 0) {
		let sum = 0;
		for (const qt of queryTokens) {
			let bt = 0;
			for (const nt of nameTokens) {
				const s = similarity(qt, nt);
				if (s > bt) bt = s;
			}
			sum += bt;
		}
		const tokenAvg = sum / queryTokens.length;
		if (tokenAvg > best) best = tokenAvg;
	}
	return best * 0.7; // plafond plus bas pour les matches purement fuzzy
}

/** Seuil minimal de similarité pour retenir un candidat fuzzy. */
const FUZZY_THRESHOLD = 0.62;

/**
 * Calcule le meilleur score d'un résultat (toutes langues + romaji dérivé).
 */
function scoreResult(r: TranslationResult, queryNorm: string, queryTokens: string[]): number {
	const candidates: (string | null)[] = [
		r.name_fr,
		r.name_en,
		r.name_roma,
		// Le japonais brut matche rarement la requête latine — on garde quand
		// même pour les recherches en kana.
		r.name_ja,
	];
	let best = 0;
	for (const c of candidates) {
		if (!c) continue;
		const s = nameScore(normalizeForSearch(c), queryNorm, queryTokens);
		if (s > best) best = s;
	}
	return best;
}

// ──────────────────────────────────────────────────────────────────────────
// Glossaire local
// ──────────────────────────────────────────────────────────────────────────

/**
 * Le glossaire de traduction — désactivé en serverless, et qui le DIT.
 *
 * ## Ce qui était là, et pourquoi c'était dangereux
 *
 * Une cascade cherchait `glossary.json` à trois endroits, dont un chemin absolu figé sur la
 * machine de développement, puis rendait `null` sans un mot. Or ce fichier (2,9 Mo) est absent
 * de l'index git : sur
 * Vercel, les trois chemins échouent. La recherche rendait donc une liste vide — pas une
 * traduction dégradée, une fonctionnalité qui disparaît en silence, sans erreur ni journal.
 *
 * ## Ce qui le remplacera
 *
 * Les quatre catégories du glossaire (auras, personnages, objets, passifs) recouvrent des
 * tables `inagle_*` déjà présentes en base. Le reconstruire depuis elles est la bonne voie —
 * mais elle demande de connaître leurs colonnes exactes, et un nom inventé compile en rendant
 * `null` en silence, ce qui recréerait précisément le défaut qu'on corrige ici.
 *
 * En attendant, l'absence est EXPLICITE : `searchGlossary` rend un tableau vide, et
 * `GLOSSAIRE_INDISPONIBLE` permet à l'appelant de distinguer « aucun résultat » de « cette
 * source n'existe pas ici ». Un échec qu'on peut lire vaut mieux qu'un vide qu'on croit
 * complet.
 */
export const GLOSSAIRE_INDISPONIBLE =
	"Glossaire local indisponible : cette instance n'embarque pas `glossary.json`. " +
	"La recherche par glossaire reprendra quand il sera servi depuis la base.";

async function loadGlossary(): Promise<Record<string, unknown> | null> {
	return null;
}

interface GlossaryItem {
	ja?: string;
	en?: string;
	fr?: string;
	code?: string;
	category?: string;
	element?: string;
	subType?: string;
	boostType?: string;
}

/**
 * Recherche dans le cache glossaire local. Tolérante : matche sur version
 * normalisée (accent-insensible), kana ↔ romaji dans les deux sens, et en
 * dernier recours une passe fuzzy sous seuil de similarité.
 */
async function searchGlossary(query: string, entityType?: string): Promise<TranslationResult[]> {
	const glossary = await loadGlossary();
	if (!glossary) return [];

	const queryNorm = normalizeForSearch(query);
	if (!queryNorm) return [];
	const queryTokens = tokenize(query);

	// Candidats kana à partir de la saisie latine (romaji → kana).
	const hiraCandidate = toHiragana(queryNorm).toLowerCase();
	const kataCandidate = toKatakana(queryNorm).toLowerCase();
	const isValidHira =
		hiraCandidate.length > 0 && [...hiraCandidate].every((c) => isKana(c) || c === "ー");
	const isValidKata =
		kataCandidate.length > 0 && [...kataCandidate].every((c) => isKana(c) || c === "ー");

	const results: TranslationResult[] = [];

	const searchCategory = (catName: string, type: string, typeLabel: string, urlPrefix: string) => {
		if (
			entityType &&
			entityType !== type &&
			!(type === "aura" && (entityType === "keshin" || entityType === "soul"))
		) {
			return;
		}
		const list = (glossary[catName] || []) as GlossaryItem[];
		for (const entry of list) {
			const enNorm = normalizeForSearch(entry.en || "");
			const frNorm = normalizeForSearch(entry.fr || "");
			const jaLower = (entry.ja || "").toLowerCase();
			const romaNorm = entry.ja ? normalizeForSearch(japaneseToRomaji(entry.ja) || "") : "";

			// Score « solide » (exact/prefix/token/contains) sur les noms latins + romaji.
			let score = 0;
			for (const cand of [frNorm, enNorm, romaNorm]) {
				if (!cand) continue;
				const s = nameScore(cand, queryNorm, queryTokens);
				if (s > score) score = s;
			}

			// Match kana direct (recherche romaji → kana ou saisie kana).
			if (score < 0.82 && entry.ja) {
				if (
					jaLower.includes(queryNorm) ||
					(isValidHira && jaLower.includes(hiraCandidate)) ||
					(isValidKata && jaLower.includes(kataCandidate))
				) {
					score = Math.max(score, 0.82);
				}
			}

			if (score < FUZZY_THRESHOLD) continue;

			const id = entry.code || entry.en || entry.ja || Math.random().toString();
			let finalType = type;
			let finalTypeLabel = typeLabel;
			let finalUrl = `${urlPrefix}/${id}`;

			if (type === "aura") {
				const isSoul =
					entry.subType === "soul" ||
					(entry.subType && entry.subType.toLowerCase().includes("soul"));
				finalType = isSoul ? "soul" : "keshin";
				finalTypeLabel = isSoul ? "Totem" : "Esprit Guerrier";
				finalUrl = isSoul ? `/aura/totems/${id}` : `/aura/esprits-guerriers/${id}`;
				if (entityType && entityType !== finalType) continue;
			}

			results.push({
				id,
				type: finalType,
				typeLabel: finalTypeLabel,
				name_fr: entry.fr || entry.en || null,
				name_en: entry.en || null,
				name_ja: entry.ja || null,
				name_roma: entry.ja ? japaneseToRomaji(entry.ja) : null,
				url: finalUrl,
				score,
				fuzzy: score < 0.78,
			});
		}
	};

	searchCategory("characters", "character", "Personnage", "/chara");
	searchCategory("techniques", "skill", "Technique", "/skill");
	searchCategory("auras", "aura", "Aura", "/aura");
	searchCategory("passives", "skill", "Technique", "/skill");
	searchCategory("teams", "team", "Équipe", "/team");
	searchCategory("items", "item", "Objet", "/item");
	searchCategory("terms", "term", "Terme", "#");

	return results;
}

// ──────────────────────────────────────────────────────────────────────────
// Supabase
// ──────────────────────────────────────────────────────────────────────────

/** Échappe une valeur pour un pattern ilike PostgREST (retire la ponctuation dangereuse). */
function ilikeSafe(value: string): string {
	return value.replaceAll(/[%_,().*\\]/g, "").trim();
}

/**
 * Construit la liste des patterns OR (`col.ilike.%x%`) pour un set de colonnes
 * et un set de variantes de requête. Reste shim-safe (patterns simples).
 */
function buildOrFilters(columns: string[], variants: string[]): string {
	const seen = new Set<string>();
	const parts: string[] = [];
	for (const variant of variants) {
		const v = ilikeSafe(variant);
		if (!v) continue;
		for (const col of columns) {
			const f = `${col}.ilike.%${v}%`;
			if (!seen.has(f)) {
				seen.add(f);
				parts.push(f);
			}
		}
	}
	return parts.join(",");
}

/**
 * Recherche dans les tables Supabase / miroir SQLite. Récupère un set un peu
 * plus large (variantes ilike + premier token / préfixe) que la stricte chaîne,
 * le ranking/post-filtrage fuzzy est fait en JS dans searchTranslations.
 */
async function searchSupabase(query: string, entityType?: string): Promise<TranslationResult[]> {
	const supabase = await createClient();
	const queryNorm = normalizeForSearch(query);
	if (!queryNorm) return [];
	const tokens = tokenize(query);

	// Variantes ilike : requête entière + chaque token (récupère un sur-ensemble
	// que le ranking JS resserre ensuite — compense l'absence d'unaccent côté DB).
	const variants = new Set<string>();
	variants.add(queryNorm);
	for (const t of tokens) {
		if (t.length >= 2) variants.add(t);
	}
	// Préfixe (>= 3 chars) pour attraper les troncatures ("tornad" → "tornado").
	if (queryNorm.length >= 4) variants.add(queryNorm.slice(0, Math.max(3, queryNorm.length - 1)));

	// Variantes kana (romaji → hiragana/katakana) pour matcher name_ja.
	const kanaVariants = new Set<string>();
	const hiraCandidate = toHiragana(queryNorm);
	const kataCandidate = toKatakana(queryNorm);
	if ([...hiraCandidate].every((c) => isKana(c) || c === "ー") && hiraCandidate !== queryNorm) {
		kanaVariants.add(hiraCandidate);
	}
	if ([...kataCandidate].every((c) => isKana(c) || c === "ー") && kataCandidate !== queryNorm) {
		kanaVariants.add(kataCandidate);
	}

	const latinVariants = [...variants];
	const allVariants = [...variants, ...kanaVariants];

	// name_fr/name_en : variantes latines (accent-insensitif émulé en récupérant
	// un set large puis filtré JS). name_ja : variantes latines + kana.
	const orFilterString = [
		buildOrFilters(["name_fr", "name_en"], latinVariants),
		buildOrFilters(["name_ja"], allVariants),
	]
		.filter(Boolean)
		.join(",");

	// NB : inagle_characters n'a PAS de colonne name_roma → ne jamais filtrer
	// dessus (no such column dans le miroir SQLite). Romaji dérivé via name_ja.
	const charOrFilterString = orFilterString;

	const results: TranslationResult[] = [];
	const queries: Array<PromiseLike<void>> = [];

	const shouldQuery = (type: string) => !entityType || entityType === type;

	// Limites : large quand un filtre type est actif (set ciblé), plus modéré en
	// recherche globale (toutes tables en parallèle). Le ranking JS coupe ensuite.
	const lim = (filtered: number, all: number) => (entityType ? filtered : all);

	if (shouldQuery("character")) {
		queries.push(
			supabase
				.from("inagle_characters")
				.select("slug, name_fr, name_en, name_ja")
				.or(charOrFilterString)
				.not("internal_code", "like", "%_5000")
				.order("name_fr", { ascending: true, nullsFirst: false })
				.limit(lim(60, 15))
				.then(({ data }: any) => {
					if (data) {
						for (const c of data) {
							results.push({
								id: c.slug || c.id,
								name_en: c.name_en,
								name_fr: c.name_fr,
								name_ja: c.name_ja,
								name_roma: japaneseToRomaji(c.name_ja),
								type: "character",
								typeLabel: ENTITY_TYPE_CONFIG.character.label,
								url: `/chara/${c.slug}`,
							});
						}
					}
				})
		);
	}

	if (shouldQuery("skill")) {
		queries.push(
			supabase
				.from("inagle_skills")
				.select("id, name_fr, name_en, name_ja")
				.or(orFilterString)
				.order("name_fr", { ascending: true, nullsFirst: false })
				.limit(lim(50, 15))
				.then(({ data }: any) => {
					if (data) {
						for (const s of data) {
							results.push({
								id: s.id,
								name_en: s.name_en,
								name_fr: s.name_fr,
								name_ja: s.name_ja,
								name_roma: japaneseToRomaji(s.name_ja),
								type: "skill",
								typeLabel: ENTITY_TYPE_CONFIG.skill.label,
								url: `/skill/${s.id}`,
							});
						}
					}
				})
		);
	}

	if (shouldQuery("item")) {
		queries.push(
			supabase
				.from("inagle_items")
				.select("id, name_fr, name_en, name_ja, category")
				.or(orFilterString)
				.neq("category", "special_tactics")
				.order("name_fr", { ascending: true, nullsFirst: false })
				.limit(lim(50, 12))
				.then(({ data }: any) => {
					if (data) {
						for (const i of data) {
							results.push({
								id: i.id,
								name_en: i.name_en,
								name_fr: i.name_fr,
								name_ja: i.name_ja,
								name_roma: japaneseToRomaji(i.name_ja),
								type: "item",
								typeLabel: ENTITY_TYPE_CONFIG.item.label,
								url: `/item/${i.id}`,
							});
						}
					}
				})
		);
	}

	if (shouldQuery("tactic")) {
		queries.push(
			supabase
				.from("inagle_items")
				.select("id, name_fr, name_en, name_ja")
				.or(orFilterString)
				.eq("category", "special_tactics")
				.order("name_fr", { ascending: true, nullsFirst: false })
				.limit(lim(50, 12))
				.then(({ data }: any) => {
					if (data) {
						for (const t of data) {
							results.push({
								id: t.id,
								name_en: t.name_en,
								name_fr: TACTIC_FR[t.name_en] || t.name_fr,
								name_ja: t.name_ja,
								name_roma: japaneseToRomaji(t.name_ja),
								type: "tactic",
								typeLabel: ENTITY_TYPE_CONFIG.tactic.label,
								url: `/tactic/${t.id}`,
							});
						}
					}
				})
		);
	}

	if (shouldQuery("team")) {
		queries.push(
			supabase
				.from("inagle_teams")
				.select("id, name_fr, name_en, name_ja")
				.or(orFilterString)
				.order("name_fr", { ascending: true, nullsFirst: false })
				.limit(lim(50, 12))
				.then(({ data }: any) => {
					if (data) {
						for (const t of data) {
							results.push({
								id: t.id,
								name_en: t.name_en,
								name_fr: t.name_fr,
								name_ja: t.name_ja,
								name_roma: japaneseToRomaji(t.name_ja),
								type: "team",
								typeLabel: ENTITY_TYPE_CONFIG.team.label,
								url: `/team/${t.id}`,
							});
						}
					}
				})
		);
	}

	if (shouldQuery("keshin")) {
		queries.push(
			supabase
				.from("inagle_keshins_clean")
				.select("id, name_fr, name_en, name_ja")
				.or(orFilterString)
				.order("name_fr", { ascending: true, nullsFirst: false })
				.limit(lim(50, 10))
				.then(({ data }: any) => {
					if (data) {
						for (const k of data) {
							results.push({
								id: k.id,
								name_en: k.name_en,
								name_fr: k.name_fr,
								name_ja: k.name_ja,
								name_roma: japaneseToRomaji(k.name_ja),
								type: "keshin",
								typeLabel: ENTITY_TYPE_CONFIG.keshin.label,
								url: `/aura/esprits-guerriers/${k.id}`,
							});
						}
					}
				})
		);
	}

	if (shouldQuery("soul")) {
		queries.push(
			supabase
				.from("inagle_souls_clean")
				.select("id, name_fr, name_en, name_ja")
				.or(orFilterString)
				.order("name_fr", { ascending: true, nullsFirst: false })
				.limit(lim(50, 10))
				.then(({ data }: any) => {
					if (data) {
						for (const s of data) {
							results.push({
								id: s.id,
								name_en: s.name_en,
								name_fr: s.name_fr,
								name_ja: s.name_ja,
								name_roma: japaneseToRomaji(s.name_ja),
								type: "soul",
								typeLabel: ENTITY_TYPE_CONFIG.soul.label,
								url: `/aura/totems/${s.id}`,
							});
						}
					}
				})
		);
	}

	// allSettled : si une table échoue (colonne manquante, opérateur shim non
	// supporté…), on garde quand même les résultats des autres entités au lieu
	// de tout perdre comme le faisait Promise.all.
	await Promise.allSettled(queries);

	return results;
}

// ──────────────────────────────────────────────────────────────────────────
// Recherche unifiée
// ──────────────────────────────────────────────────────────────────────────

/**
 * Recherche de traductions unifiée : tables Supabase + glossaire local, avec
 * normalisation accent-insensible, conversion romaji↔kana, matching multi-mots
 * (AND désordonné) et passe fuzzy de secours (Levenshtein) si peu de résultats
 * exacts. Dédup + tri par pertinence (exact > prefix > token > fuzzy).
 */
export async function searchTranslations(
	query: string,
	entityType?: string
): Promise<TranslationResult[]> {
	if (!query || query.length < 2) {
		return [];
	}

	const queryNorm = normalizeForSearch(query);
	if (!queryNorm) return [];
	const queryTokens = tokenize(query);

	const [dbResults, glossaryResults] = await Promise.all([
		searchSupabase(query, entityType),
		searchGlossary(query, entityType),
	]);

	// Merge + dédup (DB prioritaire : slugs/URLs exacts).
	const seen = new Set<string>();
	const merged: TranslationResult[] = [];

	for (const r of dbResults) {
		const key = `${r.type}-${r.id}`.toLowerCase();
		if (!seen.has(key)) {
			seen.add(key);
			merged.push(r);
		}
	}
	for (const r of glossaryResults) {
		const key = `${r.type}-${r.id}`.toLowerCase();
		const nameKey = `${r.type}-${r.name_fr || r.name_en}`.toLowerCase();
		if (!seen.has(key) && !seen.has(nameKey)) {
			seen.add(key);
			seen.add(nameKey);
			merged.push(r);
		}
	}

	// Score JS de chaque résultat (les lignes DB n'ont pas de score préalable).
	for (const r of merged) {
		const s = scoreResult(r, queryNorm, queryTokens);
		// On conserve le score glossaire s'il est plus élevé (kana direct…).
		r.score = Math.max(r.score ?? 0, s);
		r.fuzzy = (r.score ?? 0) < 0.78;
	}

	// Filtre : on ne garde que ce qui dépasse le seuil fuzzy. Les lignes DB
	// remontées par ilike mais qui ne matchent pas après normalisation
	// (faux positifs du sur-ensemble) sont écartées ici.
	let kept = merged.filter((r) => (r.score ?? 0) >= FUZZY_THRESHOLD);

	// Si la passe stricte/fuzzy ne donne rien mais que la DB a quand même
	// renvoyé des lignes (sur-ensemble large), on les garde en dernier recours
	// pour ne pas afficher un écran vide trompeur.
	if (kept.length === 0 && merged.length > 0) {
		kept = merged
			.map((r) => ({ ...r, fuzzy: true }))
			.sort((a, b) => (b.score ?? 0) - (a.score ?? 0))
			.slice(0, 12);
	}

	// Tri final : score décroissant, puis exact-non-fuzzy d'abord, puis nom.
	kept.sort((a, b) => {
		const ds = (b.score ?? 0) - (a.score ?? 0);
		if (Math.abs(ds) > 1e-6) return ds;
		const an = (a.name_fr || a.name_en || "").toLowerCase();
		const bn = (b.name_fr || b.name_en || "").toLowerCase();
		return an.localeCompare(bn);
	});

	return kept.slice(0, 50);
}
