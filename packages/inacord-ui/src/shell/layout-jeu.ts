/**
 * La logique PURE du rendu d'un layout de menu exporte par `nie-game --export-layout`.
 *
 * ## Pourquoi ce module ne contient aucun composant
 *
 * Un layout est une DONNEE : 34 objets, chacun avec sa transformation, sa priorite de dessin,
 * sa texture et ses fentes de texte. Tout ce qui se calcule a partir de cette donnee — la
 * position CSS d'un objet, l'ordre de peinture, l'URL de sa texture, l'echelle du canevas — se
 * verifie sans monter un arbre React. C'est ce qui rend le rendu testable : `bun test` couvre
 * ces fonctions, le composant n'est plus qu'un cablage.
 *
 * ## Ce que l'export donne, et ce qu'il ne donne PAS
 *
 * Mesure sur `mainmenu01` (34 objets) : 24 d'entre eux portent exactement la position
 * (640, 360), c'est-a-dire le CENTRE du canevas — la valeur par defaut. Cinq autres portent des
 * coordonnees hors du canevas 1280x720 (jusqu'a x=4298, y=2390). Autrement dit, l'export ne
 * connait la position reelle que d'une minorite de widgets : le runtime du jeu les place, et
 * cette etape n'est pas capturee.
 *
 * Ce module ne corrige rien et n'invente rien : il rend ce que la donnee dit, et
 * [`bilanLayout`] COMPTE l'ecart pour qu'il soit dit plutot que subi. Une interface qui
 * repositionnerait ces objets « pour faire joli » afficherait une reconstruction en la faisant
 * passer pour une mesure.
 */
import type { CSSProperties } from "react";

/** Les dimensions logiques du canevas du menu. `mainmenu01` : 1280x720. */
export interface CanvasLayout {
	w: number;
	h: number;
}

/**
 * La transformation d'un objet.
 *
 * `anchorX`/`anchorY` sont exprimes en fraction de la taille de l'objet : `0.5` place le point
 * (x, y) au centre de l'objet, `0` en haut a gauche. Toutes les valeurs exportees valent `0.5`.
 */
export interface TransformLayout {
	x: number;
	y: number;
	anchorX: number;
	anchorY: number;
	scaleX: number;
	scaleY: number;
	/** Rotation. Toutes les valeurs exportees valent `0` — cf. [`OptionsStyle.uniteRotation`]. */
	rot: number;
}

/** La texture d'un objet, telle que l'export la designe. */
export interface SpriteLayout {
	/** Chemin VFS SANS le prefixe `data/` — c'est le piege principal, cf. [`cheminVfsSprite`]. */
	logicalPath: string;
	/** Chemin PNG relatif tel que l'exporteur le suggere. Non employe pour construire l'URL. */
	pngUrl?: string | null;
	w: number;
	h: number;
}

/** Une fente de texte, remplie par la localisation au moment de l'export. */
export interface SlotTexte {
	slot: string;
	text: string;
}

/** Un objet du layout. */
export interface ObjetLayout {
	name: string;
	/** Ordre de peinture. Croissant = dessine par-dessus, cf. [`objetsTries`]. */
	drawPriority: number;
	visible: boolean;
	transform: TransformLayout;
	sprite?: SpriteLayout | null;
	text?: SlotTexte[] | null;
	parent?: string | null;
	charModel?: unknown;
	primitive?: unknown;
	drawType?: number;
}

/** Un layout complet, tel que `nie-game --runtime --export-layout` l'ecrit. */
export interface LayoutJeu {
	/** Nom de l'ecran (`mainmenu01`). */
	screen: string;
	/** Langue des fentes de texte au moment de l'export. */
	locale?: string;
	generatedBy?: string;
	canvas: CanvasLayout;
	objects: ObjetLayout[];
}

/**
 * Le chemin VFS d'une texture, depuis le `logicalPath` de l'export.
 *
 * **Le piege** : l'export ecrit `dx11/menu/...`, sans le `data/` de tete, alors que toutes les
 * routes de ressources attendent le chemin VFS complet (`data/dx11/menu/...`). Une URL
 * construite sur le `logicalPath` brut rend 404, et un 404 sur ces routes ne se rattache jamais
 * spontanement a l'URL : on cherche le decodage.
 *
 * L'extension `.g4tx` est CONSERVEE ici — c'est `urlTexture()` de la source qui la retire, et
 * dupliquer cette regle serait la desynchroniser.
 */
