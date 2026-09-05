import { describe, expect, test } from "bun:test";
import { auditerLiaisons, crc32Nom, iconePart } from "./liaisons";
import { composerUrlAvatar } from "./composition";
import type { Catalogue, Part } from "./types";

const part = (id: string, modeles: string[] = [], modeles2: string[] = []): Part => ({
	id, itemNo: 7, viewNo: 99, gender: 0, resource: id, resourceHash: "00000000", resource2: null,
	modeles, modeles2, icone: "icon_ava_eye_003", iconeHash: crc32Nom("icon_ava_eye_003"),
	atlasCentre: null, poseTransMode: null, poseTrans: null, poseScale: null, resource2Connu: false,
});
function catalogue(parts: Part[], type = 6): Catalogue {
	return { categories: [{ faceSettingType: type, prefixe: "test", parts, couleurs: [] }],
		modelesDeBase: { morphologies: ["male"], visages: [{ noseType: "0", resources: ["face"] }], accessoires: [] } } as unknown as Catalogue;
}
const etat = { choix: {}, valeurs: {}, champs: {}, genre: 0, morphologie: 0 };
describe("Liaisons catalogue → icône → composition", () => {
	test("CRC IEEE connu et URL indépendante des rangs", () => {
		expect(crc32Nom("123456789")).toBe("CBF43926");
		expect(iconePart("https://cdn.test/", part("choix"))).toBe("https://cdn.test/g4tx/dx11/menu/200_icon/21_icon_avatar/icon_ava_eye.g4tx/icon_ava_eye_003.png?w=200&format=webp");
	});
	test("absence et noms malformés ne prennent jamais l’icône voisine", () => {
		expect(iconePart("https://cdn.test", { icone: null })).toBeNull();
		expect(iconePart("https://cdn.test", { icone: "../icon_003" })).toBeNull();
	});
	test("audit détecte hash erroné, doublon et chemin invalide", () => {
		const p = { ...part("p", ["data/../secret"]), iconeHash: "00000000" };
		const rapport = auditerLiaisons(catalogue([p, p]), "https://cdn.test");
		expect(rapport[0].erreurs).toHaveLength(2);
		expect(rapport[1].erreurs).toHaveLength(3);
	});
	test("réordonner les vignettes ne change pas la texture d’un identifiant sélectionné", () => {
		const a = part("a", ["data/dx11/chara/_facetex/01_eye/eye_a.g4tx"]);
		const b = part("b", ["data/dx11/chara/_facetex/01_eye/eye_b.g4tx"]);
		const selection = { ...etat, choix: { 6: "b" } };
		const url = composerUrlAvatar(catalogue([a, b]), "https://cdn.test", selection)!;
		expect(url).toBe(composerUrlAvatar(catalogue([b, a]), "https://cdn.test", selection)!);
		expect(new URL(url).searchParams.get("face")).toBe("01_eye/eye_b");
	});
	test("les cheveux arrière seuls et avant/arrière sont tous conservés", () => {
		const a = part("a", [], ["data/common/chara/20_EDIT/_hairB/hair_b/hair_b.g4md"]);
		const b = part("b", ["data/common/chara/20_EDIT/_hairF/hair_f/hair_f.g4md"], a.modeles2);
		const urlA = composerUrlAvatar(catalogue([a, b], 4), "https://cdn.test", { ...etat, choix: { 4: "a" } })!;
		const urlB = composerUrlAvatar(catalogue([a, b], 4), "https://cdn.test", { ...etat, choix: { 4: "b" } })!;
		expect(urlA).toContain("_hairB/hair_b");
		expect(urlB).toContain("_hairB/hair_b");
		expect(urlB).toContain("_hairF/hair_f");
	});
});
