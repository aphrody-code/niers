#!/usr/bin/env bun
/**
 * Audit d'assemblage : quelle PART des personnages du jeu `nie-model-serve` sait-il assembler ?
 *
 * L'objectif « tous les personnages aussi bien assemblés que Byron Love et Shawn Froste » n'est
 * mesurable que si l'on sait d'abord combien sont seulement *assemblables*. Ce script répond à
 * cette question sur un échantillon reproductible, et rend un taux AVEC son dénominateur et son
 * intervalle de confiance — jamais un pourcentage nu.
 *
 * Le piège qu'il évite explicitement : confondre « le service a REFUSÉ » (404, avec une raison —
 * c'est un échec réel, imputable au dépôt) et « le service n'a PAS RÉPONDU » (délai dépassé,
 * connexion coupée, 5xx — c'est un INDÉTERMINÉ, imputable à la mesure). Un faux négatif a déjà
 * été produit aujourd'hui en mélangeant les deux. Les indéterminés sont donc retentés, puis
 * SORTIS du dénominateur, et comptés à part.
 *
 * Population : les `internal_code` distincts de `inagle_characters` (gisement `extrait`,
 * var/mirror.sqlite). Échantillonnage déterministe : un hachage FNV-1a du code mélangé à la
 * graine ordonne la population, on prend les N premiers. Même graine ⇒ même échantillon.
 *
 * Usage :
 *   bun --bun scripts/validation/audit-assemblage.ts                 # 200 personnages, graine 1
 *   bun --bun scripts/validation/audit-assemblage.ts --sample 300
 *   bun --bun scripts/validation/audit-assemblage.ts --all           # les 5 723
 *   bun --bun scripts/validation/audit-assemblage.ts --codes c05024700,c01000010
 *   bun --bun scripts/validation/audit-assemblage.ts --sortie var/outputs/audit-x
 *
 * Sorties : `<sortie>/resultats.ndjson` (une ligne par personnage) et `<sortie>/resume.json`.
 */

import { Database } from "bun:sqlite";
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

// ---------------------------------------------------------------------------------------------
// Arguments

interface Options {
	base: string;
	miroir: string;
	echantillon: number;
	graine: number;
	tout: boolean;
	codes: string[];
	concurrence: number;
	delaiMs: number;
	tentatives: number;
	sortie: string;
}

function lireOptions(argv: string[]): Options {
	const racine = resolve(import.meta.dir, "..", "..");
	const o: Options = {
		base: process.env.NIE_CDN_URL ?? "http://127.0.0.1:8790",
		miroir: join(racine, "var", "mirror.sqlite"),
		echantillon: 200,
		graine: 1,
		tout: false,
		codes: [],
		concurrence: 3,
		delaiMs: 120_000,
		tentatives: 3,
		sortie: join(racine, "var", "outputs", "audit-assemblage", horodatage()),
	};
	for (let i = 0; i < argv.length; i++) {
		const a = argv[i];
		const v = () => {
			const x = argv[++i];
			if (x === undefined) throw new Error(`${a} attend une valeur`);
			return x;
		};
		if (a === "--base") o.base = v().replace(/\/$/, "");
		else if (a === "--miroir") o.miroir = v();
		else if (a === "--sample" || a === "--echantillon") o.echantillon = Number(v());
		else if (a === "--seed" || a === "--graine") o.graine = Number(v());
		else if (a === "--all" || a === "--tout") o.tout = true;
		else if (a === "--codes") o.codes = v().split(/[,\s]+/).filter(Boolean);
		else if (a === "--concurrence") o.concurrence = Number(v());
		else if (a === "--delai") o.delaiMs = Number(v());
		else if (a === "--tentatives") o.tentatives = Number(v());
		else if (a === "--sortie") o.sortie = v();
		else if (a === "-h" || a === "--help") {
			console.log(
				[
					"audit-assemblage — taux de personnages assemblables par nie-model-serve",
					"",
					"  --sample N        taille de l'échantillon (défaut 200)",
					"  --seed N          graine de l'échantillonnage déterministe (défaut 1)",
					"  --all             toute la population (5 723 codes) — long",
					"  --codes a,b,c     audite exactement ces codes",
					"  --concurrence N   requêtes simultanées (défaut 3)",
					"  --delai MS        délai d'attente par requête (défaut 120000)",
					"  --tentatives N    tentatives avant de classer INDÉTERMINÉ (défaut 3)",
					"  --base URL        base du service (défaut http://127.0.0.1:8790)",
					"  --sortie CHEMIN   dossier de sortie",
				].join("\n"),
			);
			process.exit(0);
		} else throw new Error(`option inconnue : ${a}`);
	}
	return o;
}

