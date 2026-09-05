import { chromium } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

const PAGES = {
	accueil: "http://127.0.0.1:3003/",
	cross: "http://127.0.0.1:3003/cross",
	chara: "http://127.0.0.1:3003/chara",
	"chara-mark": "http://127.0.0.1:3003/chara/mark-evans-0x3055CF22",
	skill: "http://127.0.0.1:3003/skill",
	"skill-mur": "http://127.0.0.1:3003/skill/whd00010",
	aura: "http://127.0.0.1:3003/aura",
	"aura-keshin": "http://127.0.0.1:3003/aura/esprits-guerriers/keshin_0x8CEAA470",
	item: "http://127.0.0.1:3003/item",
	"item-guts": "http://127.0.0.1:3003/item/0x5F0F1EAC",
	passive: "http://127.0.0.1:3003/passive",
	tactic: "http://127.0.0.1:3003/tactic",
	"tactic-shinano": "http://127.0.0.1:3003/tactic/0x790F3F39",
	boutique: "http://127.0.0.1:3003/boutique",
	"boutique-esprits": "http://127.0.0.1:3003/boutique/1526934762",
	drops: "http://127.0.0.1:3003/drops",
	gallery: "http://127.0.0.1:3003/gallery",
	save: "http://127.0.0.1:3003/save",
	cpk: "http://127.0.0.1:3003/cpk",
	quete: "http://127.0.0.1:3003/quete",
	"quete-classroom": "http://127.0.0.1:3003/quete/0x8933A62B",
	succes: "http://127.0.0.1:3003/succes",
	"succes-sable": "http://127.0.0.1:3003/succes/activity_story_miniquest_006",
	stade: "http://127.0.0.1:3003/stade",
	"stade-index60": "http://127.0.0.1:3003/stade/0x2FF907D4",
	entraineur: "http://127.0.0.1:3003/entraineur",
	"entraineur-silvia": "http://127.0.0.1:3003/entraineur/1",
	compare: "http://127.0.0.1:3003/tools/compare",
	"random-team": "http://127.0.0.1:3003/tools/random-team",
	translator: "http://127.0.0.1:3003/tools/translator",
	"my-team": "http://127.0.0.1:3003/tools/my-team",
	news: "http://127.0.0.1:3003/news",
	dashboard: "http://127.0.0.1:3003/dashboard",
	capsule: "http://127.0.0.1:3003/capsule",
	"capsule-detail": "http://127.0.0.1:3003/capsule/0x6AE74506",
	equipe: "http://127.0.0.1:3003/equipe",
	"equipe-detail": "http://127.0.0.1:3003/equipe/0xF01BB293",
	invocation: "http://127.0.0.1:3003/invocation",
	"keshin-models": "http://127.0.0.1:3003/keshin",
	modeles: "http://127.0.0.1:3003/modeles",
	"patch-notes": "http://127.0.0.1:3003/patch-notes",
	"patch-notes-detail": "http://127.0.0.1:3003/patch-notes/ps-steam_ver_1_4_2"
};

// Paramétrables : les deux chemins d'origine désignaient le dossier de travail d'un agent et
// le dépôt d'une machine précise — introuvables partout ailleurs.
const ARTIFACT_DIR = process.env.AZALEE_ARTIFACT_DIR ?? resolve(import.meta.dirname, "../.artifacts/screenshots");
const DOCS_DIR =
	process.env.AZALEE_SCREENSHOTS_DIR ?? resolve(import.meta.dirname, "../../../docs/tools/screenshots");

mkdirSync(ARTIFACT_DIR, { recursive: true });
mkdirSync(DOCS_DIR, { recursive: true });

async function main() {
	console.log("Launching Chromium...");
	const browser = await chromium.launch({
		headless: true,
		executablePath: "/usr/local/chrome/chrome",
		args: ["--no-sandbox", "--disable-setuid-sandbox"]
	});
	const context = await browser.newContext({
		viewport: { width: 1280, height: 800 },
		deviceScaleFactor: 1.5,
		locale: "fr-FR",
		timezoneId: "Europe/Paris"
	});

	const page = await context.newPage();

	for (const [name, url] of Object.entries(PAGES)) {
		const filename = `${name}.png`;
		const artifactPath = join(ARTIFACT_DIR, filename);
		const docsPath = join(DOCS_DIR, filename);

		console.log(`Navigating to ${url}...`);
		try {
			await page.goto(url, { waitUntil: "load", timeout: 45000 });
			
			// Si c'est une page de modèles 3D WebGL, on attend 8s de rendu, sinon 3s.
			const is3D = name === "keshin-models" || name === "modeles" || name.includes("keshin") || name.includes("model");
			await page.waitForTimeout(is3D ? 8000 : 3000);

			console.log(`Taking screenshot: ${filename}`);
			await page.screenshot({ path: artifactPath, fullPage: false });
			await page.screenshot({ path: docsPath, fullPage: false });
		} catch (error) {
			console.error(`Failed to capture ${name}:`, error);
		}
	}

	await browser.close();
	console.log("Done!");
}

main().catch(console.error);

