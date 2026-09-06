import { describe, expect, test } from "bun:test";
import { SETTINGS_DEFAULTS } from "../../lib/settings";
import {
	cycleValue,
	formatValue,
	isSettingVisible,
	SETTING_DEFINITIONS,
	visibleFamilies,
	visibleSettings,
} from "./settings-model";

/** Ce qu'Aphrody mesure sur `nie-site` : lecture seule, pas de disque, pas d'outils. */
const WEB = {
	vfs: true,
	texture: true,
	modele: true,
	avatar: true,
	audio: true,
	video: true,
	wiki: true,
	ecriture: false,
	disque: false,
	outils: false,
};
const DESKTOP = { ...WEB, ecriture: true, disque: true, outils: true };

describe("le modèle des réglages", () => {
	test("chaque identifiant est une clé du magasin, une seule fois, avec SA valeur par défaut", () => {
		const ids = SETTING_DEFINITIONS.map((d) => d.id);
		expect(new Set(ids).size).toBe(ids.length);
		for (const def of SETTING_DEFINITIONS) {
			expect(def.default).toBe(SETTINGS_DEFAULTS[def.id]);
		}
		// Et réciproquement : un réglage du magasin sans définition serait invisible partout.
		for (const key of Object.keys(SETTINGS_DEFAULTS)) {
			expect(ids as string[]).toContain(key);
		}
	});

	test("un réglage non portable nomme la capacité qui le rend visible", () => {
		for (const def of SETTING_DEFINITIONS) {
			if (!def.portable) expect(def.requires).toBeDefined();
		}
	});

	test("sur le web, seuls les portables sont visibles ; sur le bureau, tous", () => {
		const web = SETTING_DEFINITIONS.filter((d) => isSettingVisible(d, WEB));
		expect(web.every((d) => d.portable)).toBe(true);
		expect(web.map((d) => d.id)).toEqual([
			"locale",
			"listDensity",
			"reducedMotion",
			"theme",
			"accentTheme",
			"fontScale",
			"uiZoom",
		]);
		expect(SETTING_DEFINITIONS.every((d) => isSettingVisible(d, DESKTOP))).toBe(true);
		// Tant que la mesure court, rien de non portable n'apparaît.
		expect(SETTING_DEFINITIONS.filter((d) => isSettingVisible(d, null)).length).toBe(web.length);
	});

	test("une famille sans réglage visible n'est pas un onglet", () => {
		expect(visibleFamilies(WEB).map((f) => f.id)).toEqual(["general", "display"]);
		expect(visibleFamilies(DESKTOP).map((f) => f.id)).toEqual([
			"general",
			"display",
			"paths",
			"tools",
		]);
		expect(visibleSettings("paths", WEB)).toEqual([]);
	});

	test("← → bouclent sur les choix et les bascules, s'arrêtent aux bornes d'une plage", () => {
		const locale = SETTING_DEFINITIONS.find((d) => d.id === "locale")!;
		expect(cycleValue(locale, "fr", 1)).toBe("en");
		expect(cycleValue(locale, "fr", -1)).toBe("ja");
		const motion = SETTING_DEFINITIONS.find((d) => d.id === "reducedMotion")!;
		expect(cycleValue(motion, false, 1)).toBe(true);
		const zoom = SETTING_DEFINITIONS.find((d) => d.id === "uiZoom")!;
		expect(cycleValue(zoom, 1, 1)).toBe(1.1);
		expect(cycleValue(zoom, 1.5, 1)).toBe(1.5);
		expect(cycleValue(zoom, 0.7, -1)).toBe(0.7);
		// Arrondi au pas : pas de 1.2000000000000002.
		expect(cycleValue(zoom, 1.1, 1)).toBe(1.2);
	});

	test("la valeur affichée est un libellé, jamais un identifiant brut", () => {
		const theme = SETTING_DEFINITIONS.find((d) => d.id === "theme")!;
		expect(formatValue(theme, "dark")).toBe("Sombre");
		const zoom = SETTING_DEFINITIONS.find((d) => d.id === "uiZoom")!;
		expect(formatValue(zoom, 1.2)).toBe("120 %");
		const gameDir = SETTING_DEFINITIONS.find((d) => d.id === "gameDir")!;
		expect(formatValue(gameDir, "")).toBe("Automatique");
	});
});