function horodatage(): string {
	return new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
}

// ---------------------------------------------------------------------------------------------
// Population et échantillonnage déterministe

function fnv1a(s: string): number {
	let h = 0x811c9dc5;
	for (let i = 0; i < s.length; i++) {
		h ^= s.charCodeAt(i);
		h = Math.imul(h, 0x01000193) >>> 0;
	}
	return h >>> 0;
}

function population(miroir: string): { code: string; nom: string }[] {
	const db = new Database(miroir, { readonly: true });
	try {
		const lignes = db
			.query<{ code: string; nom: string | null }, []>(
				`SELECT internal_code AS code, MIN(COALESCE(name_en, name_fr, name_ja)) AS nom
				   FROM inagle_characters
				  WHERE internal_code IS NOT NULL AND internal_code <> ''
				  GROUP BY internal_code
				  ORDER BY internal_code`,
			)
			.all();
		return lignes.map((l) => ({ code: l.code, nom: l.nom ?? "" }));
	} finally {
		db.close();
	}
}

function echantillonner(
	pop: { code: string; nom: string }[],
	n: number,
	graine: number,
): { code: string; nom: string }[] {
	if (n >= pop.length) return pop;
	return [...pop]
		.map((p) => ({ p, r: fnv1a(`${graine}:${p.code}`) }))
		.sort((a, b) => a.r - b.r || (a.p.code < b.p.code ? -1 : 1))
		.slice(0, n)
		.map((x) => x.p)
		.sort((a, b) => (a.code < b.code ? -1 : 1));
}

// ---------------------------------------------------------------------------------------------
// Interrogation du service

/** `refus` = le service a répondu et a dit non. `indetermine` = il n'a pas répondu. */
type Statut = "assemblable" | "refus" | "indetermine";

interface Resultat {
	code: string;
	nom: string;
	statut: Statut;
	http: number | null;
	raison: string | null;
	ms: number;
	tentatives: number;
	/** Signaux de qualité lus dans le rapport d'assemblage, quand il y en a un. */
	mode?: string | null;
	pieces?: number;
	roles?: string[];
	materiaux_sans_texture?: string[];
	notes?: number;
	rapport_detaille?: boolean;
	glb_octets?: number | null;
	glb_sha256?: string | null;
}

/**
 * Récolte, dans le journal systemd du service, la CHAÎNE COMPLÈTE des erreurs d'assemblage.
 *
 * Le corps HTTP d'un 404 n'imprime que le contexte externe (`{e}`, cf. main.rs) : « assemblage
 * personnage cXXXXXXXX », identique pour TOUS les refus, donc inclassable. Le service, lui,
 * journalise `{e:#}` — la chaîne entière, avec la vraie cause. Sans cette récolte, la liste des
 * échecs existerait mais serait muette sur le pourquoi ; c'est elle qui rend l'audit actionnable.
 */
async function raisonsDuJournal(depuis: Date): Promise<Map<string, string>> {
	const par = new Map<string, string>();
	try {
		const p = Bun.spawn(
			[
				"journalctl",
				"-u",
				"nie-model-serve",
				// Fenêtre RELATIVE : un horodatage absolu se ferait interpréter en heure locale
				// alors qu'on tient de l'UTC — sur une machine décalée, la fenêtre raterait tout.
				"--since",
				`${Math.max(1, Math.ceil((Date.now() - depuis.getTime()) / 1000))} seconds ago`,
				"--no-pager",
				"-o",
				"cat",
			],
			{ stdout: "pipe", stderr: "ignore" },
		);
		const texte = await new Response(p.stdout).text();
		await p.exited;
		for (const ligne of texte.split("\n")) {
			const m = ligne.match(/(?:rapport|assemblage) (\S+) échoué\s*:\s*(.+)$/);
			if (!m) continue;
			// « assemblage <code>: <cause> » → on ne garde que la cause.
			par.set(m[1]!, m[2]!.replace(/^assemblage \S+ \S+:\s*/, "").trim());
		}
	} catch {
		// journalctl absent ou service non-systemd : on se rabat sur le corps HTTP.
	}
	return par;
}

