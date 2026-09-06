/**
 * Le modèle des réglages — la liste typée de tout ce qu'Inacord laisse régler.
 *
 * ## Pourquoi il existe
 *
 * Inacord porte ses réglages dans `lib/settings.ts` (le magasin) et les dessine dans
 * `SettingsView.tsx` (l'écran), sans rien entre les deux : l'écran connaît chaque champ par
 * son nom, son libellé, ses bornes. Aphrody doit montrer les MÊMES réglages dans un autre écran
 * — celui des Options du jeu — et ne peut pas recopier cette connaissance sans qu'elle diverge.
 *
 * Ici, chaque réglage est une **définition** : son identifiant (celui du magasin, donc celui
 * d'Inacord — la synchronisation future n'a rien à traduire), sa famille (un onglet), son
 * libellé, sa description (la barre du bas), sa forme (`toggle`, `choice`, `range`, `text`) et
 * ce dont il a besoin de l'hôte pour avoir un sens.
 *
 * ## Portable, ou caché
 *
 * `portable: true` : le réglage a un sens dans un navigateur comme dans la fenêtre Tauri.
 * `portable: false` : il ne vaut que si l'hôte sait faire quelque chose — et ce quelque chose
 * est nommé par `requires`, une capacité du contrat `asset-source`. L'écran ne le montre que
 * si `useCapacites()` la mesure vraie. Aucune condition d'hôte n'est écrite dans un composant :
 * c'est la capacité qui décide, et un réglage caché n'est jamais montré-puis-en-échec.
 */
import type { CapacitesSource } from "@niers/asset-source";
import {
	ACCENT_THEMES,
	type AccentTheme,
	type ListDensity,
	type Locale,
	type Settings,
	type ThemeMode,
} from "../../lib/settings";

/** L'identifiant d'un réglage : une clé du magasin partagé, rien d'autre. */
export type SettingId = keyof Settings;

/** Une famille de réglages — un onglet de l'écran Options. */
export type SettingFamily = "general" | "display" | "paths" | "tools";

/** Une option d'un réglage à choix. */
export interface SettingOption<V = string> {
	value: V;
	label: string;
}

interface SettingBase<K extends SettingId> {
	id: K;
	family: SettingFamily;
	label: string;
	/** La phrase de la barre de description, sous la liste. */
	description: string;
	default: Settings[K];
	/** A un sens dans un navigateur. Faux ⇒ `requires` nomme la capacité qui le rend visible. */
	portable: boolean;
	/** La capacité de l'hôte sans laquelle ce réglage est caché. */
	requires?: keyof CapacitesSource;
}

export interface ToggleSetting<K extends SettingId> extends SettingBase<K> {
	kind: "toggle";
}

export interface ChoiceSetting<K extends SettingId> extends SettingBase<K> {
	kind: "choice";
	options: readonly SettingOption<Settings[K]>[];
}

export interface RangeSetting<K extends SettingId> extends SettingBase<K> {
	kind: "range";
	min: number;
	max: number;
	step: number;
	/** Comment afficher la valeur (`1.2` → « 120 % »). */
	format: (value: number) => string;
}

export interface TextSetting<K extends SettingId> extends SettingBase<K> {
	kind: "text";
	placeholder?: string;
}

export type SettingDefinition<K extends SettingId = SettingId> =
	| ToggleSetting<K>
	| ChoiceSetting<K>
	| RangeSetting<K>
	| TextSetting<K>;

/** Les familles, dans l'ordre des onglets, avec leur sous-titre (« Paramètres du jeu »). */
export const SETTING_FAMILIES: readonly { id: SettingFamily; label: string }[] = [
	{ id: "general", label: "Paramètres généraux" },
	{ id: "display", label: "Affichage" },
	{ id: "paths", label: "Chemins" },
	{ id: "tools", label: "Outils" },
];

/** Libellés des langues, dans leur propre langue — les mêmes qu'Inacord (`LOCALE_LABELS`). */
export const LOCALE_OPTIONS: readonly SettingOption<Locale>[] = [
	{ value: "fr", label: "Français" },
	{ value: "en", label: "English" },
	{ value: "ja", label: "日本語" },
];