export function cheminVfsSprite(logicalPath: string): string {
	const nu = logicalPath.replace(/^\/+/, "");
	return nu.startsWith("data/") ? nu : `data/${nu}`;
}

/**
 * Les objets dans leur ordre de PEINTURE : `drawPriority` croissant, le plus grand par-dessus.
 *
 * Le sens de l'axe se DEDUIT de la donnee et non d'une convention supposee : sur `mainmenu01`,
 * `mainmenu01_00_background` porte 650, les panneaux de fond 651, la liste 652, puis les guides
 * de boutons 654-655 et 745. Le fond est le plus PETIT de son groupe, donc un `drawPriority`
 * plus grand se dessine plus tard, donc au-dessus.
 *
 * Le tri est stable par construction (departage par l'index d'origine) : deux objets de meme
 * priorite gardent l'ordre de declaration, qui est leur seule identite relative.
 */
export function objetsTries<T extends { drawPriority: number }>(objets: readonly T[]): T[] {
	// `sort` et non `toSorted` : ce paquet est monte par DEUX hotes, et `apps/inacord` cible
	// ES2022 (`tsconfig.json`), ou `toSorted` n'existe pas. Le typecheck de `nie-web` (ESNext) et
	// celui d'`inacord-ui` passaient tous les deux — seul `bun run typecheck` complet voyait
	// l'erreur, dans le troisieme paquet. Une bibliotheque partagee tient au denominateur commun
	// de ses hotes. Le tableau vient d'un `map`, donc la mutation ne touche rien de partage.
	return objets
		.map((objet, index) => ({ objet, index }))
		.sort((a, b) => a.objet.drawPriority - b.objet.drawPriority || a.index - b.index)
		.map(({ objet }) => objet);
}

/** Reglages du style d'un objet. */
export interface OptionsStyle {
	/**
	 * L'unite de `transform.rot`.
	 *
	 * **Non mesurable sur ce corpus** : les 34 objets de `mainmenu01` portent `rot = 0`, valeur
	 * pour laquelle les deux unites coincident. Le defaut `rad` est l'unite native des donnees
	 * de layout Level-5 ; l'option existe pour qu'un export qui prouverait le contraire se
	 * corrige sans toucher au composant.
	 */
	uniteRotation?: "rad" | "deg";
	/** Decalage applique au `z-index`, pour empiler un layout sous une autre couche. */
	baseZ?: number;
}

/** La taille propre d'un objet, en pixels du canevas. `null` quand la donnee ne la dit pas. */
export function tailleObjet(objet: ObjetLayout): { w: number; h: number } | null {
	const s = objet.sprite;
	if (!s || !(s.w > 0) || !(s.h > 0)) return null;
	return { w: s.w, h: s.h };
}

/**
 * La position CSS absolue d'un objet, dans le repere du canevas.
 *
 * ## L'ordre des fonctions de `transform` n'est pas decoratif
 *
 * `transformOrigin: 0 0` place l'origine locale sur le coin haut-gauche, et la chaine se lit de
 * DROITE a GAUCHE : `translate` d'abord — qui amene le point d'ancrage sur l'origine — puis
 * `scale`, puis `rotate`. Le point d'ancrage se retrouve donc exactement sur (`left`, `top`),
 * c'est-a-dire sur (x, y), quelles que soient l'echelle et la rotation.
 *
 * Avec l'origine par defaut (`50% 50%`) ou dans l'ordre inverse, l'echelle deplacerait aussi le
 * point d'ancrage : l'erreur croit avec l'echelle, donc elle est invisible sur les objets a
 * `scale = 1` — la majorite — et ne se voit que sur les autres.
 */
