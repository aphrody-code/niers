/**
 * La structure des écrans de l'éditeur, telle que les fichiers du jeu la déclarent.
 *
 * ## Les grilles viennent des `objbin`, pas d'un relevé
 *
 * Chaque conteneur de l'éditeur a son objet de menu dans `gamedata/menu/obj/`, et son nom **porte
 * sa géométrie** :
 *
 * | objet | grille |
 * |---|---|
 * | `avatar01_51_icon_list_body_gender_2x1` | 2×1 — les deux styles |
 * | `avatar01_11_icon_list_body_body_3x1` | 3×1 — le carrousel de physionomie |
 * | `avatar01_14_icon_list_body_preset_3x3` | 3×3 — l'échantillon de visage |
 * | `avatar01_19_icon_list_body_large_4x2` | 4×2 — forme de visage, nez, oreilles |
 * | `avatar01_18_icon_list_body_large_4x4` | 4×4 — coupes de cheveux |
 * | `avatar01_20_icon_list_body_large_3x1` | 3×1 — les habits |
 * | `avatar01_22_icon_list_body_middle_6x2` | 6×2 — bouches, sourcils |
 * | `avatar01_40_icon_list_body_ability_4x1` | 4×1 — les quatre éléments |
 * | `avatar01_35_color_preset_list_10x4` | 10×4 — palette de peau |
 * | `avatar01_34b_color_preset_list_12x5` | 12×5 — palette d'œil |
 * | `avatar01_34_color_preset_list_13x5` | 13×5 — palette de cheveux |
 * | `avatar01_47_list_body_voice_2x8` | 2×8 — les voix |
 *
 * ## L'association rubrique → catégorie
 *
 * Le catalogue ne l'énonce pas : c'est le code du menu qui la fait. Elle est ici **déduite par
 * recoupement du nombre de parts avec les captures du jeu**, et chaque ligne se vérifie :
 * forme de visage 6 (`faceSettingType` 2, six vignettes à l'écran), nez 7 (type 9, sept vignettes),
 * col 2 / manches 2 / ourlet 3 (types 19, 20, 21 — exactement les comptes affichés), coupes 98
 * (type 4, ce que le code de partage réserve : 98 valeurs), yeux 72 et pupilles 49 (types 6 et 7,
 * mêmes comptes dans le code de partage). Aucune ligne n'est posée sans ce recoupement.
 *
 * ## Les plages des curseurs
 *
 * Elles viennent du **code de partage** : ses emplacements de catégorie 3 et 6 réservent 15
 * valeurs (0 à 14, milieu 7 — les valeurs 6, 7 et 14 lues sur les captures), ceux de catégorie 5
 * en réservent 64 (0 à 63 — les 3, 18 et 63 de l'écran de couleur de peau).
 */

/** Nombre de valeurs d'un curseur de morphing (emplacements de catégorie 3 et 6). */
export const CURSEUR_MAX = 14;
/** Valeur centrale d'un curseur de morphing, celle que le jeu affiche au départ. */
export const CURSEUR_DEFAUT = 7;
/** Nombre de valeurs d'une composante de couleur (emplacements de catégorie 5). */
export const COULEUR_MAX = 63;

/** Une grille de vignettes. */
export type Grille = { colonnes: number; lignes: number };

/** Rôle des deux bouts d'un curseur — les paires de `edit_bar_iconNN` du jeu. */
export type RoleBouts =
	| "moinsPlus"
	| "largeur"
	| "longueur"
	| "verticale"
	| "echelle"
	| "rotation"
	| "couleur";

/** Un curseur d'une rubrique : son rôle de bouts et, quand le jeu le nomme, son libellé. */
export type Reglage = { bouts: RoleBouts; hash?: string };

/** Une section d'un panneau de rubrique. */
export type Section = {
	/** Hachage du titre ; absent quand le jeu n'affiche pas de titre pour cette section. */
	hash?: string;
	/** `faceSettingType` de la catégorie de parts à présenter. */
	categorie?: number;
	/** Grille de vignettes de la section. */
	grille?: Grille;
	/** Curseurs de la section. */
	reglages?: Reglage[];
	/** Palette de préréglages puis curseurs de teinte, saturation et luminosité. */
	couleurDe?: number;
	/**
	 * Rendu en lignes de sélecteur plutôt qu'en grille (rubriques Yeux et Extras).
	 *
	 * `icone` est le rang de `edit_win_iconNN`, apparié à la planche des dix-huit pictogrammes de
	 * l'éditeur : 02 tête, 03 visage, 04 main, 05 cheveux, 06 œil, 07 nez, 08 bouche, 09 sourcil,
	 * 10 oreille, 11 lunettes, 12 voix, 13 habits, 14 stats, 16 personnalité, 17 nom.
	 */
	lignes?: { hash: string; categorie: number; couleurDe?: number; icone: string }[];
};

/** Une rubrique de la colonne de gauche. */
export type Rubrique = {
	/** Rang de l'icône `icon_edit_listNN` de la plaque. */
	icone: number;
	sections: Section[];
	/** Hachage d'une note d'aide affichée au bas du panneau. */
	aide?: string;
};

