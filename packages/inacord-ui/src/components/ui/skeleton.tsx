import { cn } from "../../lib/utils";

/**
 * Le rectangle d'attente d'un contenu qui arrive.
 *
 * Écrit ici plutôt qu'importé : le squelette du wiki venait de `@rosegriffon/ui`, et ce paquet
 * partagé est monté par **deux hôtes** qui ne sont pas Rose Griffon — Aphrody et Inacord sont
 * des projets `aphrody-dev`. Cinq lignes valent mieux qu'une dépendance de marque (CLAUDE.md
 * § *Propriété*).
 *
 * Il porte `aria-hidden` : un lecteur d'écran n'a rien à annoncer d'une forme qui attend, et
 * l'état de chargement se dit ailleurs, en texte.
 */
export function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
	return (
		<div
			aria-hidden="true"
			data-slot="skeleton"
			className={cn("animate-pulse rounded-md bg-muted", className)}
			{...props}
		/>
	);
}
