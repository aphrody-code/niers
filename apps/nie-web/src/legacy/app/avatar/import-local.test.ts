import { expect, test } from "bun:test";
import { decompresserGzip, grille, importerLocal, lireRegions, preparerGlb, resoudreUri } from "./import-local";

function glb(document: unknown): Uint8Array {
	const json = new TextEncoder().encode(JSON.stringify(document));
	const bytes = new Uint8Array(20 + Math.ceil(json.length / 4) * 4); bytes.fill(32, 20); bytes.set(json, 20);
	const view = new DataView(bytes.buffer);
	view.setUint32(0, 0x46546c67, true); view.setUint32(4, 2, true); view.setUint32(8, bytes.length, true);
	view.setUint32(12, bytes.length - 20, true); view.setUint32(16, 0x4e4f534a, true);
	return bytes;
}
const document = { asset: { version: "2.0" }, meshes: [{ primitives: [] }] };

test("grille chara exacte, ordre ligne puis colonne", () => {
	const r = grille(96, 128, 3, 4);
	expect(r).toHaveLength(12);
	expect(r[4]).toEqual({ nom: "Image 5", x: 32, y: 32, largeur: 32, hauteur: 32 });
	for (const args of [[95, 128, 3, 4], [96, 128, 0, 4], [96, 128, NaN, 4], [1e6, 1e6, 100, 100]])
		expect(() => grille(...args as [number, number, number, number])).toThrow();
});
test("atlas NIE : rectangles source conservés et limites vérifiées", () => {
	const sprites = [{ nom: "face", x: 3, y: 4, largeur: 20, hauteur: 30 }];
	expect(lireRegions({ largeur: 100, hauteur: 100, sprites }, 100, 100)).toEqual(sprites);
	expect(() => lireRegions({ largeur: 100, hauteur: 100, sprites: [{ ...sprites[0], x: 99 }] }, 100, 100)).toThrow();
	expect(() => lireRegions({ largeur: 99, hauteur: 100, sprites }, 100, 100)).toThrow();
});
test("URI : sélection locale et données embarquées, jamais réseau ou traversée", () => {
	const local = new Map([["body.png", "blob:local-image"]]);
	expect(resoudreUri("textures/body.png", local)).toBe("blob:local-image");
	expect(resoudreUri("data:image/png;base64,AAAA", local)).toContain("data:");
	for (const uri of ["https://example.com/private", "//evil/a", "../body.png", "%2e%2e/body.png", "C:\\file", "data:image/svg+xml;base64,AAAA", "unknown.bin", "%FE"])
		expect(() => resoudreUri(uri, local)).toThrow();
});
test("GLB : réécrit JSON sans changer les données BIN", async () => {
	const original = glb({ ...document, images: [{ uri: "tex.png" }] });
	const bytes = new Uint8Array(original.length + 12); bytes.set(original);
	const v = new DataView(bytes.buffer); v.setUint32(8, bytes.length, true);
	v.setUint32(original.length, 4, true); v.setUint32(original.length + 4, 0x004e4942, true); bytes.set([1, 2, 3, 4], original.length + 8);
	const result = new Uint8Array(await preparerGlb(bytes, new Map([["tex.png", "blob:tex"]])).arrayBuffer());
	expect(Array.from(result.slice(-4))).toEqual([1, 2, 3, 4]);
	expect(new DataView(result.buffer).getUint32(8, true)).toBe(result.length);
	const json = JSON.parse(new TextDecoder().decode(result.subarray(20, 20 + new DataView(result.buffer).getUint32(12, true))));
	expect(json.images[0].uri).toBe("blob:tex");
});
test("GLB invalide ou URL distante refusés avant chargement", () => {
	expect(() => preparerGlb(new Uint8Array(20), new Map())).toThrow();
	expect(() => preparerGlb(glb({ ...document, images: [{ uri: "https://example.com/tex.png" }] }), new Map())).toThrow();
	const truncated = glb(document).slice(0, -1);
	expect(() => preparerGlb(truncated, new Map())).toThrow();
});
test("gzip réel et import local GLB compressé", async () => {
	const bytes = glb(document);
	const compressed = Bun.gzipSync(bytes.slice());
	expect(await decompresserGzip(new Blob([compressed]))).toEqual(bytes);
	const result = await importerLocal([new File([compressed], "test.glb.gz")]);
	expect(result.type).toBe("3d"); expect(result.nom).toBe("test.glb");
	result.liberer(); result.liberer();
});
test("limites, doublons et formats non pris en charge", async () => {
	await expect(importerLocal([])).rejects.toThrow();
	await expect(importerLocal(Array.from({ length: 201 }, (_, i) => new File(["a"], `${i}.png`)))).rejects.toThrow();
	await expect(importerLocal([new File(["a"], "a.png"), new File(["b"], "a.png")])).rejects.toThrow();
	await expect(importerLocal([new File(["a"], "model.fbx")])).rejects.toThrow("Formats");
	await expect(importerLocal([new File(["a"], "model.g4md")])).rejects.toThrow("paire");
});
