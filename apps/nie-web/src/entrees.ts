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

/**
 * L'explorateur — **la seule page**, décidé par l'utilisateur le 2026-09-06.
 *
 * Parcourir un dossier, chercher dans les 255 308 entrées et lire ce que le serveur sait d'un
 * asset étaient trois destinations de menu. C'est un seul geste : *où est ce fichier, et que
 * sait-on de lui*. La page a donc une barre de filtres, une liste, et un panneau de droite qui
 * parle du **dossier courant** ou de l'**asset sélectionné**.
 */
export const EXPLORATEUR = "explorateur";

/**
 * Les deux URL héritées des écrans fusionnés.
 *
 * Elles restent **reconnues** — elles mènent à l'explorateur — sans être des entrées de menu :
 * casser une adresse déjà publiée (`sitemap.xml` compris) pour changer un menu, ce serait payer
 * une décision d'affichage avec les liens des autres.
 */
export const ALIAS = ["recherche", "donnees"] as const;

/**
 * Les catalogues que le serveur publie sous forme d'URL, dans son document d'accueil.
 *
 * Ce ne sont pas des noms devinés : `nie-site` sert `/textures`, `/modeles`, `/sons` et
 * `/videos` avec leurs métadonnées propres, et les liste dans le HTML rendu sans JavaScript.
 * Les garder ici permet d'afficher le menu complet avant que `/api/v1/health` ait répondu —
 * sans cette liste, l'accueil montrait une seule tuile pendant tout le chargement.
 */
export const CATALOGUES = ["textures", "modeles", "sons", "videos"] as const;

/**
 * Les médias — **une seule page**, décidé par l'utilisateur le 2026-09-06.
 *
 * Quatre entrées de menu pour quatre filtres du même index faisaient quatre destinations là où
 * il n'y a qu'une question : *montre-moi ce que le jeu contient, de ce type-là*. La vue est
 * devenue un réglage de la page, au même titre que le tri, et les quatre URL y mènent toujours.
 */
export const MEDIAS = "medias";

/**
 * Les Options — l'écran des réglages du jeu, avec les réglages d'Inacord dedans.
 *
 * Segment anglais (`/settings`), comme toute URL nouvelle. La tuile porte l'engrenage du jeu.
 */
export const SETTINGS = "settings";

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
	[MEDIAS]: { libelle: "Médias", glyphe: "image" },
	[EXPLORATEUR]: { libelle: "Explorer", glyphe: "arbre" },
	[SETTINGS]: { libelle: "Options", glyphe: "engrenage" },
};

/** Le libellé d'une entrée, ou son nom brut si le site ne la connaît pas. */
export function libelleEntree(vue: string): string {
	return HABILLAGE[vue]?.libelle ?? vue;
}

/**
 * Les routes que l'application reconnaît — le menu n'en montre que deux.
 *
 * `/recherche` et `/donnees` mènent à l'explorateur ; `/textures`, `/modeles`, `/sons` et
 * `/videos` mènent aux médias, sur leur vue. Aucune n'est une tuile, et toutes restent
 * servies : casser une adresse déjà publiée (`sitemap.xml` compris) pour changer un menu, ce
 * serait payer une décision d'affichage avec les liens des autres.
 */
export function routesReconnues(etat: SanteApi | null): string[] {
	return [...entreesMenu(etat).map((e) => e.vue), ...ALIAS, ...CATALOGUES];
}

/**
 * Les entrées du menu : les médias, l'explorateur, puis les Options.
 *
 * L'ordre vient du serveur quand il a répondu — c'est lui qui décide de la place d'un
 * catalogue. L'explorateur ferme toujours la marche : il ne parcourt pas un catalogue mais
 * l'arborescence, et le serveur ne le publie pas comme une vue.
 */
export function entreesMenu(_etat: SanteApi | null): EntreeMenu[] {
	return [MEDIAS, EXPLORATEUR, SETTINGS].map((vue) => ({
		vue,
		libelle: libelleEntree(vue),
		glyphe: HABILLAGE[vue]?.glyphe ?? "arbre",
	}));
}
