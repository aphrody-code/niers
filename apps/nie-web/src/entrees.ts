/**
 * Les entrées du site — une seule liste, pour tout l'écran.
 *
 * ## Pourquoi ce fichier existe
 *
 * La liste vivait en double : une fois dans `App.tsx` (pour le routage), une fois dans
 * `MenuPrincipal.tsx` (pour la rangée de tuiles), chacune avec son habillage. Les deux ont
 * dérivé — l'accueil affichait des comptes que la barre de navigation n'avait pas, et la barre
 * nommait « Catalogues » une liste qui contenait l'explorateur. Un seul endroit décide
 * désormais de ce que le site propose.
 *
 * ## Ce qui n'est PAS ici
 *
 * Aucun chiffre. Les totaux publiés par le serveur (54 203 textures, 255 308 entrées indexées)
 * décrivent l'index, pas ce qu'on peut faire du site : les afficher sur une tuile ajoute une
 * donnée d'inventaire là où l'on choisit une destination. Le jeu, lui, ne met aucun compte sur
 * les tuiles de son menu.
 */
import type { SanteApi } from "@niers/asset-source";
import type { NomGlyphe } from "@niers/inacord-ui";

/** L'explorateur n'est pas un catalogue : il montre la structure, pas une sélection. */
export const EXPLORATEUR = "explorateur";

/**
 * Les données du jeu et de la série — 224 tables mesurées, pas une liste écrite ici.
 *
 * Ce n'est ni un catalogue (qui filtre l'espace VFS) ni l'explorateur (qui en montre la
 * structure) : ce sont les **valeurs**, celles que `/api/v1/entites` sert avec la recherche, le
 * tri, l'égalité, les intervalles et l'export. Elles n'avaient aucune porte dans l'interface.
 */
export const DONNEES = "donnees";

/**
 * La recherche qui traverse l'arbre entier.
 *
 * Distincte de l'explorateur, et la distinction est la raison d'être des deux : l'explorateur
 * répond à « qu'y a-t-il **ici** », cette page à « où est **ceci** ». Les fondre donnerait un
 * préfixe qu'on navigue *et* un préfixe qu'on tape, sur le même écran.
 */
export const RECHERCHE = "recherche";

/**
 * Les catalogues que le serveur publie sous forme d'URL, dans son document d'accueil.
 *
 * Ce ne sont pas des noms devinés : `nie-site` sert `/textures`, `/modeles`, `/sons` et
 * `/videos` avec leurs métadonnées propres, et les liste dans le HTML rendu sans JavaScript.
 * Les garder ici permet d'afficher le menu complet avant que `/api/v1/health` ait répondu —
 * sans cette liste, l'accueil montrait une seule tuile pendant tout le chargement.
 */
export const CATALOGUES = ["textures", "modeles", "sons", "videos"] as const;

/** Une entrée du menu : sa route, son libellé, son pictogramme. */
export interface EntreeMenu {
	/** Le segment d'URL — c'est aussi l'identité de l'entrée. */
	vue: string;
	libelle: string;
	glyphe: NomGlyphe;
}

/**
 * Le seul habillage figé du site : un libellé et un pictogramme par entrée connue.
 *
 * Une vue que le serveur publierait sans figurer ici s'affiche sous SON nom, avec un
 * pictogramme neutre — jamais sous un libellé inventé.
 */
const HABILLAGE: Record<string, { libelle: string; glyphe: NomGlyphe }> = {
	textures: { libelle: "Textures", glyphe: "image" },
	modeles: { libelle: "Modèles", glyphe: "cube" },
	sons: { libelle: "Sons", glyphe: "onde" },
	videos: { libelle: "Vidéos", glyphe: "film" },
	[EXPLORATEUR]: { libelle: "Explorer", glyphe: "arbre" },
	[DONNEES]: { libelle: "Données", glyphe: "arbre" },
	[RECHERCHE]: { libelle: "Rechercher", glyphe: "arbre" },
};

/** Le libellé d'une entrée, ou son nom brut si le site ne la connaît pas. */
export function libelleEntree(vue: string): string {
	return HABILLAGE[vue]?.libelle ?? vue;
}

/**
 * Les entrées du menu : les catalogues connus, complétés par ceux que le serveur publie en
 * plus, puis l'explorateur.
 *
 * L'ordre vient du serveur quand il a répondu — c'est lui qui décide de la place d'un
 * catalogue. L'explorateur ferme toujours la marche : il ne parcourt pas un catalogue mais
 * l'arborescence, et le serveur ne le publie pas comme une vue.
 */
export function entreesMenu(etat: SanteApi | null): EntreeMenu[] {
	const noms = etat?.vues.length
		? etat.vues.map((v) => v.nom)
		: [...CATALOGUES];
	return [...noms, EXPLORATEUR, RECHERCHE, DONNEES].map((vue) => ({
		vue,
		libelle: libelleEntree(vue),
		glyphe: HABILLAGE[vue]?.glyphe ?? "arbre",
	}));
}
