/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { dansLeDepot } from "../../lib/racine";

// Recon web EXHAUSTIF d'un site officiel Inazuma Eleven (cible par défaut :
// victory-road/fr). Complète les scrapers `news.ts`/`re.ts` (qui ne sauvent que
// le HTML par page) en capturant TOUT : endpoints (URL + méthode + status +
// type), sous-domaines, full CDN URLs, images, CSS, JS, payloads JSON, plus les
// sorties `bxc detect`/`bxc recon`/HAR et un `llms.txt` structuré.
//
// On s'appuie sur le binaire `bxc` (profil `http` = curl-impersonate, fiable et
// rapide ici ; `mirror`/`max` ne sont pas exploitables sans Chromium natif
// installé). Chaque page seed est passée à `bxc recon --snapshot-dir` (HTML
// rendu côté serveur), puis on parse le HTML pour TOUTES les ressources
// référencées et on les résout en URLs absolues. Chaque ressource unique est
// ensuite sondée (HEAD/GET via fetch) pour récupérer status + content-type, et
// tout corps JSON est persisté.

const BXC_BIN = process.env.BXC_BIN || "bxc";
// Les binaires Bun standalone lisent le bunfig.toml du cwd. Le dépôt niers en
// possède un preload IEVR ; isoler le CLI natif évite qu'il tente de charger
// ce preload avec le mauvais workspace.
const BXC_CWD = process.env.BXC_CWD || (BXC_BIN.includes("\\") || BXC_BIN.includes("/") ? dirname(BXC_BIN) : undefined);
const DEFAULT_PROFILE = process.env.BXC_RECON_PROFILE || "http";

export interface WebReconOptions {
	/** URL racine (défaut victory-road/fr). */
	rootUrl?: string;
	/** Pages seed additionnelles (URLs absolues) à crawler en plus de la découverte. */
	seeds?: string[];
	/** Répertoire de sortie des artefacts. */
	outDir: string;
	/** Profil bxc (défaut `http`). */
	profile?: string;
	/** Domaines internes considérés comme « pages » à crawler récursivement. */
	pageHostAllow?: string[];
	/** Préfixe de chemin que les pages crawlées doivent partager (défaut `/victory-road/`). */
	pagePathPrefix?: string;
	/** Sonder (fetch) chaque ressource pour status/content-type (défaut true). */
	probeAssets?: boolean;
	/** Nombre max de ressources sondées (sécurité, défaut 2000). */
	maxProbes?: number;
}

export interface Endpoint {
	url: string;
	method: string;
	status: number | null;
	contentType: string | null;
	kind: ResourceKind;
	referrers: string[];
}

type ResourceKind =
	| "page"
	| "stylesheet"
	| "script"
	| "image"
	| "font"
	| "media"
	| "iframe"
	| "json"
	| "manifest"
	| "link"
	| "preload"
	| "other";

export interface WebReconResult {
	root: string;
	pages: string[];
	endpoints: Endpoint[];
	subdomains: string[];
	cdnUrls: string[];
	images: string[];
	css: string[];
	js: string[];
	json: Array<{ url: string; file: string }>;
	frameworks: unknown;
	artifacts: Record<string, { path: string; bytes: number }>;
}

interface BxcRunResult {
	code: number;
	stdout: string;
	stderr: string;
}

/** Exécute `bxc` avec args et renvoie stdout/stderr (timeout dur). */
function runBxc(args: string[], timeoutMs = 90000): Promise<BxcRunResult> {
	return new Promise((resolve) => {
		const child = spawn(BXC_BIN, args, { stdio: ["ignore", "pipe", "pipe"], cwd: BXC_CWD });
		let stdout = "";
		let stderr = "";
		const timer = setTimeout(() => {
			try {
				child.kill("SIGKILL");
			} catch {
				/* noop */
			}
		}, timeoutMs);
		child.stdout.on("data", (d) => (stdout += d.toString()));
		child.stderr.on("data", (d) => (stderr += d.toString()));
		child.on("close", (code) => {
			clearTimeout(timer);
			resolve({ code: code ?? -1, stdout, stderr });
		});
		child.on("error", (err) => {
			clearTimeout(timer);
			resolve({ code: -1, stdout, stderr: stderr + String(err) });
		});
	});
}

