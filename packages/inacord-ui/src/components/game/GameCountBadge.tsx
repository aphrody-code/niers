/**
 * La pastille de compte du jeu : un pictogramme, puis « 13/13 ».
 *
 * Reproduit la pastille grise du pied de `data/menu/filters_elements.png`, qui dit combien de
 * joueurs les filtres en cours retiennent sur le total.
 *
 * Le compte est lu tel quel : `aria-live="polite"` pour qu'un changement de filtre s'entende
 * sans interrompre, et un libellé complet pour le lecteur d'écran — « 13 sur 13 » se lit, « 13/13 »
 * non.
 */
import type { ReactNode } from "react";
import { cx } from "./GameKeyHint";

export function GameCountBadge({
	count,
	total,
	icon,
	unit = "élément",
	className,
}: {
	count: number;
	total?: number;
	icon?: ReactNode;
	/** Le nom de ce qui est compté, pour le libellé accessible. */
	unit?: string;
	className?: string;
}) {
	const shown = total !== undefined ? `${count.toLocaleString("fr")}/${total.toLocaleString("fr")}` : count.toLocaleString("fr");
	const spoken =
		total !== undefined
			? `${count.toLocaleString("fr")} sur ${total.toLocaleString("fr")} ${unit}${total > 1 ? "s" : ""}`
			: `${count.toLocaleString("fr")} ${unit}${count > 1 ? "s" : ""}`;
	return (
		<span
			className={cx("game-count-badge", className)}
			aria-live="polite"
			aria-label={spoken}
			style={{ display: "inline-flex", alignItems: "center", gap: 10 }}
		>
			{icon ? (
				<span aria-hidden="true" style={{ display: "inline-flex", lineHeight: 0 }}>
					{icon}
				</span>
			) : null}
			<span aria-hidden="true">{shown}</span>
		</span>
	);
}
