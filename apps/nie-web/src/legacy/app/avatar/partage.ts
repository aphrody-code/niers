/**
 * Le code de partage d'avatar : encodage et décodage.
 *
 * Rien n'est écrit en dur ici. La spécification vient du catalogue (`codePartage`, produit par
 * `niers avatar export` depuis `m_CharaEditCodeInfoList`) et se compose de trois éléments :
 *
 * - un **alphabet de 64 caractères**, celui que le jeu emploie — il évite les glyphes qui se
 *   confondent à la lecture (`0`/`O`, `1`/`l`/`I`, `Z`/`2`), ce qui se voit à ce qu'il commence
 *   à `3` ;
 * - un total de **410 bits** ;
 * - **86 emplacements**, chacun avec son nombre de bits, sa catégorie et son nombre de valeurs
 *   admises. Leur somme fait exactement 410, ce qui se vérifie à l'exécution.
 *
 * 64 caractères, c'est 6 bits chacun : 410 bits tiennent donc sur 69 caractères, le dernier
 * n'étant rempli qu'aux deux tiers.
 *
 * ## Ce que ce module garantit, et ce qu'il ne garantit pas
 *
 * L'aller-retour est **exact** : `decoder(encoder(v))` rend `v`, et le test le vérifie sur des
 * valeurs limites. Chaque valeur est bornée par le `valeurs` de son emplacement.
 *
 * En revanche, **la compatibilité avec les codes du jeu n'est pas établie** : elle exigerait de
 * savoir à quel réglage de l'éditeur correspond chaque couple `(categorie, param)`, ce qu'aucune
 * source lue à ce jour ne dit. Un code produit ici se relit donc ici ; le prétendre lisible par
 * le jeu serait une supposition, pas un fait.
 */

import type { Catalogue } from "./types";

/** Un emplacement du code, tel que le catalogue le décrit. */
export type Emplacement = {
	bits: number;
	categorie: number;
	emplacement: number;
	param: number;
	paramSub: number;
	valeurs: number;
};

/** La spécification du code, extraite du catalogue. */
export type SpecPartage = {
	alphabet: string[];
	bits: number;
	emplacements: Emplacement[];
};

/** Nombre de bits qu'un caractère de l'alphabet transporte. */
const BITS_PAR_CARACTERE = 6;

/**
 * Vérifie que la spécification se tient, et rend un message si elle ne se tient pas.
 *
 * Un catalogue tronqué ou d'une autre version produirait sinon des codes silencieusement faux.
 */
export function verifierSpec(spec: SpecPartage): string | null {
	if (spec.alphabet.length !== 1 << BITS_PAR_CARACTERE) {
		return `alphabet de ${spec.alphabet.length} caractères, ${1 << BITS_PAR_CARACTERE} attendus`;
	}
	if (new Set(spec.alphabet).size !== spec.alphabet.length) {
		return "l'alphabet contient des caractères en double";
	}
	const somme = spec.emplacements.reduce((t, e) => t + e.bits, 0);
	if (somme !== spec.bits) {
		return `les emplacements totalisent ${somme} bits, ${spec.bits} annoncés`;
	}
	const trop = spec.emplacements.find((e) => e.valeurs > 1 << e.bits);
	if (trop) {
		return `l'emplacement ${trop.emplacement} admet ${trop.valeurs} valeurs sur ${trop.bits} bits`;
	}
	return null;
}

/** Extrait la spécification du catalogue, ou `null` si elle y manque. */
export function specDe(catalogue: Catalogue): SpecPartage | null {
	const cp = catalogue.codePartage;
	if (!cp?.alphabet?.length || !cp.emplacements?.length) return null;
	return { alphabet: cp.alphabet, bits: cp.bits, emplacements: cp.emplacements };
}

/**
 * Encode une valeur par emplacement en code de partage.
 *
 * `valeurs` doit avoir autant d'entrées que la spécification a d'emplacements ; une valeur hors
 * bornes est ramenée dans les bornes plutôt que de produire un code invalide.
 */
