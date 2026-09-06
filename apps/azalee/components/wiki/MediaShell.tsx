/**
 * Reste de l'ossature des pages média.
 *
 * Les collections (`/gallery`, `/textures`, `/sons`, `/videos`, `/modeles`, `/mode`) ont migré
 * vers l'explorateur de bureau — cf. `docs/MIGRATION-EXPLORATEUR.md`. La sous-navigation,
 * l'en-tête, l'état vide, le bouton de retour et le titre d'entrée n'avaient donc plus de page
 * à servir : seule la ligne de compte survit, partagée avec `/skill`.
 */

/** Ligne de compte au-dessus d'une grille : ce qui est montré, sur quel total. */
export function MediaCount({ left, right }: { left: React.ReactNode; right?: React.ReactNode }) {
	return (
		<div className="
    flex items-center justify-between gap-3 px-1 text-xs font-medium uppercase tracking-wider text-on-surface-variant
  ">
			<span>{left}</span>
			{right && <span>{right}</span>}
		</div>
	);
}
