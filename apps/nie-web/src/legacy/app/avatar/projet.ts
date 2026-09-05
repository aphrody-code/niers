import type { Catalogue } from "./types";

export type EtatAvatar = {
	choix: Record<number, string>;
	valeurs: Record<string, number>;
	champs: Record<string, string>;
	genre: number;
	morphologie: number;
};
export type Projet = {
	format: "nie-avatar";
	version: 1;
	nom: string;
	avatar: EtatAvatar;
	transformation: { rotation: number; echelle: number };
};

export function nouveauProjet(avatar: EtatAvatar): Projet {
	return { format: "nie-avatar", version: 1, nom: "Mon avatar", avatar,
		transformation: { rotation: 0, echelle: 1 } };
}

const objet = (v: unknown): v is Record<string, unknown> =>
	typeof v === "object" && v !== null && !Array.isArray(v);
const nombre = (v: unknown, min: number, max: number): v is number =>
	typeof v === "number" && Number.isFinite(v) && v >= min && v <= max;

/** Valide un document local sans accepter d'URL, chemin ou code exécutable. */
export function lireProjet(texte: string, catalogue: Catalogue): Projet {
	if (texte.length > 100_000) throw new Error("Projet trop volumineux (100 Ko maximum).");
	const p: unknown = JSON.parse(texte);
	if (!objet(p) || p.format !== "nie-avatar" || p.version !== 1 ||
		typeof p.nom !== "string" || p.nom.length > 120 || !objet(p.avatar) ||
		!objet(p.transformation)) throw new Error("Format de projet NIE non reconnu.");
	const a = p.avatar;
	if (!objet(a.choix) || !objet(a.valeurs) || !objet(a.champs) ||
		!nombre(a.genre, 0, 1) || !Number.isInteger(a.genre) ||
		!nombre(a.morphologie, 0, catalogue.modelesDeBase.morphologies.length - 1) ||
		!Number.isInteger(a.morphologie) ||
		!nombre(p.transformation.rotation, -180, 180) ||
		!nombre(p.transformation.echelle, 0.25, 4)) throw new Error("Réglages du projet invalides.");
	const choix: Record<number, string> = {};
	for (const [cle, id] of Object.entries(a.choix)) {
		const cat = catalogue.categories.find(c => String(c.faceSettingType) === cle);
		if (!cat || typeof id !== "string" || (id !== "" && !cat.parts.some(part => part.id === id)))
			throw new Error(`Pièce inconnue dans la catégorie ${cle}.`);
		choix[Number(cle)] = id;
	}
	const valeurs: Record<string, number> = {};
	for (const [cle, v] of Object.entries(a.valeurs)) {
		if (!/^[a-zA-Z0-9_.-]{1,80}$/.test(cle) || ["__proto__", "constructor", "prototype"].includes(cle) ||
			!nombre(v, -10000, 10000)) throw new Error("Valeur numérique invalide.");
		if (cle.startsWith("couleur.")) {
			const cat = catalogue.categories.find(c => `couleur.${c.faceSettingType}` === cle);
			if (!cat || !Number.isInteger(v) || v < -1 || v >= cat.couleurs.length)
				throw new Error("Couleur absente du catalogue.");
		}
		if (cle === "taille" && (!Number.isInteger(v) || v < 0 || v > 14))
			throw new Error("Taille hors limites.");
		valeurs[cle] = v;
	}
	const champs: Record<string, string> = {};
	for (const [cle, v] of Object.entries(a.champs)) {
		if (!/^[a-zA-Z0-9_.-]{1,80}$/.test(cle) || ["__proto__", "constructor", "prototype"].includes(cle) ||
			typeof v !== "string" || v.length > 500) throw new Error("Champ texte invalide.");
		champs[cle] = v;
	}
	return { format: "nie-avatar", version: 1, nom: p.nom,
		avatar: { choix, valeurs, champs, genre: a.genre, morphologie: a.morphologie },
		transformation: { rotation: p.transformation.rotation, echelle: p.transformation.echelle } };
}

export type Historique = { passes: Projet[]; present: Projet; futurs: Projet[] };
export type ActionProjet = { type: "modifier"; projet: Projet } | { type: "annuler" | "retablir" };
/** Historique borné ; une nouvelle branche invalide uniquement le rétablissement. */
export function reduireProjet(h: Historique, action: ActionProjet): Historique {
	if (action.type === "modifier") {
		if (JSON.stringify(h.present) === JSON.stringify(action.projet)) return h;
		return { passes: [...h.passes.slice(-49), h.present], present: action.projet, futurs: [] };
	}
	if (action.type === "annuler") {
		const precedent = h.passes.at(-1);
		return precedent ? { passes: h.passes.slice(0, -1), present: precedent, futurs: [h.present, ...h.futurs] } : h;
	}
	const suivant = h.futurs[0];
	return suivant ? { passes: [...h.passes, h.present], present: suivant, futurs: h.futurs.slice(1) } : h;
}

export function telecharger(blob: Blob, nom: string): void {
	const url = URL.createObjectURL(blob);
	const lien = document.createElement("a");
	lien.href = url;
	lien.download = nom;
	lien.click();
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}