/**
 * Les dix rubriques de l'onglet « Visage et coupe de cheveux », dans l'ordre de la colonne.
 *
 * Les rangs d'icône sont ceux de `icon_edit_list01..14`, appariés à la planche des quatorze
 * pictogrammes : 01 échantillon, 02 forme, 03 main, 04 cheveux, 05 œil, 06 nez, 07 bouche,
 * 08 sourcil, 09 oreille, 10 lunettes — puis 11 haut-parleur (voix), 12 buste (stats de base),
 * 14 silhouette (personnalité). L'ordre des icônes n'est donc pas celui des rubriques.
 */
export const RUBRIQUES_VISAGE: Rubrique[] = [
	// Échantillon de visage — les 36 visages prédéfinis, en 3×3.
	{ icone: 1, sections: [{ hash: "D49774C1", categorie: 1, grille: { colonnes: 3, lignes: 3 } }] },
	// Forme de visage — 6 formes en 4×2, puis largeur, mâchoire et longueur.
	{
		icone: 2,
		sections: [
			{ hash: "974BB0E6", categorie: 2, grille: { colonnes: 4, lignes: 2 } },
			{ hash: "1FDE773B", reglages: [{ bouts: "largeur" }] },
			{ hash: "C6934216", reglages: [{ bouts: "largeur" }] },
			{ hash: "1B348EFD", reglages: [{ bouts: "longueur" }] },
		],
	},
	// Couleur de peau — palette 10×4 puis les trois composantes.
	{ icone: 3, sections: [{ couleurDe: 3 }] },
	// Coupe de cheveux — 98 coupes en 4×4, la frange, puis la couleur de cheveux.
	{
		icone: 4,
		sections: [
			{ hash: "A821E377", categorie: 4, grille: { colonnes: 4, lignes: 4 } },
			{ hash: "3B3DD0F3", categorie: 5, grille: { colonnes: 4, lignes: 4 } },
			{ hash: "5AD8E080", couleurDe: 4 },
		],
	},
	// Yeux — trois sous-choix en lignes, comme le jeu les présente.
	{
		icone: 5,
		sections: [
			{
				lignes: [
					{ hash: "04A7737A", categorie: 6, couleurDe: 6, icone: "06" },
					{ hash: "322CBA6F", categorie: 7, icone: "06" },
					{ hash: "310114DA", categorie: 8, icone: "06" },
				],
			},
		],
	},
	// Nez — 7 formes en 4×2 et sa position verticale.
	{
		icone: 6,
		sections: [
			{ hash: "B42AEEBD", categorie: 9, grille: { colonnes: 4, lignes: 2 } },
			{ hash: "C7AAA035", reglages: [{ bouts: "verticale" }] },
		],
	},
	// Bouche — 23 bouches en 6×2, position verticale et échelle, puis la couleur des lèvres.
	{
		icone: 7,
		sections: [
			{ hash: "34FBC64A", categorie: 10, grille: { colonnes: 6, lignes: 2 } },
			{ hash: "C7AAA035", reglages: [{ bouts: "verticale" }, { bouts: "echelle" }] },
			{ hash: "56FD4CEE", couleurDe: 10 },
		],
	},
	// Sourcils — 40 sourcils en 6×2, quatre réglages, puis leur couleur.
	{
		icone: 8,
		sections: [
			{ hash: "F70F637F", categorie: 11, grille: { colonnes: 6, lignes: 2 } },
			{
				hash: "C7AAA035",
				reglages: [
					{ bouts: "verticale" },
					{ bouts: "largeur" },
					{ bouts: "echelle" },
					{ bouts: "rotation" },
				],
			},
			{ hash: "206063FF", couleurDe: 11 },
		],
	},
	// Oreilles — 6 oreilles en 4×2.
	{ icone: 9, sections: [{ hash: "CBEB0846", categorie: 12, grille: { colonnes: 4, lignes: 2 } }] },
	// Extras — deux emplacements, chacun une part et sa couleur.
	{
		icone: 10,
		sections: [
			{ lignes: [{ hash: "F7A91AF1", categorie: 13, couleurDe: 13, icone: "02" }] },
			{ lignes: [{ hash: "6EA04B4B", categorie: 14, couleurDe: 14, icone: "11" }] },
		],
	},
];

/** Les trois rubriques de l'onglet « Stats », dans l'ordre de la colonne. */
export const RUBRIQUES_STATS = [
	{ icone: 12, cle: "base" },
	{ icone: 14, cle: "personnalite" },
	{ icone: 11, cle: "voix" },
] as const;

/** Les trois sections de l'onglet « Habits » — col, manches, ourlet, en 3×1. */
export const SECTIONS_HABITS: Section[] = [
	{ hash: "C577FD49", categorie: 19, grille: { colonnes: 3, lignes: 1 } },
	{ hash: "7588F056", categorie: 20, grille: { colonnes: 3, lignes: 1 } },
	{ hash: "80CC3108", categorie: 21, grille: { colonnes: 3, lignes: 1 } },
];
