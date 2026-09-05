// Registre des VRAIES icones de menu du jeu IEVR (atlas g4tx decodes en webp).
// Source autoritative = table de sous-textures native des conteneurs G4TX
// (data/dx11/menu/200_icon/<famille>/<nom>.g4tx), re-decodee depuis les bytes
// bruts du CDN puis verifiee visuellement (silhouette blanche sur fond sombre).
//
// Les coordonnees (x,y,w,h) sont les CELLULES d'auteur de l'atlas (l'icone est
// centree dans sa cellule). Les webp servis dans /public/game-icons/ sont des
// MASQUES : canal alpha boosté x5, RGB force a blanc -> teintés en currentColor
// par GameSpriteIcon (s'adapte aux 4 themes clair/sombre comme une icone lucide).
//
// 100% client-safe : aucun import serveur (bun:sqlite / node / server-only).

/** Un atlas g4tx normalise en webp-masque dans /public/game-icons/. */
export interface GameIconAtlas {
	/** URL publique du masque webp (sous /public). */
	src: string;
	/** Largeur native de la planche (px) — pour le mask-size CSS. */
	sheetW: number;
	/** Hauteur native de la planche (px). */
	sheetH: number;
}

/** Une icone : reference d'atlas + rectangle de cellule + libelle FR. */
export interface GameIcon {
	/** Cle de l'atlas dans GAME_ICON_ATLASES. */
	atlas: GameIconAtlasKey;
	/** Position X de la cellule dans l'atlas (px). */
	x: number;
	/** Position Y de la cellule dans l'atlas (px). */
	y: number;
	/** Largeur de la cellule (px). */
	w: number;
	/** Hauteur de la cellule (px). */
	h: number;
	/** Libelle francais (accessibilite / doc). */
	nameFr: string;
}

// Atlas disponibles (téléchargés depuis cdn.rosegriffon.fr/tex/<vfs>.png puis
// convertis en masque webp : convert in.png -channel A -evaluate multiply 5
// +channel -fill white -colorize 100 out.webp).
export const GAME_ICON_ATLASES = {
	// 17_icon_category.g4tx — grille 3x4 de cellules 256x200 (categories d'inventaire).
	category: {
		sheetH: 812,
		sheetW: 776,
		src: "/game-icons/icon_category.webp",
	},
	// 17_icon_category2.g4tx — grille 2x4 de cellules 192x120 (batiments / lieux de ville).
	category2: {
		sheetH: 492,
		sheetW: 388,
		src: "/game-icons/icon_category2.webp",
	},
} as const satisfies Record<string, GameIconAtlas>;

export type GameIconAtlasKey = keyof typeof GAME_ICON_ATLASES;

// Mapping nom -> cellule. Coordonnees lues dans la table g4tx puis VERIFIEES
// visuellement (rendu masque blanc) : la grille reelle prime sur tout libelle.
export const GAME_ICONS = {
	// — 17_icon_category (grille 3x4, cellule 256x200) —
	uniform: { atlas: "category", h: 200, nameFr: "Uniforme / Maillot", w: 256, x: 0, y: 0 },
	shoe: { atlas: "category", h: 200, nameFr: "Equipement (chaussure)", w: 256, x: 260, y: 0 },
	ticket: { atlas: "category", h: 200, nameFr: "Ticket", w: 256, x: 520, y: 0 },
	emblem: { atlas: "category", h: 200, nameFr: "Embleme / Ecusson", w: 256, x: 0, y: 204 },
	necklace: { atlas: "category", h: 200, nameFr: "Equipement (collier)", w: 256, x: 260, y: 204 },
	chest: { atlas: "category", h: 200, nameFr: "Coffre / Objet", w: 256, x: 520, y: 204 },
	// (0,408) = medaille/sifflet (forme collier ferme) — utilisé pour l'entraineur.
	medal: { atlas: "category", h: 200, nameFr: "Medaille / Sifflet", w: 256, x: 0, y: 408 },
	moneybag: { atlas: "category", h: 200, nameFr: "Bourse / Monnaie", w: 256, x: 260, y: 408 },
	creature: { atlas: "category", h: 200, nameFr: "Tenue / Creature", w: 256, x: 520, y: 408 },
	"star-token": { atlas: "category", h: 200, nameFr: "Jeton (etoile)", w: 256, x: 0, y: 612 },
	emote: { atlas: "category", h: 200, nameFr: "Lien / Emote", w: 256, x: 260, y: 612 },
	nameplate: { atlas: "category", h: 200, nameFr: "Plaque de nom", w: 256, x: 520, y: 612 },

	// — 17_icon_category2 (grille 2x4, cellule 192x120) —
	house: { atlas: "category2", h: 120, nameFr: "Maison / Batiment", w: 192, x: 0, y: 0 },
	car: { atlas: "category2", h: 120, nameFr: "Vehicule", w: 192, x: 196, y: 0 },
	stadium: { atlas: "category2", h: 120, nameFr: "Stade", w: 192, x: 0, y: 124 },
	tree: { atlas: "category2", h: 120, nameFr: "Arbre / Nature", w: 192, x: 196, y: 124 },
	sofa: { atlas: "category2", h: 120, nameFr: "Mobilier (canape)", w: 192, x: 0, y: 248 },
	flower: { atlas: "category2", h: 120, nameFr: "Fleur / Plante", w: 192, x: 196, y: 248 },
	road: { atlas: "category2", h: 120, nameFr: "Route", w: 192, x: 0, y: 372 },
} as const satisfies Record<string, GameIcon>;

export type GameIconKey = keyof typeof GAME_ICONS;
