/**
 * Table d'expérience des joueurs — types et calculs PURS (client-safe).
 *
 * Jumeau client-safe de `lib/wiki/exp-table.ts` : aucun accès base, aucun module
 * Node, importable depuis un îlot `"use client"`.
 *
 * ## Sémantique de `need_exp` (vérité terrain, pas une hypothèse)
 *
 * Source : table `inagle_exp_table` du miroir SQLite (100 lignes, niveaux 1→100),
 * poussée par `packages/inagle/src/cli-push.ts:917` depuis le parseur
 * `packages/inagle/src/parsers/chara-exp-table.ts` (fichier de jeu
 * `character/chara_exp_table_config_*.cfg.bin.json`, liste `m_charaExpTableList`).
 *
 * `needExp[L]` est l'expérience nécessaire pour passer du niveau **L au niveau L+1**,
 * et NON le cumul pour atteindre L. Deux implémentations de production le prouvent :
 *
 * - `packages/inagle/src/parsers/chara-exp-table.ts` (`getCumulativeExp`) somme les
 *   entrées dont `entry.level < targetLevel` ;
 * - `packages/inagle/src/characters/evolution.ts:279` accumule
 *   `expEntries[entry.level - 2].needExp` pour le cumul du niveau `entry.level`,
 *   c'est-à-dire le `needExp` du niveau précédent.
 *
 * Conséquence assumée : le `needExp` de la DERNIÈRE ligne (niveau 100) ne finance
 * aucun passage de niveau, puisqu'il n'existe pas de niveau 101. Il est conservé
 * tel quel dans les entrées (on n'efface pas une donnée réelle) mais il n'entre
 * jamais dans un cumul, exactement comme dans les deux implémentations ci-dessus.
 */

/** Une ligne de `inagle_exp_table`, normalisée. */
export interface ExpLevelEntry {
	/** Niveau du joueur (1→100 dans les données réelles). */
	level: number;
	/** EXP nécessaire pour passer de `level` à `level + 1`. */
	needExp: number;
}

/** Table d'expérience prête à l'emploi (triée, bornes et total pré-calculés). */
export interface ExpTableData {
	/** Entrées triées par niveau croissant, doublons écartés. */
	entries: ExpLevelEntry[];
	/** Niveau le plus bas présent en base (`1` dans les données réelles). */
	minLevel: number;
	/** Niveau le plus haut présent en base (`100` dans les données réelles). */
	maxLevel: number;
	/** EXP cumulée pour aller de `minLevel` à `maxLevel`. */
	totalExp: number;
}

/** Résultat de la recherche inverse « j'ai tant d'EXP, je suis niveau ? ». */
export interface LevelFromExp {
	/** Niveau atteint avec cette quantité d'EXP. */
	level: number;
	/** EXP déjà engrangée à l'intérieur du niveau courant. */
	expIntoLevel: number;
	/** EXP totale du palier courant (`null` au niveau maximum). */
	expForNextLevel: number | null;
	/** EXP restante avant le niveau suivant (`null` au niveau maximum). */
	expToNextLevel: number | null;
	/** Avancement dans le palier courant, de 0 à 1 (vaut 1 au niveau maximum). */
	progress: number;
	/** `true` si l'EXP fournie atteint ou dépasse le niveau maximum. */
	capped: boolean;
	/** EXP excédentaire au-delà du niveau maximum (0 sinon). */
	overflow: number;
}

/** Table vide, utilisée quand la lecture base échoue (jamais de valeur inventée). */
const EMPTY_TABLE: ExpTableData = {
	entries: [],
	maxLevel: 0,
	minLevel: 0,
	totalExp: 0,
};

/** Ramène une valeur inconnue à un entier fini positif ou nul. */
function toPositiveInt(value: unknown): number | null {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return null;
	}
	const rounded = Math.trunc(value);
	return rounded >= 0 ? rounded : null;
}

/**
 * Construit la table exploitable à partir des lignes brutes : filtre les valeurs
 * inutilisables, trie par niveau, écarte les doublons de niveau (première ligne
 * gagnante) et pré-calcule bornes et cumul total.
 */
export function buildExpTableData(entries: readonly ExpLevelEntry[]): ExpTableData {
	const byLevel = new Map<number, number>();
	for (const entry of entries) {
		const level = toPositiveInt(entry?.level);
		const needExp = toPositiveInt(entry?.needExp);
		if (level === null || needExp === null || level <= 0 || byLevel.has(level)) {
			continue;
		}
		byLevel.set(level, needExp);
	}

	if (byLevel.size === 0) {
		return EMPTY_TABLE;
	}

	const sorted: ExpLevelEntry[] = [...byLevel.entries()]
		.map(([level, needExp]) => ({ level, needExp }))
		.sort((a, b) => a.level - b.level);

	const minLevel = sorted[0]!.level;
	const maxLevel = sorted[sorted.length - 1]!.level;

	let totalExp = 0;
	for (const entry of sorted) {
		if (entry.level < maxLevel) {
			totalExp += entry.needExp;
		}
	}

	return { entries: sorted, maxLevel, minLevel, totalExp };
}

