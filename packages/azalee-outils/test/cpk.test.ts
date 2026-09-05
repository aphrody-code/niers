/**
 * Index CPK (250 800 fichiers) — listing, pagination, recherche, métadonnées.
 *
 * Source réelle : `apps/azalee/data/cpk-index.ndjson.gz`, matérialisé en SQLite
 * de cache au premier accès. Les tests sont skippés proprement si l'artefact est
 * absent (checkout frais sans données).
 */

import { describe, expect, test } from "bun:test";

import { fileMeta, listDir, listDirPaged, searchFiles, totalFiles } from "../src/cpk/index";
import { cpkAssetKind, cpkAssetUrl, cpkThumbUrl, normalizeDir } from "@rosegriffon/azalee/cpk/shared";
import { resolveDataFile } from "../src/config";

const hasIndex = resolveDataFile("cpk-index.ndjson.gz") !== null || Boolean(process.env.CPK_INDEX_PATH);
const suite = describe.skipIf(!hasIndex);

suite("totalFiles + racine de l'arbre", () => {
	test("l'index couvre les fichiers des CPK IEVR", () => {
		const total = totalFiles();
		// 250 800 entrées à ce jour (common 193 540 / dx11 57 260).
		expect(total).toBeGreaterThan(200_000);
	});

	test("la racine liste les tops et n'a aucun fichier direct", () => {
		const root = listDir("");
		expect(root.dir).toBe("data");
		expect(root.files).toHaveLength(0);
		expect(root.dirs.length).toBeGreaterThan(0);
		const noms = root.dirs.map((d) => d.name);
		expect(noms).toContain("common");
		expect(noms).toContain("dx11");
		// Les compteurs récursifs des tops totalisent l'index entier.
		expect(root.dirs.reduce((acc, d) => acc + d.count, 0)).toBe(totalFiles());
		// `data` et "" désignent la même racine logique.
		expect(listDir("data")).toEqual(root);
	});
});

suite("listDir — sous-dossiers et fichiers directs", () => {
	test("un répertoire intermédiaire n'expose que ses sous-dossiers", () => {
		const dx11 = listDir("data/dx11");
		expect(dx11.dir).toBe("data/dx11");
		expect(dx11.dirs.length).toBeGreaterThan(0);
		// Trié par nom.
		const noms = dx11.dirs.map((d) => d.name);
		expect([...noms].sort()).toEqual(noms);
		expect(noms).toContain("menu");
		// Aucun segment vide ne doit fuiter du découpage SQL.
		expect(noms.every((n) => n.length > 0)).toBe(true);
		// La somme des sous-arbres ne dépasse pas le compte du parent.
		const parent = listDir("").dirs.find((d) => d.name === "dx11");
		expect(dx11.dirs.reduce((acc, d) => acc + d.count, 0)).toBeLessThanOrEqual(parent?.count ?? 0);
	});

	test("les slashs de bord sont normalisés (même listing)", () => {
		expect(listDir("/data/dx11/")).toEqual(listDir("data/dx11"));
	});

	test("un répertoire inexistant renvoie un listing vide (pas d'exception)", () => {
		const vide = listDir("data/nexiste-pas-du-tout");
		expect(vide.dirs).toHaveLength(0);
		expect(vide.files).toHaveLength(0);
	});
});