const FONT_EXT = /\.(woff2?|ttf|otf|eot)(\?|$)/i;
const IMG_EXT = /\.(png|jpe?g|gif|webp|svg|avif|ico|bmp)(\?|$)/i;
const MEDIA_EXT = /\.(mp4|webm|m3u8|mp3|wav|ogg|m4a|adx|hca)(\?|$)/i;
const JSON_EXT = /\.json(\?|$)/i;

function classifyByUrl(url: string): ResourceKind {
	if (JSON_EXT.test(url)) return "json";
	if (/\.css(\?|$)/i.test(url)) return "stylesheet";
	if (/\.m?js(\?|$)/i.test(url)) return "script";
	if (IMG_EXT.test(url)) return "image";
	if (FONT_EXT.test(url)) return "font";
	if (MEDIA_EXT.test(url)) return "media";
	if (/manifest/i.test(url)) return "manifest";
	return "other";
}

/** Extrait toutes les URLs de ressources d'un document HTML (attributs + inline). */
function extractResources(html: string, baseUrl: string): Map<string, ResourceKind> {
	const out = new Map<string, ResourceKind>();
	const add = (raw: string | undefined, kind: ResourceKind) => {
		if (!raw) return;
		const v = raw.trim();
		if (!v || v.startsWith("data:") || v.startsWith("javascript:") || v.startsWith("#")) return;
		let abs: string;
		try {
			abs = new URL(v, baseUrl).toString();
		} catch {
			return;
		}
		if (!/^https?:/i.test(abs)) return;
		const existing = out.get(abs);
		// json/script/stylesheet/image priment sur "other"/"link"
		if (!existing || existing === "other" || existing === "link") out.set(abs, kind);
	};

	// <link rel=...> : stylesheet, preload, icon, manifest
	for (const m of html.matchAll(/<link\b[^>]*>/gi)) {
		const tag = m[0];
		const href = tag.match(/href=["']([^"']+)["']/i)?.[1];
		const rel = (tag.match(/rel=["']([^"']+)["']/i)?.[1] || "").toLowerCase();
		const asAttr = (tag.match(/\bas=["']([^"']+)["']/i)?.[1] || "").toLowerCase();
		if (!href) continue;
		if (rel.includes("stylesheet")) add(href, "stylesheet");
		else if (rel.includes("manifest")) add(href, "manifest");
		else if (rel.includes("preload") || rel.includes("prefetch")) {
			if (asAttr === "style") add(href, "stylesheet");
			else if (asAttr === "script") add(href, "script");
			else if (asAttr === "font") add(href, "font");
			else if (asAttr === "image") add(href, "image");
			else add(href, "preload");
		} else if (rel.includes("icon") || rel.includes("apple-touch")) add(href, "image");
		else add(href, classifyByUrl(href));
	}
	// <script src=...>
	for (const m of html.matchAll(/<script\b[^>]*\bsrc=["']([^"']+)["'][^>]*>/gi)) add(m[1], "script");
	// <img src / data-src / srcset>
	for (const m of html.matchAll(/<img\b[^>]*>/gi)) {
		const tag = m[0];
		add(tag.match(/\bsrc=["']([^"']+)["']/i)?.[1], "image");
		add(tag.match(/\bdata-src=["']([^"']+)["']/i)?.[1], "image");
		const srcset = tag.match(/\bsrcset=["']([^"']+)["']/i)?.[1];
		if (srcset)
			for (const part of srcset.split(","))
				add(part.trim().split(/\s+/)[0], "image");
	}
	// <source src / srcset> (picture, video, audio)
	for (const m of html.matchAll(/<source\b[^>]*>/gi)) {
		const tag = m[0];
		add(tag.match(/\bsrc=["']([^"']+)["']/i)?.[1], "media");
		const srcset = tag.match(/\bsrcset=["']([^"']+)["']/i)?.[1];
		if (srcset)
			for (const part of srcset.split(","))
				add(part.trim().split(/\s+/)[0], "image");
	}
	// <video|audio src / poster>
	for (const m of html.matchAll(/<(?:video|audio)\b[^>]*>/gi)) {
		const tag = m[0];
		add(tag.match(/\bsrc=["']([^"']+)["']/i)?.[1], "media");
		add(tag.match(/\bposter=["']([^"']+)["']/i)?.[1], "image");
	}
	// <iframe src>
	for (const m of html.matchAll(/<iframe\b[^>]*\bsrc=["']([^"']+)["'][^>]*>/gi)) add(m[1], "iframe");
	// inline style url(...) et style="background:url(...)"
	for (const m of html.matchAll(/url\(\s*["']?([^"')]+)["']?\s*\)/gi)) {
		const u = m[1];
		if (u) add(u, classifyByUrl(u));
	}
	// <a href> (pages internes + sorties)
	for (const m of html.matchAll(/<a\b[^>]*\bhref=["']([^"']+)["'][^>]*>/gi)) add(m[1], "link");
	// meta og:image / twitter:image
	for (const m of html.matchAll(/<meta\b[^>]*>/gi)) {
		const tag = m[0];
		const prop = (
			tag.match(/(?:property|name)=["']([^"']+)["']/i)?.[1] || ""
		).toLowerCase();
		const content = tag.match(/content=["']([^"']+)["']/i)?.[1];
		if (prop.includes("image") && content) add(content, "image");
	}
	return out;
}

