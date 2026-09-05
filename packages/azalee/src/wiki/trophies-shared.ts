/**
 * @file lib/wiki/trophies-shared.ts
 * @description Types et helpers PURS (client-safe) de la section « Succès ».
 *
 * Aucun accès à une base ici : ce module est importable depuis un îlot
 * `"use client"`. L'accès data réel vit dans `wiki/trophies.ts` (serveur).
 *
 * Source : table `inagle_trophies` (228 lignes). Le `code` encode la catégorie via
 * son préfixe : `trophy_*` = succès officiels (trophées du jeu), `activity_*` =
 * objectifs d'activité in-game (collecte, exploration, mini-quêtes). Le 2e segment
 * du code donne un sous-groupe (`story`, `collection`, `battle`, `whole`, …).
 */

/** Catégorie de haut niveau dérivée du préfixe du `code`. */
export type TrophyCategory = "trophy" | "activity";

/** Texte localisé tri-langue (les seules langues présentes en DB). */
export interface LocalizedText {
	fr: string;
	en: string;
	ja: string;
}

/** Succès normalisé pour le rendu wiki (champs RÉELS du miroir uniquement). */
export interface Trophy {
	/** Identifiant stable = `code` (ex. `trophy_story_main_01`). Clé de route. */
	id: string;
	/** Code interne in-game (identique à `trophy_id`). */
	code: string;
	/** Catégorie de haut niveau (succès officiel vs activité in-game). */
	category: TrophyCategory;
	/** 2e segment du code (`story`, `collection`, `whole`, …) — sous-groupe brut. */
	group: string;
	/** Libellé FR lisible du sous-groupe. */
	groupLabel: string;
	/** Nom localisé FR (repli EN/JA), pour l'affichage principal. */
	name: string;
	/** Description localisée FR (repli EN/JA), vide si non fournie par le jeu. */
	desc: string;
	/** Noms bruts par langue. */
	names: LocalizedText;
	/** Descriptions brutes par langue. */
	descriptions: LocalizedText;
}

/** Libellés FR des catégories de haut niveau. */
export const CATEGORY_LABELS: Record<TrophyCategory, string> = {
	trophy: "Trophées",
	activity: "Activités",
};

/** Libellés FR des sous-groupes connus (2e segment du code). */
const GROUP_LABELS: Record<string, string> = {
	story: "Histoire",
	main: "Histoire principale",
	collection: "Collection",
	battle: "Combats",
	chronicle: "Chroniques",
	town: "Ville",
	complete: "Complétion",
	whole: "Progression globale",
	explore: "Exploration",
	miniquest: "Mini-quêtes",
	achieve: "Accomplissements",
};

/** Premier segment du `code` → catégorie de haut niveau. */
export function trophyCategory(code: string): TrophyCategory {
	return code.startsWith("trophy") ? "trophy" : "activity";
}

/** 2e segment du `code` (sous-groupe brut), `""` si le code n'a qu'un segment. */
export function trophyGroup(code: string): string {
	const parts = code.split("_");
	return parts.length > 1 ? parts[1] : "";
}

/** Libellé FR lisible d'un sous-groupe (repli : segment capitalisé). */
export function groupLabel(group: string): string {
	if (!group) return "Divers";
	return GROUP_LABELS[group] ?? group.charAt(0).toUpperCase() + group.slice(1);
}
