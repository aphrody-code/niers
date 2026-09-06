/**
 * Fabrique d'embeds — la forme commune de TOUTES les réponses des deux bots.
 *
 * ── CE QUE CE MODULE GARANTIT ──────────────────────────────────────────────
 *  1. AUCUN message refusé par l'API pour cause de dépassement. Chaque texte
 *     est borné à sa limite de champ, et le budget global de 6 000 caractères
 *     est tenu à l'ajout : un champ qui ne rentre plus n'est PAS posé, au lieu
 *     de faire échouer l'envoi entier (`LIMITES`, `BUDGET_EMBED`).
 *  2. Une identité constante : couleur du profil, pied de page portant la
 *     marque, horodatage. Un membre reconnaît une réponse du bot avant même de
 *     l'avoir lue.
 *  3. Une grammaire unique — titre avec icône, ligne de contexte pour les
 *     filtres, champs courts en ligne, pied pour la provenance et la position.
 *
 * ── POURQUOI UNE CLASSE ET PAS DES FONCTIONS ───────────────────────────────
 * Le budget se tient à l'AJOUT, pas à la fin : savoir qu'on a dépassé une fois
 * l'embed construit ne dit pas quoi retirer. `Fiche` compte ce qui a été posé
 * et refuse ce qui ne rentre plus, en le disant (`tronquee`) pour que la
 * commande puisse écrire « 12 sur 340 » plutôt que de laisser croire à une
 * liste complète.
 *
 * `EmbedBuilder` reste accessible (`.embed`) : les commandes existantes
 * continuent de fonctionner sans réécriture.
 */
import { EmbedBuilder, type APIEmbedField } from "discord.js";

import {
	BUDGET_EMBED,
	COULEURS,
	ICONES,
	ICONE_INTENTION,
	LIMITES,
	MARQUE,
	type Intention,
	type NomIcone,
} from "./theme";

// ─── Texte ──────────────────────────────────────────────────────────────────

import { borner, decouper, joindre } from "./texte";

export { borner, cle, decouper, echapper, ilYA, joindre, leJour, puces, rangs } from "./texte";

// ─── Embed ──────────────────────────────────────────────────────────────────

export interface OptionsEmbed {
	/** Nature de la réponse : décide la couleur, et l'icône du titre par défaut. */
	intention?: Intention;
	/** Icône de titre — nom du lexique, émoji littérale, ou `""` pour aucune. */
	icone?: NomIcone | string;
	titre?: string;
	/** Lien du titre. Une URL invalide fait LEVER `EmbedBuilder` : elle est filtrée. */
	url?: string | null;
	description?: string;
	/**
	 * Ligne de contexte affichée au-dessus du titre (zone « auteur »).
	 * Sa vocation : les FILTRES actifs et la portée d'une recherche, c'est-à-dire
	 * ce qui explique pourquoi la liste ne contient pas tout.
	 */
	contexte?: string;
	miniature?: string | null;
	image?: string | null;
	/** Couleur imposée (couleur d'élément d'une fiche de jeu, par exemple). */
	couleur?: number | null;
	/** Horodater l'embed. Vrai par défaut : une donnée sans date se périme en silence. */
	horodater?: boolean;
}

/** Une URL est-elle utilisable telle quelle par l'API des embeds ? */
export function urlValide(valeur: string | null | undefined): valeur is string {
	if (typeof valeur !== "string" || valeur.length === 0) {
		return false;
	}
	try {
		const url = new URL(valeur);
		return url.protocol === "https:" || url.protocol === "http:";
	} catch {
		return false;
	}
}

function resoudreIcone(options: OptionsEmbed): string {
	if (typeof options.icone === "string") {
		return options.icone in ICONES ? ICONES[options.icone as NomIcone] : options.icone;
	}
	return ICONE_INTENTION[options.intention ?? "marque"];
}