function hostOf(url: string): string | null {
	try {
		return new URL(url).hostname;
	} catch {
		return null;
	}
}

/** Sonde une ressource : GET (suit redirections), renvoie status + content-type ; persiste le JSON. */
async function probe(
	url: string,
	kind: ResourceKind,
	jsonDir: string,
	jsonSink: Array<{ url: string; file: string }>
): Promise<{ status: number | null; contentType: string | null; kind: ResourceKind }> {
	try {
		const res = await fetch(url, {
			headers: {
				"user-agent":
					"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36",
				accept: "*/*",
			},
			redirect: "follow",
			signal: AbortSignal.timeout(20000),
		});
		const ct = res.headers.get("content-type");
		let resolvedKind = kind;
		const isJson = (ct || "").includes("json") || JSON_EXT.test(url);
		if (isJson && res.ok) {
			resolvedKind = "json";
			try {
				const text = await res.text();
				JSON.parse(text); // valide
				const safe = url.replace(/^https?:\/\//, "").replace(/[^A-Za-z0-9._-]/g, "_").slice(0, 180);
				const file = join(jsonDir, `${safe || "payload"}.json`);
				await writeFile(file, text);
				jsonSink.push({ url, file });
			} catch {
				/* corps non-JSON malgré le content-type : on ignore */
			}
		}
		return { status: res.status, contentType: ct, kind: resolvedKind };
	} catch {
		return { status: null, contentType: null, kind };
	}
}

async function pool<T>(items: T[], limit: number, fn: (it: T) => Promise<void>): Promise<void> {
	const queue = [...items];
	const workers = Array.from({ length: Math.min(limit, queue.length) }, async () => {
		for (;;) {
			const it = queue.shift();
			if (it === undefined) break;
			await fn(it);
		}
	});
	await Promise.all(workers);
}

/**
 * Crawl recon exhaustif d'un site Inazuma. Écrit tous les artefacts demandés dans
 * `outDir` et renvoie un résumé structuré.
 */
export async function crawlWebRecon(opts: WebReconOptions): Promise<WebReconResult> {
	const root = opts.rootUrl ?? "https://www.inazuma.jp/victory-road/fr/";
	const profile = opts.profile ?? DEFAULT_PROFILE;
	// Par défaut, on ne CRAWLE (= visite récursive) que les pages sous le chemin
	// racine (ex. /victory-road/fr/) — la cible. Les URLs vers d'autres locales
	// (EN patchnotes, etc.) ou domaines restent capturées comme ENDPOINTS (sondées
	// pour status/type), mais ne déclenchent pas une expansion complète de l'arbre
	// EN/JA/… (sinon le crawl explose à 120 pages hors cible).
	const rootPath = (() => {
		try {
			return new URL(root).pathname.replace(/[^/]*$/, "");
		} catch {
			return "/victory-road/";
		}
	})();
	const pathPrefix = opts.pagePathPrefix ?? rootPath;
	const probeAssets = opts.probeAssets ?? true;
	const maxProbes = opts.maxProbes ?? 2000;
	const rootHost = hostOf(root) ?? "www.inazuma.jp";
	const pageHostAllow = opts.pageHostAllow ?? [rootHost];
	const outDir = opts.outDir;
	const jsonDir = join(outDir, "json");
	const snapDir = join(outDir, "snapshots");
	await mkdir(jsonDir, { recursive: true });
	await mkdir(snapDir, { recursive: true });

	const artifacts: WebReconResult["artifacts"] = {};
	const writeArtifact = async (name: string, content: string | Uint8Array) => {
		const path = join(outDir, name);
		await writeFile(path, content);
		const bytes = typeof content === "string" ? Buffer.byteLength(content) : content.byteLength;
		artifacts[name] = { path, bytes };
	};

	// --- 1. bxc detect (root + sous-domaine zukan) ---
	console.log("[WebRecon] bxc detect…");
	const detectRoot = await runBxc(["detect", root, "--json"], 90000);
	let frameworks: unknown = null;
	try {
		frameworks = JSON.parse(detectRoot.stdout);
	} catch {
		frameworks = { error: "detect parse failed", raw: detectRoot.stdout.slice(0, 4000), stderr: detectRoot.stderr.slice(0, 2000) };
	}
	const detectZukan = await runBxc(["detect", "https://zukan.inazuma.jp/fr/", "--json"], 90000);
	let zukanDetect: unknown = null;
	try {
		zukanDetect = JSON.parse(detectZukan.stdout);
	} catch {
		zukanDetect = { error: "detect parse failed" };
	}
	await writeArtifact("detect.json", JSON.stringify({ root: frameworks, "zukan.inazuma.jp": zukanDetect }, null, 2));

	// --- 2. bxc recon (root) → recon.md ---
	console.log("[WebRecon] bxc recon…");
	const reconSnap = join(snapDir, "recon-root");
	await mkdir(reconSnap, { recursive: true });
	const recon = await runBxc(
		["recon", root, "--profile", profile, "--snapshot-dir", reconSnap, "--output", join(outDir, "recon.md")],
		120000
	);
	if (recon.code === 0) {
		const f = Bun.file(join(outDir, "recon.md"));
		artifacts["recon.md"] = { path: join(outDir, "recon.md"), bytes: await f.size };
	} else {
		await writeArtifact("recon.md", `# recon failed (code ${recon.code})\n\n${recon.stderr || recon.stdout}`);
	}

	// --- 3. Découverte des pages + extraction des ressources ---
	console.log("[WebRecon] Découverte des pages et ressources…");
	const visited = new Set<string>();
	const queue: string[] = [];
	const enqueue = (u: string) => {
		// Retire le fragment et normalise le slash final (toutes les pages crawlées
		// sont des URLs de répertoire, sans extension — garanti par isCrawlablePage).
		const base = u.split("#")[0]!;
		const norm = base.endsWith("/") ? base : `${base}/`;
		if (!visited.has(norm) && !queue.includes(norm)) queue.push(norm);
	};
	enqueue(root);
	for (const s of opts.seeds ?? []) enqueue(s);

	// resources: abs URL -> { kind, referrers }
	const resources = new Map<string, { kind: ResourceKind; referrers: Set<string> }>();
	const recordRes = (url: string, kind: ResourceKind, ref: string) => {
		const e = resources.get(url);
		if (e) {
			e.referrers.add(ref);
			if (e.kind === "other" || e.kind === "link" || e.kind === "preload") e.kind = kind;
		} else {
			resources.set(url, { kind, referrers: new Set([ref]) });
		}
	};

	const LOCALES = new Set(["fr", "en", "ja", "es", "de", "it", "zh-tw", "zh-cn", "pt-br"]);
	const isCrawlablePage = (url: string): boolean => {
		try {
			const u = new URL(url);
			if (!pageHostAllow.includes(u.hostname)) return false;
			if (!u.pathname.startsWith(pathPrefix)) return false;
			// pages = pas d'extension de fichier
			if (/\.[a-z0-9]{2,5}$/i.test(u.pathname)) return false;
			// Rejette les artefacts du sélecteur de langue. Sur ce site, la SEULE
			// position légitime d'un segment locale est juste après `victory-road`
			// (ex. /victory-road/fr/…). Les liens du sélecteur sont des hrefs relatifs
			// (`en/`, `fr/`, …) qui, résolus contre une page profonde, produisent des
			// URLs comme /victory-road/fr/story/en/ ou /victory-road/fr/character/de/ —
			// des stubs de ~2 ko sans contenu propre. On rejette donc tout segment
			// locale apparaissant APRÈS le 1er segment suivant `victory-road`.
			const segs = u.pathname.split("/").filter(Boolean); // ["victory-road","fr","story","en"]
			const vrIdx = segs.indexOf("victory-road");
			if (vrIdx !== -1) {
				// segs[vrIdx+1] = locale canonique (autorisé) ; au-delà, un locale = artefact
				for (let i = vrIdx + 2; i < segs.length; i++) {
					if (LOCALES.has(segs[i]!)) return false;
				}
			}
			return true;
		} catch {
			return false;
		}
	};

	const pages: string[] = [];
	let pageCount = 0;
	const MAX_PAGES = 120;
	while (queue.length && pageCount < MAX_PAGES) {
		const pageUrl = queue.shift()!;
		if (visited.has(pageUrl)) continue;
		visited.add(pageUrl);

		const snap = join(snapDir, `p${pageCount}`);
		await mkdir(snap, { recursive: true });
		const r = await runBxc(["recon", pageUrl, "--profile", profile, "--snapshot-dir", snap], 90000);
		let html = "";
		try {
			html = await Bun.file(join(snap, "http.html")).text();
		} catch {
			/* pas de snapshot */
		}
		if (!html) {
			console.warn(`[WebRecon] Pas de HTML pour ${pageUrl} (code ${r.code})`);
			continue;
		}
		pages.push(pageUrl);
		pageCount++;
		console.log(`[WebRecon] [${pageCount}] ${pageUrl} (${html.length} octets)`);

		const res = extractResources(html, pageUrl);
		for (const [url, kind] of res) {
			if (kind === "link") {
				// candidat page interne
				if (isCrawlablePage(url)) enqueue(url);
				recordRes(url, "link", pageUrl);
			} else if (kind === "iframe") {
				recordRes(url, "iframe", pageUrl);
			} else {
				recordRes(url, kind, pageUrl);
			}
		}
	}

	// --- 4. Sonder les ressources (status + content-type + JSON) ---
	const jsonSink: Array<{ url: string; file: string }> = [];
	const endpoints: Endpoint[] = [];
	const allResEntries = [...resources.entries()];
	// inclure aussi les pages comme endpoints (déjà 200 puisque HTML obtenu)
	for (const p of pages) {
		endpoints.push({
			url: p,
			method: "GET",
			status: 200,
			contentType: "text/html",
			kind: "page",
			referrers: [],
		});
	}

	if (probeAssets) {
		console.log(`[WebRecon] Sondage de ${Math.min(allResEntries.length, maxProbes)} ressources…`);
		const toProbe = allResEntries.slice(0, maxProbes);
		await pool(toProbe, 12, async ([url, meta]) => {
			const { status, contentType, kind } = await probe(url, meta.kind, jsonDir, jsonSink);
			endpoints.push({
				url,
				method: "GET",
				status,
				contentType,
				kind,
				referrers: [...meta.referrers].slice(0, 5),
			});
		});
	} else {
		for (const [url, meta] of allResEntries)
			endpoints.push({
				url,
				method: "GET",
				status: null,
				contentType: null,
				kind: meta.kind,
				referrers: [...meta.referrers].slice(0, 5),
			});
	}

	// --- 5. Agrégations ---
	endpoints.sort((a, b) => a.url.localeCompare(b.url));
	const allUrls = endpoints.map((e) => e.url);
	const subdomains = [
		...new Set(allUrls.map(hostOf).filter((h): h is string => !!h)),
	].sort();
	const cdnHosts = new Set([
		"iijcdn.jp",
		"cloudfront.net",
		"jocdn.net",
		"akamaized.net",
		"fastly.net",
		"gstatic.com",
		"googleapis.com",
		"ytimg.com",
	]);
	const cdnUrls = allUrls
		.filter((u) => {
			const h = hostOf(u);
			return h ? [...cdnHosts].some((c) => h.endsWith(c)) : false;
		})
		.sort();
	const images = endpoints.filter((e) => e.kind === "image").map((e) => e.url).sort();
	const css = endpoints.filter((e) => e.kind === "stylesheet").map((e) => e.url).sort();
	const js = endpoints.filter((e) => e.kind === "script").map((e) => e.url).sort();

	await writeArtifact("endpoints.json", JSON.stringify(endpoints, null, 2));
	await writeArtifact("subdomains.txt", subdomains.join("\n") + "\n");
	await writeArtifact("cdn-urls.txt", cdnUrls.join("\n") + "\n");
	await writeArtifact("images.json", JSON.stringify(images, null, 2));
	await writeArtifact("css.txt", css.join("\n") + "\n");
	await writeArtifact("js.txt", js.join("\n") + "\n");

	// --- 6. HAR (document-level, multi-pages clés) ---
	console.log("[WebRecon] bxc har record…");
	const harPages = [root, ...pages.slice(0, 14)];
	const uniqHar = [...new Set(harPages)];
	const harEntries: unknown[] = [];
	for (const hp of uniqHar) {
		const tmp = join(snapDir, `har-${harEntries.length}.har`);
		const hr = await runBxc(["har", "record", hp, tmp, "--profile", profile], 60000);
		if (hr.code === 0) {
			try {
				const h = await Bun.file(tmp).json();
				if (h?.log?.entries) harEntries.push(...h.log.entries);
			} catch {
				/* skip */
			}
		}
	}
	// Compléter avec les ressources sondées (status connu) → HAR enrichi
	for (const e of endpoints) {
		if (e.status == null) continue;
		harEntries.push({
			startedDateTime: new Date().toISOString(),
			request: { method: e.method, url: e.url, headers: [], queryString: [], cookies: [], headersSize: -1, bodySize: -1 },
			response: {
				status: e.status,
				statusText: "",
				httpVersion: "HTTP/1.1",
				headers: e.contentType ? [{ name: "content-type", value: e.contentType }] : [],
				content: { size: -1, mimeType: e.contentType ?? "" },
				redirectURL: "",
				headersSize: -1,
				bodySize: -1,
				cookies: [],
			},
			cache: {},
			timings: { send: 0, wait: 0, receive: 0 },
			_kind: e.kind,
		});
	}
	const har = {
		log: {
			version: "1.2",
			creator: { name: "bxc+ie-crawl/web-recon", version: "0.6.0" },
			pages: pages.map((p, i) => ({
				startedDateTime: new Date().toISOString(),
				id: `page_${i}`,
				title: p,
				pageTimings: {},
			})),
			entries: harEntries,
		},
	};
	await writeArtifact("network.har", JSON.stringify(har, null, 2));

	// --- 7. frameworks.md (synthèse detect) ---
	const fwMd = buildFrameworksMd(frameworks, zukanDetect, subdomains, cdnUrls);
	await writeArtifact("frameworks.md", fwMd);

	// --- 8. llms.txt ---
	const llms = buildLlmsTxt({
		root,
		pages,
		endpoints,
		subdomains,
		cdnUrls,
		images,
		css,
		js,
		json: jsonSink,
		frameworks,
		zukanDetect,
	});
	await writeArtifact("llms.txt", llms);

	return {
		root,
		pages,
		endpoints,
		subdomains,
		cdnUrls,
		images,
		css,
		js,
		json: jsonSink,
		frameworks,
		artifacts,
	};
}

/**
 * Wrapper « tâche » pour l'orchestrateur cron : recon web exhaustif de
 * victory-road/fr vers le data root standard. Non bloquant pour le pipeline
 * (renvoie {success} comme les autres crawlers, n'émet pas d'exception).
 */
export async function crawlWebReconTask(
	outDir = dansLeDepot("data", "inazuma.jp", "victory-road", "_recon", "fr")
): Promise<{ success: boolean; pages?: number; endpoints?: number; error?: string }> {
	try {
		const res = await crawlWebRecon({ outDir });
		return { success: true, pages: res.pages.length, endpoints: res.endpoints.length };
	} catch (err) {
		const msg = err instanceof Error ? err.message : String(err);
		console.error("[WebRecon] Échec :", msg);
		return { success: false, error: msg };
	}
}

function fmtDetect(d: any): string {
	if (!d || typeof d !== "object") return "_n/a_";
	const cats: Array<[string, string]> = [
		["frontend", "Frontend"],
		["backend", "Backend"],
		["cms", "CMS"],
		["framework", "Framework"],
		["library", "Librairies"],
		["language", "Langage"],
		["server", "Serveur"],
		["cdn", "CDN"],
		["analytics", "Analytics"],
		["tagManagers", "Tag managers"],
		["hosting", "Hébergement"],
	];
	const lines: string[] = [];
	for (const [key, label] of cats) {
		const arr = d[key];
		if (Array.isArray(arr) && arr.length) {
			const names = arr
				.map((x: any) => (typeof x === "string" ? x : x?.name || JSON.stringify(x)))
				.join(", ");
			lines.push(`- **${label}**: ${names}`);
		}
	}
	return lines.length ? lines.join("\n") : "_Aucun signal de framework détecté (site statique server-rendered)._";
}

function buildFrameworksMd(root: any, zukan: any, subdomains: string[], cdnUrls: string[]): string {
	return `# Frameworks & infrastructure — Inazuma Victory Road (FR)

Source : \`bxc detect\` (multi-signal : DNS, IP, CNAME, reverse PTR, headers, body, wappalyzergo).

## www.inazuma.jp (site principal)

- **HTTP status**: ${root?.httpStatus ?? "?"}
- **IPs**: ${(root?.resolvedIps ?? []).join(", ") || "n/a"}
- **CNAME**: ${(root?.cnameChain ?? []).join(" → ") || "n/a"}
- **Reverse PTR**: ${root?.reversePtr ? JSON.stringify(root.reversePtr) : "n/a"}

${fmtDetect(root)}

Note : le serveur d'origine répond \`Server: openresty\` (recon http) ; la couche edge est l'**IIJ CDN** (CNAME \`*.id*.cas.iijcdn.jp\`, PTR \`*.jocdn.net\`).

## zukan.inazuma.jp (Codex / base de données personnages)

- **HTTP status**: ${zukan?.httpStatus ?? "?"}
- **IPs**: ${(zukan?.resolvedIps ?? []).join(", ") || "n/a"}
- **CNAME**: ${(zukan?.cnameChain ?? []).join(" → ") || "n/a"}

${fmtDetect(zukan)}

Note : \`zukan\` est hébergé sur **AWS** (ELB \`*.ap-northeast-1.elb.amazonaws.com\`, Tokyo), backend \`nginx\` (PHP côté applicatif d'après analyses antérieures), assets statiques sur **CloudFront**.

## Sous-domaines / hôtes observés (${subdomains.length})

${subdomains.map((s) => `- \`${s}\``).join("\n")}

