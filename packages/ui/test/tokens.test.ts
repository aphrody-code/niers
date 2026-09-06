import { describe, expect, test } from "bun:test";

import {
	analyserBlocs,
	extraireTokens,
	NOMS_THEMES,
	oklchVersHex,
	rendreModule,
	resoudreVar,
	versCamel,
} from "../scripts/generer-tokens";
import { FORMES, MARQUE, MARQUES_TIERCES, THEMES } from "../src/tokens";

// `URL.pathname` n'est PAS un chemin de fichier : il rend `/C:/Users/…` sous
// Windows, que `Bun.file` n'ouvre pas, et il laisse les `%20` d'un dossier
// contenant un espace — donc il casse aussi sous Linux. `fileURLToPath` fait
// la conversion officielle. L'échec était total et muet : la lecture a lieu au
// CHARGEMENT du module, si bien que la suite entière rendait « 0 pass, 1 fail »
// au lieu de désigner l'assertion fautive.
const CHEMIN_CSS = Bun.fileURLToPath(new URL("../src/styles.css", import.meta.url));
const CHEMIN_TOKENS = Bun.fileURLToPath(new URL("../src/tokens.ts", import.meta.url));
const css = await Bun.file(CHEMIN_CSS).text();

describe("conversion oklch → sRGB", () => {
	// Valeurs de référence : les trois primaires sRGB exprimées en oklch par
	// la spec CSS Color 4. Si la matrice Oklab est fausse, elles dérivent.
	test.each([
		["oklch(1 0 0)", "#ffffff"],
		["oklch(0 0 0)", "#000000"],
		["oklch(0.62796 0.25768 29.234)", "#ff0000"],
		["oklch(0.86644 0.29483 142.495)", "#00ff00"],
		["oklch(0.45201 0.31321 264.052)", "#0000ff"],
	])("%s → %s", (entree, attendu) => {
		expect(oklchVersHex(entree)).toBe(attendu);
	});

	test("borne les couleurs hors gamut au lieu de déborder", () => {
		// Chroma volontairement inatteignable en sRGB : chaque canal doit rester
		// dans [0,255] (sans bornage, `octet()` produirait « #NaN » ou « 1a4 »).
		expect(oklchVersHex("oklch(0.7 0.9 150)")).toMatch(/^#[0-9a-f]{6}$/);
	});

	test("ignore ce qui n'est pas de l'oklch", () => {
		expect(oklchVersHex("#0c1730")).toBeNull();
		expect(oklchVersHex("rgba(255, 196, 110, 0.2)")).toBeNull();
	});
});

describe("analyse de styles.css", () => {
	test("isole le bloc :root sans confondre avec :root.theme-roy", () => {
		const blocs = analyserBlocs(css);
		const racines = blocs.filter((b) => b.selecteurs.includes(":root"));
		expect(racines.length).toBe(1);
		expect(racines[0]!.declarations.get("--rg-marine")).toBe("oklch(0.2 0.05 264)");
	});

	test("ne remonte pas les blocs @keyframes comme des jeux de tokens", () => {
		const blocs = analyserBlocs(css);
		expect(blocs.some((b) => b.selecteurs.some((s) => s === "from" || s === "to"))).toBe(false);
	});

	test("descend dans @layer base pour trouver les 4 thèmes", () => {
		const blocs = analyserBlocs(css);
		for (const nom of NOMS_THEMES) {
			expect(blocs.some((b) => b.selecteurs.includes(`.${nom}`))).toBe(true);
		}
	});
});

describe("résolution des var()", () => {
	const base = new Map([["--a", "#111111"]]);

	test("cascade local puis :root", () => {
		expect(resoudreVar("var(--a)", new Map(), base)).toBe("#111111");
		expect(resoudreVar("var(--a)", new Map([["--a", "#222222"]]), base)).toBe("#222222");
	});

	test("suit une chaîne d'indirections", () => {
		const local = new Map([
			["--b", "var(--c)"],
			["--c", "var(--a)"],
		]);
		expect(resoudreVar("var(--b)", local, base)).toBe("#111111");
	});

	test("coupe un cycle au lieu de boucler", () => {
		const local = new Map([
			["--x", "var(--y)"],
			["--y", "var(--x)"],
		]);
		expect(resoudreVar("var(--x)", local, new Map())).toBe("var(--x)");
	});

	test("utilise la valeur de repli", () => {
		expect(resoudreVar("var(--inconnu, #abcdef)", new Map(), new Map())).toBe("#abcdef");
	});
});

describe("tokens exposés", () => {
	test("aucune couleur ne reste en oklch (Canvas 2D la rendrait noire)", () => {
		const valeurs = [
			...Object.values(MARQUE),
			...Object.values(MARQUES_TIERCES),
			...NOMS_THEMES.flatMap((n) => [
				...Object.values(THEMES[n].md3),
				...Object.values(THEMES[n].shadcn),
			]),
		];
		expect(valeurs.filter((v) => v.includes("oklch"))).toEqual([]);
		expect(valeurs.filter((v) => v.includes("var("))).toEqual([]);
	});

	test("les 4 thèmes exposent les mêmes rôles MD3", () => {
		const reference = Object.keys(THEMES["theme-roy"].md3).sort();
		// 36 rôles : l'en-tête de styles.css en annonce 35, mais `on-background`
		// est bien présent dans les 4 thèmes — c'est le commentaire qui a dérivé.
		expect(reference.length).toBe(36);
		for (const nom of NOMS_THEMES) {
			expect(Object.keys(THEMES[nom].md3).sort()).toEqual(reference);
		}
	});

	test("un thème hérite de :root les rôles qu'il ne redéclare pas", () => {
		// `.theme-roy` ne déclare ni scrim ni shadow : la cascade doit les fournir.
		const blocRoy = analyserBlocs(css).find((b) => b.selecteurs.includes(".theme-roy"));
		expect(blocRoy?.declarations.has("--md-sys-color-scrim")).toBe(false);
		expect(THEMES["theme-roy"].md3.scrim).toBe("#000000");
	});

	test("le pont shadcn est résolu, pas laissé en référence", () => {
		// azalee-light écrit `--card: var(--md-sys-color-surface-container-low)`.
		expect(THEMES["theme-azalee-light"].shadcn.card).toBe(
			THEMES["theme-azalee-light"].md3.surfaceContainerLow
		);
	});

	test("l'échelle de formes reprend les 7 corners M3", () => {
		expect(FORMES).toEqual({
			none: 0,
			extraSmall: 4,
			small: 8,
			medium: 12,
			large: 16,
			extraLarge: 28,
			full: 9999,
		});
	});

	test("les couleurs de marque sont les 4 teintes de la DA", () => {
		for (const cle of ["marine", "griffon", "brique", "rose"] as const) {
			expect(MARQUE[cle]).toMatch(/^#[0-9a-f]{6}$/);
		}
	});
});

describe("non-dérive", () => {
	test("src/tokens.ts est bien la projection de styles.css", async () => {
		// Les fins de ligne sont normalisées des DEUX côtés avant comparaison.
		// Le générateur émet du LF ; `core.autocrlf=true` restitue le fichier
		// committé en CRLF sur un poste Windows. Comparer les octets bruts y
		// mesure la configuration de checkout, pas la projection — et le
		// diagnostic est odieux, `toBe` affichant un écart « -0 / +0 »
		// parfaitement invisible. Ce qui est testé, c'est le CONTENU.
		const enLf = (texte: string) => texte.replaceAll("\r\n", "\n");
		const committe = await Bun.file(CHEMIN_TOKENS).text();
		expect(enLf(rendreModule(extraireTokens(css)))).toBe(enLf(committe));
	});

	test("le module généré reste pur (aucun import)", async () => {
		const committe = await Bun.file(CHEMIN_TOKENS).text();
		expect(committe).not.toMatch(/^\s*import\s/m);
		expect(committe).not.toMatch(/require\(/);
	});
});

describe("versCamel", () => {
	test.each([
		["on-primary-container", "onPrimaryContainer"],
		["surface", "surface"],
		["extra-large", "extraLarge"],
	])("%s → %s", (entree, attendu) => {
		expect(versCamel(entree)).toBe(attendu);
	});
});
