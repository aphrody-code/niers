/**
 * Forme du catalogue d'avatar servi par `nie-model-serve` sur `/avatar/catalog.json`.
 *
 * Ces types décrivent exactement ce que `niers avatar export` écrit — ils ne réorganisent rien.
 * Les champs `Hash` sont des CRC-32 en hexadécimal sans préfixe (`"06DE46FB"`), tels que le jeu
 * les porte dans ses `cfg.bin`.
 */

/** Une part sélectionnable : une coupe de cheveux, un œil, une bouche… */
export type Part = {
	/** CRC-32 de la part, référencé par les recettes de presets. */
	id: string;
	/** Rang dans la catégorie. */
	itemNo: number;
	/** Rang d'affichage dans la grille de l'éditeur. */
	viewNo: number;
	/** Restriction de genre (`0` = tous). */
	gender: number;
	/** Nom du modèle principal en clair (`hairF001`), vide si le jeu ne le fournit pas. */
	resource: string;
	resourceHash: string;
	/** Nom du modèle secondaire (cheveux arrière, teinte) quand il existe. */
	resource2: string | null;
	/** Chemins VFS réels du modèle / de la texture. */
	modeles: string[];
	modeles2: string[];
	/**
	 * Nom de l'icône de vignette (`icon_ava_face05_001`), `null` si le hash n'a pas été résolu.
	 * À servir par `/avatar/icon/<nom>.png`.
	 */
	icone: string | null;
	iconeHash: string;
	/** Centre `(u, v, w, h)` de la part dans l'atlas de visage, quand le jeu en déclare un. */
	atlasCentre: number[] | null;
	/** Mode de translation par défaut (entier), séparé de ses composantes. */
	poseTransMode: number | null;
	poseTrans: number[] | null;
	poseScale: number[] | null;
	resource2Connu: boolean;
};

/** Une catégorie de parts, avec sa palette de couleurs. */
export type Categorie = {
	/** Identifiant de la catégorie côté jeu. */
	faceSettingType: number;
	/** Préfixe commun des noms de ressources — dérivé des fichiers, pas rédigé. */
	prefixe: string;
	parts: Part[];
	/** CRC-32 des couleurs de la palette associée. */
	couleurs: string[];
	/** Écran du jeu correspondant, quand son nom concorde avec le préfixe des ressources. */
	ecran?: string | null;
	/**
	 * Grille de la palette, recoupée par deux sources : le nombre de couleurs et le nom des
	 * écrans du jeu (`chara_edit_color_menu_10x4_skin`, `_12x5`, `_13x5`).
	 */
	grillePalette?: { colonnes: number; lignes: number } | null;
};

/** Une ligne de recette : « pour cet emplacement, cette valeur / cette part / cette couleur ». */
export type LigneRecette = {
	emplacement: number;
	valeur: number;
	couleur: number;
	part: string | null;
};

/** Un visage prédéfini et sa recette complète. */
export type Preset = {
	presetID: string;
	nom: string | null;
	/** Morphologies auxquelles le preset s'applique. */
	morphologies: string[];
	recette: LigneRecette[];
};

/** Un emplacement du code de partage. */
export type Emplacement = {
	emplacement: number;
	/** Largeur en bits dans le code. */
	bits: number;
	/** Nombre de valeurs distinctes codables. */
	valeurs: number;
	categorie: number;
	param: number;
	paramSub: number;
};

/** Un curseur de morphing. */
export type Curseur = {
	part: string;
	/** Axe de la déformation. */
	axe: number;
	defaut: number;
	min: number;
	max: number;
	morphologies: string[];
};

