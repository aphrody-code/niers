import type { Catalogue, Categorie, Part } from "./types";

/** Libellés éditoriaux de l'atelier ; les identifiants techniques restent ceux du catalogue. */
export const NOMS_CATEGORIES: Record<number, string> = {
	1: "Visages prédéfinis", 2: "Forme du visage", 3: "Peau", 4: "Coiffure", 5: "Frange",
	6: "Yeux", 7: "Pupilles", 8: "Reflets", 9: "Nez", 10: "Bouche", 11: "Sourcils",
	12: "Oreilles", 13: "Marques du visage", 14: "Accessoires", 16: "Genre", 17: "Morphologie",
	18: "Poitrine", 19: "Col", 20: "Manches", 21: "Ourlet",
};

export function nomCategorie(c: Categorie): string { return NOMS_CATEGORIES[c.faceSettingType] ?? `Pièces ${c.faceSettingType}`; }
export function nomPart(p: Part, index: number): string {
	const nom = p.resource && p.resource !== "0xFFFFFFFF" ? p.resource : p.resource2;
	return nom && nom !== "0xFFFFFFFF" ? nom : `Variante ${index + 1}`;
}

/** URL issue du nom d'icône du catalogue, jamais du rang de sa vignette dans l'interface. */
export function iconePart(cdn: string, part: Pick<Part, "icone">): string | null {
	if (!part.icone) return null;
	if (!/^[A-Za-z0-9_]+_[0-9]+$/.test(part.icone)) return null;
	const atlas = part.icone.slice(0, part.icone.lastIndexOf("_"));
	return `${cdn.replace(/\/$/, "")}/g4tx/dx11/menu/200_icon/21_icon_avatar/${atlas}.g4tx/${part.icone}.png?w=200&format=webp`;
}

/** CRC-32 IEEE des noms du catalogue, indépendant des URL de présentation. */
export function crc32Nom(nom: string): string {
	let crc = 0xffffffff;
	for (const octet of new TextEncoder().encode(nom)) {
		crc ^= octet;
		for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
	}
	return ((crc ^ 0xffffffff) >>> 0).toString(16).toUpperCase().padStart(8, "0");
}

export type Liaison = { categorie: number; part: string; nom: string; icone: string | null; iconeHash: string;
	url: string | null; ressources: string[]; erreurs: string[]; avertissements: string[] };

/** Inventaire exhaustif, comprenant aussi les vignettes absentes et les paramètres sans texture. */
export function auditerLiaisons(catalogue: Catalogue, cdn: string): Liaison[] {
	return catalogue.categories.flatMap(c => {
		const ids = new Set<string>();
		return c.parts.map((p, i) => {
			const erreurs: string[] = [], avertissements: string[] = [];
			if (ids.has(p.id)) erreurs.push("Identifiant de pièce dupliqué dans la catégorie"); ids.add(p.id);
			if (p.icone && crc32Nom(p.icone) !== p.iconeHash.toUpperCase()) erreurs.push("Le nom de vignette ne correspond pas à son CRC-32 source");
			const url = iconePart(cdn, p);
			if (p.icone && !url) erreurs.push("Nom d’icône non pris en charge");
			if (!p.icone && !["00000000", "FFFFFFFF"].includes(p.iconeHash.toUpperCase())) erreurs.push("Hash de vignette présent mais nom non résolu");
			if (!p.icone) avertissements.push("Le catalogue ne fournit pas de vignette : ne pas lui substituer l’icône voisine");
			const ressources = [...new Set([...p.modeles, ...(p.modeles2 ?? [])])];
			for (const path of ressources) if (!path.startsWith("data/") || path.includes("..") || /[?#\\]/.test(path)) erreurs.push(`Chemin de ressource invalide : ${path}`);
			if (!ressources.length) avertissements.push("Paramètre/recette sans fichier de texture ou de maillage direct");
			return { categorie: c.faceSettingType, part: p.id, nom: nomPart(p, i), icone: p.icone, iconeHash: p.iconeHash,
				url, ressources, erreurs, avertissements };
		});
	});
}