/** Normalise la raison d'un refus en une CLASSE, pour pouvoir la compter. */
function classerRaison(raison: string): string {
	const r = raison.toLowerCase();
	if (/glb introuvable ou illisible/.test(r)) return "aucun GLB source dans le VFS";
	if (/chara_model/.test(r)) return "absent de chara_model";
	if (/objbin/.test(r)) return "objbin introuvable";
	if (/aucune pièce|aucune piece|0 pièce|aucun maillage|vide/.test(r)) return "aucune pièce assemblée";
	if (/squelette|skeleton/.test(r)) return "squelette introuvable";
	if (/texture|g4tx/.test(r)) return "texture introuvable";
	if (/g4mg|g4md|maillage|mesh/.test(r)) return "maillage illisible";
	if (/introuvable|absent|non trouvé|not found|no such file/.test(r)) return "asset VFS absent";
	if (/code invalide/.test(r)) return "code invalide";
	return `autre : ${raison.slice(0, 60)}`;
}

async function interroger(o: Options, p: { code: string; nom: string }): Promise<Resultat> {
	const url = `${o.base}/model-report/${p.code}.json`;
	const debut = Date.now();
	let dernier = "";
	for (let essai = 1; essai <= o.tentatives; essai++) {
		try {
			const rep = await fetch(url, { signal: AbortSignal.timeout(o.delaiMs) });
			const texte = await rep.text();
			if (rep.status === 200) {
				let r: Record<string, unknown> = {};
				try {
					r = JSON.parse(texte) as Record<string, unknown>;
				} catch {
					// 200 mais corps illisible : le service a répondu, c'est un refus de fait.
					return {
						code: p.code,
						nom: p.nom,
						statut: "refus",
						http: 200,
						raison: "rapport illisible (JSON invalide)",
						ms: Date.now() - debut,
						tentatives: essai,
					};
				}
				const pieces = Array.isArray(r.pieces) ? (r.pieces as Record<string, unknown>[]) : [];
				return {
					code: p.code,
					nom: p.nom,
					statut: "assemblable",
					http: 200,
					raison: null,
					ms: Date.now() - debut,
					tentatives: essai,
					mode: typeof r.mode === "string" ? r.mode : null,
					pieces: pieces.length,
					roles: pieces.map((x) => String(x.role ?? "?")),
					materiaux_sans_texture: Array.isArray(r.materials_without_texture)
						? (r.materials_without_texture as string[])
						: [],
					notes: Array.isArray(r.notes) ? r.notes.length : 0,
					// Keshin/armures : le service rend `{ "code": … }` sans détail d'assemblage.
					rapport_detaille: pieces.length > 0 || typeof r.mode === "string",
					glb_octets: typeof r.glb_bytes === "number" ? r.glb_bytes : null,
					glb_sha256: typeof r.glb_sha256 === "string" ? r.glb_sha256 : null,
				};
			}
			if (rep.status === 404 || rep.status === 400) {
				// Le service a répondu et a refusé : échec RÉEL, avec sa raison.
				const raison = texte.replace(/^modèle \S+ non disponible : /, "").trim().slice(0, 400);
				return {
					code: p.code,
					nom: p.nom,
					statut: "refus",
					http: rep.status,
					raison: raison || `HTTP ${rep.status}`,
					ms: Date.now() - debut,
					tentatives: essai,
				};
			}
			// 5xx : le service est en peine, pas le modèle. On retente.
			dernier = `HTTP ${rep.status}`;
		} catch (e) {
			dernier = e instanceof Error ? `${e.name}: ${e.message}` : String(e);
		}
		if (essai < o.tentatives) await Bun.sleep(500 * essai);
	}
	return {
		code: p.code,
		nom: p.nom,
		statut: "indetermine",
		http: null,
		raison: dernier,
		ms: Date.now() - debut,
		tentatives: o.tentatives,
	};
}

// ---------------------------------------------------------------------------------------------
// Statistiques

/** Intervalle de Wilson à 95 % — honnête sur petit échantillon, contrairement à l'approximation
 *  normale qui déborde de [0,1] près des bornes. */
function wilson95(succes: number, total: number): [number, number] {
	if (total === 0) return [0, 1];
	const z = 1.959_963_985;
	const p = succes / total;
	const d = 1 + (z * z) / total;
	const c = p + (z * z) / (2 * total);
	const e = z * Math.sqrt((p * (1 - p)) / total + (z * z) / (4 * total * total));
	return [Math.max(0, (c - e) / d), Math.min(1, (c + e) / d)];
}