const THEME_OPTIONS: readonly SettingOption<ThemeMode>[] = [
	{ value: "system", label: "Système" },
	{ value: "light", label: "Clair" },
	{ value: "dark", label: "Sombre" },
];

/** Les mêmes libellés qu'Inacord (`ACCENT_THEME_LABELS`), dans le même ordre. */
const ACCENT_LABELS: Record<AccentTheme, string> = {
	spacedrive: "Spacedrive",
	midnight: "Midnight",
	noir: "Noir",
	slate: "Slate",
	nord: "Nord",
	mocha: "Mocha",
};

const DENSITY_OPTIONS: readonly SettingOption<ListDensity>[] = [
	{ value: "comfortable", label: "Confortable" },
	{ value: "compact", label: "Compacte" },
];

const percent = (v: number) => `${Math.round(v * 100)} %`;

/**
 * Tous les réglages, dans l'ordre d'affichage.
 *
 * Les valeurs par défaut sont celles de `lib/settings.ts` — recopiées ici pour que la
 * définition se lise seule, et vérifiées contre le magasin par le test du modèle.
 */
export const SETTING_DEFINITIONS: readonly SettingDefinition[] = [
	// ── Général ─────────────────────────────────────────────────────────────────────────────
	{
		id: "locale",
		family: "general",
		kind: "choice",
		label: "Langue du texte",
		description: "Change la langue de l'interface. Sur le web, la page est rechargée dans sa langue.",
		options: LOCALE_OPTIONS,
		default: "fr",
		portable: true,
	},
	{
		id: "listDensity",
		family: "general",
		kind: "choice",
		label: "Densité des listes",
		description: "Resserre ou détend l'espacement des listes et des grilles.",
		options: DENSITY_OPTIONS,
		default: "comfortable",
		portable: true,
	},
	{
		id: "reducedMotion",
		family: "general",
		kind: "toggle",
		label: "Réduire les animations",
		description: "Supprime les glissements et les pulsations de l'interface.",
		default: false,
		portable: true,
	},
	{
		id: "outilsAvances",
		family: "general",
		kind: "toggle",
		label: "Outils avancés",
		description:
			"Affiche les outils de spécialiste dans la barre latérale : RE, Viola, Live mod, Lua.",
		default: false,
		// Ces outils lisent la mémoire du jeu et désassemblent des scripts : ils n'existent
		// que sur un hôte qui sait les exécuter.
		portable: false,
		requires: "outils",
	},

	// ── Affichage ───────────────────────────────────────────────────────────────────────────
	{
		id: "theme",
		family: "display",
		kind: "choice",
		label: "Thème",
		description: "Clair, sombre, ou celui du système.",
		options: THEME_OPTIONS,
		default: "system",
		portable: true,
	},
	{
		id: "accentTheme",
		family: "display",
		kind: "choice",
		label: "Palette sombre",
		description: "La variante de palette utilisée en thème sombre.",
		options: ACCENT_THEMES.map((value) => ({ value, label: ACCENT_LABELS[value] })),
		default: "spacedrive",
		portable: true,
	},
	{
		id: "fontScale",
		family: "display",
		kind: "range",
		label: "Taille du texte",
		description: "Échelle de la taille de police de base ; tout le reste suit.",
		min: 0.8,
		max: 1.4,
		step: 0.1,
		format: percent,
		default: 1,
		portable: true,
	},
	{
		id: "uiZoom",
		family: "display",
		kind: "range",
		label: "Zoom de l'interface",
		description: "Agrandit ou réduit toute l'interface.",
		min: 0.7,
		max: 1.5,
		step: 0.1,
		format: percent,
		default: 1,
		portable: true,
	},

	// ── Chemins (disque de la machine) ──────────────────────────────────────────────────────
	{
		id: "gameDir",
		family: "paths",
		kind: "text",
		label: "Répertoire du jeu",
		description: "Vide = auto-détection (NIE_GAME_DIR, dossier courant, puis Steam).",
		default: "",
		portable: false,
		requires: "disque",
	},
	{
		id: "wikiDb",
		family: "paths",
		kind: "text",
		label: "Miroir wiki (SQLite)",
		description: "Vide = résolution automatique. Sert à afficher les noms réels dans l'explorateur.",
		default: "",
		portable: false,
		requires: "disque",
	},
	{
		id: "blenderExe",
		family: "paths",
		kind: "text",
		label: "Blender",
		description: "Le chemin de blender.exe, pour l'extension niers-blender.",
		default: "",
		portable: false,
		requires: "disque",
	},
	{
		id: "azaleeUrl",
		family: "paths",
		kind: "text",
		label: "Résolveur distant Azalée",
		description: "L'origine du résolveur de personnages, pour le pont Blender.",
		default: "",
		portable: false,
		requires: "outils",
	},
	{
		id: "modelServiceUrl",
		family: "paths",
		kind: "text",
		label: "Service de modèles",
		description: "L'origine de nie-model-serve. Sur le web, le site en tient lieu.",
		default: "",
		portable: false,
		requires: "outils",
	},

	// ── Outils ──────────────────────────────────────────────────────────────────────────────
	{
		id: "bridgeEnabled",
		family: "tools",
		kind: "toggle",
		label: "Pont MCP",
		description: "Autorise le serveur MCP à piloter cette fenêtre par le pont local.",
		default: true,
		portable: false,
		requires: "outils",
	},
];

