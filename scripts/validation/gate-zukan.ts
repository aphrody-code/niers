#!/usr/bin/env bun
/**
 * Gate de qualité : compare le GLB assemblé par `nie-model-serve` aux vues de référence du zukan.
 *
 * C'est la seule façon de dire « ce personnage est aussi bien assemblé que Shawn Froste » sans
 * l'affirmer. Un 200 sur `/model-full/` prouve seulement qu'un GLB est sorti ; il ne dit rien de
 * la tête posée à l'envers, du maillot manquant ou de la texture rose.
 *
 * Chaîne :
 *   1. le GLB est rendu sous 8 angles par model-viewer dans un Chromium headless (WebGL 2 via
 *      SwiftShader — vérifié sur cette machine), fond transparent ;
 *   2. rendu ET référence sont NORMALISÉS : détourage sur le fond, recadrage à la boîte
 *      englobante, remise à l'échelle sur une toile carrée commune. Sans cette étape on mesurerait
 *      le cadrage de deux caméras différentes, pas la conformité du modèle ;
 *   3. deux métriques par angle :
 *        - `iou`  : recouvrement des SILHOUETTES. Robuste au style de rendu (le zukan est en toon
 *                   avec contours, nous sommes en PBR) — c'est la mesure de forme.
 *        - `ssim` : structure + luminance sur l'image composée sur blanc. Sensible au style, donc
 *                   à lire comme un indicateur relatif entre personnages, jamais comme une note
 *                   absolue de conformité.
 *
 * CE QUE CETTE GATE NE PROUVE PAS : l'égalité pixel à pixel avec le jeu. Le zukan n'est pas
 * `nie.exe`, et notre rendu n'est pas le sien. Elle mesure « la bonne silhouette, aux bons
 * endroits, avec les bonnes couleurs », pas le pixel-perfect (cf. la note du dépôt à ce sujet).
 *
 * Usage :
 *   bun --bun scripts/validation/gate-zukan.ts --codes c05024700,c01000010
 *   bun --bun scripts/validation/gate-zukan.ts --tous          # tous ceux qui ont une référence
 */

import { existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";

const RACINE = resolve(import.meta.dir, "..", "..");
const REFS = join(RACINE, "var", "outputs", "zukan-reference");
const SORTIE = join(RACINE, "var", "outputs", "gate-zukan");
const MODEL_VIEWER = join(RACINE, "apps", "azalee", "public", "vendor", "model-viewer.min.js");
const CHROME = process.env.NIE_CHROME ?? "/usr/local/bin/chromium";
const BASE_SERVICE = process.env.NIE_CDN_URL ?? "http://127.0.0.1:8790";

const TAILLE = 750; // même côté que les planches `_fullbody` du zukan
const TOILE = 512; // toile de normalisation
const ANGLES = 8;

// ---------------------------------------------------------------------------------------------
// Rendu : model-viewer dans un Chromium headless

const PAGE = /* html */ `<!doctype html><meta charset="utf-8">
<style>html,body{margin:0;background:transparent}model-viewer{width:${TAILLE}px;height:${TAILLE}px;--poster-color:transparent;background:transparent}</style>
<script type="module" src="/mv.js"></script>
<model-viewer id="v" src="/m.glb" camera-orbit="0deg 90deg auto" interaction-prompt="none"
  environment-image="neutral" exposure="1.15" shadow-intensity="0" disable-zoom
  interpolation-decay="1" min-camera-orbit="-Infinity auto auto" max-camera-orbit="Infinity auto auto"
  style="background:transparent"></model-viewer>
<script type="module">
const v = document.getElementById('v');
const dire = (m) => fetch('/journal?m=' + encodeURIComponent(m));
const trames = async (n) => { for (let k = 0; k < n; k++) await new Promise(r => requestAnimationFrame(r)); };
try {
  await new Promise((ok, ko) => {
    v.addEventListener('load', ok, { once: true });
    v.addEventListener('error', e => ko(new Error('model-viewer error: ' + (e.detail?.type ?? '?'))), { once: true });
    setTimeout(() => ko(new Error('délai de chargement du GLB dépassé')), 120000);
  });
  await v.updateComplete; await trames(10);

  // Le RAYON est figé une fois pour toutes. Avec « auto » model-viewer recadre à chaque
  // changement d'orbite : les quatre dernières vues sortaient deux fois plus petites que les
  // quatre premières, ce que la normalisation par boîte englobante masquait en dégradant la
  // résolution. On lit le cadrage automatique une seule fois, puis on l'impose.
  const o = v.getCameraOrbit();
  const rayon = o.radius;
  const cible = v.getCameraTarget();
  v.cameraTarget = cible.x + 'm ' + cible.y + 'm ' + cible.z + 'm';

  for (let i = 0; i < ${ANGLES}; i++) {
    // SENS DE ROTATION : +45° par planche, pas −45°. Mesuré, pas supposé — avec −45° les vues
    // cardinales (0 et 180°, invariantes au sens) restaient bonnes et seules les diagonales
    // s'effondraient, signature d'une inversion. IoU moyen sur Shawn Froste : 0,657 avec −45°,
    // 0,815 avec +45°, chaque angle en gain. Le harnais manuel préexistant
    // (scripts/validation/model-validation.ts) utilise −45° : sa comparaison côte à côte
    // confrontait donc des angles différents sur 6 vues sur 8.
    v.cameraOrbit = (i * 45) + 'deg 90deg ' + rayon + 'm';
    // model-viewer INTERPOLE la caméra : \`updateComplete\` ne résout pas la fin du mouvement,
    // il résout la fin du rendu de la trame suivante. Sans attendre l'amortissement, les
    // captures sont prises en vol et les angles ne correspondent plus à ceux du zukan
    // (constaté : la vue 4 rendait une face là où la référence montrait un dos).
    await v.updateComplete;
    await trames(24);
    await new Promise(r => setTimeout(r, 150));
    await trames(4);
    const url = v.toDataURL('image/png');
    await fetch('/shot?i=' + i, { method: 'POST', body: url });
  }
  await dire('FINI rayon=' + rayon.toFixed(3));
} catch (e) { await dire('ERREUR ' + (e?.message ?? e)); }
</script>`;

interface Rendu {
	ok: boolean;
	detail: string;
	dossier: string;
}

async function rendre(code: string, dossier: string): Promise<Rendu> {
	mkdirSync(dossier, { recursive: true });
	let resoudre!: (r: Rendu) => void;
	const fini = new Promise<Rendu>((r) => {
		resoudre = r;
	});
	let recus = 0;

	const serveur = Bun.serve({
		hostname: "127.0.0.1",
		port: 0,
		idleTimeout: 240,
		async fetch(req) {
			const u = new URL(req.url);
			if (u.pathname === "/") return new Response(PAGE, { headers: { "Content-Type": "text/html; charset=utf-8" } });
			if (u.pathname === "/mv.js") {
				return new Response(Bun.file(MODEL_VIEWER), { headers: { "Content-Type": "text/javascript" } });
			}
			if (u.pathname === "/m.glb") {
				const amont = await fetch(`${BASE_SERVICE}/model-full/${code}.glb`);
				if (!amont.ok) {
					resoudre({ ok: false, detail: `service : HTTP ${amont.status}`, dossier });
					return new Response("amont", { status: 502 });
				}
				return new Response(amont.body, { headers: { "Content-Type": "model/gltf-binary" } });
			}
			if (u.pathname === "/shot") {
				const i = Number(u.searchParams.get("i"));
				const data = await req.text();
				const b64 = data.slice(data.indexOf(",") + 1);
				await Bun.write(join(dossier, `rendu_r${i}.png`), Buffer.from(b64, "base64"));
				recus++;
				return new Response("ok");
			}
			if (u.pathname === "/journal") {
				const m = u.searchParams.get("m") ?? "";
				resoudre(
					m.startsWith("FINI")
						? { ok: recus === ANGLES, detail: `${recus}/${ANGLES} vues · ${m.slice(5)}`, dossier }
						: { ok: false, detail: m, dossier },
				);
				return new Response("ok");
			}
			return new Response("absent", { status: 404 });
		},
	});

	const url = `http://127.0.0.1:${serveur.port}/`;
	const chrome = Bun.spawn(
		[
			CHROME,
			"--headless=new",
			"--no-sandbox",
			"--disable-gpu",
			"--use-gl=angle",
			"--use-angle=swiftshader",
			// Sans ce drapeau, Chromium 147 REFUSE le repli logiciel WebGL (« Automatic fallback to
			// software WebGL has been deprecated ») : la page se charge, model-viewer n'obtient
			// jamais de contexte, et l'attente expire sans un mot. L'échec est totalement muet —
			// c'est ce qui a fait passer la première version de cette gate pour un blocage réseau.
			"--enable-unsafe-swiftshader",
			"--hide-scrollbars",
			`--window-size=${TAILLE},${TAILLE}`,
			"--disable-dev-shm-usage",
			url,
		],
		{ stdout: "ignore", stderr: "ignore" },
	);

	const garde = setTimeout(() => resoudre({ ok: false, detail: "délai global dépassé", dossier }), 240_000);
	const r = await fini;
	clearTimeout(garde);
	chrome.kill();
	await chrome.exited.catch(() => {});
	serveur.stop(true);
	return r;
}

// ---------------------------------------------------------------------------------------------
// Normalisation et métriques (ImageMagick)

async function im(args: string[]): Promise<string> {
	const p = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
	const [out, err] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text()]);
	await p.exited;
	// `compare` écrit sa métrique sur stderr et sort en code non nul dès que les images diffèrent :
	// un code non nul n'est PAS une panne ici, c'est le résultat.
	return (out + err).trim();
}

