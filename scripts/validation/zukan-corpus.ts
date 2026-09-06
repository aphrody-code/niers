#!/usr/bin/env bun
/**
 * Constitue le CORPUS DE RÉFÉRENCE zukan : pour N personnages, les 8 vues officielles du
 * visualiseur 3D de `zukan.inazuma.jp`.
 *
 * C'est le chaînon manquant de la gate de qualité. « Aussi bien assemblé que Byron Love et Shawn
 * Froste » ne veut rien dire tant qu'on n'a pas, pour chaque personnage, une image *de référence*
 * à laquelle comparer notre GLB. Avant ce script, un seul personnage en avait
 * (`var/outputs/zukan-reference/c05024700`, posé à la main par `download-zukan-reference.ts` qui
 * codait l'URL en dur).
 *
 * Ce qui rend la généralisation possible : le paramètre `q` de la page `chara_model_view` n'est
 * pas un identifiant opaque, c'est `{"character_id":["<code>"]}` dont chaque octet est XORé avec
 * 0xFF puis encodé en base64url. Vérifié sur la page déjà capturée :
 *   base64url("hN2cl56NnpyLmo2glpvdxaTdnM_Kz83LyM_P3aKC") ⊕ 0xFF = {"character_id":["c05024700"]}
 * On peut donc fabriquer l'URL de n'importe quel personnage au lieu de la copier.
 *
 * ATTENTION — `inagle_characters.zukan_hash` N'EST PAS ce chemin-là : il pointe la carte
 * portrait (`<hash>.png`, un seul fichier), pas la planche 3D 8 vues. Mesuré : le hash de Shawn
 * en base est `k/n/p/npvivtwkt3m` alors que ses 8 vues vivent sous `k/q/a/qa8avoz8nvk`. Utiliser
 * `zukan_hash` pour la gate 3D donnerait des 403 en série.
 *
 * Le script est idempotent : un personnage dont le `manifest.json` est déjà complet est sauté.
 *
 * Usage :
 *   bun --bun scripts/validation/zukan-corpus.ts --codes c01000010,c02021100
 *   bun --bun scripts/validation/zukan-corpus.ts --depuis-cache 40   # les codes déjà assemblés
 *   bun --bun scripts/validation/zukan-corpus.ts --depuis-audit var/outputs/audit-.../resultats.ndjson --nb 50
 */

import { existsSync, mkdirSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const RACINE = resolve(import.meta.dir, "..", "..");
const SORTIE = join(RACINE, "var", "outputs", "zukan-reference");
const LANGUE = "en";

/** `{"character_id":["<code>"]}` ⊕ 0xFF, en base64url — le paramètre `q` du visualiseur. */
export function encoderQ(code: string): string {
	const clair = Buffer.from(JSON.stringify({ character_id: [code] }), "utf8");
	const brouille = Buffer.from(clair.map((b) => b ^ 0xff));
	return brouille.toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** Décodage inverse — sert d'auto-test : le script vérifie l'aller-retour avant tout réseau. */
export function decoderQ(q: string): string {
	const brut = Buffer.from(q.replace(/-/g, "+").replace(/_/g, "/"), "base64");
	return Buffer.from(brut.map((b) => b ^ 0xff)).toString("utf8");
}

interface Vue {
	index: number;
	vue: "portrait" | "fullbody";
	fichier: string;
	url: string;
	largeur: number;
	hauteur: number;
	octets: number;
}

interface Issue {
	code: string;
	etat: "recupere" | "deja" | "sans-modele-3d" | "erreur";
	vues?: number;
	base?: string;
	detail?: string;
}

async function recuperer(code: string, forcer: boolean): Promise<Issue> {
	const racine = join(SORTIE, code);
	const manifeste = join(racine, "manifest.json");
	if (!forcer && existsSync(manifeste)) {
		try {
			const m = JSON.parse(await Bun.file(manifeste).text()) as { frames?: unknown[] };
			if (Array.isArray(m.frames) && m.frames.length > 0) {
				return { code, etat: "deja", vues: m.frames.length };
			}
		} catch {
			/* manifeste corrompu : on retélécharge */
		}
	}

	const q = encoderQ(code);
	const page = `https://zukan.inazuma.jp/${LANGUE}/chara_model_view/?q=${q}`;
	const rep = await fetch(page, { signal: AbortSignal.timeout(30_000) });
	if (!rep.ok) return { code, etat: "erreur", detail: `page HTTP ${rep.status}` };
	const html = await rep.text();

	const modele = html.match(/const modelId = '([^']+)'/)?.[1];
	const nb = Number(html.match(/const imageCount = (\d+)/)?.[1]);
	const base = html.match(/return `(https:\/\/[^`]+)` \+ `_r/)?.[1];
	if (!modele || !base || !nb) {
		// Page servie mais sans visualiseur : ce personnage n'a pas de planche 3D au zukan.
		return { code, etat: "sans-modele-3d", detail: "visualiseur absent de la page" };
	}
	if (nb > 64) return { code, etat: "erreur", detail: `imageCount aberrant : ${nb}` };

	mkdirSync(racine, { recursive: true });
	await Bun.write(join(racine, "source.html"), html);

	const vues: Vue[] = [];
	for (const suffixe of ["", "_fullbody"] as const) {
		for (let i = 0; i < nb; i++) {
			const url = `${base}_r${i}${suffixe}.png`;
			const img = await fetch(url, { signal: AbortSignal.timeout(30_000) });
			if (!img.ok) return { code, etat: "erreur", detail: `${url} → HTTP ${img.status}` };
			const octets = Buffer.from(await img.arrayBuffer());
			// Un CDN qui rend une page d'erreur en 200 se trahit ici, pas trois étapes plus loin.
			if (octets.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") {
				return { code, etat: "erreur", detail: `PNG invalide : ${url}` };
			}
			const fichier = `${code}_r${i}${suffixe}.png`;
			await Bun.write(join(racine, fichier), octets);
			vues.push({
				index: i,
				vue: suffixe ? "fullbody" : "portrait",
				fichier,
				url,
				largeur: octets.readUInt32BE(16),
				hauteur: octets.readUInt32BE(20),
				octets: octets.length,
			});
			await Bun.sleep(120); // on ne martèle pas un service tiers
		}
	}

	await Bun.write(
		manifeste,
		`${JSON.stringify(
			{
				source: page,
				code,
				modelId: modele,
				base,
				recupereLe: new Date().toISOString(),
				frames: vues.map((v) => ({
					index: v.index,
					view: v.vue,
					file: v.fichier,
					url: v.url,
					width: v.largeur,
					height: v.hauteur,
					bytes: v.octets,
				})),
			},
			null,
			2,
		)}\n`,
	);
	return { code, etat: "recupere", vues: vues.length, base };
}

