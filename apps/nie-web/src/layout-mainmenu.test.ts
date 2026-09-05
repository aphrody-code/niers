/**
 * Ce que le rendu d'un layout doit garantir, verifie sur le VRAI fichier embarque.
 *
 * Les fonctions testees ici sont pures : conversion d'une transformation en position CSS, ordre
 * de peinture, construction de l'URL d'une texture, decoupe du balisage de couleur. Le composant
 * qui les cable n'ajoute aucune regle — c'est ce qui rend ce fichier suffisant.
 *
 * Les comptes portent sur `donnees/mainmenu01.layout.json` et NON sur des valeurs recopiees :
 * si un reexport change le layout, ces tests changent de couleur au lieu de laisser une page
 * fausse passer pour juste.
 */
import { describe, expect, test } from "bun:test";

import {
	auCentreParDefaut,
	bilanLayout,
	cheminVfsSprite,
	dansCanvas,
	echellePourZone,
	estMuet,
	type LayoutJeu,
	lireLayout,
	type ObjetLayout,
	objetsTries,
	segmentsTexte,
	styleObjet,
	tailleObjet,
	texteNu,
} from "@niers/inacord-ui";

import brut from "./donnees/mainmenu01.layout.json";

const LAYOUT: LayoutJeu = lireLayout(brut);

/** Un objet minimal, pour les tests qui ne portent pas sur le fichier reel. */
function objet(partiel: Partial<ObjetLayout> = {}): ObjetLayout {
	return {
		name: "test",
		drawPriority: 0,
		visible: true,
		transform: { x: 0, y: 0, anchorX: 0, anchorY: 0, scaleX: 1, scaleY: 1, rot: 0 },
		...partiel,
	};
}

describe("cheminVfsSprite", () => {
	test("ajoute le `data/` que l'export omet", () => {
		// Le piege principal : l'export ecrit `dx11/...`, toutes les routes attendent
		// `data/dx11/...`. Sans ce prefixe, chaque texture rend 404 — et un 404 sur ces routes
		// se cherche du cote du decodage, jamais du cote de l'URL.
		expect(cheminVfsSprite("dx11/menu/100_mainmenu/mainmenu01/x/x.g4tx")).toBe(
			"data/dx11/menu/100_mainmenu/mainmenu01/x/x.g4tx",
		);
	});

	test("n'ajoute pas un second `data/`", () => {
		expect(cheminVfsSprite("data/dx11/x.g4tx")).toBe("data/dx11/x.g4tx");
	});

	test("une barre de tete ne cree pas de segment vide", () => {
		expect(cheminVfsSprite("/dx11/x.g4tx")).toBe("data/dx11/x.g4tx");
	});

	test("l'extension du jeu est CONSERVEE", () => {
		// C'est `urlTexture()` de la source qui retire le `.g4tx` ; dupliquer la regle ici la
		// desynchroniserait le jour ou l'amont changerait.
		expect(cheminVfsSprite("dx11/x.g4tx")).toEndWith(".g4tx");
	});

	test("tous les sprites du layout reel donnent un chemin sous `data/`", () => {
		const chemins = LAYOUT.objects
			.map((o) => o.sprite?.logicalPath)
			.filter((p): p is string => Boolean(p))
			.map(cheminVfsSprite);
		expect(chemins.length).toBe(26);
		expect(chemins.every((c) => c.startsWith("data/dx11/"))).toBe(true);
	});
});

describe("objetsTries", () => {
	test("le plus grand `drawPriority` passe en dernier, donc au-dessus", () => {
		const tries = objetsTries([
			{ drawPriority: 655, n: "guide" },
			{ drawPriority: 300, n: "description" },
			{ drawPriority: 650, n: "fond" },
		]);
		expect(tries.map((o) => o.n)).toEqual(["description", "fond", "guide"]);
	});

	test("a priorite egale, l'ordre de declaration est conserve", () => {
		// Deux objets de meme priorite n'ont pas d'autre identite relative que leur ordre : un
		// tri instable les ferait permuter d'un rendu a l'autre, sans que rien ne change.
		const memes = Array.from({ length: 12 }, (_, i) => ({ drawPriority: 655, n: i }));
		expect(objetsTries(memes).map((o) => o.n)).toEqual(memes.map((o) => o.n));
	});

	test("ne modifie pas le tableau d'origine", () => {
		const source = [{ drawPriority: 2 }, { drawPriority: 1 }];
		objetsTries(source);
		expect(source.map((o) => o.drawPriority)).toEqual([2, 1]);
	});

	test("le fond du layout reel est peint avant les guides de boutons", () => {
		const tries = objetsTries(LAYOUT.objects);
		const rang = (nom: string) => tries.findIndex((o) => o.name === nom);
		expect(rang("mainmenu01_00_background")).toBeLessThan(rang("mainmenu01_07_button_guide"));
	});
});

