/**
 * Les URL du gisement **jeu**, avant et après leur passage par `@niers/catalog/jeu`.
 *
 * Ce fichier n'existe que pour une raison : `apps/azalee` sert un site public, et chacune de ces
 * chaînes est une clé de cache chez nginx comme chez les navigateurs. Faire passer les
 * constructeurs par la façade n'a d'intérêt que si **rien ne bouge** — et la seule preuve
 * recevable est la comparaison à la forme d'avant, recopiée ici **en dur**, jamais recalculée par
 * la façade elle-même (ce qui ne prouverait que sa cohérence avec soi).
 *
 * Chaque littéral ci-dessous a été relevé sur le code d'origine, à savoir :
 *
 * * `shared.ts` — `${CDN_BASE}/dx11/…png`, `/model-full/…glb`, `/raw/`, `/cfg/`, `/audio/`,
 *   `/video/`, et la variante `?w=&format=webp` des vignettes ;
 * * `live.ts` — `/vfs/ls`, `/vfs/find`, `/vfs/stat`, `/vfs/stats`, `/export/`, `/tex-info/` ;
 * * `video.ts` — `/video/catalog.json`, `/video/<x>`, `?track=audio`, `?info=1` ;
 * * `audio.ts` — `/audio/<x>[?id=]`, `/audio-info/<x>` ;
 * * `models.ts` — `/model-full/` et `/model-chr/<sous-domaine>/`.
 */
import { beforeEach, describe, expect, test } from "bun:test";

import { cpkAudioCueUrl, cpkAudioInfoUrl, formatDuration } from "@rosegriffon/azalee/cpk/audio";
import { exportUrl, texUrl, vfsFind, vfsLs, vfsStat, vfsStats } from "@rosegriffon/azalee/cpk/live";
import { modelGlbUrl } from "@rosegriffon/azalee/cpk/models";
import {
	cpkAssetUrl,
	cpkAudioUrl,
	cpkCfgUrl,
	cpkRawUrl,
	cpkThumbUrl,
	cpkVideoUrl,
} from "@rosegriffon/azalee/cpk/shared";
import {
	formatDefinition,
	formatDuree,
	formatOctets,
	formatSortie,
	ordreRubrique,
	videoAudioUrl,
	videoCatalogUrl,
	videoDownloadUrl,
	videoInfoUrl,
	videoUrl,
} from "../src/cpk/video";
import type { FilmDto } from "../src/cpk/video";

/** L'origine que le wiki servait en dur, dans les cinq modules, sous le nom `CDN_BASE`. */
const CDN = "https://cdn.rosegriffon.fr";

describe("shared.ts — le contenu décodé d'un fichier de l'index", () => {
	test("une texture passe par la `location` nginx `/dx11/`, pas par `/tex/`", () => {
		// AVANT : `${CDN_BASE}/dx11/${path.slice("data/dx11/".length).replace(/\.g4tx$/i, "")}.png`
		expect(cpkAssetUrl("data/dx11/menu/200_icon/02_icon_item/icon_item01.g4tx")).toBe(
			`${CDN}/dx11/menu/200_icon/02_icon_item/icon_item01.png`,
		);
		// AVANT, branche `stripDataPrefix` : un g4tx hors `data/dx11/` perd juste `data/`.
		expect(cpkAssetUrl("data/common/x.g4tx")).toBe(`${CDN}/dx11/common/x.png`);
	});

	test("un modèle passe par `/model-full/<basename>.glb`", () => {
		// AVANT : `${CDN_BASE}/model-full/${name.replace(/\.(g4md|g4mg)$/i, "")}.glb`
		expect(cpkAssetUrl("data/common/chr/_waza/n031708/n031708.g4md")).toBe(
			`${CDN}/model-full/n031708.glb`,
		);
		expect(cpkAssetUrl("data/common/chr/_item/b001.g4mg")).toBe(`${CDN}/model-full/b001.glb`);
	});

	test("tout le reste tombe sur `/raw/<chemin complet>`", () => {
		// AVANT : `${CDN_BASE}/raw/${path}` — le préfixe `data/` est CONSERVÉ.
		expect(cpkAssetUrl("data/common/gamedata/game_param.cfg.bin")).toBe(
			`${CDN}/raw/data/common/gamedata/game_param.cfg.bin`,
		);
		expect(cpkRawUrl("data/common/gamedata/game_param.cfg.bin")).toBe(
			`${CDN}/raw/data/common/gamedata/game_param.cfg.bin`,
		);
	});

	test("les trois décodages nommés gardent leur forme", () => {
		expect(cpkCfgUrl("data/common/gamedata/game_param.cfg.bin")).toBe(
			`${CDN}/cfg/data/common/gamedata/game_param.cfg.bin.json`,
		);
		expect(cpkAudioUrl("data/common/sound_asset/bgm.acb")).toBe(
			`${CDN}/audio/data/common/sound_asset/bgm.acb`,
		);
		expect(cpkVideoUrl("data/common/movie/ev01_00050.usm")).toBe(
			`${CDN}/video/data/common/movie/ev01_00050.usm`,
		);
	});

	test("la vignette est la texture plus `?w=&format=webp`, dans cet ordre", () => {
		// AVANT : `${cpkAssetUrl(path, ext)}?w=${width}&format=webp`.
		expect(cpkThumbUrl("data/dx11/menu/200_icon/icon_item01.g4tx")).toBe(
			`${CDN}/dx11/menu/200_icon/icon_item01.png?w=400&format=webp`,
		);
		expect(cpkThumbUrl("data/dx11/menu/200_icon/icon_item01.g4tx", "g4tx", 96)).toBe(
			`${CDN}/dx11/menu/200_icon/icon_item01.png?w=96&format=webp`,
		);
		// AVANT : `null` dès que l'extension n'est pas une image — rien d'autre ne se redimensionne.
		expect(cpkThumbUrl("data/common/gamedata/game_param.cfg.bin")).toBeNull();
		expect(cpkThumbUrl("data/common/chr/_item/b001.g4mg")).toBeNull();
	});
});

