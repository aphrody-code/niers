/**
 * L'encart d'information du menu principal.
 *
 * Reproduit le coin haut-gauche de `data/menu/main_menu.png` : une carte claire cerclée de
 * sombre avec un titre coloré et deux lignes, puis un bandeau « X Informations » en dessous.
 *
 * Le bandeau est un vrai bouton et la touche est branchée par l'appelant (`GameHintBar` ou
 * `useGameKeys`) : sans `onActivate`, le bandeau n'est pas dessiné — jamais une touche sans
 * gestionnaire.
 */
import type { ReactNode } from "react";
import { GameKeyHint, cx } from "./GameKeyHint";

export function GameInfoWindow({
	title,
	children,
	action,
	className,
}: {
	title: ReactNode;
	children?: ReactNode;
	/** Le bandeau sous la carte : sa touche, son libellé, son action. */
	action?: { keyLabel: string; label: ReactNode; onActivate: () => void };
	className?: string;
}) {
	return (
		<aside className={cx("game-info-window", className)} style={{ display: "flex", flexDirection: "column", gap: 4 }}>
			<div className="game-info-window__title">{title}</div>
			{children ? <div className="game-info-window__body">{children}</div> : null}
			{action ? (
				<GameKeyHint keyLabel={action.keyLabel} onActivate={action.onActivate} className="game-info-window__action">
					{action.label}
				</GameKeyHint>
			) : null}
		</aside>
	);
}
