/**
 * La geometrie de l'ecran `mainmenu01`, mesuree sur une capture du jeu.
 *
 * ## D'ou viennent ces nombres
 *
 * De `scripts/validation/mesurer-mainmenu.py`, joue sur
 * `data/design/aphrody-ui-ref-mainmenu-7.1.2.png` (2497x1414) et rejoue sur une seconde
 * capture de la meme version (2048x1159) : les deux rendent les memes valeurs a 1-2 px du
 * canevas pres. Tout est ramene au canevas 1280x720, celui de l'export de layout.
 *
 * ```
 * uv run scripts/validation/mesurer-mainmenu.py
 * ```
 *
 * ## Ce que cette source vaut, et ce qu'elle ne vaut pas
 *
 * Un screenshot est le **rang 4** des sources de la skill `pixel-perfect` : il a ete
 * redimensionne et compresse. Il sert donc aux POSITIONS et aux ordres de grandeur — pas aux
 * couleurs destinees a du code, qui se reprennent sur la texture du VFS.
 *
 * La vraie source de geometrie serait le layout runtime (`nie-game --runtime --export-layout`),
 * mais il ne porte PAS la position des widgets de cet ecran : 24 de ses 34 objets restent sur
 * le centre par defaut du canevas et 5 en sortent. Tant que le placement n'est pas reverse
 * (`docs/mainmenu01-analyse-visuelle.md` § 5 : il vient de la machine d'etat C++ `G4RA` et des
 * callbacks Lua `Setup*`), ce fichier est une RECONSTRUCTION mesuree sur une image, jamais une
 * mesure du binaire. Ne pas presenter ce qu'il produit comme pixel-perfect.
 *
 * ## Aphrody n'est pas le jeu
 *
 * La FORME vient d'ici ; le CONTENU est celui du site. Le jeu aligne 8 tuiles dans sa rangee,
 * Aphrody en a 5 parce qu'il expose 5 entrees reelles — inventer trois tuiles mortes pour
 * remplir la largeur ferait joli et mentirait. Les tuiles gardent en revanche la taille et la
 * pente mesurees, et la rangee reste centree sur le centre mesure.
 */

/** Une boite du canevas : coin haut-gauche, largeur, hauteur. */
export interface Boite {
	x: number;
	y: number;
	l: number;
	h: number;
}

/**
 * La pente des parallelogrammes du menu, en `dx/dy`.
 *
 * Negative : plus bas dans l'ecran, plus a gauche. Mesuree sur trois bords independants —
 * premiere tuile de la rangee, huitieme tuile, troisieme tuile de la rangee basse — qui rendent
 * -0,400 / -0,400 / -0,403 avec R2 = 1,000. C'est la valeur que `skewX` doit reproduire.
 *
 * L'analyse anterieure la disait « non mesurable » (R2 < 0,45) : elle ajustait les bords d'une
 * BOITE qui coupait la forme. Un bord se lit ligne par ligne, dans une fenetre qui ne contient
 * que lui.
 */
export const PENTE_TUILE = -0.4;

/** L'angle equivalent, pour `transform: skewX(...)`. `atan(-0,4)` = -21,8°. */
export const ANGLE_TUILE_DEG = -21.8;

/**
 * La pente des bords interieurs des deux grands panneaux : ils s'ecartent vers le HAUT, a
 * l'inverse des tuiles.
 *
 * Mesuree sur le bord gauche du panneau droit : -0,546, R2 = 1,000. Le bord droit du panneau
 * gauche, lui, n'est PAS mesurable proprement (R2 = 0,875 < 0,95, le sprite du personnage
 * deborde du cadre) : on lui applique la meme pente en miroir, et on le dit ici plutot que de
 * publier un +0,355 auquel personne ne devrait se fier.
 */
export const PENTE_PANNEAU = 0.546;

/**
 * Le fond de l'ecran : un blanc tres legerement verdi, 69,0 % de la surface de la capture.
 *
 * C'est l'ecart le plus visible qu'avait la premiere reconstruction, qui posait un degrade
 * bleu : le menu du jeu est un ecran CLAIR, presque blanc, sur lequel les bleus ressortent.
 */