export function styleObjet(objet: ObjetLayout, options: OptionsStyle = {}): CSSProperties {
	const { uniteRotation = "rad", baseZ = 0 } = options;
	const t = objet.transform;
	const taille = tailleObjet(objet);
	const unite = uniteRotation === "deg" ? "deg" : "rad";
	return {
		position: "absolute",
		left: `${t.x}px`,
		top: `${t.y}px`,
		// Sans sprite, l'objet n'a pas de taille propre : il se dimensionne sur son contenu
		// (ses fentes de texte) plutot que de s'effondrer a zero et de disparaitre sans un mot.
		width: taille ? `${taille.w}px` : "max-content",
		height: taille ? `${taille.h}px` : "auto",
		transformOrigin: "0 0",
		transform: `rotate(${t.rot}${unite}) scale(${t.scaleX}, ${t.scaleY}) translate(${-t.anchorX * 100}%, ${-t.anchorY * 100}%)`,
		zIndex: baseZ + objet.drawPriority,
	};
}

/**
 * L'echelle qui fait tenir le canevas dans une zone, sans le deformer.
 *
 * Le rapport est le MEME sur les deux axes : etirer un menu concu en 16/9 pour remplir une
 * fenetre plus haute deplacerait chaque widget d'une quantite differente selon sa position,
 * et aucun repere ne permettrait plus de comparer le rendu a la reference.
 *
 * Rend `0` pour une zone vide ou non encore mesuree — l'appelant distingue ainsi « pas encore
 * mesure » de « mesure a 1 ».
 */
export function echellePourZone(zoneL: number, zoneH: number, canvas: CanvasLayout): number {
	if (!(zoneL > 0) || !(zoneH > 0) || !(canvas.w > 0) || !(canvas.h > 0)) return 0;
	return Math.min(zoneL / canvas.w, zoneH / canvas.h);
}

/** Le point d'ancrage de l'objet tombe-t-il dans le canevas ? */
export function dansCanvas(objet: ObjetLayout, canvas: CanvasLayout): boolean {
	const { x, y } = objet.transform;
	return x >= 0 && y >= 0 && x <= canvas.w && y <= canvas.h;
}

/** L'objet est-il pose sur le centre exact du canevas, c'est-a-dire a la position par defaut ? */
export function auCentreParDefaut(objet: ObjetLayout, canvas: CanvasLayout): boolean {
	return objet.transform.x === canvas.w / 2 && objet.transform.y === canvas.h / 2;
}

/**
 * Un objet muet : visible, mais sans rien a montrer — ni texture, ni texte.
 *
 * Ce n'est pas un detail d'affichage, c'est le mode d'echec a surveiller : un objet muet occupe
 * une place dans le layout et ne produit aucun pixel, donc rien ne signale son absence.
 */
export function estMuet(objet: ObjetLayout): boolean {
	const aSprite = tailleObjet(objet) !== null;
	const aTexte = (objet.text ?? []).some((t) => t.text.trim() !== "");
	return !aSprite && !aTexte;
}

/** Le compte de ce qu'un layout contient reellement. */
export interface BilanLayout {
	/** Nombre total d'objets. */
	total: number;
	/** Objets marques `visible`. */
	visibles: number;
	/** Objets qui produiront une image. */
	avecSprite: number;
	/** Objets qui produiront du texte. */
	avecTexte: number;
	/** Objets visibles sans image ni texte — ceux qui ne produiront aucun pixel. */
	muets: number;
	/** Objets dont le point d'ancrage sort du canevas. */
	horsCanvas: number;
	/** Objets restes sur le centre exact, c'est-a-dire jamais positionnes par l'export. */
	auCentre: number;
	/** Textures distinctes referencees, par leur chemin VFS complet. */
	textures: string[];
}

/**
 * Ce que le layout contient, compte plutot qu'affirme.
 *
 * Sert au panneau de diagnostic ET aux tests : un chiffre annonce dans une conversation se
 * perime, un chiffre calcule a partir du fichier suit le fichier.
 */