/**
 * Détoure le fond puis recadre sur la boîte englobante et remet à l'échelle sur une toile carrée.
 * `origine` distingue les deux fonds : le zukan est sur du blanc opaque (remplissage par
 * diffusion depuis le coin, qui préserve les zones blanches INTÉRIEURES — le short blanc de
 * Shawn en dépend), notre rendu est déjà transparent.
 */
async function normaliser(entree: string, sortie: string, origine: "zukan" | "rendu"): Promise<void> {
	const commun = ["-trim", "+repage", "-resize", `${TOILE}x${TOILE}`, "-background", "none", "-gravity", "center", "-extent", `${TOILE}x${TOILE}`];
	if (origine === "zukan") {
		await im([
			"convert",
			entree,
			"-alpha",
			"set",
			"-fuzz",
			"6%",
			"-fill",
			"none",
			"-floodfill",
			"+0+0",
			"white",
			...commun,
			`PNG32:${sortie}`,
		]);
	} else {
		await im(["convert", entree, "-alpha", "set", ...commun, `PNG32:${sortie}`]);
	}
}

async function masque(entree: string, sortie: string): Promise<void> {
	await im(["convert", entree, "-alpha", "extract", "-threshold", "50%", `PNG8:${sortie}`]);
}

async function moyenne(chemin: string): Promise<number> {
	const s = await im(["convert", chemin, "-format", "%[fx:mean]", "info:"]);
	return Number.parseFloat(s) || 0;
}

/** Intersection sur union des deux silhouettes. */
async function iou(a: string, b: string, tmp: string): Promise<number> {
	const inter = join(tmp, "inter.png");
	const union = join(tmp, "union.png");
	await im(["convert", a, b, "-compose", "multiply", "-composite", inter]);
	await im(["convert", a, b, "-compose", "lighten", "-composite", union]);
	const [mi, mu] = await Promise.all([moyenne(inter), moyenne(union)]);
	return mu > 0 ? mi / mu : 0;
}

/**
 * Lit une image en niveaux de gris 8 bits, composée sur blanc, sous forme de tableau brut.
 * On passe par ImageMagick pour le décodage seulement — le calcul, lui, se fait ici.
 */
async function grisBrut(chemin: string, cote: number): Promise<Uint8Array> {
	const p = Bun.spawn(
		["magick", chemin, "-background", "white", "-alpha", "remove", "-alpha", "off", "-colorspace", "Gray", "-depth", "8", `gray:-`],
		{ stdout: "pipe", stderr: "ignore" },
	);
	const buf = new Uint8Array(await new Response(p.stdout).arrayBuffer());
	await p.exited;
	if (buf.length !== cote * cote) {
		throw new Error(`${chemin} : ${buf.length} octets pour ${cote}×${cote} attendus`);
	}
	return buf;
}

/** Convolution séparable par un noyau 1D, en float. */
function convoluer(src: Float64Array, w: number, h: number, noyau: Float64Array): Float64Array {
	const r = (noyau.length - 1) / 2;
	const tmp = new Float64Array(w * h);
	const out = new Float64Array(w * h);
	for (let y = 0; y < h; y++) {
		for (let x = 0; x < w; x++) {
			let s = 0;
			for (let k = -r; k <= r; k++) {
				const xx = Math.min(w - 1, Math.max(0, x + k));
				s += src[y * w + xx]! * noyau[k + r]!;
			}
			tmp[y * w + x] = s;
		}
	}
	for (let y = 0; y < h; y++) {
		for (let x = 0; x < w; x++) {
			let s = 0;
			for (let k = -r; k <= r; k++) {
				const yy = Math.min(h - 1, Math.max(0, y + k));
				s += tmp[yy * w + x]! * noyau[k + r]!;
			}
			out[y * w + x] = s;
		}
	}
	return out;
}