/**
 * Embed nu à la marque du profil. C'est le point d'entrée de TOUTE réponse.
 *
 * Il n'ajoute PAS de pied : le pied dit la provenance (`/commande`) et la
 * position (`page 2/5`), deux informations que seule l'appelante connaît. Voir
 * `pied()` et `piedDeCommande()` (`lib/ui/reponse.ts`).
 */
export function creerEmbed(options: OptionsEmbed = {}): EmbedBuilder {
	const embed = new EmbedBuilder().setColor(
		options.couleur ?? COULEURS[options.intention ?? "marque"]
	);

	const icone = resoudreIcone(options);
	if (options.titre && options.titre.trim().length > 0) {
		const titre = icone ? `${icone} ${options.titre}` : options.titre;
		embed.setTitle(borner(titre, LIMITES.titre));
	}
	if (urlValide(options.url)) {
		embed.setURL(options.url);
	}
	if (options.description && options.description.trim().length > 0) {
		embed.setDescription(borner(options.description, LIMITES.description));
	}
	if (options.contexte && options.contexte.trim().length > 0) {
		embed.setAuthor({ name: borner(options.contexte, LIMITES.auteur) });
	}
	if (urlValide(options.miniature)) {
		embed.setThumbnail(options.miniature);
	}
	if (urlValide(options.image)) {
		embed.setImage(options.image);
	}
	if (options.horodater !== false) {
		embed.setTimestamp();
	}
	return embed;
}

export interface OptionsPied {
	/** Précision de provenance : `/agenda semaine`, « Lu en direct chez X »… */
	note?: string | null;
	/** Position dans une liste paginée. Les deux sont requis pour être affichés. */
	page?: number | null;
	pages?: number | null;
	/** Remplacer la marque par autre chose n'est pas prévu : elle est toujours là. */
	icone?: string;
}

/**
 * Pied de page normalisé : `note · page 2/5 · Rose Griffon · rosegriffon.fr`.
 *
 * ── POURQUOI IL EST OBLIGATOIRE ────────────────────────────────────────────
 * Les 68 pieds écrits à la main dans ce dépôt avant l'unification remplaçaient
 * la marque au lieu de s'y ajouter : sur les listes paginées, le domaine du
 * site disparaissait, et rien ne disait plus quel bot avait répondu — sur un
 * serveur où les deux vivent côte à côte, c'est la seule information qui
 * permette de savoir à qui redemander.
 */
export function piedTexte(options: OptionsPied = {}): { text: string; iconURL: string } {
	const position =
		typeof options.page === "number" &&
		typeof options.pages === "number" &&
		options.pages > 1 &&
		options.page > 0
			? `page ${options.page}/${options.pages}`
			: null;
	return {
		text: borner(
			joindre([options.note ?? null, position, MARQUE.nom, MARQUE.domaine]),
			LIMITES.pied
		),
		iconURL: options.icone ?? MARQUE.icone,
	};
}

/**
 * Même pied, posé sur l'embed et rendant l'embed — la forme à utiliser au
 * milieu d'une chaîne `rgEmbed().setTitle(…).setFooter(piedTexte(…))` est
 * `piedTexte`, celle-ci sert quand l'embed est déjà dans une variable.
 */
export function pied(embed: EmbedBuilder, options: OptionsPied = {}): EmbedBuilder {
	embed.setFooter(piedTexte(options));
	return embed;
}

// ─── Fiche : embed à budget tenu ────────────────────────────────────────────

export interface OptionsChamp {
	/** Champ court, posé en colonne. Discord en range trois par ligne. */
	enLigne?: boolean;
	/** Poser le champ même si sa valeur est vide (rare : préférer l'omettre). */
	garderVide?: boolean;
}

/**
 * Embed dont le budget est tenu à l'ajout.
 *
 * Une commande construit sa réponse en empilant des champs sans jamais compter :
 * `fiche.champ(...)` refuse ce qui ne rentre plus et le signale. C'est la
 * différence entre une réponse tronquée (lisible, honnête) et un message qui
 * n'arrive jamais.
 */