export function encoder(spec: SpecPartage, valeurs: number[]): string {
	const bits: number[] = [];
	spec.emplacements.forEach((e, i) => {
		const max = Math.max(0, e.valeurs - 1);
		const v = Math.min(Math.max(Math.trunc(valeurs[i] ?? 0), 0), max);
		// Bit de poids fort en premier : c'est l'ordre de lecture naturel du code.
		for (let b = e.bits - 1; b >= 0; b--) bits.push((v >> b) & 1);
	});

	let sortie = "";
	for (let i = 0; i < bits.length; i += BITS_PAR_CARACTERE) {
		let index = 0;
		for (let b = 0; b < BITS_PAR_CARACTERE; b++) {
			index = (index << 1) | (bits[i + b] ?? 0);
		}
		sortie += spec.alphabet[index];
	}
	return sortie;
}

/**
 * Décode un code de partage en valeur par emplacement.
 *
 * Rend `null` si le code contient un caractère étranger à l'alphabet ou s'il est trop court : un
 * code abîmé doit être refusé, pas interprété au mieux.
 */
export function decoder(spec: SpecPartage, code: string): number[] | null {
	const rang = new Map(spec.alphabet.map((c, i) => [c, i]));
	const bits: number[] = [];
	for (const c of code.trim()) {
		const index = rang.get(c);
		if (index === undefined) return null;
		for (let b = BITS_PAR_CARACTERE - 1; b >= 0; b--) bits.push((index >> b) & 1);
	}
	if (bits.length < spec.bits) return null;

	const valeurs: number[] = [];
	let pos = 0;
	for (const e of spec.emplacements) {
		let v = 0;
		for (let b = 0; b < e.bits; b++) v = (v << 1) | (bits[pos + b] ?? 0);
		pos += e.bits;
		const max = Math.max(0, e.valeurs - 1);
		valeurs.push(Math.min(v, max));
	}
	return valeurs;
}

/** Longueur d'un code pour cette spécification. */
export function longueurCode(spec: SpecPartage): number {
	return Math.ceil(spec.bits / BITS_PAR_CARACTERE);
}

/**
 * Range les choix de l'éditeur dans les emplacements du code.
 *
 * Chaque catégorie du catalogue occupe un emplacement, dans l'ordre de son `faceSettingType` :
 * la valeur rangée est le **rang** de la part choisie dans sa catégorie. Les emplacements
 * excédentaires restent à zéro.
 *
 * Ce rangement est **réversible** — c'est ce que garantit le test d'aller-retour — mais il n'est
 * pas celui du jeu : établir la correspondance entre les couples `(categorie, param)` du code et
 * les réglages de l'éditeur demanderait une source qu'aucun fichier lu ne fournit. Un code produit
 * ici se relit donc ici.
 */
export function valeursDepuisChoix(
	catalogue: Catalogue,
	spec: SpecPartage,
	choix: Record<number, string>,
	genre: number,
	morphologie: number,
): number[] {
	const valeurs = Array.from({ length: spec.emplacements.length }, () => 0);
	const categories = [...catalogue.categories].sort(
		(a, b) => a.faceSettingType - b.faceSettingType,
	);

	// Les deux premiers emplacements portent le genre et la morphologie, qui ne sont pas des parts.
	valeurs[0] = genre;
	valeurs[1] = morphologie;

	categories.forEach((cat, i) => {
		const cible = i + 2;
		if (cible >= valeurs.length) return;
		const id = choix[cat.faceSettingType];
		const rang = id ? cat.parts.findIndex((p) => p.id === id) : -1;
		valeurs[cible] = Math.max(0, rang);
	});
	return valeurs;
}

/** Rétablit les choix depuis les valeurs décodées d'un code. */
export function choixDepuisValeurs(
	catalogue: Catalogue,
	valeurs: number[],
): { choix: Record<number, string>; genre: number; morphologie: number } {
	const choix: Record<number, string> = {};
	const categories = [...catalogue.categories].sort(
		(a, b) => a.faceSettingType - b.faceSettingType,
	);
	categories.forEach((cat, i) => {
		const rang = valeurs[i + 2] ?? 0;
		const part = cat.parts[rang];
		if (part) choix[cat.faceSettingType] = part.id;
	});
	return { choix, genre: valeurs[0] ?? 0, morphologie: valeurs[1] ?? 0 };
}