describe("styleObjet", () => {
	test("la position est celle de la transformation, en pixels du canevas", () => {
		const s = styleObjet(objet({ transform: { x: 538.5, y: 337.25, anchorX: 0.5, anchorY: 0.5, scaleX: 1, scaleY: 1, rot: 0 } }));
		expect(s.position).toBe("absolute");
		expect(s.left).toBe("538.5px");
		expect(s.top).toBe("337.25px");
	});

	test("l'ancrage se traduit en `translate`, en pourcentage de l'objet", () => {
		// Un pourcentage, et non des pixels : il suit la taille de l'objet, y compris quand
		// celle-ci vient du contenu.
		// `${-0}` s'écrit « 0 » en JavaScript : un ancrage nul ne produit pas de signe.
		expect(styleObjet(objet()).transform).toContain("translate(0%, 0%)");
		const centre = styleObjet(
			objet({ transform: { x: 0, y: 0, anchorX: 0.5, anchorY: 0.5, scaleX: 1, scaleY: 1, rot: 0 } }),
		);
		expect(centre.transform).toContain("translate(-50%, -50%)");
	});

	test("l'ordre des fonctions place l'ancre sur (x, y) MALGRE l'echelle", () => {
		// La chaine se lit de droite a gauche : translate d'abord, puis scale, puis rotate, avec
		// une origine en haut a gauche. Dans l'autre ordre, l'echelle deplacerait aussi l'ancre —
		// une erreur invisible sur les objets a `scale = 1`, c'est-a-dire sur la majorite.
		const s = styleObjet(
			objet({ transform: { x: 10, y: 20, anchorX: 0.5, anchorY: 0.5, scaleX: 0.66, scaleY: 0.66, rot: 0 } }),
		);
		expect(s.transformOrigin).toBe("0 0");
		expect(s.transform).toBe("rotate(0rad) scale(0.66, 0.66) translate(-50%, -50%)");
	});

	test("l'unite de rotation est explicite et reglable", () => {
		// Les 34 objets exportes portent `rot = 0` : l'unite n'est pas mesurable sur ce corpus.
		// L'option existe pour qu'un export qui prouverait `deg` se corrige sans toucher au
		// composant.
		const t = { x: 0, y: 0, anchorX: 0, anchorY: 0, scaleX: 1, scaleY: 1, rot: 1.5 };
		expect(styleObjet(objet({ transform: t })).transform).toContain("rotate(1.5rad)");
		expect(styleObjet(objet({ transform: t }), { uniteRotation: "deg" }).transform).toContain(
			"rotate(1.5deg)",
		);
	});

	test("la taille vient du sprite, et `z-index` de la priorite de dessin", () => {
		const s = styleObjet(
			objet({ drawPriority: 655, sprite: { logicalPath: "dx11/x.g4tx", w: 304, h: 68 } }),
		);
		expect(s.width).toBe("304px");
		expect(s.height).toBe("68px");
		expect(s.zIndex).toBe(655);
	});

	test("`baseZ` empile un layout entier sous une autre couche", () => {
		expect(styleObjet(objet({ drawPriority: 655 }), { baseZ: -1000 }).zIndex).toBe(-345);
	});

	test("sans sprite, l'objet se dimensionne sur son contenu au lieu de disparaitre", () => {
		const s = styleObjet(objet({ sprite: null }));
		expect(s.width).toBe("max-content");
		expect(s.height).toBe("auto");
	});

	test("un sprite de taille nulle n'est pas une taille", () => {
		expect(tailleObjet(objet({ sprite: { logicalPath: "x", w: 0, h: 0 } }))).toBeNull();
	});
});

describe("echellePourZone", () => {
	test("le meme rapport sur les deux axes : le menu ne se deforme pas", () => {
		expect(echellePourZone(2560, 1440, { w: 1280, h: 720 })).toBe(2);
		// Zone plus haute que large : c'est la largeur qui contraint.
		expect(echellePourZone(640, 720, { w: 1280, h: 720 })).toBe(0.5);
		// Zone plus large que haute : c'est la hauteur.
		expect(echellePourZone(2560, 360, { w: 1280, h: 720 })).toBe(0.5);
	});

	test("une zone non mesuree rend 0, pas 1", () => {
		// Distinguer « pas encore mesure » de « mesure a 1 » evite d'afficher une frame a la
		// mauvaise taille avant la premiere mesure.
		expect(echellePourZone(0, 0, { w: 1280, h: 720 })).toBe(0);
		expect(echellePourZone(-10, 100, { w: 1280, h: 720 })).toBe(0);
	});
});

