/**
 * Un guide de touche du jeu — une capsule grise avec la touche, puis son libellé.
 *
 * Reproduit les guides du bas d'écran de `data/menu/options.png` (« Confirmer », « V Appliquer »,
 * « X Réinitialiser ») et ceux des dialogues (« Tab Réinitialiser », « Alt Confirmer » dans
 * `filters_elements.png`).
 *
 * `onActivate` est OBLIGATOIRE : un guide de touche annonce une affordance, et le dépôt a déjà
 * dessiné des « F » et des « V » que rien n'écoutait. Le guide est donc un vrai bouton — la
 * souris et le lecteur d'écran actionnent la même chose que la touche. La touche elle-même est
 * branchée par `useGameKeys` (voir `GameHintBar`), pas ici : un composant ne pose pas
 * d'écouteur global à son insu.
 */
import type { ReactNode } from "react";

/** Fusionne des classes en ignorant les vides. */
export function cx(...parts: (string | false | null | undefined)[]): string {
	return parts.filter(Boolean).join(" ");
}

/** La capsule d'une touche, seule. */
export function GameKeyCap({ children, className }: { children: ReactNode; className?: string }) {
	return <kbd className={cx("game-key-cap", className)}>{children}</kbd>;
}

export function GameKeyHint({
	keyLabel,
	children,
	onActivate,
	disabled = false,
	className,
}: {
	/** La touche telle qu'elle s'affiche : « Tab », « Alt », « X ». */
	keyLabel: string;
	/** Le libellé de l'action. */
	children: ReactNode;
	onActivate: () => void;
	disabled?: boolean;
	className?: string;
}) {
	return (
		<button
			type="button"
			className={cx("game-key-hint", className)}
			onClick={onActivate}
			disabled={disabled}
			style={{ display: "inline-flex", alignItems: "center", gap: 8, font: "inherit" }}
		>
			<GameKeyCap>{keyLabel}</GameKeyCap>
			<span>{children}</span>
		</button>
	);
}
