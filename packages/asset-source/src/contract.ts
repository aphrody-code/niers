/**
 * `AssetSource` — la porte unique par laquelle l'interface partagée atteint les ressources du
 * jeu, quel que soit l'hôte qui la monte.
 *
 * ## Pourquoi un contrat plutôt qu'un client
 *
 * La même interface tourne dans deux hôtes qui n'ont pas les mêmes pouvoirs : **Inacord**
 * (Tauri) parle au disque, aux CPK et au jeu en cours ; **Aphrody** (navigateur) ne parle qu'à
 * `nie-site` en HTTP. Sur les 147 commandes que l'hôte desktop expose, une soixantaine sont
 * portables — lire le VFS, les données du jeu, une texture, un modèle — et le reste ne le sera
 * jamais : exécuter du Lua, écrire un mod, piloter Blender, lire la mémoire du jeu.
 *
 * ## `capacites` : dire ce qu'on ne sait pas faire
 *
 * Le contrat ne prétend donc pas que tout existe partout. Chaque hôte DÉCLARE ses capacités, et
 * l'interface s'y adapte — elle masque un bouton plutôt que de le laisser échouer au clic.
 * L'alternative, un contrat uniforme dont la moitié des méthodes rejette à l'exécution, ferait
 * découvrir l'absence par une erreur, ce qui est le contraire d'une interface.
 *
 * Les méthodes hors du socle portable sont donc **optionnelles** : leur présence se teste, et
 * `capacites` dit à l'avance ce qui répondra.
 */
import type { Capacites, Fichier, Page, SanteApi, VueCatalogue } from "./nie-site";

/** Ce qu'un hôte sait faire. Mesuré à l'exécution, jamais supposé. */
export interface CapacitesSource {
	/** Lire l'arborescence et le contenu du VFS. Les deux hôtes le savent. */
	vfs: boolean;
	/** Servir une texture décodée en image. */
	texture: boolean;
	/** Servir un modèle 3D assemblé. */
	modele: boolean;
	/** Composer un avatar. */
	avatar: boolean;
	/** Servir une piste audio décodée. */
	audio: boolean;
	/** Servir une vidéo transcodée. */
	video: boolean;
	/** Lire les tables du wiki (personnages, techniques, objets…). */
	wiki: boolean;
	/** Écrire : mods, remplacement de texture, export de CPK. Desktop seul. */
	ecriture: boolean;
	/** Atteindre le disque de la machine. Desktop seul. */
	disque: boolean;
	/** Exécuter du Lua, lire la mémoire du jeu, piloter Blender, la forge. Desktop seul. */
	outils: boolean;
}

/** Une entrée du VFS, telle que les deux hôtes la rendent. */
export type EntreeVfs = Fichier;

/** Le contenu direct d'un préfixe : ses sous-dossiers et ses fichiers. */
export interface ContenuDossier {
	prefixe: string;
	dossiers: string[];
	fichiers: EntreeVfs[];
	/** Nombre de fichiers retenus par le filtre. Absent : l'hôte ne le compte pas. */
	total?: number;
	/** Nombre de fichiers du dossier AVANT filtre — ce qui donne son sens au précédent. */
	totalSansFiltre?: number;
	/**
	 * Ce que l'hôte a réellement appliqué.
	 *
	 * Republié, et pas seulement accepté : c'est la leçon du lot 8 côté serveur, et elle vaut
	 * ici pour la même raison. Un filtre appliqué sans être avoué ne se distingue pas, vu de
	 * l'appelant, d'un filtre avalé.
	 */
	filtres?: {
		q?: string;
		ext?: string;
		/**
		 * L'extension demandée n'existe nulle part dans ce dossier.
		 *
		 * Distinct de « zéro résultat » : le serveur applique bien le filtre et retient 0, mais
		 * il sait dire que la valeur elle-même est introuvable. Sans ce drapeau, une faute de
		 * frappe se présente comme un dossier vide.
		 */
		extInconnue?: boolean;
	};
}

/**
 * Ce qu'on peut demander en parcourant un dossier.
 *
 * **L'asymétrie est réelle et assumée.** Aphrody filtre côté serveur (`/b?q=&ext=`, index trié
 * de 255 308 chemins) ; Inacord reçoit le dossier entier par IPC et filtre en mémoire. Les deux
 * rendent la même chose pour un dossier, et c'est ce qui compte ici : un dossier du VFS tient
 * en quelques milliers d'entrées, jamais en 255 308. Là où l'asymétrie serait fausse — la
 * pagination d'une VUE, qui recouvre six extensions — le contrat ne l'expose pas du tout (voir
 * `catalogue`).
 */
export interface OptionsParcours {
	/** Sous-chaîne comparée sans casse au chemin entier. Absente : aucun filtre. */
	q?: string;
	/** Extension exacte, sans le point. Absente : aucun filtre. */
	ext?: string;
	signal?: AbortSignal;
}