/**
 * SSIM (Wang et al. 2004) : fenêtre gaussienne 11×11 σ=1,5, C1=(0,01·255)², C2=(0,03·255)².
 *
 * Écrit ici parce que `compare -metric SSIM` de l'ImageMagick 7.1.2 de cette machine NE CALCULE
 * PAS de SSIM : sur deux images IDENTIQUES il rend `0 (0)` là où un SSIM vaut 1, et il rend la
 * même valeur pour SSIM et pour DSSIM (qui devraient être complémentaires). Il retombe en silence
 * sur une métrique de différence. Un score « SSIM » lu de cette commande serait un chiffre faux
 * présenté comme une mesure — exactement ce qu'il ne faut pas produire.
 */
function ssimGris(a: Uint8Array, b: Uint8Array, cote: number): number {
	const n = cote * cote;
	const fa = new Float64Array(n);
	const fb = new Float64Array(n);
	for (let i = 0; i < n; i++) {
		fa[i] = a[i]!;
		fb[i] = b[i]!;
	}
	const sigma = 1.5;
	const rayon = 5;
	const noyau = new Float64Array(2 * rayon + 1);
	let somme = 0;
	for (let k = -rayon; k <= rayon; k++) {
		const v = Math.exp(-(k * k) / (2 * sigma * sigma));
		noyau[k + rayon] = v;
		somme += v;
	}
	for (let i = 0; i < noyau.length; i++) noyau[i]! /= somme;

	const faa = new Float64Array(n);
	const fbb = new Float64Array(n);
	const fab = new Float64Array(n);
	for (let i = 0; i < n; i++) {
		faa[i] = fa[i]! * fa[i]!;
		fbb[i] = fb[i]! * fb[i]!;
		fab[i] = fa[i]! * fb[i]!;
	}
	const mua = convoluer(fa, cote, cote, noyau);
	const mub = convoluer(fb, cote, cote, noyau);
	const saa = convoluer(faa, cote, cote, noyau);
	const sbb = convoluer(fbb, cote, cote, noyau);
	const sab = convoluer(fab, cote, cote, noyau);

	const c1 = (0.01 * 255) ** 2;
	const c2 = (0.03 * 255) ** 2;
	let total = 0;
	for (let i = 0; i < n; i++) {
		const ma = mua[i]!;
		const mb = mub[i]!;
		const va = saa[i]! - ma * ma;
		const vb = sbb[i]! - mb * mb;
		const cab = sab[i]! - ma * mb;
		total += ((2 * ma * mb + c1) * (2 * cab + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2));
	}
	return total / n;
}

async function ssim(a: string, b: string): Promise<number> {
	const [ga, gb] = await Promise.all([grisBrut(a, TOILE), grisBrut(b, TOILE)]);
	return ssimGris(ga, gb, TOILE);
}

// ---------------------------------------------------------------------------------------------

interface Score {
	code: string;
	statut: "compare" | "sans-reference" | "rendu-echoue";
	detail?: string;
	angles?: { i: number; iou: number; ssim: number }[];
	iou_moyen?: number;
	ssim_moyen?: number;
	iou_min?: number;
}

async function evaluer(code: string): Promise<Score> {
	const refDir = join(REFS, code);
	if (!existsSync(join(refDir, "manifest.json"))) {
		return { code, statut: "sans-reference", detail: "aucune planche zukan récupérée" };
	}
	const dossier = join(SORTIE, code);
	mkdirSync(dossier, { recursive: true });
	const r = await rendre(code, dossier);
	if (!r.ok) return { code, statut: "rendu-echoue", detail: r.detail };

	const tmp = join(dossier, "tmp");
	mkdirSync(tmp, { recursive: true });
	const angles: { i: number; iou: number; ssim: number }[] = [];
	for (let i = 0; i < ANGLES; i++) {
		const ref = join(refDir, `${code}_r${i}_fullbody.png`);
		const rend = join(dossier, `rendu_r${i}.png`);
		if (!existsSync(ref) || !existsSync(rend)) continue;
		const refN = join(dossier, `norm_ref_r${i}.png`);
		const rendN = join(dossier, `norm_rendu_r${i}.png`);
		await normaliser(ref, refN, "zukan");
		await normaliser(rend, rendN, "rendu");
		const mRef = join(tmp, `mref${i}.png`);
		const mRend = join(tmp, `mrend${i}.png`);
		await masque(refN, mRef);
		await masque(rendN, mRend);
		angles.push({
			i,
			iou: Number((await iou(mRef, mRend, tmp)).toFixed(4)),
			ssim: Number((await ssim(refN, rendN)).toFixed(4)),
		});
	}
	rmSync(tmp, { recursive: true, force: true });
	if (!angles.length) return { code, statut: "rendu-echoue", detail: "aucun couple d'images comparable" };

	// Planche de contrôle : la mesure ne remplace pas l'œil, elle le guide.
	await im([
		"montage",
		...angles.flatMap((a) => [join(dossier, `norm_ref_r${a.i}.png`), join(dossier, `norm_rendu_r${a.i}.png`)]),
		"-tile",
		"4x",
		"-geometry",
		"192x192+2+2",
		"-background",
		"white",
		join(dossier, `${code}_planche.png`),
	]);

	const moy = (f: (a: (typeof angles)[number]) => number) => angles.reduce((n, a) => n + f(a), 0) / angles.length;
	return {
		code,
		statut: "compare",
		angles,
		iou_moyen: Number(moy((a) => a.iou).toFixed(4)),
		ssim_moyen: Number(moy((a) => a.ssim).toFixed(4)),
		iou_min: Number(Math.min(...angles.map((a) => a.iou)).toFixed(4)),
	};
}