// -------------------------------------------------------------------------------------------

function codesDuCache(n: number): string[] {
	const dir = join(RACINE, "var", "model-cache");
	if (!existsSync(dir)) return [];
	return readdirSync(dir)
		.filter((f) => /^c\d{8}\.glb$/.test(f))
		.map((f) => f.replace(/\.glb$/, ""))
		.sort()
		.slice(0, n);
}

async function codesDeLAudit(chemin: string, n: number): Promise<string[]> {
	const texte = await Bun.file(chemin).text();
	return texte
		.split("\n")
		.filter(Boolean)
		.map((l) => JSON.parse(l) as { code: string; statut: string })
		.filter((r) => r.statut === "assemblable")
		.map((r) => r.code)
		.filter((c) => /^c\d{8}$/.test(c))
		.slice(0, n);
}

async function main() {
	// Auto-test de l'encodage AVANT tout appel réseau : si l'aller-retour casse, toutes les URL
	// seraient fausses et l'on prendrait une pluie de 404 pour « ces personnages n'ont pas de
	// modèle 3D ». On refuse de partir plutôt que de produire cette conclusion-là.
	const temoin = "c05024700";
	if (decoderQ(encoderQ(temoin)) !== JSON.stringify({ character_id: [temoin] })) {
		throw new Error("encodage q : aller-retour cassé");
	}
	if (encoderQ(temoin) !== "hN2cl56NnpyLmo2glpvdxaTdnM_Kz83LyM_P3aKC") {
		throw new Error("encodage q : ne reproduit pas l'URL de référence connue");
	}

	const argv = process.argv.slice(2);
	let codes: string[] = [];
	let nb = 40;
	let forcer = false;
	let concurrence = 2;
	for (let i = 0; i < argv.length; i++) {
		const a = argv[i];
		const v = () => argv[++i] ?? "";
		if (a === "--codes") codes = v().split(/[,\s]+/).filter(Boolean);
		else if (a === "--nb") nb = Number(v());
		else if (a === "--forcer") forcer = true;
		else if (a === "--concurrence") concurrence = Number(v());
		else if (a === "--depuis-cache") {
			nb = Number(v()) || nb;
			codes = codesDuCache(nb);
		} else if (a === "--depuis-audit") codes = await codesDeLAudit(v(), nb);
		else if (a === "-h" || a === "--help") {
			console.log(
				[
					"zukan-corpus — télécharge les 8 vues de référence du zukan pour N personnages",
					"  --codes a,b,c            codes explicites",
					"  --depuis-cache N         les N premiers codes déjà assemblés (var/model-cache)",
					"  --depuis-audit F --nb N  les N premiers assemblables d'un resultats.ndjson",
					"  --forcer                 retélécharge même si le manifeste existe",
					"  --concurrence N          personnages en parallèle (défaut 2)",
				].join("\n"),
			);
			return;
		}
	}
	if (!codes.length) codes = codesDuCache(nb);
	if (!codes.length) throw new Error("aucun code à traiter");

	console.error(`corpus zukan : ${codes.length} personnages → ${SORTIE}`);
	const issues: Issue[] = [];
	const file = [...codes];
	await Promise.all(
		Array.from({ length: Math.max(1, concurrence) }, async () => {
			for (;;) {
				const c = file.shift();
				if (!c) return;
				try {
					const r = await recuperer(c, forcer);
					issues.push(r);
					console.error(`  ${c} : ${r.etat}${r.vues ? ` (${r.vues} vues)` : ""}${r.detail ? ` — ${r.detail}` : ""}`);
				} catch (e) {
					issues.push({ code: c, etat: "erreur", detail: String(e) });
					console.error(`  ${c} : erreur — ${e}`);
				}
			}
		}),
	);

	const par = (e: Issue["etat"]) => issues.filter((i) => i.etat === e).length;
	const resume = {
		horodatage: new Date().toISOString(),
		demandes: codes.length,
		recuperes: par("recupere"),
		deja_presents: par("deja"),
		sans_modele_3d: par("sans-modele-3d"),
		erreurs: par("erreur"),
		couverture_disque: readdirSync(SORTIE).filter((d) => existsSync(join(SORTIE, d, "manifest.json"))).length,
		detail: issues.sort((a, b) => (a.code < b.code ? -1 : 1)),
	};
	await Bun.write(join(SORTIE, "corpus-resume.json"), `${JSON.stringify(resume, null, 2)}\n`);
	console.log(JSON.stringify({ ...resume, detail: undefined }, null, 2));
}

await main();