/** Options communes aux listes paginées. `parPage` est borné par le serveur. */
export interface OptionsPage {
	page?: number;
	parPage?: number;
	/** Motif de recherche, comparé sans casse au chemin entier. Absent : aucun filtre. */
	q?: string;
	/** Extension exacte, sans le point. Absente : aucun filtre. */
	ext?: string;
	/** Critère de tri : `nom` (défaut) ou `taille`. */
	tri?: "nom" | "taille";
	/** Sens de tri : `asc` (défaut) ou `desc`. */
	ordre?: "asc" | "desc";
	signal?: AbortSignal;
}

/**
 * La source de ressources d'un hôte.
 *
 * Le socle — `capacites`, `sante`, `vfs`, `catalogue`, `urlFichier` — est obligatoire : sans
 * lui, l'interface ne peut rien afficher. Tout le reste se déclare.
 */
export interface AssetSource {
	/** Nom de l'hôte, pour les diagnostics (`inacord`, `aphrody`). */
	readonly hote: string;

	/** Ce que cet hôte sait faire, ici et maintenant. */
	capacites(): Promise<CapacitesSource>;

	/** L'état du service : le VFS est-il monté, le gisement ouvert. */
	sante(signal?: AbortSignal): Promise<SanteApi>;

	/** Le contenu d'un préfixe du VFS. Préfixe vide = la racine. */
	parcourir(prefixe: string, options?: OptionsParcours): Promise<ContenuDossier>;

	/**
	 * Une page d'un filtre enregistré (`textures`, `modeles`, `sons`, `videos`).
	 *
	 * Ces vues ne désignent jamais un fichier : ce sont des filtres sur l'espace VFS.
	 *
	 * **Optionnel, et la raison tient à une asymétrie réelle.** `nie-site` indexe le VFS et sait
	 * paginer sur un jeu d'extensions en une requête ; la recherche native d'Inacord ne prend
	 * qu'UNE extension par appel. Une vue en compte jusqu'à six : le desktop ne pourrait donc
	 * rendre une page qu'en concaténant six recherches, dont l'ordre et le décalage ne se
	 * recomposent pas — la page 2 ne suivrait pas la page 1.
	 *
	 * Plutôt que de livrer une pagination fausse sous un nom identique, l'hôte qui ne sait pas
	 * le faire ne l'expose pas, et l'interface énumère par `parcourir()`. Une méthode qui feint
	 * la symétrie coûte plus cher qu'une méthode absente : l'absence se teste, le résultat faux
	 * se découvre en production.
	 */
	catalogue?(vue: VueCatalogue, options?: OptionsPage): Promise<Page<EntreeVfs>>;

	/**
	 * L'URL d'une ressource par son chemin VFS **verbatim**, extension du jeu conservée.
	 *
	 * Le chemin voyage en segment, jamais en query (amendement A3). Un chemin cité de mémoire
	 * est presque toujours faux : les fichiers du jeu portent un numéro de version.
	 */
	urlFichier(cheminVfs: string): string;

	/** L'URL d'une texture décodée en PNG. Absent si `capacites().texture` est faux. */
	urlTexture?(cheminVfs: string): string;

	/** L'URL d'un modèle assemblé (GLB). Absent si `capacites().modele` est faux. */
	urlModele?(code: string): string;

	/** L'URL d'une piste audio décodée. */
	urlAudio?(cheminVfs: string, awbId?: number | null): string;

	/** L'URL d'une vidéo transcodée. */
	urlVideo?(cheminVfs: string): string;

	/**
	 * Une vignette prête à poser dans un `<img src>`.
	 *
	 * Asynchrone, et c'est ce qui compte : les deux hôtes produisent une URL, mais pas de la
	 * même façon. Aphrody rend une URL HTTP servie par `/assets` ; Inacord décode la texture
	 * en natif et rend une URL de données (`data:image/png;base64,…`), faute de serveur local.
	 * Un contrat qui n'exposerait qu'une URL synchrone obligerait le desktop à contrefaire un
	 * serveur, ou le web à contrefaire un décodeur — et ce genre de contrainte finit toujours
	 * par fuir dans les composants.
	 *
	 * Rend `null` quand la ressource n'a pas de vignette (format non pris en charge).
	 *
	 * @param cote côté de la vignette, en pixels.
	 * @param racineJeu racine du jeu, quand l'hôte en a une (desktop). Ignoré sur le web.
	 */
	vignette?(cheminVfs: string, cote: number, racineJeu?: string): Promise<string | null>;

	/** Lecture d'une table du wiki. Absent si `capacites().wiki` est faux. */
	wiki?<T>(table: string, options?: OptionsPage): Promise<Page<T>>;
}

/** Les capacités d'un hôte qui ne sait rien faire — base d'un objet partiel. */
export const AUCUNE_CAPACITE: CapacitesSource = {
	vfs: false,
	texture: false,
	modele: false,
	avatar: false,
	audio: false,
	video: false,
	wiki: false,
	ecriture: false,
	disque: false,
	outils: false,
};

/** Traduit les capacités mesurées d'un `nie-site` en capacités de source. */
export function capacitesDepuisServeur(c: Capacites): CapacitesSource {
	const vfs = c.vfs === "pret" && c.vfs_contenu;
	return {
		...AUCUNE_CAPACITE,
		vfs,
		texture: vfs,
		modele: vfs,
		avatar: vfs,
		audio: vfs,
		video: vfs,
		wiki: c.gisement,
	};
}
