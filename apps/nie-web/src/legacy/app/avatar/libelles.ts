/**
 * Les libellés de l'éditeur, désignés par leur **hachage du jeu**.
 *
 * Aucun texte n'est écrit ici. Ce module ne contient que des CRC-32 — les identifiants que les
 * scripts de l'éditeur emploient pour demander un libellé à `menu_text` — et le résolveur qui les
 * change en texte à partir du catalogue (`panneaux[].libelles`, produit par `niers avatar export`).
 *
 * Conséquence : changer la locale du catalogue change toute l'interface, et un libellé que le jeu
 * n'a pas ne peut pas apparaître, faute de hachage à écrire.
 *
 * Les groupes ci-dessous suivent les scripts d'où les hachages viennent : `chara_edit_menu` pour le
 * cadre, `chara_edit_parts_menu_inc` pour les titres de sections et de curseurs,
 * `chara_edit_parts_menu_status` / `_voice` / `_fashion` pour les listes de choix.
 */

import type { Catalogue } from "./types";

/** Hachages du cadre : onglets, barre de commandes, dialogues. */
export const H = {
	// Onglets (`chara_edit_menu`).
	ongletStyle: "B4AFB528",
	ongletPhysionomie: "1C9870DB",
	ongletVisage: "60C38DF5",
	ongletHabits: "543F94CC",
	ongletStats: "08E8DA76",
	ongletNom: "9913056F",

	// Cadre.
	titre: "1D32DB18",
	codeAvatar: "4D6B0AF9",
	suivant: "6D5F5829",
	termine: "254046EA",

	// Barre de commandes.
	choisir: "98CA12DE",
	zoom: "B5AF3276",
	tourner: "E4B1AA12",
	expression: "284162E5",
	modele: "09B04929",
	cacherCheveux: "CE58DF01",
	afficherCheveux: "502455E6",
	apercuPoses: "C41EED61",
	voirApparence: "909AB60F",
	voixEcoute: "D9933F1D",
	changerPose: "7209A57C",

	// Sections et curseurs (`chara_edit_parts_menu_inc`).
	formeVisage: "974BB0E6",
	largeurVisage: "1FDE773B",
	largeurMachoire: "C6934216",
	longueurVisage: "1B348EFD",
	prereglageCouleur: "77C5F6E7",
	ajusterCouleur: "F0C6AC4F",
	coupeCheveux: "A821E377",
	frange: "3B3DD0F3",
	couleurCheveux: "5AD8E080",
	typeYeux: "04A7737A",
	typePupilles: "322CBA6F",
	reflets: "310114DA",
	position: "C7AAA035",
	couleur: "94BC3183",
	nez: "B42AEEBD",
	bouche: "34FBC64A",
	couleurLevres: "56FD4CEE",
	sourcils: "F70F637F",
	couleursSourcils: "206063FF",
	oreilles: "CBEB0846",
	extra1: "F7A91AF1",
	extra2: "6EA04B4B",
	echantillonVisage: "D49774C1",
	taille: "2A880EB0",
	taillePoitrine: "6C63FFE5",
	col: "C577FD49",
	manches: "7588F056",
	ourlet: "80CC3108",
	elements: "9AE3A976",
	positionPrincipale: "4516AD4E",
	positionSecondaire: "846314C7",
	typeBuild: "E44675CD",
	personnalite1: "37B7E87C",
	personnalite2: "AEBEB9C6",
	personnalite: "061859E6",
	type: "824D4819",
	nom: "9913056F",
	surnom: "282D3ACD",
	nomUniforme: "80C3302E",
	numeroMaillot: "9BC94347",
	techniquesAApprendre: "6FD8E4BA",

	// Messages d'aide et avertissements.
	aidePosition: "F8026C9D",
	aidePersonnalite: "057E584B",
	aideBuild: "EE1516A2",
	aideNom: "46CA9A13",
	avertissementHabits: "5370BC9C",

	// Genre.
	masculin: "D6D7F0BC",
	feminin: "724506F8",
} as const;

/** Les quatre éléments, dans l'ordre du script de statistiques. */
export const ELEMENTS = ["1AD56B6C", "5D4CADCF", "6300365D", "0B6C830B"] as const;

/** Les six types de build, dans l'ordre du script, avec leur description. */
export const BUILDS = [
	{ hash: "EC23081A", aide: "FD5E579E" },
	{ hash: "1A816337", aide: "F49A2B51" },
	{ hash: "C89E2F88", aide: "CABFF2FF" },
	{ hash: "56304F26", aide: "B82B0740" },
	{ hash: "768005BC", aide: "881FBE21" },
	{ hash: "1C435F33", aide: "B2DC1F46" },
] as const;

/** Les 24 personnalités de voix, dans l'ordre du script. */
export const PERSONNALITES_VOIX = [
	"524BDB5C",
	"CB428AE6",
	"BC45BA70",
	"22212FD3",
	"55261F45",
	"CC2F4EFF",
	"BB287E69",
	"2B9763F8",
	"5C90536E",
	"3C57DA8B",
	"4B50EA1D",
	"D259BBA7",
	"A55E8B31",
	"3B3A1E92",
	"4C3D2E04",
	"D5347FBE",
	"A2334F28",
	"328C52B9",
	"458B622F",
	"177A8948",
	"607DB9DE",
	"F974E864",
	"8E73D8F2",
	"10174D51",
] as const;

/** Les deux types de voix. */
export const TYPES_VOIX = ["752A7D5B", "EC232CE1"] as const;

/** Les cinq styles d'habits (`chara_edit_parts_menu_fashion`). */
export const STYLES_HABITS = [
	"32E2996F",
	"7F751E8A",
	"17FEF7EC",
	"9E6BC849",
	"CF3A12C8",
] as const;

/**
 * Les huit morphologies, dans l'ordre de `nie_data::chara_edit::BODY_TYPES`.
 *
 * Sept libellés pour huit entrées : `male` et `female` partagent « Moyen », le genre étant porté
 * par l'onglet Style. Les hachages viennent de `chara_edit_menu`.
 */
export const MORPHOLOGIES = [
	"81E951EF", // male → Moyen
	"81E951EF", // female → Moyen
	"139E99F3", // small → Petit
	"9823C3C9", // smallfat → Petit (enrobé)
	"DC086AC2", // tall → Grand
	"3D942D07", // tallmuscle → Grand (musclé)
	"537D20E4", // muscle → Musclé
	"96ADF126", // big → Gros
] as const;

/** Un libellé résolu : son texte et ses glyphes spéciaux. */
export type Libelle = { libelle: string; gaiji: string[] };

/**
 * Construit le résolveur `hash → libellé` à partir du catalogue.
 *
 * Un même hachage peut apparaître dans plusieurs panneaux avec le même texte ; le premier gagne.
 * Un hachage absent rend `undefined` — l'appelant n'affiche alors rien, plutôt qu'un identifiant.
 */
export function resolveur(catalogue: Catalogue): (hash: string) => Libelle | undefined {
	const table = new Map<string, Libelle>();
	for (const panneau of catalogue.panneaux ?? []) {
		for (const l of panneau.libelles) {
			if (!table.has(l.hash)) table.set(l.hash, { libelle: l.libelle, gaiji: l.gaiji ?? [] });
		}
	}
	for (const r of catalogue.rubriques ?? []) {
		if (!table.has(r.hash)) table.set(r.hash, { libelle: r.libelle, gaiji: [] });
	}
	return (hash: string) => table.get(hash);
}
