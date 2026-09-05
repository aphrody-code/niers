/**
 * Modèle de données PUR de la page flagship `/demo` (démo 3D complète d'un personnage).
 *
 * Zéro dépendance three/@theatre/wasm/Node : toutes les structures sont sérialisables et
 * traversent la frontière RSC → îlots `"use client"`. La page serveur (`app/demo/page.tsx`)
 * construit un {@link DemoData} depuis `data/aphrody-dossier.json` (via les helpers PURS
 * `cpkAssetUrl`/`cpkAudioUrl`/`cpkThumbUrl` + `getSkillCutin`/`cutinHasAssets`), puis les
 * îlots consomment ces types sans jamais retoucher l'index CPK ni le SQLite.
 */

/** Une FORME du personnage (une ère : IE1 / GO / Ares) — un modèle 3D complet distinct. */
export interface DemoFormData {
	/** Code interne, ex. `c01001900` | `c05026590` | `c07080010`. */
	code: string;
	/** Libellé de l'ère (`series[].label`), ex. `Inazuma Eleven (IE1)`. */
	label: string;
	/** Sous-dossier des visages, ex. `01_ie1` | `05_go2` | `07_ares`. */
	faceSubdir: string;
	/** `true` si c'est la forme canonique (variante `is_primary`). */
	isPrimary: boolean;
	/** GLB complet texturé (corps + visage + uniforme), assemblé live par nie-model-serve. */
	modelGlbUrl: string;
	/** Voix JP décodée en WAV (`/audio/<acb>`), lisible par `<audio>`. */
	voiceUrl: string;
	/** Icône de portrait (g4tx → png), ou `null` si absente. */
	iconUrl: string | null;
	/** Texture de visage (g4tx → png), ou `null`. */
	faceTextureUrl: string | null;
	/** Paramètres de croissance pour le moteur de stats (positions + pattern). */
	growth: { mainPosition: number; subPosition: number; growthPattern: number };
}

/** Une TECHNIQUE (hissatsu) du personnage, avec ses assets de cut-in. */
export interface DemoSkill {
	/** Code interne du skill, ex. `whs00340`. */
	skillIdStr: string;
	/** Nom d'event du cut-in, ex. `ev60_00340`. */
	eventIdName: string;
	/** Élément localisé. */
	elementName: { fr: string; en: string; ja: string };
	/** Catégorie localisée (Tir / Dribble / …). */
	categoryName: { fr: string; en: string; ja: string };
	powerMin: number;
	powerMax: number;
	consumeTp: number;
	/** Type d'évolution (0..7) → glyphes gaiji. */
	growthType: number;
	/** Niveau d'apprentissage. */
	learnLevel: number;
	/** Bannière telop FR (nom rendu), ou `null`. */
	telopUrl: string | null;
	/** Texture du cut-in (g4tx → png), ou `null`. */
	textureUrl: string | null;
	/** Mesh waza assemblé (`/model-chr/waza/<event>.glb`). */
	wazaGlbUrl: string;
	/** `true` si le modèle 3D du cut-in est réellement servi (gate anti-404). */
	has3d: boolean;
}

/** Une texture téléchargeable de la galerie. */
export interface DemoTexture {
	label: string;
	/** PNG plein format (téléchargeable direct). */
	url: string;
	/** Vignette WebP redimensionnée (`?w=400`), ou `null`. */
	thumbUrl: string | null;
	group: "face" | "icon" | "waza" | "telop";
}

/** Une réplique de dialogue (flavour). */
export interface DemoDialogue {
	lineFr: string;
	lineJa: string;
}

/** Données complètes de la démo, sérialisées par la page serveur. */
export interface DemoData {
	identity: {
		nameFr: string;
		nameEn: string;
		nameJa: string;
		nicknameEn: string;
		nicknameJa: string;
		epithetFr: string;
		epithetEn: string;
		epithetJa: string;
		kana: string;
		teamNameEn: string;
		mainPosition: number;
		subPosition: number;
		elementId: number;
		constellationFr: string;
		constellationEn: string;
		zukanOrder: number;
	};
	forms: DemoFormData[];
	skills: DemoSkill[];
	textures: DemoTexture[];
	precomputedStats: {
		lv1: Record<string, number>;
		lv50: Record<string, number>;
		lv99: Record<string, number>;
	};
	references: { title: string; detail: string }[];
	dialogues: DemoDialogue[];
}

/**
 * Option chargeable dans la scène héro. CONTRAINTE : `CutinScene.fitObject` recentre/normalise
 * CHAQUE GLB à ~2 u individuellement → la scène n'affiche qu'UNE option à la fois (forme seule,
 * ou cut-in = waza + ses couches d'effet superposées). Pas de composition spatiale perso+waza.
 */
export interface DemoModelOption {
	/** `form:<code>` | `waza:<event>`. */
	id: string;
	label: string;
	kind: "form" | "waza";
	/** Code (forme) ou event_id_name (waza). */
	code: string;
	glbUrl: string;
	/** `false` = forme (texture normale), `true` = waza (brille dans le Bloom). */
	fromEffect: boolean;
	/** Voix de la forme (export audio), `null` pour un waza. */
	voiceUrl: string | null;
	/** Vignette (icône de forme ou texture de cut-in). */
	iconUrl: string | null;
	/** event_id_name pour un waza (→ fetchCutinManifest / telop), `null` pour une forme. */
	eventIdName: string | null;
}
