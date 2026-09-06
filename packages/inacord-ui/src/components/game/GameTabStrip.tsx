/**
 * La barre d'onglets du jeu : des icônes en rangée, l'active sur fond bleu vif, et de part et
 * d'autre les capsules « W » et « C » qui la font tourner.
 *
 * Reproduit la rangée d'icônes de `data/menu/filters_elements.png` (neuf familles de filtres,
 * « Éléments » écrit sous la rangée) et celle de `options.png` (quatre onglets, « Paramètres du
 * jeu » sous la rangée).
 *
 * `role="tablist"` avec focus tournant : les flèches gauche/droite changent d'onglet, et les
 * deux capsules sont de vrais boutons dont les touches sont branchées par l'appelant à travers
 * `useGameKeys` — la barre expose `onPrevious`/`onNext` pour cela, elle ne pose aucun écouteur
 * global elle-même.
 */
import type { ReactNode } from "react";
import { GameKeyCap, cx } from "./GameKeyHint";

export interface GameTab {
	id: string;
	label: string;
	icon: ReactNode;
}

export function GameTabStrip({
	tabs,
	value,
	onChange,
	previousKey = "W",
	nextKey = "C",
	showLabel = true,
	className,
}: {
	tabs: readonly GameTab[];
	value: string;
	onChange: (id: string) => void;
	/** Le libellé de la capsule de gauche. `null` la retire. */
	previousKey?: string | null;
	nextKey?: string | null;
	/** Écrit le libellé de l'onglet actif sous la rangée, comme le jeu. */
	showLabel?: boolean;
	className?: string;
}) {
	const index = Math.max(
		0,
		tabs.findIndex((tab) => tab.id === value),
	);
	const step = (delta: number) => {
		if (tabs.length === 0) return;
		const next = tabs[(index + delta + tabs.length) % tabs.length];
		if (next) onChange(next.id);
	};
	const current = tabs[index];

	return (
		<div className={cx("game-tab-strip", className)} style={{ display: "flex", flexDirection: "column", alignItems: "center" }}>
			<div
				role="tablist"
				aria-label="Familles"
				style={{ display: "flex", alignItems: "center", gap: 8 }}
				onKeyDown={(event) => {
					if (event.key === "ArrowRight") {
						event.preventDefault();
						step(1);
					} else if (event.key === "ArrowLeft") {
						event.preventDefault();
						step(-1);
					}
				}}
			>
				{previousKey ? (
					<button
						type="button"
						className="game-tab-strip__key"
						onClick={() => step(-1)}
						aria-label="Famille précédente"
						style={{ font: "inherit" }}
					>
						<GameKeyCap>{previousKey}</GameKeyCap>
					</button>
				) : null}
				{tabs.map((tab) => {
					const active = tab.id === current?.id;
					return (
						<button
							key={tab.id}
							type="button"
							role="tab"
							aria-selected={active}
							aria-label={tab.label}
							title={tab.label}
							tabIndex={active ? 0 : -1}
							className={cx("game-tab", active && "game-tab--active")}
							onClick={() => onChange(tab.id)}
							style={{ font: "inherit", display: "inline-flex", alignItems: "center", justifyContent: "center" }}
						>
							<span aria-hidden="true" style={{ display: "inline-flex", lineHeight: 0 }}>
								{tab.icon}
							</span>
						</button>
					);
				})}
				{nextKey ? (
					<button
						type="button"
						className="game-tab-strip__key"
						onClick={() => step(1)}
						aria-label="Famille suivante"
						style={{ font: "inherit" }}
					>
						<GameKeyCap>{nextKey}</GameKeyCap>
					</button>
				) : null}
			</div>
			{showLabel && current ? (
				<div className="game-tab-strip__label" aria-hidden="true" style={{ textAlign: "center" }}>
					{current.label}
				</div>
			) : null}
		</div>
	);
}