/** Correction de population finie : l'échantillon n'est pas tiré dans un puits infini. */
function fpc(n: number, N: number): number {
	return N > 1 && n < N ? Math.sqrt((N - n) / (N - 1)) : 1;
}

// ---------------------------------------------------------------------------------------------

async function main() {
	const o = lireOptions(process.argv.slice(2));

	// Le service doit être debout AVANT de commencer : sinon tout serait « indéterminé » et le
	// rapport dirait faussement « on ne sait pas » là où la panne est locale et connue.
	try {
		const sante = await fetch(`${o.base}/health`, { signal: AbortSignal.timeout(10_000) });
		if (!sante.ok) throw new Error(`/health → HTTP ${sante.status}`);
	} catch (e) {
		console.error(`nie-model-serve injoignable sur ${o.base} : ${e}`);
		console.error("Rien n'est mesurable sans lui — l'audit s'arrête plutôt que de rendre 100 % d'indéterminés.");
		process.exit(2);
	}

	const pop = population(o.miroir);
	const cible = o.codes.length
		? o.codes.map((c) => pop.find((p) => p.code === c) ?? { code: c, nom: "" })
		: o.tout
			? pop
			: echantillonner(pop, o.echantillon, o.graine);

	mkdirSync(o.sortie, { recursive: true });
	const fluxPath = join(o.sortie, "resultats.ndjson");
	const debutRun = new Date(Date.now() - 5_000);

	console.error(
		`population ${pop.length} codes distincts — échantillon ${cible.length}` +
			(o.codes.length ? " (liste explicite)" : o.tout ? " (tout)" : ` (graine ${o.graine})`),
	);

	const resultats: Resultat[] = [];
	let fait = 0;
	const file = [...cible];
	const ouvriers = Array.from({ length: Math.max(1, o.concurrence) }, async () => {
		for (;;) {
			const p = file.shift();
			if (!p) return;
			const r = await interroger(o, p);
			resultats.push(r);
			fait++;
			if (fait % 25 === 0 || fait === cible.length) {
				const ok = resultats.filter((x) => x.statut === "assemblable").length;
				const ref = resultats.filter((x) => x.statut === "refus").length;
				const ind = resultats.filter((x) => x.statut === "indetermine").length;
				console.error(`  ${fait}/${cible.length} — ok ${ok} · refus ${ref} · indéterminé ${ind}`);
			}
		}
	});
	await Promise.all(ouvriers);

	// Enrichir les refus avec la chaîne d'erreur complète lue dans le journal du service.
	const journal = await raisonsDuJournal(debutRun);
	let enrichis = 0;
	for (const r of resultats) {
		const j = journal.get(r.code);
		if (r.statut === "refus" && j) {
			r.raison = j;
			enrichis++;
		}
	}

	resultats.sort((a, b) => (a.code < b.code ? -1 : 1));
	await Bun.write(fluxPath, `${resultats.map((r) => JSON.stringify(r)).join("\n")}\n`);

	const ok = resultats.filter((r) => r.statut === "assemblable");
	const refus = resultats.filter((r) => r.statut === "refus");
	const ind = resultats.filter((r) => r.statut === "indetermine");
	const determines = ok.length + refus.length;
	const taux = determines ? ok.length / determines : 0;
	const [bas, haut] = wilson95(ok.length, determines);
	const f = fpc(determines, pop.length);
	const demi = ((haut - bas) / 2) * f;

	// Ventilation des refus par CLASSE de raison — c'est là qu'est le travail restant.
	const parRaison = new Map<string, { n: number; exemples: string[] }>();
	for (const r of refus) {
		const c = classerRaison(r.raison ?? "");
		const e = parRaison.get(c) ?? { n: 0, exemples: [] };
		e.n++;
		if (e.exemples.length < 5) e.exemples.push(`${r.code} : ${(r.raison ?? "").slice(0, 120)}`);
		parRaison.set(c, e);
	}

	// Signaux de dégradation parmi les assemblables : un 200 n'est pas une garantie de qualité.
	const sansTexture = ok.filter((r) => (r.materiaux_sans_texture?.length ?? 0) > 0);
	const sansDetail = ok.filter((r) => r.rapport_detaille === false);
	const parMode = new Map<string, number>();
	for (const r of ok) parMode.set(r.mode ?? "n/a", (parMode.get(r.mode ?? "n/a") ?? 0) + 1);
	const parPieces = new Map<number, number>();
	for (const r of ok) parPieces.set(r.pieces ?? 0, (parPieces.get(r.pieces ?? 0) ?? 0) + 1);

	const resume = {
		horodatage: new Date().toISOString(),
		base: o.base,
		miroir: o.miroir,
		population: pop.length,
		echantillon: cible.length,
		graine: o.codes.length || o.tout ? null : o.graine,
		assemblables: ok.length,
		refus: refus.length,
		indetermines: ind.length,
		denominateur_determine: determines,
		taux: Number(taux.toFixed(6)),
		ic95: [Number((taux - demi).toFixed(6)), Number((taux + demi).toFixed(6))],
		ic95_wilson_brut: [Number(bas.toFixed(6)), Number(haut.toFixed(6))],
		correction_population_finie: Number(f.toFixed(6)),
		projection_population: {
			centre: Math.round(taux * pop.length),
			bas: Math.round(Math.max(0, taux - demi) * pop.length),
			haut: Math.round(Math.min(1, taux + demi) * pop.length),
		},
		refus_par_raison: Object.fromEntries(
			[...parRaison.entries()].sort((a, b) => b[1].n - a[1].n).map(([k, v]) => [k, v]),
		),
		refus_enrichis_par_journal: enrichis,
		indetermines_detail: ind.slice(0, 20).map((r) => ({ code: r.code, raison: r.raison, ms: r.ms })),
		qualite: {
			assemblables_avec_rapport_detaille: ok.length - sansDetail.length,
			assemblables_sans_rapport_detaille: sansDetail.length,
			avec_materiau_sans_texture: sansTexture.length,
			modes: Object.fromEntries([...parMode.entries()].sort((a, b) => b[1] - a[1])),
			pieces_histogramme: Object.fromEntries([...parPieces.entries()].sort((a, b) => a[0] - b[0])),
			glb_octets_median: mediane(ok.map((r) => r.glb_octets ?? 0).filter((n) => n > 0)),
		},
		duree_ms: {
			total: resultats.reduce((n, r) => n + r.ms, 0),
			mediane: mediane(resultats.map((r) => r.ms)),
			max: Math.max(0, ...resultats.map((r) => r.ms)),
		},
		fichiers: { resultats: fluxPath },
	};

	const resumePath = join(o.sortie, "resume.json");
	await Bun.write(resumePath, `${JSON.stringify(resume, null, 2)}\n`);

	// Rapport lisible sur stdout — un taux ne se cite JAMAIS sans son dénominateur.
	const pct = (x: number) => `${(x * 100).toFixed(2)} %`;
	console.log("");
	console.log(`Population         : ${pop.length} internal_code distincts (inagle_characters)`);
	console.log(`Échantillon        : ${cible.length}`);
	console.log(`Assemblables       : ${ok.length}`);
	console.log(`Refus (échec réel) : ${refus.length}`);
	console.log(`Indéterminés       : ${ind.length}  (hors dénominateur)`);
	console.log(`Dénominateur       : ${determines}`);
	console.log(
		`TAUX               : ${pct(taux)}  IC95 [${pct(Math.max(0, taux - demi))} ; ${pct(Math.min(1, taux + demi))}]`,
	);
	console.log(
		`Projection sur ${pop.length} : ${resume.projection_population.centre} ` +
			`[${resume.projection_population.bas} ; ${resume.projection_population.haut}] personnages assemblables`,
	);
	if (parRaison.size) {
		console.log("\nRefus par raison :");
		for (const [k, v] of [...parRaison.entries()].sort((a, b) => b[1].n - a[1].n)) {
			console.log(`  ${String(v.n).padStart(5)}  ${k}`);
			console.log(`         ex. ${v.exemples[0]}`);
		}
	}
	console.log("\nQualité des assemblables :");
	console.log(`  rapport détaillé      : ${ok.length - sansDetail.length}/${ok.length}`);
	console.log(`  matériau sans texture : ${sansTexture.length}`);
	console.log(`  modes                 : ${JSON.stringify(resume.qualite.modes)}`);
	console.log(`\nSorties : ${resumePath}`);
}

function mediane(xs: number[]): number {
	if (!xs.length) return 0;
	const s = [...xs].sort((a, b) => a - b);
	const m = s.length >> 1;
	return s.length % 2 ? s[m]! : Math.round((s[m - 1]! + s[m]!) / 2);
}

await main();