describe("live.ts — le pont VFS et l'export", () => {
	/** Les URL réellement demandées par `fetch`, dans l'ordre. */
	let demandes: string[] = [];

	beforeEach(() => {
		demandes = [];
		// Le pont n'expose pas ses URL : elles ne sont observables qu'au moment du `fetch`. On
		// l'intercepte plutôt que de réécrire la construction dans le test — c'est la vraie URL
		// qui part sur le réseau qu'on veut comparer, pas une copie de la formule.
		globalThis.fetch = ((entree: string | URL | Request) => {
			demandes.push(String(entree));
			return Promise.resolve(new Response("{}", { headers: { "content-type": "application/json" } }));
		}) as typeof fetch;
	});

	test("`/vfs/ls` porte path, limit puis offset", async () => {
		// AVANT : `${CDN_BASE}/vfs/ls?${new URLSearchParams({ path, limit, offset })}`
		await vfsLs("data/dx11/menu");
		expect(demandes[0]).toBe(`${CDN}/vfs/ls?path=data%2Fdx11%2Fmenu&limit=1000&offset=0`);
		await vfsLs("data/common/sound_asset/ja", 20000, 500);
		expect(demandes[1]).toBe(
			`${CDN}/vfs/ls?path=data%2Fcommon%2Fsound_asset%2Fja&limit=20000&offset=500`,
		);
	});

	test("`/vfs/find` porte q, limit, offset puis ext", async () => {
		// AVANT : `${CDN_BASE}/vfs/find?${new URLSearchParams({ q, limit, offset })}` + `ext` posé
		// après, donc toujours en dernier.
		await vfsFind("mark");
		expect(demandes[0]).toBe(`${CDN}/vfs/find?q=mark&limit=200&offset=0`);
		await vfsFind("icon_item", { ext: "g4tx", limit: 50, offset: 100 });
		expect(demandes[1]).toBe(`${CDN}/vfs/find?q=icon_item&limit=50&offset=100&ext=g4tx`);
		// Une extension vide n'était jamais écrite (`if (options.ext)`), et ne l'est toujours pas.
		await vfsFind("mark", { ext: "" });
		expect(demandes[2]).toBe(`${CDN}/vfs/find?q=mark&limit=200&offset=0`);
	});

	test("`/vfs/stat` et `/vfs/stats` ne bougent pas", async () => {
		await vfsStat("data/common/movie/ev01_00050.usm");
		expect(demandes[0]).toBe(
			`${CDN}/vfs/stat?path=data%2Fcommon%2Fmovie%2Fev01_00050.usm`,
		);
		await vfsStats();
		expect(demandes[1]).toBe(`${CDN}/vfs/stats`);
	});

	test("`/export` porte le format, et l'identifiant de cue quand il y en a un", () => {
		// AVANT : `${CDN_BASE}/export/${path}?${new URLSearchParams({ format })}` + `id` ensuite.
		expect(exportUrl("data/common/sound_asset/bgm.acb", "wav")).toBe(
			`${CDN}/export/data/common/sound_asset/bgm.acb?format=wav`,
		);
		expect(exportUrl("data/common/sound_asset/bgm.acb", "wav", { awbId: 12 })).toBe(
			`${CDN}/export/data/common/sound_asset/bgm.acb?format=wav&id=12`,
		);
		// `awbId: 0` est un identifiant valide — l'ancienne garde était `!= null`, pas `truthy`.
		expect(exportUrl("data/common/sound_asset/bgm.acb", "wav", { awbId: 0 })).toBe(
			`${CDN}/export/data/common/sound_asset/bgm.acb?format=wav&id=0`,
		);
		expect(exportUrl("data/common/sound_asset/bgm.acb", "wav", { awbId: null })).toBe(
			`${CDN}/export/data/common/sound_asset/bgm.acb?format=wav`,
		);
	});

	test("`texUrl` préfixe le chemin publié par `/tex-info`, sans le reconstruire", () => {
		// AVANT : `${CDN_BASE}${path}` puis `?w=&format=webp` si `width` est VRAI (pas « défini »).
		const servi = "/tex/dx11/menu/200_icon/icon_item01.g4tx/icon_a.png";
		expect(texUrl(servi)).toBe(`${CDN}${servi}`);
		expect(texUrl(servi, 160)).toBe(`${CDN}${servi}?w=160&format=webp`);
		expect(texUrl(servi, 0)).toBe(`${CDN}${servi}`);
	});
});