async function main() {
	const argv = process.argv.slice(2);
	let codes: string[] = [];
	for (let i = 0; i < argv.length; i++) {
		const a = argv[i];
		if (a === "--codes") codes = (argv[++i] ?? "").split(/[,\s]+/).filter(Boolean);
		else if (a === "--tous") {
			codes = readdirSync(REFS).filter((d) => existsSync(join(REFS, d, "manifest.json")));
		} else if (a === "-h" || a === "--help") {
			console.log("gate-zukan — SSIM + IoU entre le GLB assemblé et les 8 vues du zukan\n  --codes a,b\n  --tous");
			return;
		}
	}
	if (!codes.length) codes = readdirSync(REFS).filter((d) => existsSync(join(REFS, d, "manifest.json")));
	if (!existsSync(CHROME)) throw new Error(`Chromium introuvable : ${CHROME} (poser NIE_CHROME)`);
	mkdirSync(SORTIE, { recursive: true });

	// Auto-test de la métrique : SSIM(x, x) doit valoir 1. C'est ce contrôle qui a démasqué le
	// `compare -metric SSIM` d'ImageMagick, lequel rendait 0 sur deux images identiques.
	{
		const t = join(SORTIE, ".autotest.png");
		await im(["magick", "-size", `${TOILE}x${TOILE}`, "plasma:", t]);
		const g = await grisBrut(t, TOILE);
		const un = ssimGris(g, g, TOILE);
		// Témoin négatif : du bruit DÉCORRÉLÉ. Un simple décalage modulaire ne convient pas —
		// il est bijectif, préserve la variance locale, et rend un SSIM de 0,51 qui ne prouve rien.
		const bruit = new Uint8Array(g.length);
		crypto.getRandomValues(bruit);
		const bas = ssimGris(g, bruit, TOILE);
		rmSync(t, { force: true });
		if (!(un > 0.999 && bas < 0.2)) {
			throw new Error(`SSIM invalide : identité=${un.toFixed(4)}, bruit=${bas.toFixed(4)}`);
		}
	}

	const scores: Score[] = [];
	for (const c of codes.sort()) {
		const s = await evaluer(c);
		scores.push(s);
		console.error(
			s.statut === "compare"
				? `  ${c} : IoU ${s.iou_moyen} (min ${s.iou_min}) · SSIM ${s.ssim_moyen}`
				: `  ${c} : ${s.statut} — ${s.detail}`,
		);
	}

	const comparés = scores.filter((s) => s.statut === "compare");
	const resume = {
		horodatage: new Date().toISOString(),
		demandes: codes.length,
		compares: comparés.length,
		iou_moyen_global: comparés.length
			? Number((comparés.reduce((n, s) => n + (s.iou_moyen ?? 0), 0) / comparés.length).toFixed(4))
			: null,
		ssim_moyen_global: comparés.length
			? Number((comparés.reduce((n, s) => n + (s.ssim_moyen ?? 0), 0) / comparés.length).toFixed(4))
			: null,
		scores: scores.sort((a, b) => (a.iou_moyen ?? -1) - (b.iou_moyen ?? -1)),
	};
	await Bun.write(join(SORTIE, "resume.json"), `${JSON.stringify(resume, null, 2)}\n`);
	console.log(JSON.stringify({ ...resume, scores: undefined }, null, 2));
	console.log(`Détail : ${join(SORTIE, "resume.json")}`);
}

await main();
