/**
 * Une case à cocher du jeu : la boîte cyan, une pastille d'icône optionnelle, le libellé.
 *
 * Reproduit une ligne de la grille de `data/menu/filters_elements.png` (« ✓ [icône Vent] Vent »)
 * et celles de `filters_rarity.png`, où la pastille est une bannière colorée.
 *
 * C'est un VRAI `<input type="checkbox">` sous un `<label>` : la souris, le clavier (Espace) et
 * le lecteur d'écran passent tous par lui. La boîte dessinée est un `span` décoratif, et
 * l'`input` reste dans le document (déplacé hors vue, jamais `display: none`, qui le retirerait
 * du parcours au clavier).
 */
import type { ReactNode } from "react";
import { GameCursor } from "./GameCursor";
import { cx } from "./GameKeyHint";

/** Retire visuellement un élément sans le retirer du document ni du focus. */
const OFF_SCREEN = {
	position: "absolute",
	width: 1,
	height: 1,
	margin: -1,
	padding: 0,
	overflow: "hidden",
	clip: "rect(0 0 0 0)",
	whiteSpace: "nowrap",
	border: 0,
} as const;

export function GameCheck({
	checked,
	onChange,
	children,
	icon,
	cursor = false,
	tabIndex,
	inputRef,
	onFocus,
	className,
}: {
	checked: boolean;
	onChange: (checked: boolean) => void;
	children: ReactNode;
	/** La pastille d'icône entre la boîte et le libellé (l'élément, la rareté). */
	icon?: ReactNode;
	/** Dessine le curseur du jeu devant la case — l'entrée courante du clavier. */
	cursor?: boolean;
	/** Pour un focus tournant dans une grille : `0` sur l'entrée courante, `-1` ailleurs. */
	tabIndex?: number;
	inputRef?: (element: HTMLInputElement | null) => void;
	onFocus?: () => void;
	className?: string;
}) {
	return (
		<label
			className={cx("game-check", checked && "game-check--checked", className)}
			style={{ display: "inline-flex", alignItems: "center", gap: 12, cursor: "pointer" }}
		>
			<span style={{ width: 22, display: "inline-flex", justifyContent: "center" }}>
				{cursor ? <GameCursor /> : null}
			</span>
			<input
				type="checkbox"
				checked={checked}
				onChange={(event) => onChange(event.target.checked)}
				tabIndex={tabIndex}
				ref={inputRef}
				onFocus={onFocus}
				style={OFF_SCREEN}
			/>
			<span className="game-check__box" aria-hidden="true" />
			{icon ? (
				<span className="game-icon-chip" aria-hidden="true">
					{icon}
				</span>
			) : null}
			<span className="game-check__label">{children}</span>
		</label>
	);
}