suite("listDirPaged — pagination des répertoires massifs", () => {
	/** Trouve un répertoire réel contenant au moins `min` fichiers directs. */
	function repertoireDense(min: number): string | null {
		const pile = ["data/common", "data/dx11"];
		let garde = 0;
		while (pile.length > 0 && garde++ < 400) {
			const dir = pile.pop() as string;
			const listing = listDirPaged(dir, 1, 0);
			if (listing.fileTotal >= min) return dir;
			for (const sub of listing.dirs) pile.push(`${dir}/${sub.name}`);
		}
		return null;
	}

	const dense = repertoireDense(50);

	test("fileTotal est indépendant de la page, les pages sont disjointes", () => {
		expect(dense).toBeString();
		const p1 = listDirPaged(dense as string, 10, 0);
		const p2 = listDirPaged(dense as string, 10, 10);
		expect(p1.fileOffset).toBe(0);
		expect(p2.fileOffset).toBe(10);
		expect(p1.fileTotal).toBe(p2.fileTotal);
		expect(p1.fileTotal).toBeGreaterThanOrEqual(20);
		expect(p1.files).toHaveLength(10);
		expect(p2.files).toHaveLength(10);
		const noms = new Set(p1.files.map((f) => f.path));
		expect(p2.files.some((f) => noms.has(f.path))).toBe(false);
		// Les sous-dossiers ne sont JAMAIS tronqués par la pagination des fichiers.
		expect(p1.dirs).toEqual(p2.dirs);
	});

	test("la page complète correspond au listing non paginé", () => {
		const complet = listDir(dense as string);
		const page = listDirPaged(dense as string, complet.files.length, 0);
		expect(page.files).toEqual(complet.files);
		expect(page.fileTotal).toBe(complet.files.length);
	});

	test("la racine paginée n'a pas de fichiers directs", () => {
		const root = listDirPaged("", 50, 0);
		expect(root.dir).toBe("data");
		expect(root.fileTotal).toBe(0);
		expect(root.files).toHaveLength(0);
		expect(root.dirs.length).toBeGreaterThan(0);
	});

	test("un offset au-delà du total renvoie une page vide", () => {
		const listing = listDirPaged(dense as string, 10, 1_000_000);
		expect(listing.files).toHaveLength(0);
		expect(listing.fileTotal).toBeGreaterThan(0);
	});
});

suite("searchFiles — recherche par nom", () => {
	test("chaque résultat contient la sous-chaîne cherchée", () => {
		const found = searchFiles("c01000010", 20);
		expect(found.length).toBeGreaterThan(0);
		expect(found.length).toBeLessThanOrEqual(20);
		for (const file of found) {
			expect(file.name.toLowerCase()).toContain("c01000010");
			expect(file.path.endsWith(file.name)).toBe(true);
			expect(file.cpk).toMatch(/\.cpk$/);
		}
		// Tri par longueur de nom croissante (les matches les plus « exacts » d'abord).
		const longueurs = found.map((f) => f.name.length);
		expect([...longueurs].sort((a, b) => a - b)).toEqual(longueurs);
	});

	test("la recherche est insensible à la casse", () => {
		expect(searchFiles("C01000010", 5).map((f) => f.path)).toEqual(
			searchFiles("c01000010", 5).map((f) => f.path),
		);
	});

	test("une requête trop courte ne scanne rien", () => {
		expect(searchFiles("", 10)).toHaveLength(0);
		expect(searchFiles("c", 10)).toHaveLength(0);
	});

	test("les jokers SQL sont échappés (pas d'injection LIKE)", () => {
		// `%` non échappé ferait matcher toute la table.
		expect(searchFiles("%%", 10)).toHaveLength(0);
		expect(searchFiles("__", 10)).toHaveLength(0);
	});

	test("la limite est respectée", () => {
		expect(searchFiles("c0", 7)).toHaveLength(7);
	});
});