/** Borne un niveau dans l'intervalle réellement présent en base. */
export function clampLevel(data: ExpTableData, level: number): number {
	if (data.entries.length === 0 || !Number.isFinite(level)) {
		return data.minLevel;
	}
	const rounded = Math.round(level);
	if (rounded < data.minLevel) {
		return data.minLevel;
	}
	if (rounded > data.maxLevel) {
		return data.maxLevel;
	}
	return rounded;
}

/**
 * EXP du palier `level` → `level + 1`, telle qu'elle figure en base.
 * `null` si le niveau n'existe pas dans la table.
 */
export function needExpForLevel(data: ExpTableData, level: number): number | null {
	for (const entry of data.entries) {
		if (entry.level === level) {
			return entry.needExp;
		}
	}
	return null;
}

/**
 * EXP nécessaire pour passer du niveau `from` au niveau `to` : somme des paliers
 * `from`, `from + 1`, …, `to - 1`. Renvoie 0 si `to <= from`. Les deux bornes sont
 * ramenées dans l'intervalle réel de la table.
 */
export function expBetweenLevels(data: ExpTableData, from: number, to: number): number {
	if (data.entries.length === 0) {
		return 0;
	}
	const start = clampLevel(data, from);
	const end = clampLevel(data, to);
	if (end <= start) {
		return 0;
	}
	let total = 0;
	for (const entry of data.entries) {
		if (entry.level >= start && entry.level < end) {
			total += entry.needExp;
		}
	}
	return total;
}

/** EXP cumulée depuis le premier niveau de la table jusqu'à `level`. */
export function cumulativeExpToLevel(data: ExpTableData, level: number): number {
	return expBetweenLevels(data, data.minLevel, level);
}

/**
 * Recherche inverse : quel niveau atteint-on avec `exp` points d'expérience,
 * en partant du premier niveau de la table ?
 *
 * Les valeurs négatives ou non finies sont traitées comme 0. Au-delà du cumul
 * total, le niveau maximum est renvoyé avec l'excédent dans `overflow`.
 */
export function levelFromExp(data: ExpTableData, exp: number): LevelFromExp {
	if (data.entries.length === 0) {
		return {
			capped: true,
			expForNextLevel: null,
			expIntoLevel: 0,
			expToNextLevel: null,
			level: 0,
			overflow: 0,
			progress: 1,
		};
	}

	let remaining = Number.isFinite(exp) && exp > 0 ? Math.trunc(exp) : 0;
	let level = data.minLevel;

	for (const entry of data.entries) {
		if (entry.level >= data.maxLevel) {
			break;
		}
		if (entry.level < level) {
			continue;
		}
		if (remaining < entry.needExp) {
			break;
		}
		remaining -= entry.needExp;
		level = entry.level + 1;
	}

	if (level >= data.maxLevel) {
		return {
			capped: true,
			expForNextLevel: null,
			expIntoLevel: 0,
			expToNextLevel: null,
			level: data.maxLevel,
			overflow: remaining,
			progress: 1,
		};
	}

	const needed = needExpForLevel(data, level) ?? 0;
	return {
		capped: false,
		expForNextLevel: needed,
		expIntoLevel: remaining,
		expToNextLevel: Math.max(0, needed - remaining),
		level,
		overflow: 0,
		progress: needed > 0 ? Math.min(1, remaining / needed) : 0,
	};
}

/**
 * Points de la courbe : pour chaque niveau, le coût du palier et le cumul depuis
 * le premier niveau. Un seul parcours, contrairement à un appel répété de
 * `cumulativeExpToLevel`.
 */
export interface ExpCurvePoint {
	level: number;
	/** EXP du palier `level` → `level + 1` (valeur brute de la base). */
	needExp: number;
	/** EXP cumulée pour ATTEINDRE `level` depuis le premier niveau. */
	cumulative: number;
}

/** Construit la courbe complète (cumul + coût par palier) en un seul passage. */
export function buildExpCurve(data: ExpTableData): ExpCurvePoint[] {
	const points: ExpCurvePoint[] = [];
	let cumulative = 0;
	for (const entry of data.entries) {
		points.push({ cumulative, level: entry.level, needExp: entry.needExp });
		if (entry.level < data.maxLevel) {
			cumulative += entry.needExp;
		}
	}
	return points;
}

/** Formatage FR d'un nombre d'EXP (séparateurs de milliers). */
export function formatExp(value: number): string {
	if (!Number.isFinite(value)) {
		return "0";
	}
	return Math.trunc(value).toLocaleString("fr-FR");
}
