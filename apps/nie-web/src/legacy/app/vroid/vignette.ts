/**
 * URL de relais des vignettes VRoid Hub, côté navigateur.
 *
 * Les portraits vivent sur `vroid-hub.pximg.net`, hôte absent de la directive
 * `img-src` de la CSP d'azalée : une balise `<img>` pointant dessus serait
 * bloquée sans message. On passe donc par `/api/vroid/image`, qui n'accepte
 * que cet hôte.
 *
 * Module client-safe, sans dépendance : il est importé par les îlots
 * `"use client"` de la galerie.
 */

/**
 * Construit l'URL relayée d'une vignette.
 *
 * @param source URL absolue renvoyée par l'API VRoid Hub.
 * @returns l'URL interne à charger, ou `null` si la source est absente.
 */
export function urlVignette(source: string | null | undefined): string | null {
	if (!source) return null;
	return `/api/vroid/image?url=${encodeURIComponent(source)}`;
}
