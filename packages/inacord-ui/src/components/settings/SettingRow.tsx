/**
 * Une ligne de réglage : le libellé à gauche, la valeur à droite.
 *
 * La ligne active porte le curseur du jeu devant son libellé et ses deux flèches `<` `>` aux
 * bords de la bande de valeur ; les autres ne montrent que leur valeur, et un glyphe de liste
 * quand elles s'ouvrent — comme « Langue du texte » et « Langue des voix » dans le jeu. La
 * liste dépliée d'un choix est rendue par `SettingList`, SOUS la ligne : la ligne est une
 * grille à deux colonnes (`game-screens.css`), et un troisième enfant la casserait.
 */
import type { Settings } from "../../lib/settings";
import { GameCursor } from "../game/GameCursor";
import { cycleValue, formatValue, type SettingDefinition, type SettingId } from "./settings-model";

export function SettingRow<K extends SettingId>({
	def,
	value,
	focused,
	onFocus,
	onChange,
	onOpen,
}: {
	def: SettingDefinition<K>;
	value: Settings[K];
	focused: boolean;
	onFocus: () => void;
	onChange: (value: Settings[K]) => void;
	/** Enter sur la ligne : ouvre la liste d'un choix, bascule un interrupteur. */
	onOpen: () => void;
}) {
	const cyclable = def.kind !== "text";
	const hasList = def.kind === "choice";
	return (
		<li
			className={`game-setting-row${focused ? " game-setting-row--focused" : ""}`}
			data-setting={def.id}
			aria-current={focused ? "true" : undefined}
		>
			<button
				type="button"
				className="game-setting-row__label"
				onClick={onFocus}
				onDoubleClick={onOpen}
				style={{
					display: "flex",
					alignItems: "center",
					gap: "var(--jeu-espace-m)",
					border: 0,
					background: "transparent",
					color: "inherit",
					font: "inherit",
					textAlign: "left",
					cursor: "pointer",
				}}
			>
				{/* Le curseur occupe sa place même absent : le libellé ne saute pas au focus. */}
				<span style={{ display: "inline-flex", width: 36, justifyContent: "center" }}>
					{focused ? <GameCursor /> : null}
				</span>
				<span>{def.label}</span>
			</button>
			<div className="game-setting-row__value">
				{cyclable ? (
					<button
						type="button"
						className="game-setting-row__arrow game-setting-row__arrow--previous"
						aria-label="Valeur précédente"
						onClick={() => onChange(cycleValue(def, value, -1))}
						style={{ marginRight: "auto", font: "inherit", cursor: "pointer" }}
					>
						&lt;
					</button>
				) : null}
				{def.kind === "text" ? (
					<input
						className="game-setting-row__input"
						value={String(value)}
						placeholder={def.placeholder ?? "Automatique"}
						onFocus={onFocus}
						onChange={(e) => onChange(e.target.value as Settings[K])}
						style={{ font: "inherit", color: "inherit", background: "transparent", border: 0 }}
					/>
				) : (
					<span className="game-setting-row__text">{formatValue(def, value)}</span>
				)}
				{cyclable ? (
					<button
						type="button"
						className="game-setting-row__arrow game-setting-row__arrow--next"
						aria-label="Valeur suivante"
						onClick={() => onChange(cycleValue(def, value, 1))}
						style={{ marginLeft: "auto", font: "inherit", cursor: "pointer" }}
					>
						&gt;
					</button>
				) : null}
				{hasList && !focused ? (
					<span className="game-setting-row__more" aria-hidden="true">
						☰
					</span>
				) : null}
			</div>
		</li>
	);
}

/** La liste dépliée d'un choix, sous sa ligne : une option par ligne, la courante marquée. */
export function SettingChoiceList<K extends SettingId>({
	def,
	value,
	onChange,
}: {
	def: Extract<SettingDefinition<K>, { kind: "choice" }>;
	value: Settings[K];
	onChange: (value: Settings[K]) => void;
}) {
	return (
		<li
			className="game-setting-row__list"
			role="listbox"
			aria-label={def.label}
			style={{ display: "flex", flexDirection: "column", gap: "var(--jeu-espace-xs)" }}
		>
			{def.options.map((option) => {
				const selected = option.value === value;
				return (
					<button
						type="button"
						key={String(option.value)}
						role="option"
						aria-selected={selected}
						className={`game-setting-row${selected ? " game-setting-row--focused" : ""}`}
						onClick={() => onChange(option.value)}
						style={{ border: 0, font: "inherit", cursor: "pointer", padding: 0 }}
					>
						<span className="game-setting-row__label">{option.label}</span>
						<span className="game-setting-row__value">{selected ? "●" : ""}</span>
					</button>
				);
			})}
		</li>
	);
}