describe("video.ts — les cinématiques", () => {
	const CHEMIN = "data/common/movie/ev01_00050.usm";

	test("les quatre URL de la page Cinéma", () => {
		expect(videoCatalogUrl()).toBe(`${CDN}/video/catalog.json`);
		expect(videoUrl(CHEMIN)).toBe(`${CDN}/video/${CHEMIN}`);
		expect(videoAudioUrl(CHEMIN)).toBe(`${CDN}/video/${CHEMIN}?track=audio`);
		expect(videoInfoUrl(CHEMIN)).toBe(`${CDN}/video/${CHEMIN}?info=1`);
		expect(videoDownloadUrl(CHEMIN, "mp4")).toBe(`${CDN}/export/${CHEMIN}?format=mp4`);
	});

	test("les formateurs rendent exactement ce qu'ils rendaient", () => {
		expect(formatDuree(93.55)).toBe("1:34");
		expect(formatDuree(3725)).toBe("1:02:05");
		expect(formatDuree(0)).toBeNull();
		expect(formatDuree(null)).toBeNull();

		expect(formatOctets(312761536)).toBe("298 Mio");
		expect(formatOctets(4_000_000_000)).toBe("3,7 Gio");

		expect(ordreRubrique("Chapitre 03")).toBe(3);
		expect(ordreRubrique("Chronicle")).toBe(900);
		expect(ordreRubrique("Écrans-titres")).toBe(901);
		expect(ordreRubrique("Autre")).toBe(902);
	});

	test("la définition et le format de sortie se lisent toujours sur la fiche", () => {
		const film = { largeur: 1920, hauteur: 1080, codec: "h264" } as FilmDto;
		expect(formatDefinition(film)).toBe("1920×1080");
		expect(formatDefinition({ largeur: 0, hauteur: 0 } as FilmDto)).toBeNull();
		expect(formatSortie(film)).toEqual({ id: "mp4", ext: "mp4", libelle: "MP4" });
		expect(formatSortie({ codec: "vp9" } as FilmDto)).toEqual({
			id: "webm",
			ext: "webm",
			libelle: "WebM",
		});
		expect(formatSortie({ codec: "mpeg2" } as FilmDto)).toEqual({
			id: "m2v",
			ext: "m2v",
			libelle: "MPEG-2",
		});
	});
});

describe("audio.ts — les banques CRI", () => {
	const BANQUE = "data/common/sound_asset/bgm.acb";

	test("un cue s'adresse par son `awbId`, jamais par son rang", () => {
		expect(cpkAudioCueUrl(BANQUE)).toBe(`${CDN}/audio/${BANQUE}`);
		expect(cpkAudioCueUrl(BANQUE, 42)).toBe(`${CDN}/audio/${BANQUE}?id=42`);
		// `0` est un cue-id réel : la garde d'origine était `== null`, pas `falsy`.
		expect(cpkAudioCueUrl(BANQUE, 0)).toBe(`${CDN}/audio/${BANQUE}?id=0`);
		expect(cpkAudioCueUrl(BANQUE, null)).toBe(`${CDN}/audio/${BANQUE}`);
		expect(cpkAudioInfoUrl(BANQUE)).toBe(`${CDN}/audio-info/${BANQUE}`);
	});

	test("la durée d'un cue garde ses trois régimes", () => {
		expect(formatDuration(0.5)).toBe("0,50 s");
		expect(formatDuration(12.34)).toBe("12,3 s");
		expect(formatDuration(93.55)).toBe("1:34");
		expect(formatDuration(0)).toBe("—");
	});
});

describe("models.ts — les deux routes d'assemblage", () => {
	test("un personnage passe par `/model-full`, le reste par `/model-chr`", () => {
		expect(modelGlbUrl("chara", "c01000010")).toBe(`${CDN}/model-full/c01000010.glb`);
		// Le sous-domaine s'écrit SANS le tiret bas du dossier VFS.
		expect(modelGlbUrl("waza", "ev60_00340")).toBe(`${CDN}/model-chr/waza/ev60_00340.glb`);
		expect(modelGlbUrl("keshin", "k001")).toBe(`${CDN}/model-chr/keshin/k001.glb`);
	});
});