suite("fileMeta — métadonnées + URL CDN", () => {
	test("un fichier réel est décomposé en colonnes cohérentes", () => {
		const [file] = searchFiles("c01000010", 1);
		expect(file).toBeDefined();
		const meta = fileMeta(file!.path);
		expect(meta).not.toBeNull();
		expect(meta?.path).toBe(file!.path);
		expect(meta?.name).toBe(file!.name);
		expect(meta?.ext).toBe(file!.ext);
		expect(meta?.cpk).toBe(file!.cpk);
		expect(meta?.dir).toBe(file!.path.slice(0, file!.path.length - file!.name.length - 1));
		expect(meta?.depth).toBe(file!.path.split("/").length);
		// `top`/`sub` ignorent le préfixe racine `data/`.
		expect(["common", "dx11", "movie", "font"]).toContain(meta!.top);
		expect(meta?.kind).toBe(cpkAssetKind(file!.ext));
		expect(meta?.assetUrl).toBe(cpkAssetUrl(file!.path, file!.ext));
		expect(meta?.thumbUrl).toBe(cpkThumbUrl(file!.path, file!.ext));
	});

	test("une texture g4tx est mappée sur le décodage PNG live du CDN", () => {
		const [texture] = searchFiles(".g4tx", 1).length > 0 ? searchFiles(".g4tx", 1) : [];
		const cible = texture ?? searchFiles("_l.g4tx", 1)[0];
		if (!cible) return; // pas de g4tx dans cet index — rien à vérifier
		const meta = fileMeta(cible.path);
		expect(meta?.kind).toBe("image");
		expect(meta?.assetUrl).toStartWith("https://cdn.rosegriffon.fr/dx11/");
		expect(meta?.assetUrl).toEndWith(".png");
		expect(meta?.thumbUrl).toContain("format=webp");
	});

	test("un chemin absent de l'index → null (c'est peut-être un dossier)", () => {
		expect(fileMeta("data/dx11")).toBeNull();
		expect(fileMeta("data/nexiste-pas/du-tout.bin")).toBeNull();
		expect(fileMeta("")).toBeNull();
	});
});

describe("cpk/shared — helpers purs (client-safe)", () => {
	test("normalizeDir retire les slashs de bord", () => {
		expect(normalizeDir("/data/dx11/")).toBe("data/dx11");
		expect(normalizeDir("data/dx11")).toBe("data/dx11");
		expect(normalizeDir("///")).toBe("");
		expect(normalizeDir("")).toBe("");
	});

	test("cpkAssetKind classe par extension", () => {
		expect(cpkAssetKind("g4tx")).toBe("image");
		expect(cpkAssetKind("G4TX")).toBe("image");
		expect(cpkAssetKind("g4md")).toBe("model");
		expect(cpkAssetKind("g4mg")).toBe("model");
		expect(cpkAssetKind("bin")).toBe("raw");
		expect(cpkAssetKind("")).toBe("raw");
	});

	test("cpkAssetUrl route image / modèle / brut", () => {
		// Chemin réel de l'index CPK : le namespace est `200_icon/10_icon_chr`
		// (18 504 fichiers), jamais `icon_chr` — ce dernier était le nommage du
		// dump disque archivé, absent des CPK.
		expect(cpkAssetUrl("data/dx11/menu/200_icon/10_icon_chr/face/c01000010_l.g4tx")).toBe(
			"https://cdn.rosegriffon.fr/dx11/menu/200_icon/10_icon_chr/face/c01000010_l.png",
		);
		expect(cpkAssetUrl("data/common/chr/c01000010/c01000010_face.g4mg")).toBe(
			"https://cdn.rosegriffon.fr/model-full/c01000010_face.glb",
		);
		expect(cpkAssetUrl("data/common/text/fr/chara.bin")).toBe(
			"https://cdn.rosegriffon.fr/raw/data/common/text/fr/chara.bin",
		);
	});

	test("cpkThumbUrl n'existe que pour les images", () => {
		expect(cpkThumbUrl("data/dx11/menu/x.g4tx", "g4tx")).toBe(
			"https://cdn.rosegriffon.fr/dx11/menu/x.png?w=400&format=webp",
		);
		expect(cpkThumbUrl("data/dx11/menu/x.g4tx", "g4tx", 1600)).toEndWith("?w=1600&format=webp");
		expect(cpkThumbUrl("data/common/x.bin", "bin")).toBeNull();
		expect(cpkThumbUrl("data/common/x.g4md", "g4md")).toBeNull();
	});
});
