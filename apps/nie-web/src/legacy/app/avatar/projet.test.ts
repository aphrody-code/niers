import { describe, expect, test } from "bun:test";
import { lireProjet, nouveauProjet, reduireProjet, type Historique } from "./projet";
import type { Catalogue } from "./types";

const catalogue = { categories: [{ faceSettingType: 4, parts: [{ id: "cheveux" }], couleurs: ["brun"] }], modelesDeBase: { morphologies: ["male", "female"] } } as Catalogue;
const creer = () => nouveauProjet({ choix: { 4: "cheveux" }, valeurs: { "couleur.4": 0 }, champs: { nom: "Émile" }, genre: 0, morphologie: 1 });
const lire = (p: unknown) => lireProjet(JSON.stringify(p), catalogue);

describe("document NIE", () => {
	test("aller-retour complet et indépendant de l'entrée", () => {
		const p = creer(); p.transformation = { rotation: -180, echelle: 4 };
		const copie = lire(p); expect(copie).toEqual(p);
		copie.avatar.choix[4] = ""; expect(p.avatar.choix[4]).toBe("cheveux");
	});
	test("accepte les valeurs par défaut", () => {
		const p = creer(); p.avatar.choix[4] = ""; p.avatar.valeurs["couleur.4"] = -1;
		expect(lire(p)).toEqual(p);
	});
	for (const [nom, mutation] of Object.entries({
		version: (p: any) => { p.version = 2; },
		piece: (p: any) => { p.avatar.choix[4] = "inconnue"; },
		categorie: (p: any) => { p.avatar.choix[99] = ""; },
		genre: (p: any) => { p.avatar.genre = 0.5; },
		morphologie: (p: any) => { p.avatar.morphologie = 2; },
		rotation: (p: any) => { p.transformation.rotation = 181; },
		echelle: (p: any) => { p.transformation.echelle = 0; },
		infini: (p: any) => { p.transformation.echelle = Infinity; },
		couleur: (p: any) => { p.avatar.valeurs["couleur.4"] = 1; },
		fraction: (p: any) => { p.avatar.valeurs["couleur.4"] = 0.5; },
		taille: (p: any) => { p.avatar.valeurs.taille = 15; },
		texte: (p: any) => { p.avatar.champs.nom = "a".repeat(501); },
		tableau: (p: any) => { p.avatar.choix = []; },
	})) test(`refuse ${nom}`, () => { const p = creer(); mutation(p); expect(() => lire(p)).toThrow(); });
	test("refuse JSON tronqué et trop volumineux", () => {
		expect(() => lireProjet("{", catalogue)).toThrow();
		expect(() => lireProjet(" ".repeat(100001), catalogue)).toThrow();
	});
	test("refuse les clés de pollution de prototype", () => {
		for (const cle of ["__proto__", "constructor", "prototype"]) {
			const p = creer(); p.avatar.valeurs = JSON.parse(`{"${cle}":1}`);
			expect(() => lire(p)).toThrow();
		}
	});
	test("ne restaure pas les URL injectées", () => {
		const p = { ...creer(), url: "https://example.invalid/malveillant" };
		expect(lire(p)).not.toHaveProperty("url");
	});
});

describe("historique", () => {
	const init = (): Historique => ({ passes: [], present: creer(), futurs: [] });
	test("annuler et rétablir restaurent la recette et les transformations", () => {
		const h = init(); const projet = { ...creer(), transformation: { rotation: 90, echelle: 2 } };
		const modifie = reduireProjet(h, { type: "modifier", projet });
		const annule = reduireProjet(modifie, { type: "annuler" });
		expect(annule.present).toEqual(h.present);
		expect(reduireProjet(annule, { type: "retablir" }).present).toEqual(projet);
		expect(h.passes).toHaveLength(0);
	});
	test("une nouvelle branche efface le futur", () => {
		let h = init(); h = reduireProjet(h, { type: "modifier", projet: { ...creer(), nom: "A" } });
		h = reduireProjet(h, { type: "annuler" });
		h = reduireProjet(h, { type: "modifier", projet: { ...creer(), nom: "B" } });
		expect(h.futurs).toHaveLength(0);
	});
	test("bornes vides et modification identique sans effet", () => {
		const h = init(); expect(reduireProjet(h, { type: "annuler" })).toBe(h);
		expect(reduireProjet(h, { type: "retablir" })).toBe(h);
		expect(reduireProjet(h, { type: "modifier", projet: creer() })).toBe(h);
	});
	test("garde 50 étapes au maximum", () => {
		let h = init(); for (let i = 0; i < 100; i++) h = reduireProjet(h, { type: "modifier", projet: { ...creer(), nom: String(i) } });
		expect(h.passes).toHaveLength(50);
	});
});