## Hôtes CDN référencés (${new Set(cdnUrls.map((u) => { try { return new URL(u).hostname; } catch { return u; } })).size})

${[...new Set(cdnUrls.map((u) => { try { return new URL(u).hostname; } catch { return u; } }))].map((h) => `- \`${h}\``).join("\n") || "_aucun_"}
`;
}

function buildLlmsTxt(d: {
	root: string;
	pages: string[];
	endpoints: Endpoint[];
	subdomains: string[];
	cdnUrls: string[];
	images: string[];
	css: string[];
	js: string[];
	json: Array<{ url: string; file: string }>;
	frameworks: any;
	zukanDetect: any;
}): string {
	const sections = [...new Set(d.pages.map((p) => {
		const m = p.match(/\/victory-road\/fr\/([^/]+)/);
		return m ? m[1] : "(racine)";
	}))].sort();
	const byKind = (k: string) => d.endpoints.filter((e) => e.kind === k).length;
	const statusCounts: Record<string, number> = {};
	for (const e of d.endpoints) {
		const key = e.status == null ? "non-sondé" : String(e.status);
		statusCounts[key] = (statusCounts[key] ?? 0) + 1;
	}

	return `# Inazuma Eleven: Victory Road — site officiel (FR)

> Recon exhaustif de ${d.root} et de l'écosystème lié (www.inazuma.jp, zukan.inazuma.jp, CDN IIJ/CloudFront).
> Jeu LEVEL-5 (Steam app id 2799860). Site statique multilingue server-rendered (openresty) derrière l'IIJ CDN.
> Généré par @rosegriffon/cron ie-crawl/web-recon (moteur bxc 0.6.0, profil http/curl-impersonate).

