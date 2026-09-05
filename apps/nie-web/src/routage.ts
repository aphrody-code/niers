/**
 * Le routage d'Aphrody : l'entrée courante vit dans le CHEMIN, pas dans un paramètre.
 *
 * ## Pourquoi ce changement
 *
 * L'entrée vivait dans `?vue=textures`. Le serveur, lui, annonçait depuis le début quatre URL
 * distinctes — `/textures`, `/modeles`, `/sons`, `/videos` — avec pour chacune son `<title>`,
 * sa description, son canonique et son entrée au plan du site. Les deux ne se rencontraient
 * jamais : `https://aphrody.com/textures` servait les métadonnées des textures et affichait
 * l'accueil. Quatre URL indexées, un seul contenu rendu, et aucun message d'erreur nulle part.
 *
 * Un paramètre de requête n'est de toute façon pas une page distincte pour un moteur, et il ne
 * se traduit pas : `/ja/textures` doit désigner la version japonaise du catalogue de textures.
 *
 * ## Compatibilité
 *
 * `?vue=` reste compris en entrée — un signet ou un lien partagé ne doit pas se casser — mais
 * il est réécrit vers la forme canonique dès le premier rendu, sans entrée d'historique
 * supplémentaire.
 */

/** Les préfixes de langue servis par `nie-site`. Le français est à la racine, sans préfixe. */
export const PREFIXES_LANGUE = ["/en", "/ja"] as const;

/** Ce qu'un chemin dit de la langue et de la route. */
export interface CheminSepare {
	/** `""` pour le français, `/en` ou `/ja` sinon. */
	prefixe: string;
	/** La route sans son préfixe de langue, commençant toujours par `/`. */
	route: string;
}

/**
 * Sépare un chemin en préfixe de langue et route nue.
 *
 * La comparaison porte sur le SEGMENT entier : sans cela, `/enemy` serait lu comme de l'anglais
 * et sa route tronquée à `emy`.
 */
export function separerLangue(chemin: string): CheminSepare {
	for (const prefixe of PREFIXES_LANGUE) {
		if (chemin === prefixe) {
			return { prefixe, route: "/" };
		}
		if (chemin.startsWith(`${prefixe}/`)) {
			return { prefixe, route: chemin.slice(prefixe.length) };
		}
	}
	return { prefixe: "", route: chemin === "" ? "/" : chemin };
}

/**
 * L'accueil — le menu principal — n'est pas une entrée comme les autres : il vit à la RACINE.
 *
 * Le jeton existe pour que l'état de l'application ait toujours une valeur, y compris sur `/`.
 * Sans lui, l'accueil serait `null`, et chaque lecture devrait décider ce que `null` veut dire
 * — ce qui finit toujours par diverger d'un endroit à l'autre.
 */
export const ACCUEIL = "accueil";

/**
 * Le chemin canonique d'une entrée, dans la langue courante.
 *
 * L'accueil rend `/` (ou `/ja`) et non `/accueil` : le menu principal EST la racine du site, et
 * lui donner un second chemin dédoublerait la page d'accueil aux yeux d'un moteur.
 */
export function cheminPourEntree(prefixe: string, entree: string): string {
	if (entree === ACCUEIL) return prefixe || "/";
	return `${prefixe}/${entree}`;
}

/**
 * L'entrée demandée par l'URL courante, ou `null` si l'URL n'en désigne aucune.
 *
 * Trois sources, dans cet ordre : le chemin (la forme canonique), l'attribut `data-route` posé
 * par le serveur (qui a déjà fait la séparation, et fait autorité si le chemin a été réécrit
 * par un proxy), puis l'ancien `?vue=`.
 */
export function entreeDemandee(
	entrees: readonly string[],
	emplacement: { pathname: string; search: string },
	routeServeur?: string | null,
): string | null {
	const candidats = [
		separerLangue(emplacement.pathname).route.replace(/^\//, ""),
		(routeServeur ?? "").replace(/^\//, ""),
		new URLSearchParams(emplacement.search).get("vue") ?? "",
	];
	for (const candidat of candidats) {
		if (candidat && entrees.includes(candidat)) {
			return candidat;
		}
	}
	return null;
}
