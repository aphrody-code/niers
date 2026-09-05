/**
 * Décodage LAZY des assets d'un cut-in, **dans le navigateur** (wasm `nie-wasm`).
 *
 *  - `decodeEffectLayer` : un g4pk d'effet → GLB blob SI il contient une paire
 *    g4md+g4mg, sinon `null` (couche non géométrique = cas nominal, ex. ev60_00340
 *    n'a que des G4MT/G4MA/G4VS) ;
 *  - `mainModelLayer` : enveloppe le mesh waza (GLB déjà assemblé serveur) en
 *    `CutinGlbLayer` pour le traiter comme les autres couches ;
 *  - `loadCutinStoryboard` : récupère le timing caméra du `.g4cm` (best-effort,
 *    fallback générique).
 *
 * Le décodage est appelé à la demande (au 1er toggle d'une couche), pas en masse :
 * on ne décode jamais les 48 g4pk d'un coup. Client-safe (que du `fetch` + wasm).
 */
import { g4pkParse, modelToGlb } from "@/lib/cpk-wasm";
import { buildCutinShots, defaultCutinShots, type CutinStoryboard } from "./camera";
import { parseG4cm } from "./g4cm";
import type { CutinGlbLayer, CutinLayerDescriptor, CutinManifest } from "./types";

/** Lit le magic 4-ASCII d'un sous-fichier à `offset` (en-tête de ressource Level-5). */
function magicAt(bytes: Uint8Array, offset: number): string {
	if (offset + 4 > bytes.byteLength) return "";
	return String.fromCharCode(bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]);
}

/** `true` si le sous-fichier (nom ou magic) est de la famille `g4md`/`g4mg`. */
function isFamily(
	bytes: Uint8Array,
	file: { name: string; offset: number },
	ext: "g4md" | "g4mg",
	magic: "G4MD" | "G4MG",
): boolean {
	if (file.name.toLowerCase().endsWith(`.${ext}`)) return true;
	return magicAt(bytes, file.offset) === magic;
}

/**
 * Décode UNE couche g4pk → GLB blob, LAZY. Renvoie `null` (sans throw) si la couche
 * n'est pas géométrique (aucune paire g4md+g4mg) ; throw uniquement sur erreur réseau/wasm.
 */
export async function decodeEffectLayer(
	layer: CutinLayerDescriptor,
	signal?: AbortSignal,
): Promise<CutinGlbLayer | null> {
	const res = await fetch(layer.rawUrl, { signal });
	if (!res.ok) {
		throw new Error(`g4pk indisponible (${res.status}) : ${layer.vfsPath}`);
	}
	const bytes = new Uint8Array(await res.arrayBuffer());
	const parsed = await g4pkParse(bytes);

	const g4md = parsed.files.find((f) => isFamily(bytes, f, "g4md", "G4MD"));
	const g4mg = parsed.files.find((f) => isFamily(bytes, f, "g4mg", "G4MG"));
	if (!g4md || !g4mg) {
		// Effet pur non-géométrique (G4MT/G4MA/G4VS…) : pas de mesh à monter.
		return null;
	}

	const mdBytes = bytes.subarray(g4md.offset, g4md.offset + g4md.size);
	const mgBytes = bytes.subarray(g4mg.offset, g4mg.offset + g4mg.size);
	const glb = await modelToGlb(mdBytes, mgBytes);
	// `.slice()` → copie `Uint8Array<ArrayBuffer>` (BlobPart valide ; évite l'erreur
	// ArrayBufferLike/SharedArrayBuffer du retour wasm). Même pattern que app/cpk/*.
	const blob = new Blob([glb.slice()], { type: "model/gltf-binary" });
	return {
		id: layer.id,
		blobUrl: URL.createObjectURL(blob),
		fromEffect: true,
		revocable: true,
	};
}

/**
 * Enveloppe le mesh waza principal en `CutinGlbLayer`. N'assemble PAS in-browser :
 * réutilise `manifest.modelGlbUrl` (servi prêt par nie-model-serve) → la révocation
 * est un no-op (`revocable:false`).
 */
export function mainModelLayer(manifest: CutinManifest): CutinGlbLayer {
	return {
		id: "main",
		blobUrl: manifest.modelGlbUrl,
		fromEffect: false,
		revocable: false,
	};
}

/**
 * Storyboard caméra : `cameraG4cmUrl` → `parseG4cm` → `buildCutinShots` ; fallback
 * `defaultCutinShots()` si absent ou parse échoué (les valeurs g4cm sont opaques —
 * cf. `lib/cutin/g4cm.ts`).
 */
export async function loadCutinStoryboard(
	manifest: CutinManifest,
	signal?: AbortSignal,
): Promise<CutinStoryboard> {
	if (!manifest.cameraG4cmUrl) return defaultCutinShots();
	try {
		const res = await fetch(manifest.cameraG4cmUrl, { signal });
		if (!res.ok) return defaultCutinShots();
		const bytes = new Uint8Array(await res.arrayBuffer());
		return buildCutinShots(parseG4cm(bytes));
	} catch (err) {
		// Annulation propagée ; tout autre échec de parse → storyboard générique.
		if (err instanceof DOMException && err.name === "AbortError") throw err;
		return defaultCutinShots();
	}
}

/**
 * Décode les couches d'effet renderables avec une concurrence bornée (au plus
 * `limit` décodages en vol). Utilitaire optionnel pour précharger un petit lot ;
 * la scène peut aussi décoder à la demande couche par couche.
 */
export async function decodeLayersBounded(
	layers: CutinLayerDescriptor[],
	limit = 4,
	signal?: AbortSignal,
): Promise<CutinGlbLayer[]> {
	const out: CutinGlbLayer[] = [];
	let cursor = 0;
	async function worker(): Promise<void> {
		while (cursor < layers.length) {
			const layer = layers[cursor++];
			const decoded = await decodeEffectLayer(layer, signal);
			if (decoded) out.push(decoded);
		}
	}
	await Promise.all(Array.from({ length: Math.min(limit, layers.length) }, worker));
	return out;
}
