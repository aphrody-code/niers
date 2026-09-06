/**
 * La géométrie de `mainmenu01` : ce que les mesures doivent continuer de vérifier.
 *
 * Ces tests ne re-mesurent pas la capture — c'est le rôle de
 * `scripts/validation/mesurer-mainmenu.py`, et la capture est gitignorée (assets © LEVEL-5).
 * Ils vérifient les INVARIANTS que la mesure a établis, pour qu'une valeur retouchée à la main
 * dans `geometrie-mainmenu.ts` ne passe pas sans bruit :
 *
 * - toute boîte tient dans le canevas 1280×720 ;
 * - les deux rangées, mesurées SÉPARÉMENT, donnent la même largeur de tuile — c'est ce
 *   recoupement qui prouve que 137 px est une unité de la direction artistique et non le
 *   résultat d'un remplissage ;
 * - le biseau va dans le sens mesuré (le haut décalé à droite), le défaut que la première
 *   reconstruction avait inversé sans que rien ne le signale.
 */
import {
	ANGLE_TUILE_DEG,
	biseau,
	type Boite,
	BOITES,
	ECART_TUILE,
	FOND_MENU,
	LARGEUR_TUILE,
	largeurTuile,
	PENTE_PANNEAU,
	PENTE_TUILE,
} from "@niers/inacord-ui";
import { describe, expect, test } from "bun:test";

/** Le canevas du jeu, celui de l'export de layout. */
const CANVAS = { w: 1280, h: 720 };

/**
 * Les boîtes complètes (celles qui portent les quatre champs).
 *
 * `flatMap` plutôt qu'un `filter` à prédicat de type : `BOITES` est un `as const`, donc chaque
 * entrée a un type littéral (`{ x: 1042 }`, pas `{ x: number }`) qu'aucun prédicat sur `Boite`
 * n'accepte. On recopie les quatre champs, ce qui élargit les littéraux au passage.
 */
const RECTANGLES: [string, Boite][] = Object.entries(BOITES).flatMap(([nom, valeur]) => {
	const c = valeur as Partial<Boite>;
	return typeof c.x === "number" &&
		typeof c.y === "number" &&
		typeof c.l === "number" &&
		typeof c.h === "number"
		? [[nom, { x: c.x, y: c.y, l: c.l, h: c.h }] as [string, Boite]]
		: [];
});

describe("les boîtes mesurées", () => {
	test("il y en a une par bloc de l'écran, et elles portent les quatre champs", () => {
		// 12 rectangles complets ; `panneaux` et les deux bords intérieurs n'en sont pas — ils
		// décrivent une bande et deux droites, et c'est le filtre qui les écarte.
		expect(RECTANGLES.length).toBe(12);
	});

	test("aucune ne sort du canevas 1280×720", () => {
		// On collecte AVANT d'asserter : une assertion par boîte s'arrête à la première fautive
		// et cache les suivantes, alors que le message utile est la liste entière.
		const dehors = RECTANGLES.filter(
			([, b]) => b.x < 0 || b.y < 0 || b.x + b.l > CANVAS.w || b.y + b.h > CANVAS.h,
		).map(([nom, b]) => `${nom} (${b.x},${b.y},${b.l},${b.h})`);
		expect(dehors).toEqual([]);
	});

	test("les deux panneaux laissent la place du logo entre leurs bords", () => {
		// Le bord intérieur du panneau gauche le plus large (en bas) reste franchement à gauche
		// de celui du panneau droit : sans cet écart, les panneaux se rejoignent et le titre se
		// pose par-dessus — c'est exactement ce que faisait la première version.
		expect(BOITES.panneauGaucheBord.bas).toBeLessThan(BOITES.panneauDroitBord.bas);
		expect(BOITES.panneauDroitBord.bas - BOITES.panneauGaucheBord.bas).toBeGreaterThan(200);
	});

	test("les panneaux s'écartent vers le HAUT, à l'inverse des tuiles", () => {
		// Le vide central est large en haut (le logo y tient) et se resserre vers le bas.
		expect(BOITES.panneauGaucheBord.haut).toBeLessThan(BOITES.panneauGaucheBord.bas);
		expect(BOITES.panneauDroitBord.haut).toBeGreaterThan(BOITES.panneauDroitBord.bas);
	});

	test("le bandeau n'est PAS centré sur l'écran", () => {
		// Son centre mesuré tombe à 813, pas à 640 : le centrer « parce que ça semble logique »
		// est une erreur que seule la mesure attrape.
		const centre = BOITES.bandeau.x + BOITES.bandeau.l / 2;
		expect(Math.round(centre)).toBe(813);
		expect(Math.abs(centre - CANVAS.w / 2)).toBeGreaterThan(100);
	});
});

describe("la tuile est une unité de la DA", () => {
	test("les deux rangées, mesurées séparément, rendent la même largeur de tuile", () => {
		const haute = largeurTuile(BOITES.rangee.l, 8);
		const basse = largeurTuile(BOITES.rangeeBasse.l, 3);
		expect(haute).toBe(basse);
		expect(haute).toBe(LARGEUR_TUILE);
	});

	test("une rangée de n tuiles tient dans la largeur dont elle est déduite", () => {
		for (const n of [3, 5, 8]) {
			const l = largeurTuile(BOITES.rangee.l, n);
			expect(l * n + ECART_TUILE * (n - 1)).toBeLessThanOrEqual(BOITES.rangee.l + n);
		}
	});
});

describe("la pente", () => {
	test("les tuiles penchent dans le sens mesuré : le haut vers la droite", () => {
		// dx/dy negatif = plus bas dans l'ecran, plus a gauche.
		expect(PENTE_TUILE).toBeLessThan(0);
		expect(PENTE_TUILE).toBeCloseTo(-0.4, 3);
	});

	test("l'angle et la pente décrivent la même chose", () => {
		expect((Math.atan(PENTE_TUILE) * 180) / Math.PI).toBeCloseTo(ANGLE_TUILE_DEG, 1);
	});

	test("les panneaux penchent PLUS que les tuiles, et dans l'autre sens", () => {
		expect(PENTE_PANNEAU).toBeGreaterThan(0);
		expect(PENTE_PANNEAU).toBeGreaterThan(Math.abs(PENTE_TUILE));
	});

	test("le biseau d'une forme est proportionnel à sa hauteur", () => {
		expect(biseau(BOITES.rangee.h)).toBe(Math.round(0.4 * BOITES.rangee.h));
		expect(biseau(100, PENTE_PANNEAU)).toBe(55);
	});
});

describe("le fond", () => {
	test("l'écran du menu est presque blanc, pas bleu", () => {
		// #f9fdf9 : c'est 69 % de la surface de la capture. Un fond bleu — le defaut de la
		// premiere version — change la lecture de tout l'ecran.
		expect(FOND_MENU).toBe("#f9fdf9");
		const [r = 0, v = 0, b = 0] = [1, 3, 5].map((i) =>
			Number.parseInt(FOND_MENU.slice(i, i + 2), 16),
		);
		expect(Math.min(r, v, b)).toBeGreaterThan(240);
		expect(v).toBeGreaterThanOrEqual(Math.max(r, b));
	});
});