export function bilanLayout(layout: LayoutJeu): BilanLayout {
	const textures = new Set<string>();
	let visibles = 0;
	let avecSprite = 0;
	let avecTexte = 0;
	let muets = 0;
	let horsCanvas = 0;
	let auCentre = 0;
	for (const objet of layout.objects) {
		if (objet.visible) visibles += 1;
		if (tailleObjet(objet) !== null) {
			avecSprite += 1;
			if (objet.sprite) textures.add(cheminVfsSprite(objet.sprite.logicalPath));
		}
		if ((objet.text ?? []).some((t) => t.text.trim() !== "")) avecTexte += 1;
		if (objet.visible && estMuet(objet)) muets += 1;
		if (!dansCanvas(objet, layout.canvas)) horsCanvas += 1;
		if (auCentreParDefaut(objet, layout.canvas)) auCentre += 1;
	}
	return {
		total: layout.objects.length,
		visibles,
		avecSprite,
		avecTexte,
		muets,
		horsCanvas,
		auCentre,
		textures: [...textures].sort(),
	};
}

/** Un morceau de texte, avec la couleur que le balisage du jeu lui attribue. */
export interface SegmentTexte {
	texte: string;
	/** Le jeton de couleur (`TEAMPARAM01`), ou `null` hors de tout balisage. */
	couleur: string | null;
}

/**
 * Decoupe un texte du jeu selon son balisage de couleur.
 *
 * Les fentes de texte transportent le balisage BRUT : `[CTEAMPARAM01]Bonus d'equipe[C]` ouvre
 * une couleur nommee et la referme. Pose tel quel dans le DOM, le jeton s'AFFICHE — un defaut
 * visible que rien dans le pipeline ne signale, puisque la chaine est correcte du point de vue
 * de l'export.
 *
 * Le nom de la couleur est conserve plutot que traduit en teinte : la table qui associe
 * `TEAMPARAM01` a une couleur vit dans le jeu, elle n'a pas ete mesuree ici, et deviner une
 * teinte ferait passer une invention pour une donnee.
 */
export function segmentsTexte(texte: string): SegmentTexte[] {
	const segments: SegmentTexte[] = [];
	let couleur: string | null = null;
	let reste = texte;
	const balise = /\[C([^\]]*)\]/;
	for (;;) {
		const trouve = balise.exec(reste);
		if (!trouve) break;
		const avant = reste.slice(0, trouve.index);
		if (avant !== "") segments.push({ texte: avant, couleur });
		couleur = trouve[1] ? trouve[1] : null;
		reste = reste.slice(trouve.index + trouve[0].length);
	}
	if (reste !== "") segments.push({ texte: reste, couleur });
	return segments;
}

/** Le texte affichable d'une fente, balisage retire. */
export function texteNu(texte: string): string {
	return segmentsTexte(texte)
		.map((s) => s.texte)
		.join("");
}

/**
 * Valide un layout charge depuis un JSON, et le type.
 *
 * Un `as LayoutJeu` sur un import JSON ne verifie RIEN : un reexport qui renommerait `objects`
 * ou perdrait `canvas` rendrait une page vide, sans message, et le typecheck resterait vert.
 * Cette fonction echoue bruyamment a la place — l'echec le plus cher d'une interface est celui
 * qui s'affiche correctement en n'ayant rien a montrer.
 */
export function lireLayout(valeur: unknown): LayoutJeu {
	const brut = valeur as Partial<LayoutJeu> | null;
	if (!brut || typeof brut !== "object") {
		throw new Error("layout : la valeur n'est pas un objet");
	}
	const canvas = brut.canvas;
	if (!canvas || !(canvas.w > 0) || !(canvas.h > 0)) {
		throw new Error("layout : `canvas` absent ou de dimensions nulles");
	}
	if (!Array.isArray(brut.objects)) {
		throw new Error("layout : `objects` absent ou n'est pas un tableau");
	}
	for (const [i, objet] of brut.objects.entries()) {
		if (!objet || typeof objet.name !== "string" || !objet.transform) {
			throw new Error(`layout : l'objet ${i} n'a ni nom ni transformation`);
		}
	}
	return {
		screen: typeof brut.screen === "string" ? brut.screen : "?",
		locale: brut.locale,
		generatedBy: brut.generatedBy,
		canvas: { w: canvas.w, h: canvas.h },
		objects: brut.objects,
	};
}
