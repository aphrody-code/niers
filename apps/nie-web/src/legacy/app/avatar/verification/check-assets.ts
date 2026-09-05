/** Audit réel : bun apps/azalee/app/avatar/verification/check-assets.ts [origine CDN]
 * Vérifie les CRC source, les URL UI et les pixels décodés face à l'icône canonique.
 * Une identité de routage n'est pas une preuve artistique de ressemblance au modèle 3D.
 */
import sharp from "sharp";
import { auditerLiaisons } from "../liaisons";
import { composerUrlAvatar } from "../composition";
import type { Catalogue } from "../types";

const cdn = process.argv[2] ?? "https://cdn.rosegriffon.fr";
const source = await fetch(`${cdn}/avatar/catalog.json`, { signal: AbortSignal.timeout(30_000) });
if (!source.ok) throw new Error(`Catalogue : HTTP ${source.status}`);
const catalogue = await source.json() as Catalogue;
const liaisons = auditerLiaisons(catalogue, cdn);
// Parcourt chaque sélection réelle : la ressource affichée dans la bibliothèque doit
// arriver dans la requête d'assemblage, y compris la seconde moitié d'une coiffure.
const selections = catalogue.categories.flatMap(c => c.parts.map(part => {
	const url = composerUrlAvatar(catalogue, cdn, { choix: { [c.faceSettingType]: part.id }, valeurs: {}, champs: {}, genre: 0, morphologie: 0 });
	const erreurs: string[] = [];
	const parsed = url ? new URL(url) : null;
	const couches = parsed?.searchParams.get("face")?.split(",") ?? [];
	for (const chemin of [...part.modeles, ...(part.modeles2 ?? [])]) {
		if (chemin.includes("/_facetex/") && chemin.endsWith(".g4tx")) {
			const texture = chemin.split("/_facetex/")[1].replace(/\.g4tx$/, "");
			if (!couches.includes(texture)) erreurs.push(`Texture absente de la requête : ${texture}`);
		}
		if (chemin.includes("/20_EDIT/") && chemin.endsWith(".g4md")) {
			const bouts = chemin.split("/20_EDIT/")[1].split("/");
			const piece = `${bouts[0]}/${bouts[1].replace(/\.g4md$/, "")}`;
			if (!parsed?.pathname.includes(piece)) erreurs.push(`Maille absente de la requête : ${piece}`);
		}
	}
	return { categorie: c.faceSettingType, part: part.id, url, erreurs };
}));
if (!liaisons.length) throw new Error("Aucune pièce auditée : refus d’un faux vert.");
const uniques = [...new Map(liaisons.filter(l => l.icone && l.url).map(l => [l.icone!, l])).values()];
const images: { icone: string; ok: boolean; ecart?: number; erreur?: string }[] = [];
let suivant = 0;
async function image(url: string): Promise<Buffer> {
	const response = await fetch(url, { signal: AbortSignal.timeout(30_000) });
	if (!response.ok) throw new Error(`HTTP ${response.status} : ${url}`);
	if (!response.headers.get("content-type")?.startsWith("image/")) throw new Error(`Réponse non image : ${url}`);
	const bytes = new Uint8Array(await response.arrayBuffer());
	if (!bytes.length || bytes.length > 8 * 1024 * 1024) throw new Error("Taille image hors limites");
	// Même fond et mêmes dimensions pour comparer PNG canonique et vignette WebP redimensionnée.
	return sharp(bytes).resize(64, 64, { fit: "fill" }).flatten({ background: "#9ca3af" }).removeAlpha().raw().toBuffer();
}
await Promise.all(Array.from({ length: 4 }, async () => {
	for (;;) {
		const i = suivant++; if (i >= uniques.length) return;
		const liaison = uniques[i];
		try {
			const [ui, reference] = await Promise.all([image(liaison.url!), image(`${cdn}/avatar/icon/${encodeURIComponent(liaison.icone!)}.png`)]);
			if (ui.length !== reference.length) throw new Error("Dimensions décodées incohérentes");
			let difference = 0;
			for (let p = 0; p < ui.length; p++) difference += Math.abs(ui[p] - reference[p]);
			const ecart = difference / ui.length;
			images.push({ icone: liaison.icone!, ok: ecart <= 8, ecart });
		} catch (e) { images.push({ icone: liaison.icone!, ok: false, erreur: String(e) }); }
		if ((i + 1) % 25 === 0) console.log(`${i + 1}/${uniques.length} icônes vérifiées`);
	}
}));
const rapport = { date: new Date().toISOString(), cdn, nombrePieces: liaisons.length, iconesUniques: uniques.length,
	erreursLiaisons: liaisons.filter(l => l.erreurs.length).length, erreursImages: images.filter(i => !i.ok).length,
	erreursSelections: selections.filter(s => s.erreurs.length).length, selections,
	portee: "CRC et routage de toutes les pièces du catalogue ; comparaison pixels UI/canonique (écart absolu moyen <= 8/255). Les avertissements ne sont pas masqués.",
	liaisons, images };
const destination = new URL("../../../../../var/json/avatar-assets-audit.json", import.meta.url);
await Bun.write(destination, JSON.stringify(rapport, null, 2));
console.log(JSON.stringify({ ...rapport, liaisons: undefined, images: undefined, selections: undefined, rapport: destination.pathname }, null, 2));
if (rapport.erreursLiaisons || rapport.erreursImages || rapport.erreursSelections) process.exitCode = 1;