describe("dansCanvas / auCentreParDefaut / estMuet", () => {
	const canvas = { w: 1280, h: 720 };

	test("le point d'ancrage decide", () => {
		expect(dansCanvas(objet({ transform: { x: 640, y: 360, anchorX: 0.5, anchorY: 0.5, scaleX: 1, scaleY: 1, rot: 0 } }), canvas)).toBe(true);
		expect(dansCanvas(objet({ transform: { x: 2975, y: 1742, anchorX: 0.5, anchorY: 0.5, scaleX: 1, scaleY: 1, rot: 0 } }), canvas)).toBe(false);
	});

	test("le centre exact signale une position jamais posee par l'export", () => {
		expect(auCentreParDefaut(objet({ transform: { x: 640, y: 360, anchorX: 0.5, anchorY: 0.5, scaleX: 1, scaleY: 1, rot: 0 } }), canvas)).toBe(true);
		expect(auCentreParDefaut(objet({ transform: { x: 641, y: 360, anchorX: 0.5, anchorY: 0.5, scaleX: 1, scaleY: 1, rot: 0 } }), canvas)).toBe(false);
	});

	test("un objet est muet quand il ne produira aucun pixel", () => {
		expect(estMuet(objet())).toBe(true);
		expect(estMuet(objet({ sprite: { logicalPath: "x", w: 4, h: 92 } }))).toBe(false);
		expect(estMuet(objet({ text: [{ slot: "_text_btn02", text: "Suivant" }] }))).toBe(false);
		// Une fente vide ne produit rien non plus.
		expect(estMuet(objet({ text: [{ slot: "_text_btn02", text: "   " }] }))).toBe(true);
	});
});

describe("segmentsTexte", () => {
	test("le balisage de couleur n'atterrit pas dans le DOM", () => {
		// Pose tel quel, `[CTEAMPARAM01]` s'AFFICHE — un defaut visible que rien dans le pipeline
		// ne signale, puisque la chaine est correcte du point de vue de l'export.
		expect(texteNu("[CTEAMPARAM01]Bonus d'équipe[C]")).toBe("Bonus d'équipe");
	});

	test("chaque morceau garde le nom de sa couleur", () => {
		expect(segmentsTexte("avant[CROUGE]rouge[C]après")).toEqual([
			{ texte: "avant", couleur: null },
			{ texte: "rouge", couleur: "ROUGE" },
			{ texte: "après", couleur: null },
		]);
	});

	test("un texte sans balisage reste entier", () => {
		expect(segmentsTexte("Sauvegarder")).toEqual([{ texte: "Sauvegarder", couleur: null }]);
		expect(segmentsTexte("")).toEqual([]);
	});

	test("aucune fente du layout reel ne laisse passer de crochet", () => {
		for (const o of LAYOUT.objects) {
			for (const fente of o.text ?? []) {
				expect(texteNu(fente.text)).not.toInclude("[C");
			}
		}
	});
});

describe("lireLayout", () => {
	test("accepte le fichier embarque", () => {
		expect(LAYOUT.screen).toBe("mainmenu01");
		expect(LAYOUT.canvas).toEqual({ w: 1280, h: 720 });
		expect(LAYOUT.objects.length).toBe(34);
	});

	test("echoue bruyamment sur une forme cassee", () => {
		// Un `as LayoutJeu` ne verifierait rien : un reexport qui renommerait `objects` rendrait
		// une page vide, sans message, avec un typecheck vert.
		expect(() => lireLayout(null)).toThrow("n'est pas un objet");
		expect(() => lireLayout({ objects: [] })).toThrow("canvas");
		expect(() => lireLayout({ canvas: { w: 1280, h: 720 } })).toThrow("objects");
		expect(() =>
			lireLayout({ canvas: { w: 1280, h: 720 }, objects: [{ name: "x" }] }),
		).toThrow("transformation");
	});
});

describe("bilanLayout sur mainmenu01", () => {
	const bilan = bilanLayout(LAYOUT);

	test("les 34 objets sont comptes, et 26 seulement produiront une image", () => {
		expect(bilan.total).toBe(34);
		expect(bilan.visibles).toBe(34);
		expect(bilan.avecSprite).toBe(26);
		expect(bilan.avecTexte).toBe(8);
	});

	test("8 objets sont MUETS — c'est un echec de l'export, pas du rendu", () => {
		// Ces objets (`mainmenu01_00_background`, `_01_base_info`, `_04_menu_list`…) n'ont ni
		// texture ni texte : le rendu les pose, ils ne montrent rien. Le compter ici evite
		// d'annoncer « 34 objets rendus » quand 8 d'entre eux sont vides.
		expect(bilan.muets).toBe(8);
		expect(bilan.avecSprite + bilan.muets).toBe(bilan.total);
	});

	test("24 objets sur 34 n'ont jamais ete positionnes, et 5 sortent du canevas", () => {
		// Le runtime du jeu place ces widgets ; l'export ne capture pas cette etape. Le rendu ne
		// les deplace pas — il les DESIGNE.
		expect(bilan.auCentre).toBe(24);
		expect(bilan.horsCanvas).toBe(5);
	});

	test("17 textures distinctes, toutes sous `data/dx11/`", () => {
		expect(bilan.textures.length).toBe(17);
		expect(bilan.textures.every((t) => t.startsWith("data/dx11/menu/100_mainmenu/"))).toBe(true);
	});
});
