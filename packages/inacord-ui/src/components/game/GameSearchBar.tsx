/**
 * La barre de recherche du jeu : un champ clair, et la capsule de la touche qui l'ouvre.
 *
 * Reproduit le guide « X Chercher par nom de joueur » du pied de `data/menu/bank_character_detail.png`
 * et de `filters_elements.png` : dans le jeu, la touche ouvre la saisie ; ici, elle donne le
 * focus au champ, et la capsule est un bouton qui fait la même chose à la souris.
 *
 * `role="search"` sur le formulaire ; `Entrée` soumet, `Escape` vide et rend le focus. La
 * touche d'ouverture est branchée sur la fenêtre par `useGameKeys`, et seulement si l'appelant
 * en déclare une : sans `hotkey`, aucune capsule n'est dessinée.
 */
import { useMemo, useRef } from "react";
import { GameKeyCap, cx } from "./GameKeyHint";
import { useGameKeys } from "./keys";

export function GameSearchBar({
	value,
	onChange,
	onSubmit,
	placeholder = "Chercher",
	label = "Chercher",
	hotkey,
	autoFocus = false,
	className,
}: {
	value: string;
	onChange: (value: string) => void;
	/** Appelé à `Entrée` avec la valeur courante, coupée. */
	onSubmit?: (value: string) => void;
	placeholder?: string;
	/** Le libellé accessible du champ. */
	label?: string;
	/** La touche qui donne le focus au champ (« X » dans le jeu). Sans elle, pas de capsule. */
	hotkey?: string;
	autoFocus?: boolean;
	className?: string;
}) {
	const input = useRef<HTMLInputElement>(null);
	const focus = () => {
		input.current?.focus();
		input.current?.select();
	};
	const bindings = useMemo(() => (hotkey ? [{ key: hotkey, onActivate: focus }] : []), [hotkey]);
	useGameKeys(bindings);

	return (
		<form
			role="search"
			className={cx("game-search-bar", className)}
			onSubmit={(event) => {
				event.preventDefault();
				onSubmit?.(value.trim());
			}}
			style={{ display: "flex", alignItems: "center", gap: 10 }}
		>
			{hotkey ? (
				<button
					type="button"
					className="game-search-bar__key"
					onClick={focus}
					aria-label={`${label} (touche ${hotkey.toUpperCase()})`}
					style={{ font: "inherit" }}
				>
					<GameKeyCap>{hotkey.toUpperCase()}</GameKeyCap>
				</button>
			) : null}
			<input
				ref={input}
				type="search"
				className="game-search-bar__input"
				value={value}
				onChange={(event) => onChange(event.target.value)}
				onKeyDown={(event) => {
					if (event.key === "Escape" && value) {
						event.preventDefault();
						onChange("");
					}
				}}
				placeholder={placeholder}
				aria-label={label}
				autoFocus={autoFocus}
				style={{ flex: 1, minWidth: 0, font: "inherit" }}
			/>
		</form>
	);
}
