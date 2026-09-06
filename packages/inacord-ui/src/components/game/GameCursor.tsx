/**
 * Le curseur du jeu : le triangle qui pointe l'entrée courante.
 *
 * Reproduit le triangle vert et jaune posé devant « Tout » dans `data/menu/filters_elements.png`,
 * devant la ligne active de `options.png`, et devant le premier portrait de `player_roster.png`.
 *
 * Il est purement décoratif — l'état qu'il souligne est déjà porté par `aria-checked`,
 * `aria-selected` ou le focus — donc `aria-hidden`. Ses couleurs viennent de `.game-cursor`
 * (`currentColor` sur le tracé) : rien n'est posé ici.
 */
export function GameCursor({ className }: { className?: string }) {
	return (
		<span
			aria-hidden="true"
			className={["game-cursor", className].filter(Boolean).join(" ")}
			style={{ display: "inline-flex", lineHeight: 0 }}
		>
			<svg width="22" height="22" viewBox="0 0 22 22" focusable="false">
				<path d="M3 2l17 9-17 9z" fill="currentColor" />
				<path d="M7 7.5l7 3.5-7 3.5z" fill="var(--jeu-ciel-clair)" />
			</svg>
		</span>
	);
}