/**
 * Un réglage est-il visible pour cet hôte ?
 *
 * `capacites` vaut `null` tant que la mesure court : on ne montre alors QUE le portable —
 * jamais un réglage qui pourrait disparaître une seconde plus tard.
 */
export function isSettingVisible(
	def: SettingDefinition,
	capacites: CapacitesSource | null,
): boolean {
	if (def.portable) return true;
	if (!def.requires || !capacites) return false;
	return Boolean(capacites[def.requires]);
}

/** Les réglages visibles d'une famille, dans l'ordre. */
export function visibleSettings(
	family: SettingFamily,
	capacites: CapacitesSource | null,
): SettingDefinition[] {
	return SETTING_DEFINITIONS.filter(
		(d) => d.family === family && isSettingVisible(d, capacites),
	);
}

/** Les familles qui ont au moins un réglage visible — un onglet vide n'est pas un onglet. */
export function visibleFamilies(
	capacites: CapacitesSource | null,
): { id: SettingFamily; label: string }[] {
	return SETTING_FAMILIES.filter((f) => visibleSettings(f.id, capacites).length > 0);
}

/**
 * La valeur suivante d'un réglage quand on le fait défiler avec ← → (`direction` = ±1).
 *
 * Les choix et les bascules bouclent, comme dans le jeu ; une plage s'arrête à ses bornes.
 * Un texte ne défile pas : il rend sa valeur telle quelle.
 */
export function cycleValue<K extends SettingId>(
	def: SettingDefinition<K>,
	current: Settings[K],
	direction: 1 | -1,
): Settings[K] {
	switch (def.kind) {
		case "toggle":
			return !current as Settings[K];
		case "choice": {
			const options = def.options;
			const index = options.findIndex((o) => o.value === current);
			const next = (index + direction + options.length) % options.length;
			return options[next]!.value;
		}
		case "range": {
			const value = Number(current) + direction * def.step;
			// Arrondi au pas : 0.1 + 0.2 ne fait pas 0.3 en flottant.
			const decimals = String(def.step).split(".")[1]?.length ?? 0;
			const clamped = Math.min(def.max, Math.max(def.min, value));
			return Number(clamped.toFixed(decimals)) as Settings[K];
		}
		case "text":
			return current;
	}
}

/** Le libellé affiché pour la valeur d'un réglage. */
export function formatValue<K extends SettingId>(
	def: SettingDefinition<K>,
	value: Settings[K],
): string {
	switch (def.kind) {
		case "toggle":
			return value ? "Activée" : "Désactivée";
		case "choice":
			return def.options.find((o) => o.value === value)?.label ?? String(value);
		case "range":
			return def.format(Number(value));
		case "text":
			return String(value) || "Automatique";
	}
}
