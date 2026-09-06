/**
 * Le panneau bleu nuit du jeu : un titre à gauche, un corps, un pied, un filigrane.
 *
 * Reproduit le cadre du dialogue de `data/menu/filters_elements.png` — « FILTRES » en haut à
 * gauche, la grille dans un cadre plus sombre, le filigrane des quatre éléments derrière, et
 * la rangée « Réinitialiser / Confirmer / 13/13 » en pied.
 *
 * Il ne pose que des FORMES : couleurs, biseaux et filigrane viennent de `game-screens.css`.
 * `role` est celui que l'appelant décide — `dialog` pour un panneau modal, rien pour une
 * section de page.
 */
import type { CSSProperties, ReactNode } from "react";
import { cx } from "./GameKeyHint";

export function GamePanel({
	title,
	titleId,
	header,
	children,
	footer,
	watermark,
	role,
	modal = false,
	className,
	style,
	onKeyDown,
	panelRef,
}: {
	/** Le titre, en capitales dans le jeu (« FILTRES »). */
	title?: ReactNode;
	/** L'identifiant du titre, pour `aria-labelledby`. */
	titleId?: string;
	/** Ce qui suit le titre sur la même rangée : la barre d'onglets des familles. */
	header?: ReactNode;
	children: ReactNode;
	footer?: ReactNode;
	/** Le filigrane du fond de panneau — une icône géante, `aria-hidden`. */
	watermark?: ReactNode;
	role?: "dialog" | "region" | "search";
	modal?: boolean;
	className?: string;
	style?: CSSProperties;
	onKeyDown?: (event: React.KeyboardEvent<HTMLElement>) => void;
	panelRef?: (element: HTMLElement | null) => void;
}) {
	return (
		<section
			ref={panelRef}
			role={role}
			aria-modal={modal || undefined}
			aria-labelledby={titleId}
			className={cx("game-panel", className)}
			style={{ position: "relative", display: "flex", flexDirection: "column", ...style }}
			onKeyDown={onKeyDown}
		>
			{title || header ? (
				<div style={{ display: "flex", alignItems: "flex-start", gap: 24, flexWrap: "wrap" }}>
					{title ? (
						<h2 id={titleId} className="game-panel__title" style={{ margin: 0 }}>
							{title}
						</h2>
					) : null}
					{header ? <div style={{ flex: 1, minWidth: 0 }}>{header}</div> : null}
				</div>
			) : null}
			<div className="game-panel__body" style={{ position: "relative", flex: 1, minHeight: 0 }}>
				{watermark ? (
					<div
						aria-hidden="true"
						className="game-panel__watermark"
						style={{ position: "absolute", inset: 0, overflow: "hidden", pointerEvents: "none" }}
					>
						{watermark}
					</div>
				) : null}
				<div style={{ position: "relative" }}>{children}</div>
			</div>
			{footer ? (
				<div
					className="game-panel__footer"
					style={{ display: "flex", alignItems: "center", gap: 24, flexWrap: "wrap" }}
				>
					{footer}
				</div>
			) : null}
		</section>
	);
}
