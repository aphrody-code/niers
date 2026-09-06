/**
 * L'écran des Options — celui du jeu, avec les réglages d'Inacord dedans.
 *
 * ## Ce qu'il montre
 *
 * Le bandeau « Options » avec sa tuile d'engrenage, la bande d'onglets (une famille par
 * onglet, W et C aux extrémités), le sous-titre de la famille, les lignes blanches penchées,
 * la ligne active en bleu saturé avec ses flèches, la barre de description en bas, et les
 * guides de touches. Chaque touche dessinée est écoutée — c'est la règle de ce dépôt : une
 * affordance est vérifiée avant d'être dessinée. Les formes viennent de `components/game`
 * (`GameHeaderBar`, `GameTabStrip`, `GameHintBar`, `GameCursor`) ; rien n'est redessiné ici.
 *
 * ## Ce qu'il ne montre pas
 *
 * Les réglages que l'hôte ne sait pas honorer. Il ne le décide pas lui-même : il demande
 * `useCapacites()` et laisse le modèle filtrer (`visibleSettings`). Sous Aphrody, ni chemin de
 * disque, ni pont MCP, ni outils avancés — et aucune condition d'hôte n'est écrite ici.
 *
 * ## Les touches
 *
 * | Touche | Effet |
 * |---|---|
 * | ↑ ↓ | ligne précédente / suivante (dans une liste ouverte : option précédente / suivante) |
 * | ← → | valeur précédente / suivante — appliquée aussitôt |
 * | Entrée | confirme : ouvre ou ferme la liste d'un choix, bascule un interrupteur |
 * | Échap | ferme la liste ouverte, sinon revient à l'écran précédent |
 * | V | applique : `onApply` reçoit les réglages et ce qui a changé (l'hôte y fait la langue) |
 * | X | réinitialise la famille affichée à ses valeurs par défaut |
 * | W / C | onglet précédent / suivant |
 *
 * Une valeur changée par ← → est écrite tout de suite : le thème et la taille du texte se
 * voient pendant qu'on les règle, comme dans le jeu. « Appliquer » sert à ce qui ne peut pas
 * se voir sans recharger — la langue.
 */
import type { ReactNode } from "react";
import { useCallback, useMemo, useRef, useState } from "react";
import type { Settings } from "../../lib/settings";
import { GLYPHES } from "../../shell/ecran-menu";
import { useCapacites } from "../../source";
import { GameCursor } from "../game/GameCursor";
import { GameHeaderBar } from "../game/GameHeaderBar";
import { type GameHint, GameHintBar } from "../game/GameHintBar";
import { GameKeyCap } from "../game/GameKeyHint";
import { type GameTab, GameTabStrip } from "../game/GameTabStrip";
import { useGameKeys } from "../game/keys";
import { SettingList } from "./SettingList";
import {
	cycleValue,
	SETTING_FAMILIES,
	type SettingFamily,
	type SettingId,
	visibleFamilies,
	visibleSettings,
} from "./settings-model";
import { useSettings } from "./use-settings";

/** Le glyphe de chaque onglet — les formes de `ecran-menu`, aucune n'est inventée ici. */
const FAMILY_ICON: Record<SettingFamily, ReactNode> = {
	general: GLYPHES.info,
	display: GLYPHES.image,
	paths: GLYPHES.arbre,
	tools: GLYPHES.engrenage,
};

