/**
 * Helpers client-safe pour les stats de personnage (croissance).
 *
 * Aucune dépendance Node ni SQLite : ce module est importable depuis un îlot
 * « use client ». La résolution LIVE des stats (fetch CDN /cfg) vit dans le
 * pendant serveur `chara-stats.ts`.
 *
 * Les 7 stats d'IEVR (ordre d'affichage du jeu) :
 *   frappe (Kick), contrôle (Control), technique (Technique),
 *   pression (Pressure), physique (Physical), agilité (Agility),
 *   intelligence (Intelligence).
 */

/** Les 7 stats d'un personnage à un niveau donné. */
export interface CharaStats {
	kick: number;
	control: number;
	technique: number;
	pressure: number;
	physical: number;
	agility: number;
	intelligence: number;
}

/** Stats multi-niveaux résolues depuis la table de croissance (gamedata). */
export interface CharaMultiLevelStats {
	lv1: CharaStats;
	lv30: CharaStats;
	lv50: CharaStats;
	lv99: CharaStats;
	/** Somme des 7 stats au niveau 99 (cap). */
	total99: number;
}

/** Ordre canonique des 7 stats + libellés FR/EN pour l'affichage. */
export const STAT_KEYS: ReadonlyArray<{
	key: keyof CharaStats;
	fr: string;
	en: string;
}> = [
	{ key: "kick", fr: "Frappe", en: "Kick" },
	{ key: "control", fr: "Contrôle", en: "Control" },
	{ key: "technique", fr: "Technique", en: "Technique" },
	{ key: "pressure", fr: "Pression", en: "Pressure" },
	{ key: "physical", fr: "Physique", en: "Physical" },
	{ key: "agility", fr: "Agilité", en: "Agility" },
	{ key: "intelligence", fr: "Intelligence", en: "Intelligence" },
];

/** Somme des 7 stats. */
export function statTotal(s: CharaStats): number {
	return (
		s.kick +
		s.control +
		s.technique +
		s.pressure +
		s.physical +
		s.agility +
		s.intelligence
	);
}

/**
 * Position (texte FR de la DB ou code court) → `mainPosition` de la table de
 * croissance. 1=GK, 2=FW, 3=MF, 4=DF, 0=Coach (sans stats de croissance).
 */
export function positionToMainPosition(position: string | null | undefined): number | null {
	if (!position) return null;
	switch (position.trim().toLowerCase()) {
		case "gardien":
		case "gk":
		case "gar":
			return 1;
		case "attaquant":
		case "fw":
		case "att":
			return 2;
		case "milieu":
		case "milieu de terrain":
		case "mf":
		case "mil":
			return 3;
		case "défenseur":
		case "defenseur":
		case "df":
		case "def":
			return 4;
		default:
			return null;
	}
}

/**
 * Libellé de rareté (DB `rarity_label`) → `charaRank` (0-5) de la table de
 * croissance. Reproduit `rarityToGrowthRank` d'inagle : N→0, Expérimenté→2,
 * Émérite→3, Légendaire→5, Héros/BASARA → stats UR (rang 5).
 */
export function rarityLabelToGrowthRank(label: string | null | undefined): number {
	switch ((label ?? "").trim()) {
		case "En progression":
			return 1;
		case "Expérimenté":
			return 2;
		case "Émérite":
			return 3;
		case "Légendaire":
		case "Héros":
		case "BASARA":
			return 5;
		default:
			return 0; // Normal
	}
}
