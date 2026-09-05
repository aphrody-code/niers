export type SpriteCategory = "role" | "element" | "rarity" | "class";

export interface SpriteCoords {
	x: number;
	y: number;
	w: number;
	h: number;
}

// Main spritesheet for role icons
export const SPRITE_SHEET_SRC = "/icon_common2.webp";

// Class/position icons spritesheet (73x73 grid) — servi via CDN local depuis 2026-05.
export const CLASS_SPRITE_SHEET_SRC =
	"https://cdn.rosegriffon.fr/static/azalee/menu/200_icon/06_icon_class/icon_class_s.webp";
export const CLASS_SPRITE_SIZE = 73;

export const SPRITES = {
	// Size indicators
	"size-l-standard": { h: 80, w: 80, x: 20, y: 20 },
	"size-s-standard": { h: 80, w: 80, x: 340, y: 20 },
	"size-m-standard": { h: 80, w: 80, x: 20, y: 180 },

	// Role badges - Glow variant (for active/selected)
	"role-att-glow": { h: 70, w: 180, x: 580, y: 20 },
	"role-mil-glow": { h: 70, w: 180, x: 580, y: 150 },
	"role-def-glow": { h: 70, w: 180, x: 570, y: 300 },
	"role-gar-glow": { h: 70, w: 190, x: 790, y: 300 },

	// Role badges - Solid variant
	"role-att-solid": { h: 80, w: 210, x: 550, y: 610 },
	"role-mil-solid": { h: 70, w: 190, x: 270, y: 630 },
	"role-def-solid": { h: 80, w: 210, x: 0, y: 630 },
	"role-gar-solid": { h: 60, w: 190, x: 460, y: 900 },

	// Role badges - Grey variant (for inactive/disabled)
	"role-att-grey": { h: 70, w: 200, x: 780, y: 460 },
	"role-mil-grey": { h: 70, w: 190, x: 780, y: 610 },
	"role-def-grey": { h: 70, w: 200, x: 500, y: 750 },
	"role-gar-grey": { h: 70, w: 190, x: 570, y: 460 },
	"role-gar-grey-2": { h: 70, w: 200, x: 720, y: 750 },

	// Role badges - Flat variant
	"role-att-flat": { h: 90, w: 200, x: 220, y: 770 },
	"role-mil-flat": { h: 90, w: 200, x: 0, y: 790 },
	"role-def-flat": { h: 90, w: 200, x: 220, y: 880 },
	"role-tout-flat": { h: 90, w: 160, x: 790, y: 10 },

	// Role badges - Simple (small)
	"role-att-tag": { h: 50, w: 120, x: 790, y: 150 },
	"role-gar-simple": { h: 50, w: 120, x: 0, y: 920 },
	"role-mil-simple": { h: 50, w: 120, x: 670, y: 880 },
	"role-def-simple": { h: 50, w: 130, x: 810, y: 880 },
} as const;

// Class icons from icon_class_s.webp (73x73 grid, 8 columns)
// These are position/class indicator icons
export const CLASS_SPRITES = {
	// Row 0 - Main class icons (blue glow)
	"class-forward": { h: 73, w: 73, x: 0, y: 0 }, // Éclair bleu - Attaquant
	"class-defender": { h: 73, w: 73, x: 73, y: 0 }, // Triangle violet - Défenseur
	"class-goalkeeper": { h: 73, w: 73, x: 146, y: 0 }, // Ballon planète - Gardien

	// Row 2 - Additional icons
	"class-wings": { h: 73, w: 73, x: 0, y: 146 }, // Ailes blanches
	"class-midfielder": { h: 73, w: 73, x: 73, y: 146 }, // Étoile dorée - Milieu
	"class-support": { h: 73, w: 73, x: 146, y: 146 }, // Haltère violet - Support

	// Row 3 - More icons
	"class-speed": { h: 73, w: 73, x: 0, y: 219 }, // Lune blanche - Vitesse
	"class-star": { h: 73, w: 73, x: 73, y: 219 }, // Étoile dorée
	"class-power": { h: 73, w: 73, x: 146, y: 219 }, // Ballon rouge - Puissance
} as const;

export type SpriteKey = keyof typeof SPRITES;