export function SettingsScreen({
	title = "Options",
	onBack,
	onApply,
	backLabel = "Retour",
	initialFamily,
}: {
	title?: string;
	/** L'onglet ouvert à l'arrivée — un lien profond (`?tab=display`) ; sinon le premier. */
	initialFamily?: SettingFamily;
	/** Échap, ou le bouton de retour. */
	onBack: () => void;
	/**
	 * V : appelé avec les réglages courants et les identifiants qui ont changé depuis
	 * l'ouverture de l'écran (ou depuis la dernière application).
	 */
	onApply?: (settings: Settings, changed: SettingId[]) => void;
	/** Le libellé du guide de retour, à gauche de la barre du bas. */
	backLabel?: string;
}) {
	const capacites = useCapacites();
	const { settings, set, reset } = useSettings();

	const families = useMemo(() => visibleFamilies(capacites), [capacites]);
	const [family, setFamily] = useState<SettingFamily>(
		initialFamily ?? SETTING_FAMILIES[0]!.id,
	);
	const [focus, setFocus] = useState(0);
	const [listOpen, setListOpen] = useState(false);

	// Une famille peut disparaître quand la mesure des capacités arrive : on retombe sur la
	// première visible plutôt que d'afficher un onglet vide.
	const familyIndex = Math.max(
		0,
		families.findIndex((f) => f.id === family),
	);
	const current = families[familyIndex] ?? families[0];
	const definitions = useMemo(
		() => (current ? visibleSettings(current.id, capacites) : []),
		[current, capacites],
	);
	const focusIndex = Math.min(focus, Math.max(0, definitions.length - 1));
	const focused = definitions[focusIndex];

	// Ce qui a changé depuis l'ouverture : c'est ce que `onApply` reçoit.
	const opening = useRef<Settings>(settings);

	const selectFamily = useCallback((id: string) => {
		setFamily(id as SettingFamily);
		setFocus(0);
		setListOpen(false);
	}, []);

	const change = useCallback(
		<K extends SettingId>(id: K, value: Settings[K]) => {
			set({ [id]: value } as Partial<Settings>);
		},
		[set],
	);

	const cycle = useCallback(
		(direction: 1 | -1) => {
			if (!focused || focused.kind === "text") return;
			change(focused.id, cycleValue(focused, settings[focused.id], direction));
		},
		[focused, change, settings],
	);

	const move = useCallback(
		(direction: 1 | -1) => {
			const count = definitions.length;
			if (listOpen && focused?.kind === "choice") cycle(direction);
			else if (count) setFocus((f) => (f + direction + count) % count);
		},
		[definitions.length, listOpen, focused, cycle],
	);

	const confirm = useCallback(() => {
		if (!focused) return;
		if (focused.kind === "choice") setListOpen((open) => !open);
		else if (focused.kind === "toggle") cycle(1);
	}, [focused, cycle]);

	const apply = useCallback(() => {
		const ids = Object.keys(settings) as SettingId[];
		const changed = ids.filter((id) => settings[id] !== opening.current[id]);
		opening.current = settings;
		onApply?.(settings, changed);
	}, [onApply, settings]);

	const resetFamily = useCallback(() => {
		reset(definitions.map((d) => d.id));
	}, [definitions, reset]);

	const back = useCallback(() => {
		if (listOpen) setListOpen(false);
		else onBack();
	}, [listOpen, onBack]);

	const stepFamily = useCallback(
		(direction: 1 | -1) => {
			if (!families.length) return;
			selectFamily(families[(familyIndex + direction + families.length) % families.length]!.id);
		},
		[families, familyIndex, selectFamily],
	);

	// Les touches qui n'ont pas de guide dans la barre du bas : elles sont dessinées ailleurs
	// (les flèches sur la ligne active, W et C sur la bande d'onglets).
	const navigationKeys = useMemo(
		() => [
			{ key: "ArrowUp", onActivate: () => move(-1) },
			{ key: "ArrowDown", onActivate: () => move(1) },
			{ key: "ArrowLeft", onActivate: () => cycle(-1) },
			{ key: "ArrowRight", onActivate: () => cycle(1) },
			{ key: "w", onActivate: () => stepFamily(-1) },
			{ key: "c", onActivate: () => stepFamily(1) },
		],
		[move, cycle, stepFamily],
	);
	useGameKeys(navigationKeys);

	// Les guides du bas : chaque touche vient avec son action — la barre les branche.
	const hints = useMemo<GameHint[]>(
		() => [
			{ key: "Enter", keyLabel: "↵", label: "Confirmer", onActivate: confirm },
			{ key: "v", label: "Appliquer", onActivate: apply },
			{ key: "x", label: "Réinitialiser", onActivate: resetFamily },
		],
		[confirm, apply, resetFamily],
	);
	const escape = useMemo(
		() => [{ key: "Escape", onActivate: back, fromInputs: true }],
		[back],
	);
	useGameKeys(escape);

	const tabs: GameTab[] = families.map((f) => ({
		id: f.id,
		label: f.label,
		icon: FAMILY_ICON[f.id],
	}));

	return (
		// La géométrie de l'écran, mesurée sur `options.png` (2560×1440) : les lignes vont de
		// x=480 à x=2080, soit 62,5 % de la largeur ; la bande d'onglets en fait 46 %. Le
		// bandeau, la barre de description et les guides prennent toute la largeur.
		<div
			className="game-screen game-screen--settings"
			style={{
				display: "flex",
				flexDirection: "column",
				height: "100%",
				background: "var(--jeu-ciel-clair)",
				color: "var(--jeu-nuit-profonde)",
			}}
		>
			<GameHeaderBar icon={GLYPHES.engrenage} title={title} />
			<div
				className="game-screen__body"
				style={{
					flex: 1,
					display: "flex",
					flexDirection: "column",
					alignItems: "center",
					gap: "var(--jeu-espace-l)",
					padding: "var(--jeu-espace-xl) 0",
					overflow: "auto",
				}}
			>
				{current ? (
					<div style={{ width: "46%" }}>
						<GameTabStrip tabs={tabs} value={current.id} onChange={selectFamily} />
					</div>
				) : null}
				{current ? (
					<SettingList
						label={current.label}
						style={{ width: "62.5%" }}
						definitions={definitions}
						values={settings}
						focus={focusIndex}
						listOpen={listOpen}
						onFocus={(i) => {
							setFocus(i);
							setListOpen(false);
						}}
						onChange={change}
						onOpen={confirm}
					/>
				) : null}
			</div>
			<p className="game-description-bar">{focused?.description ?? ""}</p>
			<GameHintBar hints={hints}>
				<button
					type="button"
					className="game-key-hint game-key-hint--back"
					onClick={back}
					style={{ marginRight: "auto", border: 0, background: "transparent", font: "inherit", cursor: "pointer" }}
				>
					<GameKeyCap>Esc</GameKeyCap>
					<GameCursor />
					<span>{backLabel}</span>
				</button>
			</GameHintBar>
		</div>
	);
}
