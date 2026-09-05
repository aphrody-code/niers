/** Import local borné. Aucune URL distante issue d'un fichier n'est chargée. */
export const MAX_OCTETS = 64 * 1024 * 1024;
export type Region = { nom: string; x: number; y: number; largeur: number; hauteur: number };
export type Planche = { nom: string; url: string; largeur: number; hauteur: number; regions: Region[] };
export type ImportLocal = ({ type: "2d"; planches: Planche[] } | { type: "3d"; url: string }) & {
	nom: string; remarque: string; liberer(): void;
};
const objet = (v: unknown): v is Record<string, unknown> => !!v && typeof v === "object" && !Array.isArray(v);
const entier = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v);

export function grille(largeur: number, hauteur: number, colonnes: number, lignes: number): Region[] {
	if (![largeur, hauteur, colonnes, lignes].every(n => entier(n) && n > 0) ||
		colonnes * lignes > 4096 || largeur % colonnes || hauteur % lignes)
		throw new Error("La grille doit diviser exactement l’image (4096 cases maximum).");
	const w = largeur / colonnes, h = hauteur / lignes;
	return Array.from({ length: colonnes * lignes }, (_, i) => ({ nom: `Image ${i + 1}`, x: i % colonnes * w,
		y: Math.floor(i / colonnes) * h, largeur: w, hauteur: h }));
}

/** Accepte le manifeste du parseur NIE ; les rectangles doivent rester dans l'image. */
export function lireRegions(v: unknown, largeur: number, hauteur: number): Region[] {
	if (!objet(v) || v.largeur !== largeur || v.hauteur !== hauteur || !Array.isArray(v.sprites) || v.sprites.length > 4096)
		throw new Error("Atlas JSON invalide : dimensions différentes de l’image ou trop de régions.");
	return v.sprites.map(s => {
		if (!objet(s) || typeof s.nom !== "string" || s.nom.length > 200 ||
			!entier(s.x) || !entier(s.y) || !entier(s.largeur) || !entier(s.hauteur) ||
			s.x < 0 || s.y < 0 || s.largeur <= 0 || s.hauteur <= 0 ||
			s.x + s.largeur > largeur || s.y + s.hauteur > hauteur) throw new Error("Rectangle d’atlas hors limites.");
		return { nom: s.nom, x: s.x, y: s.y, largeur: s.largeur, hauteur: s.hauteur };
	});
}

export async function decompresserGzip(blob: Blob): Promise<Uint8Array> {
	if (typeof DecompressionStream === "undefined") throw new Error("Gzip indisponible dans ce navigateur. Décompressez le fichier avant l’import.");
	const reader = blob.stream().pipeThrough(new DecompressionStream("gzip")).getReader();
	const blocs: Uint8Array[] = []; let total = 0;
	try {
		for (;;) {
			const { done, value } = await reader.read(); if (done) break;
			total += value.byteLength;
			if (total > MAX_OCTETS) throw new Error("Fichier décompressé trop volumineux (64 Mio maximum).");
			blocs.push(value);
		}
	} finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
	const bytes = new Uint8Array(total); let offset = 0;
	for (const bloc of blocs) { bytes.set(bloc, offset); offset += bloc.length; }
	return bytes;
}