export const FOND_MENU = "#f9fdf9";

/**
 * Les boites mesurees, en pixels du canevas 1280x720.
 *
 * Les zones dont la sonde a touche sa propre fenetre sont marquees dans la sortie du script
 * (`SATURE`) ; celles retenues ici sont soit nettes, soit recoupees par la seconde capture.
 */
export const BOITES = {
	/** L'encart d'information du coin haut-gauche (dans le jeu : la banniere « Informations »). */
	notice: { x: 0, y: 9, l: 314, h: 126 },
	/** Le bloc central du haut (dans le jeu : le logo). */
	titre: { x: 438, y: 5, l: 412, h: 287 },
	/** La version, en haut a droite. */
	version: { x: 1169, y: 9, l: 86, h: 13 },
	/** L'encart du haut-droit (dans le jeu : « Inazuma Post »). */
	encartHautDroit: { x: 986, y: 62, l: 265, h: 63 },
	/** Les deux grands panneaux lateraux : meme bande verticale. */
	panneaux: { y: 148, h: 180 },
	/** Le bord interieur du panneau gauche, en haut puis en bas de la bande. */
	panneauGaucheBord: { haut: 421, bas: 519 },
	/** Le bord interieur du panneau droit, en haut puis en bas de la bande. */
	panneauDroitBord: { haut: 845, bas: 747 },
	/** La plaque centrale (dans le jeu : « VICTOIRES 221 »). */
	plaque: { x: 518, y: 272, l: 262, h: 82 },
	/** La rangee principale : 8 tuiles dans le jeu, sur toute la largeur. */
	rangee: { x: 57, y: 377, l: 1150, h: 86 },
	/** Le bandeau sous la rangee — il n'est PAS centre : son centre mesure est a x = 813. */
	bandeau: { x: 594, y: 459, l: 438, h: 56 },
	/** La rangee basse : 3 tuiles, centrees. */
	rangeeBasse: { x: 424, y: 530, l: 428, h: 91 },
	/** La pastille du coin bas-gauche (dans le jeu : « Deluxe Edition »). */
	coinBasGauche: { x: 25, y: 615, l: 271, h: 43 },
	/** La pile de bannieres du bas-droit (dans le jeu : les DLC). */
	bannieres: { x: 1035, y: 560, l: 225, h: 105 },
	/** Le guide de touche du bas-centre. */
	aide: { x: 611, y: 678, l: 61, h: 24 },
	/** La mention legale du bas-droit. */
	mention: { x: 1042, y: 675, l: 225, h: 27 },
} as const;

/** L'ecart entre deux tuiles d'une rangee, en pixels du canevas. */
export const ECART_TUILE = 8;

/**
 * La largeur d'une tuile pour `n` tuiles dans une rangee de largeur `l`.
 *
 * Le jeu decoupe sa rangee de 1150 px en 8 tuiles : 137 px chacune. La rangee basse, mesuree
 * separement (428 px pour 3 tuiles), rend la MEME largeur — ce qui confirme que la tuile est
 * une unite de la DA et non le resultat d'un remplissage.
 */
export function largeurTuile(l: number, n: number, ecart = ECART_TUILE): number {
	return Math.round((l - ecart * (n - 1)) / n);
}

/** La largeur d'une tuile du jeu : 137 px, deux rangees mesurees independamment. */
export const LARGEUR_TUILE = largeurTuile(BOITES.rangee.l, 8);

/**
 * Le decalage horizontal du haut par rapport au bas, pour une forme penchee de hauteur `h`.
 *
 * C'est la valeur que prennent les `clip-path` : un parallelogramme de hauteur `h` dont le bord
 * suit `PENTE_TUILE` est decale de `0,4 x h` entre son haut et son bas.
 */
export function biseau(h: number, pente = PENTE_TUILE): number {
	return Math.round(Math.abs(pente) * h);
}