/** Le catalogue complet. */
export type Catalogue = {
	/** Couleurs des palettes, relevées dans le jeu vivant par `niers mem palettes` (165). */
	couleursRgb?: Record<string, { rgb: string; alpha: number }>;
	source: string;
	codePartage: {
		bits: number;
		alphabet: string[];
		emplacements: Emplacement[];
	};
	/**
	 * Les rubriques de l'éditeur dans l'ordre où le jeu les affiche, extraites de la suite
	 * contiguë de hachages de libellés du script `chara_edit_list_menu`.
	 *
	 * L'ordre et les libellés sont établis ; l'association d'une rubrique à une catégorie
	 * technique (`faceSettingType`) ne l'est pas — elle se fait dans le code du menu.
	 */
	rubriques: { hash: string; libelle: string }[];
	/**
	 * Les libellés de chaque panneau de l'éditeur, lus dans le script Lua qui les affiche.
	 *
	 * Un panneau porte le nom de son script (`chara_edit_parts_menu_status`) et la liste ordonnée
	 * des libellés que ce script référence — titres de sections, noms de curseurs, choix, messages
	 * d'aide. Le texte est déjà dépouillé des marqueurs de style du jeu, et les glyphes spéciaux
	 * (`gaiji`) sont rendus à part pour être posés comme images.
	 *
	 * C'est la source de tous les textes de l'interface : aucun libellé n'est écrit à la main.
	 */
	panneaux: { nom: string; libelles: { hash: string; libelle: string; gaiji: string[] }[] }[];
	/**
	 * Les sept axes du radar de statistiques, dans l'ordre du dessin (frappe en haut, puis sens
	 * horaire). Les libellés viennent de `menu_text` comme les autres ; leur appartenance à cet
	 * écran vient des captures, l'éditeur ne les nommant pas dans ses propres scripts.
	 */
	statsRadar?: { hash: string; libelle: string }[];
	categories: Categorie[];
	presets: Preset[];
	avatarsFichiers: {
		nom: string;
		viewNo: number;
		charaId: string;
		modeles: string[];
		libelle: string | null;
	}[];
	voix: { banque: string; genre: number; personnalite: number; ton: number; itemNo: number }[];
	/** Personnalités — `libelle` vient de `menu_text`, la table de texte du jeu. */
	personnalites: { type: number; presentation: number; texte: string; libelle: string | null }[];
	tenues: { id: number; hash: string; morphologies: number[] }[];
	curseurs: { partsType: number; params: Curseur[] }[];
	modelesDeBase: {
		morphologies: string[];
		visages: { noseType: string; resources: string[] }[];
		accessoires: { presetID: string; resources: string[] }[];
	};
};

/**
 * Le thème de l'interface du jeu, servi par `/ui/theme.json`.
 *
 * Les couleurs ne sont pas un choix graphique : ce sont les 70 entrées de `font_color.cfg.bin`,
 * la palette que le moteur de texte du jeu utilise. `rubiHex` est la teinte des furigana, souvent
 * différente de celle du texte principal.
 */
export type Theme = {
	source: string;
	couleursTexte: {
		id: string;
		hex: string;
		rubiHex: string;
		rgb: [number, number, number];
		rubiRgb: [number, number, number];
	}[];
	/** Familles de police du jeu (atlas bitmap, une par dossier `font/<famille>/font.g4tx`). */
	polices: string[];
};

/**
 * Un objet d'un layout d'écran, tel que `nie-game --export-layout` l'écrit.
 *
 * `transform` est résolu depuis les points d'attache de l'écran (`CMenuAttachLocator`) : les
 * coordonnées sont celles du jeu, dans le repère du canevas du layout.
 */
export type LayoutObjet = {
	name: string;
	drawPriority?: number;
	visible?: boolean;
	transform?: {
		x?: number;
		y?: number;
		scaleX?: number;
		scaleY?: number;
		anchorX?: number;
		anchorY?: number;
		rot?: number;
	} | null;
	sprite?: { w: number; h: number; pngUrl: string; logicalPath?: string } | null;
	text?: unknown;
};

/** Le layout d'un écran de l'éditeur. */
export type Layout = {
	screen: string;
	locale?: string;
	canvas: { w: number; h: number };
	objects: LayoutObjet[];
};