/** Résout seulement les compagnons sélectionnés ; jamais de réseau ni de chemin absolu. */
export function resoudreUri(uri: string, fichiers: Map<string, string>): string {
	if (/^data:(application\/(octet-stream|gltf-buffer)|image\/(png|jpeg|webp|ktx2));base64,[a-z\d+/=\s]*$/i.test(uri)) return uri;
	let nom: string;
	try { nom = decodeURIComponent(uri); } catch { throw new Error("URI de ressource mal encodée."); }
	nom = nom.replace(/^\.\//, "");
	if (/[\\:?#\x00-\x1f]/.test(nom) || nom.startsWith("/") || nom.split("/").some(p => !p || p === ".."))
		throw new Error("Ressource externe ou chemin interdit dans le modèle.");
	// Le sélecteur multiple fournit des noms de fichiers, pas des chemins. Refuser les doublons en amont.
	const url = fichiers.get(nom) ?? fichiers.get(nom.split("/").pop()!);
	if (!url) throw new Error(`Ressource manquante : ${nom}. Sélectionnez aussi les textures et fichiers .bin.`);
	return url;
}

function relierGltf(v: unknown, ressources: Map<string, string>): void {
	if (!objet(v) || !objet(v.asset) || v.asset.version !== "2.0" || !Array.isArray(v.meshes) || !v.meshes.length)
		throw new Error("Modèle glTF 2.0 sans maillage ou document invalide.");
	function visiter(value: unknown, profondeur = 0) {
		if (profondeur > 64) throw new Error("Document glTF trop imbriqué.");
		if (Array.isArray(value)) { for (const el of value) visiter(el, profondeur + 1); }
		else if (objet(value)) for (const [cle, enfant] of Object.entries(value)) {
			if (cle === "uri") {
				if (typeof enfant !== "string") throw new Error("URI glTF invalide.");
				value[cle] = resoudreUri(enfant, ressources);
			} else visiter(enfant, profondeur + 1);
		}
	}
	visiter(v);
}

/** Préserve les chunks binaires ; seule la table JSON des ressources est réécrite. */
export function preparerGlb(bytes: Uint8Array, ressources: Map<string, string>): Blob {
	if (bytes.length < 20) throw new Error("GLB tronqué.");
	const vue = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const jsonSize = vue.getUint32(12, true);
	if (vue.getUint32(0, true) !== 0x46546c67 || vue.getUint32(4, true) !== 2 ||
		vue.getUint32(8, true) !== bytes.length || vue.getUint32(16, true) !== 0x4e4f534a ||
		jsonSize % 4 || jsonSize > 8 * 1024 * 1024 || 20 + jsonSize > bytes.length) throw new Error("En-tête GLB 2 invalide.");
	const document = JSON.parse(new TextDecoder().decode(bytes.subarray(20, 20 + jsonSize)));
	relierGltf(document, ressources);
	const json = new TextEncoder().encode(JSON.stringify(document));
	const taille = Math.ceil(json.length / 4) * 4;
	const entete = new Uint8Array(20 + taille); entete.fill(32, 20); entete.set(json, 20);
	const dst = new DataView(entete.buffer);
	dst.setUint32(0, 0x46546c67, true); dst.setUint32(4, 2, true);
	dst.setUint32(8, entete.length + bytes.length - 20 - jsonSize, true);
	dst.setUint32(12, taille, true); dst.setUint32(16, 0x4e4f534a, true);
	return new Blob([entete, bytes.slice(20 + jsonSize)], { type: "model/gltf-binary" });
}

export async function importerLocal(selection: File[]): Promise<ImportLocal> {
	if (!selection.length || selection.length > 200) throw new Error("Sélectionnez entre 1 et 200 fichiers.");
	if (selection.some(f => !f.size || f.size > MAX_OCTETS) || selection.reduce((n, f) => n + f.size, 0) > 128 * 1024 * 1024)
		throw new Error("Limite : 64 Mio par fichier, 128 Mio par import ; fichiers vides refusés.");
	const noms = new Set<string>();
	for (const f of selection) { if (noms.has(f.name)) throw new Error(`Nom de fichier ambigu : ${f.name}`); noms.add(f.name); }
	const urls: string[] = [];
	const creer = (blob: Blob) => { const url = URL.createObjectURL(blob); urls.push(url); return url; };
	const liberer = () => { for (const url of urls.splice(0)) URL.revokeObjectURL(url); };
	try {
		const fichiers: File[] = [];
		let tailleTotale = 0;
		for (const f of selection) {
			const fichier = /\.gz$/i.test(f.name) ? new File([(await decompresserGzip(f)).slice()], f.name.slice(0, -3)) : f;
			tailleTotale += fichier.size;
			if (tailleTotale > 128 * 1024 * 1024) throw new Error("Import décompressé supérieur à 128 Mio.");
			if (fichiers.some(v => v.name === fichier.name)) throw new Error("Doublon après décompression.");
			fichiers.push(fichier);
		}
		const modeles = fichiers.filter(f => /\.(glb|gltf|vrm|g4md)$/i.test(f.name));
		if (modeles.length > 1) throw new Error("Importez un seul modèle à la fois avec ses ressources.");
		if (modeles.length) {
			const principal = modeles[0]; let blob: Blob;
			if (/\.g4md$/i.test(principal.name)) {
				const mesh = fichiers.find(f => f.name === principal.name.replace(/g4md$/i, "g4mg"));
				if (!mesh) throw new Error("Sélectionnez la paire G4MD + G4MG de même nom.");
				const { modelToGlb } = await import("../../lib/cpk-wasm");
				blob = new Blob([(await modelToGlb(new Uint8Array(await principal.arrayBuffer()), new Uint8Array(await mesh.arrayBuffer()))).slice()], { type: "model/gltf-binary" });
			} else {
				const ressources = new Map(fichiers.filter(f => f !== principal).map(f => [f.name, creer(f)]));
				if (/\.gltf$/i.test(principal.name)) {
					if (principal.size > 8 * 1024 * 1024) throw new Error("JSON glTF supérieur à 8 Mio.");
					const doc: unknown = JSON.parse(await principal.text()); relierGltf(doc, ressources);
					blob = new Blob([JSON.stringify(doc)], { type: "model/gltf+json" });
				} else blob = preparerGlb(new Uint8Array(await principal.arrayBuffer()), ressources);
			}
			return { type: "3d", nom: principal.name, url: creer(blob), liberer,
				remarque: /\.g4md$/i.test(principal.name) ? "Conversion WASM : géométrie seule, sans textures ni squelette." :
				/\.vrm$/i.test(principal.name) ? "Aperçu glTF du VRM ; expressions et rig humanoïde VRM non édités." : "Modèle local indépendant du catalogue. Exportez le GLB pour conserver les transformations." };
		}
		const images = fichiers.filter(f => /\.(png|jpe?g|webp|g4tx)$/i.test(f.name));
		const atlas = fichiers.filter(f => /\.json$/i.test(f.name));
		if (!images.length || fichiers.length !== images.length + atlas.length || atlas.length > 1 || (atlas.length && images.length !== 1))
			throw new Error("Formats : PNG/JPEG/WebP, G4TX (+ atlas JSON), GLB/glTF/VRM, G4MD+G4MG ; gzip accepté. ZIP/FBX/OBJ : convertir en GLB d’abord.");
		const planches: Planche[] = []; let pixels = 0;
		for (const fichier of images.sort((a, b) => a.name.localeCompare(b.name, "fr", { numeric: true }))) {
			let blob: Blob = fichier; let manifeste: unknown;
			if (/\.g4tx$/i.test(fichier.name)) {
				const wasm = await import("../../lib/cpk-wasm"); const bytes = new Uint8Array(await fichier.arrayBuffer());
				blob = new Blob([(await wasm.g4txToPng(bytes)).slice()], { type: "image/png" });
				manifeste = await wasm.g4txSpriteSheet(bytes);
			}
			if (atlas[0]) { if (atlas[0].size > 1024 * 1024) throw new Error("Atlas JSON supérieur à 1 Mio."); manifeste = JSON.parse(await atlas[0].text()); }
			const bitmap = await createImageBitmap(blob);
			const largeur = bitmap.width, hauteur = bitmap.height; bitmap.close();
			pixels += largeur * hauteur;
			if (largeur > 16384 || hauteur > 16384 || pixels > 64 * 1024 * 1024) throw new Error("Images trop grandes (16 384 px par côté, 64 mégapixels cumulés).");
			const regions = manifeste ? lireRegions(manifeste, largeur, hauteur) : [];
			planches.push({ nom: fichier.name, url: creer(blob), largeur, hauteur, regions });
		}
		return { type: "2d", nom: images.length > 1 ? `${images.length} images` : images[0].name, planches, liberer,
			remarque: "Images locales : choisissez les régions de l’atlas ou une grille. Une planche n’est pas automatiquement un modèle 3D." };
	} catch (error) { liberer(); throw error; }
}
