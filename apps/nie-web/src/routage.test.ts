import { describe, expect, test } from "bun:test";

import { cheminPourEntree, entreeDemandee, separerLangue } from "./routage";

const ENTREES = ["textures", "modeles", "sons", "videos", "explorateur"] as const;

describe("separerLangue", () => {
	test("le français n'a pas de préfixe", () => {
		expect(separerLangue("/")).toEqual({ prefixe: "", route: "/" });
		expect(separerLangue("/textures")).toEqual({ prefixe: "", route: "/textures" });
	});

	test("les deux autres langues sont un segment", () => {
		expect(separerLangue("/en/textures")).toEqual({ prefixe: "/en", route: "/textures" });
		expect(separerLangue("/ja/sons")).toEqual({ prefixe: "/ja", route: "/sons" });
		// La racine d'une langue, sans barre finale.
		expect(separerLangue("/ja")).toEqual({ prefixe: "/ja", route: "/" });
	});

	test("une route qui commence par les mêmes lettres n'est pas une langue", () => {
		// Comparer sur les caractères et non sur le segment enverrait `/enemy` en anglais avec
		// une route tronquée à `emy` — une page introuvable, sans message.
		expect(separerLangue("/enemy")).toEqual({ prefixe: "", route: "/enemy" });
		expect(separerLangue("/january")).toEqual({ prefixe: "", route: "/january" });
	});
});

describe("entreeDemandee", () => {
	const emplacement = (pathname: string, search = "") => ({ pathname, search });

	test("le chemin fait foi", () => {
		expect(entreeDemandee(ENTREES, emplacement("/textures"))).toBe("textures");
		expect(entreeDemandee(ENTREES, emplacement("/ja/videos"))).toBe("videos");
		expect(entreeDemandee(ENTREES, emplacement("/en/explorateur"))).toBe("explorateur");
	});

	test("l'accueil ne désigne aucune entrée", () => {
		expect(entreeDemandee(ENTREES, emplacement("/"))).toBeNull();
		expect(entreeDemandee(ENTREES, emplacement("/ja"))).toBeNull();
	});

	test("une route inconnue ne désigne aucune entrée", () => {
		expect(entreeDemandee(ENTREES, emplacement("/inexistante"))).toBeNull();
	});

	test("l'ancienne forme `?vue=` reste comprise", () => {
		// Un signet ou un lien déjà partagé ne doit pas se casser.
		expect(entreeDemandee(ENTREES, emplacement("/", "?vue=sons"))).toBe("sons");
		expect(entreeDemandee(ENTREES, emplacement("/ja", "?vue=modeles"))).toBe("modeles");
	});

	test("le chemin l'emporte sur l'ancien paramètre", () => {
		expect(entreeDemandee(ENTREES, emplacement("/textures", "?vue=sons"))).toBe("textures");
	});

	test("la route annoncée par le serveur sert de repli", () => {
		// `data-route` est déjà séparé de sa langue par nie-site : il fait autorité quand le
		// chemin vu par le client a été réécrit en amont.
		expect(entreeDemandee(ENTREES, emplacement("/"), "/modeles")).toBe("modeles");
		// Mais il ne prime pas sur un chemin qui désigne déjà une entrée.
		expect(entreeDemandee(ENTREES, emplacement("/sons"), "/modeles")).toBe("sons");
	});
});

describe("cheminPourEntree", () => {
	test("compose le chemin canonique", () => {
		expect(cheminPourEntree("", "textures")).toBe("/textures");
		expect(cheminPourEntree("/ja", "textures")).toBe("/ja/textures");
		expect(cheminPourEntree("/en", "explorateur")).toBe("/en/explorateur");
	});

	test("aller et retour", () => {
		// Ce que l'on écrit dans l'URL doit être ce que l'on y relit.
		for (const prefixe of ["", "/en", "/ja"]) {
			for (const entree of ENTREES) {
				const chemin = cheminPourEntree(prefixe, entree);
				expect(separerLangue(chemin).prefixe).toBe(prefixe);
				expect(entreeDemandee(ENTREES, { pathname: chemin, search: "" })).toBe(entree);
			}
		}
	});
});
