/**
 * Le bandeau de tête d'un écran du jeu : une icône dans un carré, un biseau, le titre en blanc
 * sur bleu vif.
 *
 * Reproduit le haut de `data/menu/options.png` (engrenage + « Options ») et celui de
 * `filters_elements.png` (crampons sur orange + « Banque »).
 *
 * Le titre est un `h2` : la page a déjà un `h1` (hors vue, dans l'accueil, et rendu par le
 * serveur ailleurs), et ce bandeau nomme l'écran courant sous lui.
 */
import type { ReactNode } from "react";
import { cx } from "./GameKeyHint";

export function GameHeaderBar({
	icon,
	title,
	children,
	className,
}: {
	icon?: ReactNode;
	title: ReactNode;
	/** Ce qui suit le titre : un compte, une barre de recherche. */
	children?: ReactNode;
	className?: string;
}) {
	return (
		<header className={cx("game-header-bar", className)} style={{ display: "flex", alignItems: "center", gap: 24 }}>
			{icon ? (
				<span className="game-header-bar__icon" aria-hidden="true" style={{ display: "inline-flex", lineHeight: 0 }}>
					{icon}
				</span>
			) : null}
			<h2 className="game-header-bar__title" style={{ margin: 0 }}>
				{title}
			</h2>
			{children ? <div style={{ marginLeft: "auto", display: "flex", alignItems: "center", gap: 16 }}>{children}</div> : null}
		</header>
	);
}
