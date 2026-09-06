/**
 * Les réglages, côté React : lecture réactive, écriture, remise à zéro, et application au
 * document.
 *
 * Le magasin est celui d'Inacord (`lib/settings.ts`) — une seule clé `localStorage`,
 * `nie-explorer:settings`, les mêmes identifiants. Ce fichier n'en ajoute pas un second : il
 * expose ce que l'écran des Options a besoin de faire, et ce que l'hôte doit appliquer.
 */
import { useEffect } from "react";
import {
	ACCENT_THEMES,
	getSettings,
	SETTINGS_DEFAULTS,
	type Settings,
	setSettings,
	useSettings as useSettingsStore,
} from "../../lib/settings";
import type { SettingId } from "./settings-model";

export type { Settings };

/** Les réglages courants, réactifs, et les gestes pour les changer. */
export function useSettings(): {
	settings: Settings;
	set: (patch: Partial<Settings>) => void;
	reset: (ids: readonly SettingId[]) => void;
} {
	const settings = useSettingsStore();
	return { settings, set: setSettings, reset: resetSettings };
}

/** Remet ces réglages à leur valeur par défaut, et persiste. */
export function resetSettings(ids: readonly SettingId[]): void {
	const patch: Partial<Settings> = {};
	for (const id of ids) {
		// Le type de chaque champ est celui de sa clé : l'affectation par clé indexée l'oblige.
		(patch as Record<SettingId, unknown>)[id] = SETTINGS_DEFAULTS[id];
	}
	setSettings(patch);
}

/** La taille de police de base, en pixels, avant l'échelle. */
const BASE_FONT_SIZE_PX = 16;

/** Les classes des variantes de palette sombre — les mêmes que `apps/inacord/src/lib/appearance.ts`. */
const ACCENT_CLASS: Record<(typeof ACCENT_THEMES)[number], string> = {
	spacedrive: "",
	midnight: "midnight-theme",
	noir: "noir-theme",
	slate: "slate-theme",
	nord: "nord-theme",
	mocha: "mocha-theme",
};

/** Le thème résolu : `system` est tranché par `prefers-color-scheme`. */
export function resolveTheme(theme: Settings["theme"]): "light" | "dark" {
	if (theme !== "system") return theme;
	if (typeof window === "undefined" || !window.matchMedia) return "light";
	return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/**
 * Applique les réglages d'apparence au document.
 *
 * Un réglage enregistré qui ne change rien est un défaut : ce crochet est ce qui le fait
 * changer quelque chose. Il pose sur `<html>` — `data-theme`, `data-density`, `data-motion`,
 * `font-size` — ce que les feuilles de style lisent ; le `zoom` va sur `<body>`, comme sous
 * Inacord. L'hôte le monte une fois, à sa racine.
 *
 * La langue n'est PAS appliquée ici : sous Aphrody, changer de langue est une navigation
 * entière servie par `nie-site`, et c'est l'hôte qui la fait.
 */
export function useApplySettings(): void {
	const { theme, accentTheme, fontScale, uiZoom, reducedMotion, listDensity } =
		useSettingsStore();

	useEffect(() => {
		const root = document.documentElement;
		const apply = () => {
			const resolved = resolveTheme(theme);
			root.dataset.theme = resolved;
			for (const cls of Object.values(ACCENT_CLASS)) if (cls) root.classList.remove(cls);
			if (resolved === "dark" && ACCENT_CLASS[accentTheme]) {
				root.classList.add(ACCENT_CLASS[accentTheme]);
			}
		};
		apply();
		// `system` doit suivre le système quand il change, pas seulement au chargement.
		if (theme !== "system" || !window.matchMedia) return;
		const media = window.matchMedia("(prefers-color-scheme: dark)");
		media.addEventListener("change", apply);
		return () => media.removeEventListener("change", apply);
	}, [theme, accentTheme]);

	useEffect(() => {
		document.documentElement.style.fontSize = `${BASE_FONT_SIZE_PX * fontScale}px`;
	}, [fontScale]);

	useEffect(() => {
		// `zoom` n'est pas dans le typage CSSStyleDeclaration standard.
		(document.body.style as unknown as { zoom: string }).zoom = String(uiZoom);
	}, [uiZoom]);

	useEffect(() => {
		document.documentElement.dataset.motion = reducedMotion ? "reduced" : "full";
	}, [reducedMotion]);

	useEffect(() => {
		document.documentElement.dataset.density = listDensity;
	}, [listDensity]);
}

/** Lecture synchrone, hors React — pour un hôte qui décide avant de rendre (la langue). */
export { getSettings };