export class Fiche {
	readonly embed: EmbedBuilder;
	/** Vrai dès qu'un champ a été refusé faute de place. */
	tronquee = false;
	private restant: number;
	private nombreChamps = 0;

	constructor(options: OptionsEmbed = {}) {
		this.embed = creerEmbed(options);
		const donnees = this.embed.toJSON();
		const consomme =
			(donnees.title?.length ?? 0) +
			(donnees.description?.length ?? 0) +
			(donnees.author?.name.length ?? 0);
		// Le pied n'est pas encore posé : on lui réserve sa place au pire cas
		// (note + position + marque + domaine tiennent largement sous 150).
		this.restant = BUDGET_EMBED - consomme - 150;
	}

	/** Description posée après coup (quand elle dépend du nombre d'entrées trouvées). */
	description(texte: string): this {
		const borne = borner(texte, Math.min(LIMITES.description, Math.max(0, this.restant)));
		if (borne.length > 0) {
			this.embed.setDescription(borne);
			this.restant -= borne.length;
		}
		return this;
	}

	/** Ligne de contexte (filtres actifs, portée d'une recherche). */
	contexte(texte: string): this {
		const borne = borner(texte, LIMITES.auteur);
		if (borne.length > 0) {
			this.embed.setAuthor({ name: borne });
			this.restant -= borne.length;
		}
		return this;
	}

	/**
	 * Ajoute un champ s'il rentre. Renvoie `this` dans tous les cas — une
	 * commande n'a pas à tester : elle empile, puis lit `tronquee`.
	 */
	champ(nom: string, valeur: string, options: OptionsChamp = {}): this {
		const texte = (valeur ?? "").toString();
		if (texte.trim().length === 0 && !options.garderVide) {
			return this;
		}
		if (this.nombreChamps >= LIMITES.champs) {
			this.tronquee = true;
			return this;
		}
		const nomBorne = borner(nom, LIMITES.nomChamp);
		const valeurBornee = borner(texte, LIMITES.valeurChamp);
		const cout = nomBorne.length + valeurBornee.length;
		if (cout > this.restant) {
			this.tronquee = true;
			return this;
		}
		this.embed.addFields({
			name: nomBorne,
			value: valeurBornee.length === 0 ? "—" : valeurBornee,
			inline: options.enLigne ?? false,
		});
		this.restant -= cout;
		this.nombreChamps += 1;
		return this;
	}

	/** Empile plusieurs champs d'un coup, dans l'ordre donné. */
	champs(entrees: ReadonlyArray<APIEmbedField>): this {
		for (const entree of entrees) {
			this.champ(entree.name, entree.value, { enLigne: entree.inline });
		}
		return this;
	}

	/**
	 * Découpe un texte long en champs successifs, coupés sur les sauts de ligne.
	 * Le premier porte `nom`, les suivants un nom invisible — Discord refuse un
	 * nom vide, mais accepte l'espace insécable.
	 */
	bloc(nom: string, texte: string, maxChamps = 3): this {
		const morceaux = decouper(texte, LIMITES.valeurChamp, maxChamps);
		morceaux.forEach((morceau, index) => {
			this.champ(index === 0 ? nom : "​", morceau);
		});
		return this;
	}

	miniature(url: string | null | undefined): this {
		if (urlValide(url)) {
			this.embed.setThumbnail(url);
		}
		return this;
	}

	image(url: string | null | undefined): this {
		if (urlValide(url)) {
			this.embed.setImage(url);
		}
		return this;
	}

	couleur(valeur: number | null | undefined): this {
		if (typeof valeur === "number" && Number.isFinite(valeur)) {
			this.embed.setColor(valeur);
		}
		return this;
	}

	/** Pose le pied et rend l'embed prêt à envoyer. */
	finir(options: OptionsPied = {}): EmbedBuilder {
		return pied(this.embed, options);
	}
}

/** Raccourci : `fiche({...})` se lit mieux que `new Fiche({...})` dans une commande. */
export function fiche(options: OptionsEmbed = {}): Fiche {
	return new Fiche(options);
}
