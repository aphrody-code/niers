/**
 * La barre de guides de touches du bas d'écran.
 *
 * Reproduit le pied de `data/menu/options.png` — « Esc ↩ · Confirmer · V Appliquer ·
 * X Réinitialiser · B Liste de blocage » — et celui de `player_roster.png` (« X Taux d'apparition
 * par rareté »).
 *
 * Elle reçoit des ACTIONS, pas des dessins : chaque guide vient avec sa touche et son
 * gestionnaire, et la barre branche la touche sur la fenêtre. Impossible d'y afficher une touche
 * morte.
 */
import type { ReactNode } from "react";
import { GameKeyHint } from "./GameKeyHint";
import { type GameKeyBinding, useGameKeys } from "./keys";

/** Un guide : sa touche, son libellé, son action. */
export interface GameHint extends GameKeyBinding {
	label: ReactNode;
	/** Le texte de la capsule, quand il diffère de `key` (« Esc » pour `Escape`). */
	keyLabel?: string;
}

export function GameHintBar({
	hints,
	enabled = true,
	className,
	children,
}: {
	hints: readonly GameHint[];
	/** Débranche les touches (un dialogue est ouvert par-dessus, par exemple). */
	enabled?: boolean;
	className?: string;
	/** Un contenu libre à gauche des guides — un bouton de retour, une pastille. */
	children?: ReactNode;
}) {
	useGameKeys(hints, enabled);
	return (
		<div
			role="toolbar"
			aria-label="Touches"
			className={["game-hint-bar", className].filter(Boolean).join(" ")}
			style={{ display: "flex", alignItems: "center", gap: 28, flexWrap: "wrap" }}
		>
			{children}
			{hints.map((hint) => (
				<GameKeyHint
					key={hint.key}
					keyLabel={hint.keyLabel ?? hint.key.toUpperCase()}
					onActivate={hint.onActivate}
				>
					{hint.label}
				</GameKeyHint>
			))}
		</div>
	);
}