## Vue d'ensemble

- URL racine : ${d.root}
- Pages crawlées : ${d.pages.length}
- Endpoints (URLs uniques) : ${d.endpoints.length}
- Sous-domaines / hôtes : ${d.subdomains.length}
- URLs CDN : ${d.cdnUrls.length}
- Images : ${d.images.length}
- Feuilles CSS : ${d.css.length}
- Bundles JS : ${d.js.length}
- Payloads JSON capturés : ${d.json.length}

## Infrastructure

- **Site principal** (www.inazuma.jp) : edge IIJ CDN (CNAME *.cas.iijcdn.jp, PTR *.jocdn.net, IPs ${(d.frameworks?.resolvedIps ?? []).join("/")}), origine openresty. Cache-Control max-age=300. Pas de framework JS (HTML statique + jQuery/Swiper/Slick/Colorbox côté UI).
- **Codex personnages** (zukan.inazuma.jp) : AWS Tokyo (ELB ap-northeast-1), serveur ${(d.zukanDetect?.server ?? []).map((s: any) => s.evidence || s.name).join("/") || "nginx"}, assets sur CloudFront.
- **Analytics** : Google Tag Manager (GTM-KX7QTFN).
- **Fonts** : Google Fonts (Anton, Noto Sans).

## Sous-domaines / hôtes

