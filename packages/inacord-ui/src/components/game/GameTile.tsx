/**
 * Une tuile du menu principal et sa rangée.
 *
 * Reproduit les huit tuiles biseautées de `data/menu/main_menu.png` (pictogramme blanc sur
 * illustration bleue, penchées, alignées) et la rangée basse de trois.
 *
 * La forme (biseau, dégradé, liseré de la tuile active) vient de `.game-tile` dans
 * `game-screens.css` ; ce fichier ne pose que la taille — une géométrie mesurée que l'appelant
 * lui donne — et l'état : `aria-current` pour la tuile courante, `disabled` pour une tuile en
 * sourdine. Le pictogramme est `aria-hidden` parce que la tuile porte AUSSI son libellé.
 */
import type { ReactNode } from "react";
import { cx } from "./GameKeyHint";

export function GameTile({
	icon,
	label,
	active = false,
	muted = false,
	onClick,
	width,
	height,
	badge,
	className,
}: {
	icon: ReactNode;
	label: string;
	active?: boolean;
	/** Indisponible : la tuile reste visible mais ne promet rien. */
	muted?: boolean;
	onClick?: () => void;
	/** Largeur et hauteur en pixels, mesurées par l'appelant (`LARGEUR_TUILE`, `BOITES.rangee.h`). */
	width?: number;
	height?: number;
	/** Une pastille d'angle — le « ! » rouge des tuiles du jeu. */
	badge?: ReactNode;
	className?: string;
}) {
	return (
		<button
			type="button"
			onClick={muted ? undefined : onClick}
			disabled={muted}
			aria-current={active ? "true" : undefined}
			className={cx("game-tile", active && "game-tile--active", className)}
			style={{
				position: "relative",
				width,
				height,
				display: "flex",
				flexDirection: "column",
				alignItems: "center",
				justifyContent: "center",
				gap: 4,
				font: "inherit",
				cursor: muted ? "not-allowed" : onClick ? "pointer" : "default",
			}}
		>
			<span className="game-tile__icon" aria-hidden="true" style={{ display: "block", lineHeight: 0 }}>
				{icon}
			</span>
			<span>{label}</span>
			{badge ? (
				<span style={{ position: "absolute", top: 4, right: 12 }}>{badge}</span>
			) : null}
		</button>
	);
}

/** Une rangée de tuiles, centrée, comme les deux rangées du menu. */
export function GameTileRow({
	children,
	gap = 8,
	className,
}: {
	children: ReactNode;
	gap?: number;
	className?: string;
}) {
	return (
		<div
			className={cx("game-tile-row", className)}
			style={{ display: "flex", justifyContent: "center", alignItems: "flex-start", gap }}
		>
			{children}
		</div>
	);
}
