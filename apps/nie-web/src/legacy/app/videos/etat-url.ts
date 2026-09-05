/**
 * L'état de la page vidéo — filtres, recherche, film ouvert — et sa traduction en URL.
 *
 * Cet état vivait entièrement en `useState` : quatre filtres qu'aucun lien ne pouvait porter, et
 * surtout un film ouvert qu'on ne pouvait pas partager. Une cinématique n'était atteignable que
 * par « va sur /videos, tape ceci, fais défiler, clique là ».
 *
 * Il est ici parce que les deux côtés en ont besoin et doivent s'accorder au caractère près : le
 * Server Component le lit depuis `searchParams` (métadonnées, canonique, état initial), l'îlot
 * client le réécrit dans l'URL. Deux lectures divergentes donneraient une page qui se recharge en
 * boucle — le serveur rendrait un état, le client en pousserait un autre, indéfiniment. La même
 * paire `lireEtat`/`serialiserEtat` sert des deux côtés, et `serialiserEtat(lireEtat(x))` est
 * idempotent : c'est ce qui permet au client de savoir si l'URL dit déjà ce qu'il pense.
 */

/** Ce qu'on cherche quand on filtre sur la bande-son. */
export type FiltreSon = "tous" | "avec" | "sans";

/** L'état complet de la page. */
export interface EtatVideos {
	/** Rubrique retenue, ou `toutes`. La valeur EST le libellé (`Chapitre 01`). */
	rubrique: string;
	/** Code de langue retenu (`JP`, `fr`…), ou `toutes`. */
	langue: string;
	/** Filtre de bande-son. */
	son: FiltreSon;
	/** Recherche libre, telle que tapée. */
	requete: string;
	/** Radical du film ouvert en lecteur (`ev01_00050`), `null` si aucun. */
	film: string | null;
}

/** L'état d'une page nue : aucun filtre, aucun film ouvert. */
export const ETAT_VIDE: EtatVideos = {
	film: null,
	langue: "toutes",
	requete: "",
	rubrique: "toutes",
	son: "tous",
};

/**
 * Les clés qui changent le CONTENU de la page, donc les seules à entrer dans le canonique.
 *
 * `q` en est exclue, comme pour les listes du wiki (`lib/seo.ts`) : une recherche interne porte
 * sur un espace non borné, et chaque frappe deviendrait une page canonique de plus. `film` y est
 * au contraire : `?film=ev01_00050` désigne une cinématique précise, c'est un document distinct
 * de la liste et c'est exactement le lien qu'on partage.
 */
export const CLES_CANONIQUES = ["film", "langue", "rubrique", "son"] as const;

/** Première occurrence d'un paramètre : `?rubrique=a&rubrique=b` n'a pas de sens ici. */
function unSeul(valeur: string | string[] | undefined): string | undefined {
	return Array.isArray(valeur) ? valeur[0] : valeur;
}

/**
 * Lit l'état depuis les paramètres d'URL.
 *
 * Toute valeur inconnue retombe sur le défaut plutôt que de filtrer sur du vide : une URL
 * tronquée ou bricolée rend la liste complète, jamais une page vide inexplicable.
 */
export function lireEtat(params: Record<string, string | string[] | undefined>): EtatVideos {
	const son = unSeul(params.son);
	return {
		film: unSeul(params.film) || null,
		langue: unSeul(params.langue) || "toutes",
		requete: unSeul(params.q) ?? "",
		rubrique: unSeul(params.rubrique) || "toutes",
		son: son === "avec" || son === "sans" ? son : "tous",
	};
}

/**
 * Sérialise l'état en chaîne de requête, sans le `?`.
 *
 * Les valeurs par défaut n'y apparaissent pas — `/videos` reste `/videos`, pas
 * `/videos?rubrique=toutes&langue=toutes&son=tous` — et les clés sont triées pour que deux états
 * identiques produisent toujours la même chaîne. C'est cette dernière propriété que le client
 * utilise pour comparer plutôt que de pousser.
 */
export function serialiserEtat(etat: EtatVideos): string {
	const p = new URLSearchParams();
	if (etat.rubrique !== "toutes") p.set("rubrique", etat.rubrique);
	if (etat.langue !== "toutes") p.set("langue", etat.langue);
	if (etat.son !== "tous") p.set("son", etat.son);
	const q = etat.requete.trim();
	if (q !== "") p.set("q", q);
	if (etat.film) p.set("film", etat.film);
	p.sort();
	return p.toString();
}

/** Vrai dès qu'un filtre restreint la liste — ce qui décide d'offrir la réinitialisation. */
export function aDesFiltres(etat: EtatVideos): boolean {
	return (
		etat.rubrique !== "toutes" ||
		etat.langue !== "toutes" ||
		etat.son !== "tous" ||
		etat.requete.trim() !== ""
	);
}