${d.subdomains.map((s) => `- ${s}`).join("\n")}

## Sections du site (victory-road/fr)

${sections.map((s) => `- /${s === "(racine)" ? "" : s + "/"}`).join("\n")}

## Pages crawlées (${d.pages.length})

${d.pages.map((p) => `- ${p}`).join("\n")}

## Répartition des endpoints

- Par type : page=${byKind("page")}, stylesheet=${byKind("stylesheet")}, script=${byKind("script")}, image=${byKind("image")}, font=${byKind("font")}, media=${byKind("media")}, json=${byKind("json")}, iframe=${byKind("iframe")}, lien-externe=${byKind("link")}, autre=${byKind("other")}
- Par status HTTP : ${Object.entries(statusCounts).map(([k, v]) => `${k}=${v}`).join(", ")}

## Feuilles de style (${d.css.length})

${d.css.map((u) => `- ${u}`).join("\n")}

## Bundles JS (${d.js.length})

${d.js.map((u) => `- ${u}`).join("\n")}

## Payloads JSON (${d.json.length})

${d.json.length ? d.json.map((j) => `- ${j.url}`).join("\n") : "- (aucun payload JSON détecté — site sans API JSON publique sur ces pages)"}

## Liens externes notables

${[...new Set(d.endpoints.filter((e) => e.kind === "link" || e.kind === "iframe").map((e) => e.url))]
		.filter((u) => { try { return new URL(u).hostname !== "www.inazuma.jp"; } catch { return false; } })
		.sort()
		.map((u) => `- ${u}`)
		.join("\n") || "- (aucun)"}

## Images — voir images.json (${d.images.length} URLs CDN complètes)

## Artefacts

- recon.md, detect.json, frameworks.md
- network.har, endpoints.json, subdomains.txt, cdn-urls.txt
- images.json, css.txt, js.txt, json/*.json
- llms.txt (ce fichier)
`;
}
