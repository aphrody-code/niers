#!/usr/bin/env bun
/**
 * deploy.ts — publication bleu/vert d'Azalée et du site principal, **sans coupure**.
 *
 * Pourquoi ce script existe
 * -------------------------
 * `ship-azalee.sh` / `ship-website.sh` faisaient `systemctl restart` : entre l'arrêt et
 * le premier octet servi par le nouveau processus Next, nginx n'a plus personne au bout
 * du proxy → page de maintenance (azalée) ou 502 (site) pendant plusieurs secondes, et
 * les fragments `/_next/static/*` de l'ancien build disparaissent sous les onglets déjà
 * ouverts (404 puis rechargement forcé). Ici le nouveau code démarre **à côté** de
 * l'ancien, sur un second port ; on ne bascule le trafic qu'une fois sa santé prouvée.
 *
 * Topologie
 * ---------
 *   slot A = service de production (`azalee-web` :3003, `website-web` :3004)
 *   slot B = doublure de bascule et de prévisualisation (`…-b` :3013 / :3014)
 *
 * Chaque slot lit une **version figée** :
 *
 *   /home/ubuntu/rg-releases/<app>/releases/<horodatage>-<BUILD_ID>/
 *   /home/ubuntu/rg-releases/<app>/slot-a -> releases/<version>   (lien symbolique)
 *   /home/ubuntu/rg-releases/<app>/slot-b -> releases/<version>
 *   /home/ubuntu/rg-releases/<app>/static -> static-pool-<horodatage>  (alias nginx)
 *
 * Les versions sont assemblées par **liens physiques** (`cp -al`) depuis
 * `.next/standalone` : quelques milliers d'inodes, pas 1,2 Go recopié. Un `next build`
 * ultérieur remplace les fichiers du dépôt sans jamais toucher un inode déjà publié —
 * c'est ce qui rend le processus en cours insensible au build suivant, alors qu'il
 * charge ses fragments de route à la demande.
 *
 * Trois mécanismes évitent les 404 et la prod cassée pendant une bascule
 * ---------------------------------------------------------------------
 * 1. **Réservoir de fragments statiques** : nginx sert `/_next/static/` depuis l'union
 *    des `.next/static` des dernières versions. Un onglet ouvert avant le déploiement
 *    retrouve donc ses fragments. Les noms sont hachés par contenu : l'union est sûre.
 *    (Next 16 sait poser un `?dpl=` via `deploymentId`, mais en auto-hébergement aucun
 *    routeur n'exploite ce paramètre — `globalThis.NEXT_CLIENT_ASSET_SUFFIX`, cf.
 *    `next/dist/server/base-server.js` — alors que le hachage par contenu, lui, suffit.)
 * 2. **Attente du drainage nginx** : `systemctl reload` rend la main immédiatement, mais
 *    les anciens workers finissent leurs connexions AVEC L'ANCIENNE CONFIGURATION, donc
 *    en proxyfiant vers l'ancien port. Redémarrer l'ancien slot avant leur extinction
 *    produit une 502 (mesuré : 1 requête sur 77). On attend donc la disparition des
 *    workers « shutting down » avant de toucher au slot sortant.
 * 3. **Sondes réelles** : un build cassé répond souvent 200 sur `/api/health` (route
 *    isolée) et 500 sur les pages qui touchent la couche données. On sonde donc aussi
 *    des pages réelles avant de basculer.
 *
 * Séquence d'un déploiement
 * -------------------------
 *   1. garde-fous (type-check, mémoire disponible, disque, nginx déjà valide)
 *   2. build (retenté 3× — Next 16 + Bun sort en 132 après un build pourtant complet)
 *   3. assemblage de la version + réservoir de fragments
 *   4. slot B démarre sur la nouvelle version, sondé
 *   5. bascule nginx → B, attente du drainage      (la prod sert le nouveau code)
 *   6. slot A reprend la même version, sondé
 *   7. bascule nginx → A, drainage, arrêt de B     (état stable : A seul, comme avant)
 *
 * Un échec avant l'étape 5 laisse la production intacte. Un échec après la laisse sur B,
 * qui sert le nouveau code : dégradé (un seul slot), jamais coupé.
 *
 * Prévisualisation
 * ----------------
 * `preview` publie une version sur le slot B **sans toucher à la production** et la rend
 * joignable sur le domaine public à ceux qui portent le cookie `rg_preview` (nginx route
 * par cookie, cf. `map` généré). `promote` publie ensuite cette même version en prod.
 *
 * Usage
 * -----
 *   bun run deploy                      # azalée + site, build compris
 *   bun run deploy azalee               # une seule surface
 *   bun run deploy -- --no-build        # publie les artefacts déjà bâtis
 *   bun run deploy -- --no-gate         # saute le type-check préalable
 *   bun scripts/ops/deploy.ts preview azalee    # → URL d'activation du cookie
 *   bun scripts/ops/deploy.ts promote azalee    # passe la prévisualisation en prod
 *   bun scripts/ops/deploy.ts preview-off azalee
 *   bun scripts/ops/deploy.ts reload azalee     # même version, sans coupure
 *   bun scripts/ops/deploy.ts rollback azalee   # version précédente
 *   bun scripts/ops/deploy.ts status --json
 *   bun scripts/ops/deploy.ts install           # unités systemd + amonts nginx
 *
 * Tout est asynchrone : aucune fonction de `node:fs` synchrone, aucune attente active.
 * Le script ne bloque jamais la boucle d'événements, y compris pour sa propre
 * journalisation (chaînée) et pour les sondes (émises en parallèle).
 */

import { appendFile, mkdir, readdir, readFile, readlink, rename, rm, stat, symlink, writeFile } from "node:fs/promises";
import { freemem, totalmem } from "node:os";

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Racine de CE dépôt, résolue à l'exécution — `scripts/ops/` est deux niveaux sous la racine.
 * Aucun chemin de machine en dur : c'est la même doctrine que côté Rust
 * (`nie_formats::vfs::resolve_game_dir`) et que `packages/cron/src/lib/racine.ts`.
 */
const NIERS_ROOT = new URL("../..", import.meta.url).pathname.replace(/\/$/u, "");

/**
 * Racine du monorepo Rose Griffon, où DEUX surfaces n'ont pas suivi la fusion : le site
 * vitrine `apps/website` et le bot communautaire (`docs/FUSION.md`). Ce script déploie les
 * deux applications, chacune depuis SON dépôt — d'où `repoRoot` par app plutôt qu'une racine
 * unique. Avec une racine unique, le type-check et le `git rev-parse` du wiki partaient de
 * l'autre dépôt : la version publiée aurait porté la révision d'un code qu'elle ne contient pas.
 */
const RG_ROOT = process.env.RG_MONOREPO ?? "/home/ubuntu/rg";
const RELEASES_ROOT = "/home/ubuntu/rg-releases";
const NGINX_UPSTREAM_DIR = "/etc/nginx/rg-upstreams";
const NGINX_MAIN_CONF = "/etc/nginx/nginx.conf";
const BUN_BIN = "/home/ubuntu/.bun/bin/bun";
/** Versions conservées sur disque (retour arrière + réservoir de fragments). */
const DEFAULT_KEEP = 5;
/** Secondes pendant lesquelles l'ancien slot reste debout après la bascule. */
const DEFAULT_DRAIN_SECONDS = 10;
/** Marge de mémoire libre exigée en plus de l'empreinte estimée de la doublure. */
const MEMORY_MARGIN_MIB = 1024;

type SlotId = "a" | "b";

interface SlotConfig {
	unit: string;
	port: number;
}

interface AppConfig {
	key: string;
	/** Nom du répertoire d'application dans `.next/standalone/apps/<dir>`. */
	standaloneDir: string;
	/** Nom du workspace Bun, pour le type-check préalable. */
	workspace: string;
	/**
	 * Racine du dépôt qui porte cette application. Le type-check, le `git rev-parse` de la
	 * version publiée et la lecture de l'unité systemd partent de LÀ, pas d'une racine unique.
	 */
	repoRoot: string;
	/** Dossier des unités systemd dans ce dépôt, relatif à `repoRoot`. */
	unitDir: string;
	appDir: string;
	title: string;
	slots: Record<SlotId, SlotConfig>;
	/** Amont nginx de production (nom du fichier d'inclusion généré). */
	upstream: string;
	/** Amont nginx de prévisualisation. */
	previewUpstream: string;
	/** Variable nginx choisissant l'amont d'après le cookie de prévisualisation. */
	backendVariable: string;
	siteConf: string;
	upstreamOptions: string;
	publicUrl: string;
	/** Chemins sondés après démarrage ; tout code < 500 vaut « vivant ». */
	probes: string[];
	/** Répertoires du dépôt recopiés dans la version (hors `.next/standalone`). */
	extraDirs: string[];
	/** Sous-chemins remplacés par un lien vers le dépôt (données mutables partagées). */
	linkedPaths: string[];
	buildCommand: string[];
	buildEnv: Record<string, string>;
	/** Empreinte mémoire minimale supposée d'une instance fraîche, en Mio. */
	memoryFloorMiB: number;
	/** Délai maximal accordé au démarrage d'un slot, en millisecondes. */
	bootTimeoutMs: number;
}

const APPS: Record<string, AppConfig> = {
	azalee: {
		key: "azalee",
		standaloneDir: "azalee",
		workspace: "@rosegriffon/azalee-web",
		repoRoot: NIERS_ROOT,
		unitDir: "deploy/systemd",
		appDir: `${NIERS_ROOT}/apps/azalee`,
		title: "wiki Azalée",
		slots: {
			a: { unit: "azalee-web.service", port: 3003 },
			b: { unit: "azalee-web-b.service", port: 3013 },
		},
		upstream: "azalee_web",
		previewUpstream: "azalee_preview",
		backendVariable: "azalee_backend",
		siteConf: "/etc/nginx/conf.d/azalee.rosegriffon.conf",
		upstreamOptions: "max_fails=3 fail_timeout=15s",
		publicUrl: "https://azalee.rosegriffon.fr/",
		// `/gallery` a quitté le wiki (parti dans l'explorateur, cf.
		// `docs/MIGRATION-EXPLORATEUR.md`) : la sonde suit la page vivante qui l'a remplacée
		// dans le menu, pas l'URL qui ne fait plus que rediriger.
		probes: ["/api/health", "/", "/chara", "/skill", "/news", "/tools/niers"],
		extraDirs: [".next/static", "public", "data"],
		// Le miroir SQLite est republié chaque nuit dans le dépôt (`nie-miroir.timer`,
		// 04:10 UTC) et épinglé en absolu par `SQLITE_DB_PATH` : la version le vise par
		// lien plutôt que d'en figer une copie devenue périmée au premier dump.
		linkedPaths: ["data/backups"],
		buildCommand: [BUN_BIN, "--env-file=../../.env.local", "run", "build"],
		buildEnv: { SENTRY_SKIP_UPLOAD: "1", NEXT_TELEMETRY_DISABLED: "1" },
		memoryFloorMiB: 2048,
		bootTimeoutMs: 240_000,
	},
	website: {
		key: "website",
		standaloneDir: "website",
		workspace: "@rosegriffon/website",
		repoRoot: RG_ROOT,
		unitDir: "infra/systemd",
		appDir: `${RG_ROOT}/apps/website`,
		title: "site rosegriffon.fr",
		slots: {
			a: { unit: "website-web.service", port: 3004 },
			b: { unit: "website-web-b.service", port: 3014 },
		},
		upstream: "website_web",
		previewUpstream: "website_preview",
		backendVariable: "website_backend",
		siteConf: "/etc/nginx/conf.d/rosegriffon.conf",
		upstreamOptions: "",
		publicUrl: "https://rosegriffon.fr/",
		probes: ["/api/health", "/", "/shop", "/chroniques", "/community"],
		extraDirs: [".next/static", "public"],
		linkedPaths: [],
		buildCommand: [BUN_BIN, "run", "build"],
		buildEnv: { SENTRY_SKIP_UPLOAD: "1", NEXT_TELEMETRY_DISABLED: "1" },
		memoryFloorMiB: 1024,
		bootTimeoutMs: 180_000,
	},
};

// ─────────────────────────────────────────────────────────────────────────────
// Journalisation — non bloquante : les écritures sont chaînées, jamais attendues
// ─────────────────────────────────────────────────────────────────────────────

let logFile: string | null = null;
let logChain: Promise<unknown> = Promise.resolve();
/** En sortie JSON, la trace part sur stderr : stdout ne doit contenir QUE le document. */
let logToStderr = false;

function stamp(): string {
	return new Date().toISOString().replace(/[:.]/gu, "-").replace("Z", "");
}

function log(line: string): void {
	const entry = `${new Date().toISOString()} ${line}`;
	(logToStderr ? process.stderr : process.stdout).write(`${entry}\n`);
	const destination = logFile;
	if (!destination) return;
	// Le journal ne doit ni bloquer la boucle d'événements, ni faire échouer un
	// déploiement : on sérialise les écritures et on avale leurs erreurs.
	logChain = logChain.then(() => appendFile(destination, `${entry}\n`)).catch(() => undefined);
}

/** Vide la file de journalisation (fin de commande). */
const flushLog = (): Promise<unknown> => logChain;

function fail(message: string): never {
	log(`✗ ${message}`);
	throw new Error(message);
}

// ─────────────────────────────────────────────────────────────────────────────
// Exécution de commandes
// ─────────────────────────────────────────────────────────────────────────────

interface RunResult {
	code: number;
	stdout: string;
	stderr: string;
	ms: number;
}

async function run(
	command: string[],
	options: { cwd?: string; timeoutMs?: number; env?: Record<string, string>; inherit?: boolean } = {},
): Promise<RunResult> {
	const started = Date.now();
	const proc = Bun.spawn(command, {
		// Repli seulement : tout appel qui dépend d'un dépôt passe `cwd: app.repoRoot`.
		cwd: options.cwd ?? NIERS_ROOT,
		env: { ...process.env, ...(options.env ?? {}) },
		stdout: options.inherit ? "inherit" : "pipe",
		stderr: options.inherit ? "inherit" : "pipe",
		stdin: "ignore",
	});
	const timer = options.timeoutMs ? setTimeout(() => proc.kill(9), options.timeoutMs) : undefined;
	const [stdout, stderr] = options.inherit
		? ["", ""]
		: await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text()]);
	const code = await proc.exited;
	if (timer) clearTimeout(timer);
	return { code, stdout, stderr, ms: Date.now() - started };
}

async function mustRun(command: string[], options?: Parameters<typeof run>[1]): Promise<RunResult> {
	const result = await run(command, options);
	if (result.code !== 0) {
		fail(`${command.join(" ")} → code ${result.code}\n${result.stderr || result.stdout}`.trim());
	}
	return result;
}

const sudo = (command: string[], options?: Parameters<typeof run>[1]) => run(["sudo", "-n", ...command], options);
const mustSudo = (command: string[], options?: Parameters<typeof run>[1]) =>
	mustRun(["sudo", "-n", ...command], options);

async function exists(path: string): Promise<boolean> {
	try {
		await stat(path);
		return true;
	} catch {
		return false;
	}
}

let compteurFichierTemporaire = 0;

/** Écrit un fichier appartenant à root (configuration nginx, unité systemd). */
async function writeAsRoot(destination: string, content: string, mode = "0644"): Promise<void> {
	compteurFichierTemporaire += 1;
	const temporary = `/tmp/rg-deploy-${process.pid}-${compteurFichierTemporaire}`;
	await writeFile(temporary, content);
	await mustSudo(["install", "-D", "-m", mode, "-o", "root", "-g", "root", temporary, destination]);
	await rm(temporary, { force: true });
}

// ─────────────────────────────────────────────────────────────────────────────
// Chemins et état
// ─────────────────────────────────────────────────────────────────────────────

const appRoot = (app: AppConfig) => `${RELEASES_ROOT}/${app.key}`;
const releasesDir = (app: AppConfig) => `${appRoot(app)}/releases`;
const slotLink = (app: AppConfig, slot: SlotId) => `${appRoot(app)}/slot-${slot}`;
const staticLink = (app: AppConfig) => `${appRoot(app)}/static`;
const statePath = (app: AppConfig) => `${appRoot(app)}/state.json`;
const liveEnvPath = (app: AppConfig) => `${appRoot(app)}/live.env`;
const lockPath = (app: AppConfig) => `${appRoot(app)}/deploy.lock`;
const releasePath = (app: AppConfig, release: string) => `${releasesDir(app)}/${release}`;
/** Racine applicative *dans* une version : c'est le `WorkingDirectory` du service. */
const releaseAppPath = (app: AppConfig, release: string) => `${releasePath(app, release)}/apps/${app.standaloneDir}`;

interface HistoryEntry {
	at: string;
	action: string;
	release: string;
	ok: boolean;
	note?: string;
}

interface PreviewState {
	release: string;
	token: string;
	startedAt: string;
}

interface DeployState {
	app: string;
	live: SlotId;
	release: string | null;
	previousRelease: string | null;
	slots: Record<SlotId, { release: string | null }>;
	preview: PreviewState | null;
	updatedAt: string;
	history: HistoryEntry[];
}

function emptyState(app: AppConfig): DeployState {
	return {
		app: app.key,
		live: "a",
		release: null,
		previousRelease: null,
		slots: { a: { release: null }, b: { release: null } },
		preview: null,
		updatedAt: new Date().toISOString(),
		history: [],
	};
}

async function readState(app: AppConfig): Promise<DeployState> {
	try {
		const parsed = JSON.parse(await readFile(statePath(app), "utf8")) as DeployState;
		return { ...emptyState(app), ...parsed };
	} catch {
		return emptyState(app);
	}
}

async function writeState(app: AppConfig, state: DeployState): Promise<void> {
	state.updatedAt = new Date().toISOString();
	state.history = state.history.slice(-30);
	await writeFile(statePath(app), `${JSON.stringify(state, null, "\t")}\n`);
	// Miroir lisible par un script shell (watchdog, synchro du miroir SQLite) : ces
	// consommateurs tournent en root, sans Bun, et doivent connaître le slot vivant.
	const slot = app.slots[state.live];
	await writeFile(
		liveEnvPath(app),
		[
			"# Généré par scripts/ops/deploy.ts — slot actuellement servi par nginx.",
			`LIVE_SLOT=${state.live}`,
			`LIVE_UNIT=${slot.unit}`,
			`LIVE_PORT=${slot.port}`,
			`LIVE_RELEASE=${state.release ?? ""}`,
			"",
		].join("\n"),
	);
}

// ─────────────────────────────────────────────────────────────────────────────
// Verrou : un seul déploiement à la fois par application
// ─────────────────────────────────────────────────────────────────────────────

async function acquireLock(app: AppConfig): Promise<() => Promise<void>> {
	const path = lockPath(app);
	for (let attempt = 0; attempt < 2; attempt += 1) {
		try {
			// `flag: "wx"` échoue si le fichier existe : c'est l'exclusion mutuelle.
			await writeFile(path, `${process.pid}\n`, { flag: "wx" });
			return async () => {
				await rm(path, { force: true });
			};
		} catch {
			const owner = Number.parseInt((await readFile(path, "utf8").catch(() => "0")).trim(), 10);
			let vivant = false;
			try {
				process.kill(owner, 0);
				vivant = owner > 0;
			} catch {
				vivant = false;
			}
			if (vivant) fail(`un déploiement de ${app.key} est déjà en cours (pid ${owner})`);
			log(`⚠ verrou orphelin (pid ${owner} mort) — reprise`);
			await rm(path, { force: true });
		}
	}
	fail(`verrou de ${app.key} impossible à acquérir`);
}

// ─────────────────────────────────────────────────────────────────────────────
// Mémoire
// ─────────────────────────────────────────────────────────────────────────────

interface MemorySnapshot {
	totalMiB: number;
	availableMiB: number;
}

async function readMemory(): Promise<MemorySnapshot> {
	if (process.platform === "win32") {
		return {
			totalMiB: Math.round(totalmem() / 1024 / 1024),
			availableMiB: Math.round(freemem() / 1024 / 1024),
		};
	}
	const meminfo = await readFile("/proc/meminfo", "utf8");
	const champ = (nom: string): number => {
		const ligne = meminfo.split("\n").find((entry) => entry.startsWith(`${nom}:`));
		return ligne ? Math.round(Number.parseInt(ligne.replace(/[^0-9]/gu, ""), 10) / 1024) : 0;
	};
	return { totalMiB: champ("MemTotal"), availableMiB: champ("MemAvailable") };
}

/** Mémoire réellement consommée par une unité systemd, en Mio (0 si arrêtée). */
async function unitMemoryMiB(unit: string, property = "MemoryCurrent"): Promise<number> {
	if (process.platform === "win32") return 0;
	try {
		const result = await run(["systemctl", "show", unit, `--property=${property}`, "--value"]);
		const brut = Number.parseInt(result.stdout.trim(), 10);
		return Number.isFinite(brut) && brut > 0 ? Math.round(brut / 1024 / 1024) : 0;
	} catch {
		return 0;
	}
}

/**
 * Refuse de démarrer une doublure si la machine n'a pas de quoi la loger : deux
 * instances Next coexistent le temps de la bascule, et un déploiement qui déclenche
 * l'OOM killer coûterait exactement ce qu'il prétend éviter.
 */
async function checkMemory(app: AppConfig, options: { allowLow: boolean }): Promise<void> {
	const [memory, empreinteLive] = await Promise.all([readMemory(), unitMemoryMiB(app.slots.a.unit)]);
	const besoin = Math.max(empreinteLive, app.memoryFloorMiB) + MEMORY_MARGIN_MIB;
	log(
		`mémoire : ${memory.availableMiB} Mio disponibles sur ${memory.totalMiB} Mio — ` +
			`la doublure en demande ~${besoin} Mio (slot vivant : ${empreinteLive} Mio)`,
	);
	if (memory.availableMiB >= besoin) return;
	if (options.allowLow) {
		log("⚠ mémoire insuffisante mais --allow-low-memory demandé — poursuite");
		return;
	}
	fail(
		`mémoire insuffisante pour une bascule sans coupure : ${memory.availableMiB} Mio disponibles, ` +
			`${besoin} Mio nécessaires. Libérer de la mémoire, ou forcer avec --allow-low-memory ` +
			`(le déploiement redeviendrait alors une simple relance, avec coupure).`,
	);
}

// ─────────────────────────────────────────────────────────────────────────────
// Garde-fous et construction
// ─────────────────────────────────────────────────────────────────────────────

async function typeCheck(app: AppConfig): Promise<void> {
	log(`type-check ${app.workspace}`);
	const result = await run([BUN_BIN, "--filter", app.workspace, "type-check"], {
		cwd: app.repoRoot,
		timeoutMs: 15 * 60_000,
	});
	if (result.code !== 0) {
		fail(`type-check ${app.workspace} en échec :\n${(result.stdout + result.stderr).slice(-4000)}`);
	}
	log(`✓ type-check ${app.workspace} (${Math.round(result.ms / 1000)} s)`);
}

async function buildApp(app: AppConfig): Promise<void> {
	const serverJs = `${app.appDir}/.next/standalone/apps/${app.standaloneDir}/server.js`;
	const buildIdPath = `${app.appDir}/.next/BUILD_ID`;

	for (let attempt = 1; attempt <= 3; attempt += 1) {
		const start = Date.now();
		log(`build ${app.key} — tentative ${attempt}/3`);
		const result = await run(app.buildCommand, {
			cwd: app.appDir,
			env: app.buildEnv,
			inherit: true,
			timeoutMs: 30 * 60_000,
		});
		// Le succès se juge sur les ARTEFACTS, pas sur le code de sortie : Next 16 sous
		// Bun sort en 132 (« Illegal instruction ») après un build pourtant complet, et
		// écrit parfois BUILD_ID sans émettre le standalone (faux positif inverse).
		const buildIdFresh =
			(await exists(buildIdPath)) && Bun.file(buildIdPath).lastModified >= start - 2_000;
		const standaloneOk = await exists(serverJs);
		if (buildIdFresh && standaloneOk) {
			if (result.code !== 0) log(`⚠ build OK malgré le code de sortie ${result.code} (artefacts présents)`);
			log(`✓ build ${app.key} en ${Math.round(result.ms / 1000)} s`);
			return;
		}
		log(`⚠ build incomplet (BUILD_ID frais=${buildIdFresh} server.js=${standaloneOk}, code ${result.code})`);
	}
	fail(`build ${app.key} réellement échoué après 3 tentatives`);
}

// ─────────────────────────────────────────────────────────────────────────────
// Assemblage d'une version
// ─────────────────────────────────────────────────────────────────────────────

async function assembleRelease(app: AppConfig): Promise<string> {
	const standalone = `${app.appDir}/.next/standalone`;
	const serverJs = `${standalone}/apps/${app.standaloneDir}/server.js`;
	const buildIdPath = `${app.appDir}/.next/BUILD_ID`;
	if (!(await exists(serverJs))) fail(`${serverJs} absent — lancer un build d'abord`);
	if (!(await exists(buildIdPath))) fail(`${buildIdPath} absent — lancer un build d'abord`);

	const buildId = (await readFile(buildIdPath, "utf8")).trim();
	const release = `${stamp()}-${buildId}`;
	const destination = releasePath(app, release);
	log(`assemblage de la version ${release}`);

	await rm(destination, { recursive: true, force: true });
	await mkdir(destination, { recursive: true });
	// Arborescence en liens physiques : même système de fichiers, coût quasi nul, et un
	// build ultérieur ne touche pas les inodes déjà publiés (version immuable).
	await mustRun(["cp", "-al", `${standalone}/.`, destination]);

	const appPath = releaseAppPath(app, release);
	for (const relative of app.extraDirs) {
		const source = `${app.appDir}/${relative}`;
		if (!(await exists(source))) {
			log(`⚠ ${relative} absent du dépôt — ignoré`);
			continue;
		}
		// Nettoyer les fichiers SQLite transients (WAL + SHM) avant hardlink,
		// qui ne peut pas les copier ("Operation not permitted").
		if (relative === "data") {
			const walFiles = await run([
				"find",
				source,
				"-type",
				"f",
				"(",
				"-name",
				"*.sqlite-wal",
				"-o",
				"-name",
				"*.sqlite-shm",
				")",
			]);
			if (walFiles.stdout.trim()) {
				for (const file of walFiles.stdout.trim().split("\n")) {
					await rm(file, { force: true });
				}
				log(`⚠ ${walFiles.stdout.trim().split("\n").length} fichier(s) SQLite WAL/SHM supprimés avant assemblage`);
			}
		}
		const target = `${appPath}/${relative}`;
		await rm(target, { recursive: true, force: true });
		await mkdir(target.slice(0, target.lastIndexOf("/")), { recursive: true });
		await mustRun(["cp", "-al", source, target]);
	}

	for (const relative of app.linkedPaths) {
		const target = `${appPath}/${relative}`;
		await rm(target, { recursive: true, force: true });
		await symlink(`${app.appDir}/${relative}`, target);
	}

	// Le mode standalone n'embarque pas `.next/cache` : le pré-créer évite un EACCES au
	// premier écrit ISR (le service n'a d'écriture que sous rg-releases).
	await mkdir(`${appPath}/.next/cache`, { recursive: true });

	// Depuis le dépôt de CETTE application : le wiki vit ici, le site vitrine dans `rg`.
	// Sans `cwd`, la version publiée portait la révision de l'autre dépôt.
	const [commit, branch] = await Promise.all([
		run(["git", "rev-parse", "HEAD"], { cwd: app.repoRoot }),
		run(["git", "rev-parse", "--abbrev-ref", "HEAD"], { cwd: app.repoRoot }),
	]);
	await writeFile(
		`${destination}/RELEASE.json`,
		`${JSON.stringify(
			{
				app: app.key,
				release,
				buildId,
				assembledAt: new Date().toISOString(),
				commit: commit.stdout.trim(),
				branch: branch.stdout.trim(),
			},
			null,
			"\t",
		)}\n`,
	);
	log(`✓ version ${release} assemblée`);
	return release;
}

/** Versions présentes, de la plus récente à la plus ancienne. */
async function listReleases(app: AppConfig): Promise<string[]> {
	try {
		const entries = await readdir(releasesDir(app), { withFileTypes: true });
		return entries
			.filter((entry) => entry.isDirectory())
			.map((entry) => entry.name)
			.sort()
			.reverse();
	} catch {
		return [];
	}
}

/**
 * Réservoir de fragments statiques : union des `.next/static` des dernières versions.
 * nginx sert `/_next/static/` depuis ce réservoir, si bien qu'un onglet ouvert avant le
 * déploiement retrouve ses fragments au lieu d'un 404 suivi d'un rechargement forcé.
 */
async function refreshStaticPool(app: AppConfig, keep: number): Promise<void> {
	const releases = (await listReleases(app)).slice(0, keep);
	const pool = `${appRoot(app)}/static-pool-${stamp()}`;
	await mkdir(pool, { recursive: true });
	for (const release of releases) {
		const source = `${releaseAppPath(app, release)}/.next/static`;
		if (!(await exists(source))) continue;
		// `-n` : pas d'écrasement, donc la version la plus récente (traitée en premier)
		// l'emporte. Les noms étant hachés par contenu, une collision serait de toute
		// façon un fichier identique.
		await run(["cp", "-aln", `${source}/.`, pool]);
	}
	const link = staticLink(app);
	const temporary = `${link}.tmp-${process.pid}`;
	await rm(temporary, { force: true });
	await symlink(pool, temporary);
	// Le renommage est atomique : nginx ne voit jamais de lien absent.
	await rename(temporary, link);
	const entries = await readdir(appRoot(app));
	await Promise.all(
		entries
			.filter((entry) => entry.startsWith("static-pool-") && `${appRoot(app)}/${entry}` !== pool)
			.map((entry) => rm(`${appRoot(app)}/${entry}`, { recursive: true, force: true })),
	);
	log(`✓ réservoir statique reconstruit depuis ${releases.length} version(s)`);
}

// ─────────────────────────────────────────────────────────────────────────────
// Slots
// ─────────────────────────────────────────────────────────────────────────────

async function pointSlot(app: AppConfig, slot: SlotId, release: string): Promise<void> {
	if (!(await exists(`${releaseAppPath(app, release)}/server.js`))) {
		fail(`version ${release} inutilisable : server.js absent`);
	}
	const link = slotLink(app, slot);
	const temporary = `${link}.tmp-${process.pid}`;
	await rm(temporary, { force: true });
	await symlink(releasePath(app, release), temporary);
	await rename(temporary, link);
	log(`slot ${slot} → ${release}`);
}

async function currentSlotRelease(app: AppConfig, slot: SlotId): Promise<string | null> {
	try {
		return (await readlink(slotLink(app, slot))).split("/").pop() ?? null;
	} catch {
		return null;
	}
}

async function unitActive(unit: string): Promise<boolean> {
	if (process.platform === "win32") return false;
	try {
		return (await run(["systemctl", "is-active", "--quiet", unit])).code === 0;
	} catch {
		return false;
	}
}

async function startSlot(app: AppConfig, slot: SlotId): Promise<void> {
	const { unit } = app.slots[slot];
	log(`démarrage de ${unit}`);
	const result = await sudo(["systemctl", "restart", unit], { timeoutMs: 180_000 });
	if (result.code !== 0) fail(`systemctl restart ${unit} → ${result.stderr || result.stdout}`);
}

async function stopSlot(app: AppConfig, slot: SlotId): Promise<void> {
	const { unit } = app.slots[slot];
	if (!(await unitActive(unit))) return;
	const pic = await unitMemoryMiB(unit, "MemoryPeak");
	log(`arrêt de ${unit}${pic > 0 ? ` (pic mémoire ${pic} Mio)` : ""}`);
	await sudo(["systemctl", "stop", unit], { timeoutMs: 120_000 });
	// Une doublure laissée en `failed` ferait passer un diagnostic de routine pour une
	// panne : on nettoie l'état d'échec après un arrêt volontaire.
	await sudo(["systemctl", "reset-failed", unit], { timeoutMs: 15_000 });
}

interface ProbeReport {
	ok: boolean;
	bootMs: number;
	results: { path: string; status: number; ms: number }[];
}

async function probeSlot(app: AppConfig, slot: SlotId): Promise<ProbeReport> {
	const { port, unit } = app.slots[slot];
	const base = `http://127.0.0.1:${port}`;
	const started = Date.now();
	const deadline = started + app.bootTimeoutMs;

	// Phase 1 — attendre un 200 sur /api/health (ouverture de la base, du miroir SQLite…).
	let healthy = false;
	while (Date.now() < deadline) {
		if (!(await unitActive(unit))) {
			const journal = await run(["journalctl", "-u", unit, "-n", "20", "--no-pager"]);
			fail(`${unit} s'est arrêtée pendant le démarrage :\n${journal.stdout}`);
		}
		try {
			const response = await fetch(`${base}/api/health`, { signal: AbortSignal.timeout(10_000) });
			if (response.status === 200) {
				healthy = true;
				break;
			}
		} catch {
			/* pas encore à l'écoute */
		}
		await Bun.sleep(500);
	}
	const bootMs = Date.now() - started;
	if (!healthy) {
		const journal = await run(["journalctl", "-u", unit, "-n", "30", "--no-pager"]);
		log(`✗ ${unit} n'a pas répondu 200 sur /api/health en ${Math.round(bootMs / 1000)} s`);
		log(journal.stdout);
		return { ok: false, bootMs, results: [] };
	}

	// Phase 2 — pages réelles, en parallèle : un build cassé répond 200 sur /api/health
	// (route isolée) et 500 sur tout ce qui touche la couche données.
	const results = await Promise.all(
		app.probes.map(async (path) => {
			const start = Date.now();
			try {
				const response = await fetch(`${base}${path}`, {
					redirect: "manual",
					signal: AbortSignal.timeout(30_000),
					headers: { "user-agent": "rg-deploy/1.0 (probe)" },
				});
				return { path, status: response.status, ms: Date.now() - start };
			} catch (error) {
				log(`✗ sonde ${path} : ${error instanceof Error ? error.message : String(error)}`);
				return { path, status: 0, ms: Date.now() - start };
			}
		}),
	);
	const ok = results.every((entry) => entry.status > 0 && entry.status < 500);
	const résumé = results.map((entry) => `${entry.path}=${entry.status}`).join(" ");
	const empreinte = await unitMemoryMiB(unit);
	log(
		`${ok ? "✓" : "✗"} slot ${slot} (:${port}) démarré en ${Math.round(bootMs / 1000)} s, ` +
			`${empreinte} Mio — ${résumé}`,
	);
	return { ok, bootMs, results };
}

// ─────────────────────────────────────────────────────────────────────────────
// nginx : amonts générés, bascule, drainage
// ─────────────────────────────────────────────────────────────────────────────

function upstreamContent(app: AppConfig, port: number): string {
	const options = app.upstreamOptions ? ` ${app.upstreamOptions}` : "";
	return [
		`# ${app.upstream} — généré par scripts/ops/deploy.ts, NE PAS ÉDITER À LA MAIN.`,
		"# Bascule bleu/vert : ce fichier est le seul endroit où nginx apprend quel slot sert.",
		`upstream ${app.upstream} { server 127.0.0.1:${port}${options}; keepalive 32; }`,
		"",
	].join("\n");
}

/**
 * Amont de prévisualisation + aiguillage par cookie. Sans prévisualisation active, la
 * table ne contient que la ligne par défaut : un visiteur porteur d'un vieux cookie
 * retombe donc sur la production au lieu d'un 502.
 */
function previewContent(app: AppConfig, preview: PreviewState | null): string {
	const lignes = [
		`# ${app.previewUpstream} — généré par scripts/ops/deploy.ts, NE PAS ÉDITER À LA MAIN.`,
		`upstream ${app.previewUpstream} { server 127.0.0.1:${app.slots.b.port} max_fails=0; keepalive 8; }`,
		"",
		`map $cookie_rg_preview $${app.backendVariable} {`,
		`\tdefault ${app.upstream};`,
	];
	if (preview) {
		lignes.push(`\t# prévisualisation ${preview.release} ouverte le ${preview.startedAt}`);
		lignes.push(`\t"${preview.token}" ${app.previewUpstream};`);
	}
	lignes.push("}", "");
	return lignes.join("\n");
}

/** Workers nginx actuels (enfants du maître). */
async function nginxWorkerPids(): Promise<number[]> {
	try {
		const master = Number.parseInt((await readFile("/run/nginx.pid", "utf8")).trim(), 10);
		if (!Number.isFinite(master)) return [];
		const { stdout } = await run(["pgrep", "-P", String(master)]);
		return stdout.split("\n").filter(Boolean).map((entry) => Number.parseInt(entry, 10));
	} catch {
		return [];
	}
}

/**
 * Recharge nginx puis attend la disparition des workers d'AVANT le rechargement.
 *
 * `systemctl reload` rend la main dès que le maître a reçu le signal, mais les anciens
 * workers finissent leurs connexions AVEC L'ANCIENNE CONFIGURATION, donc en proxyfiant
 * vers l'ancien port : toucher au slot sortant avant leur extinction produit des 502
 * (mesuré, 1 requête sur 77). On identifie les workers par PID plutôt que par leur titre
 * « shutting down » — le titre n'apparaît qu'après quelques millisecondes, et une sonde
 * trop précoce conclurait à tort que le drainage est fini.
 * `worker_shutdown_timeout` (posé par `install`) borne cette fenêtre à 30 s.
 */
async function reloadNginxAndDrain(maxMs = 90_000): Promise<number> {
	const avant = await nginxWorkerPids();
	const test = await sudo(["nginx", "-t"], { timeoutMs: 30_000 });
	if (test.code !== 0) fail(`nginx -t refuse la configuration :\n${test.stderr || test.stdout}`);
	const reload = await sudo(["systemctl", "reload", "nginx"], { timeoutMs: 60_000 });
	if (reload.code !== 0) fail(`rechargement nginx échoué :\n${reload.stderr || reload.stdout}`);

	const début = Date.now();
	while (Date.now() - début < maxMs) {
		const maintenant = new Set(await nginxWorkerPids());
		const survivants = avant.filter((pid) => maintenant.has(pid));
		if (survivants.length === 0) return Date.now() - début;
		await Bun.sleep(250);
	}
	log(`⚠ des workers nginx drainent encore après ${Math.round(maxMs / 1000)} s`);
	return -1;
}

async function switchTraffic(app: AppConfig, slot: SlotId): Promise<void> {
	const { port } = app.slots[slot];
	await writeAsRoot(`${NGINX_UPSTREAM_DIR}/${app.upstream}.conf`, upstreamContent(app, port));
	const drain = await reloadNginxAndDrain();
	log(`✓ trafic basculé sur le slot ${slot} (:${port})${drain >= 0 ? ` — drainage ${drain} ms` : ""}`);
}

async function verifyPublic(app: AppConfig): Promise<number> {
	try {
		const response = await fetch(app.publicUrl, {
			redirect: "manual",
			signal: AbortSignal.timeout(20_000),
			headers: { "user-agent": "rg-deploy/1.0 (verification)" },
		});
		log(`${response.status < 400 ? "✓" : "✗"} ${app.publicUrl} → ${response.status}`);
		return response.status;
	} catch (error) {
		log(`✗ ${app.publicUrl} injoignable : ${error instanceof Error ? error.message : String(error)}`);
		return 0;
	}
}

// ─────────────────────────────────────────────────────────────────────────────
// Publication d'une version (cœur commun à deploy / reload / rollback / promote)
// ─────────────────────────────────────────────────────────────────────────────

interface PublishOptions {
	drainSeconds: number;
	keep: number;
	action: string;
	allowLowMemory: boolean;
}

/**
 * Publie `release` sans coupure : doublure B → bascule → slot A → re-bascule.
 * L'état stable reste « A seul debout », identique à la topologie historique.
 */
async function publishRelease(app: AppConfig, release: string, options: PublishOptions): Promise<boolean> {
	const state = await readState(app);
	const previousRelease = state.release;

	// Le garde-fou mémoire est rejoué ICI : entre les vérifications d'entrée et ce point
	// il s'est écoulé un build entier, pendant lequel la machine a pu se remplir.
	await checkMemory(app, { allowLow: options.allowLowMemory });

	// 1. La doublure prend la nouvelle version pendant que A continue de servir.
	await pointSlot(app, "b", release);
	state.slots.b.release = release;
	await startSlot(app, "b");
	const probeB = await probeSlot(app, "b");
	if (!probeB.ok) {
		await stopSlot(app, "b");
		state.history.push({
			at: new Date().toISOString(),
			action: options.action,
			release,
			ok: false,
			note: "doublure KO, production intacte",
		});
		await writeState(app, state);
		fail(`la doublure refuse la version ${release} — production intacte sur le slot ${state.live}`);
	}

	// 2. Bascule : la production sert le nouveau code. Aucun instant sans amont vivant.
	await switchTraffic(app, "b");
	state.live = "b";
	state.release = release;
	state.previousRelease = previousRelease;
	await writeState(app, state);
	await verifyPublic(app);

	// 3. Le slot de production reprend la même version, sondé avant de récupérer le trafic.
	await pointSlot(app, "a", release);
	state.slots.a.release = release;
	await startSlot(app, "a");
	const probeA = await probeSlot(app, "a");
	if (!probeA.ok) {
		log("⚠ le slot A refuse la version — la production reste sur la doublure B (dégradé, pas coupé)");
		state.history.push({
			at: new Date().toISOString(),
			action: options.action,
			release,
			ok: false,
			note: "production maintenue sur le slot B",
		});
		await writeState(app, state);
		return false;
	}

	// 4. Retour sur A, drainage, extinction de la doublure.
	await switchTraffic(app, "a");
	state.live = "a";
	await writeState(app, state);
	log(`drainage applicatif ${options.drainSeconds} s avant l'arrêt de la doublure`);
	await Bun.sleep(options.drainSeconds * 1_000);
	await stopSlot(app, "b");

	const status = await verifyPublic(app);
	const ok = status > 0 && status < 400;
	state.history.push({ at: new Date().toISOString(), action: options.action, release, ok });
	await writeState(app, state);
	await pruneReleases(app, options.keep);
	return ok;
}

async function pruneReleases(app: AppConfig, keep: number): Promise<void> {
	const releases = await listReleases(app);
	const gardées = new Set(releases.slice(0, keep));
	for (const slot of ["a", "b"] as SlotId[]) {
		const current = await currentSlotRelease(app, slot);
		if (current) gardées.add(current);
	}
	const state = await readState(app);
	if (state.release) gardées.add(state.release);
	if (state.previousRelease) gardées.add(state.previousRelease);
	if (state.preview) gardées.add(state.preview.release);
	for (const release of releases) {
		if (gardées.has(release)) continue;
		await rm(releasePath(app, release), { recursive: true, force: true });
		log(`purge de la version ${release}`);
	}
}

// ─────────────────────────────────────────────────────────────────────────────
// Commandes
// ─────────────────────────────────────────────────────────────────────────────

interface Flags {
	build: boolean;
	gate: boolean;
	drainSeconds: number;
	keep: number;
	to?: string;
	json: boolean;
	allowLowMemory: boolean;
}

async function preflight(app: AppConfig, flags: Flags): Promise<void> {
	for (const slot of ["a", "b"] as SlotId[]) {
		const { unit } = app.slots[slot];
		const loaded = await run(["systemctl", "show", unit, "--property=LoadState", "--value"]);
		if (loaded.stdout.trim() === "not-found") {
			fail(`unité ${unit} absente — lancer d'abord : bun scripts/ops/deploy.ts install ${app.key}`);
		}
	}
	if (!(await exists(`${NGINX_UPSTREAM_DIR}/${app.upstream}.conf`))) {
		fail(`amont nginx ${app.upstream} non installé — lancer : bun scripts/ops/deploy.ts install ${app.key}`);
	}
	const free = await run(["df", "--output=avail", "-BG", RELEASES_ROOT]);
	const gigas = Number.parseInt(free.stdout.split("\n")[1]?.trim().replace("G", "") ?? "0", 10);
	if (gigas > 0 && gigas < 5) fail(`espace disque insuffisant (${gigas} Go libres sous ${RELEASES_ROOT})`);
	const test = await sudo(["nginx", "-t"], { timeoutMs: 30_000 });
	if (test.code !== 0) fail(`configuration nginx déjà invalide avant déploiement :\n${test.stderr}`);
	await checkMemory(app, { allowLow: flags.allowLowMemory });
}

/** Termine une prévisualisation : le slot B est sur le point d'être réquisitionné. */
async function closePreview(app: AppConfig, raison: string): Promise<void> {
	const state = await readState(app);
	if (!state.preview) return;
	log(`prévisualisation ${state.preview.release} close (${raison})`);
	state.preview = null;
	await writeState(app, state);
	await writeAsRoot(`${NGINX_UPSTREAM_DIR}/${app.previewUpstream}.conf`, previewContent(app, null));
	await reloadNginxAndDrain();
}

async function commandDeploy(app: AppConfig, flags: Flags): Promise<boolean> {
	const unlock = await acquireLock(app);
	try {
		log(`── déploiement ${app.title} ──`);
		await preflight(app, flags);
		if (flags.gate) await typeCheck(app);
		if (flags.build) await buildApp(app);
		await closePreview(app, "le slot B sert de doublure de bascule");
		const release = await assembleRelease(app);
		await refreshStaticPool(app, flags.keep);
		return await publishRelease(app, release, {
			drainSeconds: flags.drainSeconds,
			keep: flags.keep,
			allowLowMemory: flags.allowLowMemory,
			action: "deploy",
		});
	} finally {
		await unlock();
	}
}

async function commandReload(app: AppConfig, flags: Flags): Promise<boolean> {
	const unlock = await acquireLock(app);
	try {
		const state = await readState(app);
		const release = state.release ?? (await currentSlotRelease(app, "a"));
		if (!release) fail(`aucune version publiée pour ${app.key} — lancer un déploiement`);
		log(`── relance sans coupure de ${app.title} (version ${release}) ──`);
		await preflight(app, flags);
		await closePreview(app, "le slot B sert de doublure de bascule");
		return await publishRelease(app, release, {
			drainSeconds: flags.drainSeconds,
			keep: flags.keep,
			allowLowMemory: flags.allowLowMemory,
			action: "reload",
		});
	} finally {
		await unlock();
	}
}

async function commandRollback(app: AppConfig, flags: Flags): Promise<boolean> {
	const unlock = await acquireLock(app);
	try {
		const state = await readState(app);
		const candidates = await listReleases(app);
		const target = flags.to ?? state.previousRelease ?? candidates.find((entry) => entry !== state.release);
		if (!target) fail(`aucune version antérieure disponible pour ${app.key}`);
		if (!(await exists(releasePath(app, target)))) fail(`version ${target} introuvable sur disque`);
		log(`── retour arrière ${app.title} : ${state.release ?? "?"} → ${target} ──`);
		await preflight(app, flags);
		await closePreview(app, "le slot B sert de doublure de bascule");
		await refreshStaticPool(app, flags.keep);
		return await publishRelease(app, target, {
			drainSeconds: flags.drainSeconds,
			keep: flags.keep,
			allowLowMemory: flags.allowLowMemory,
			action: "rollback",
		});
	} finally {
		await unlock();
	}
}

/** Publie une version sur le slot B, joignable par cookie, sans toucher à la production. */
async function commandPreview(app: AppConfig, flags: Flags): Promise<boolean> {
	const unlock = await acquireLock(app);
	try {
		log(`── prévisualisation ${app.title} ──`);
		await preflight(app, flags);
		if (flags.gate) await typeCheck(app);
		if (flags.build) await buildApp(app);
		const release = flags.to ?? (await assembleRelease(app));
		if (!(await exists(releasePath(app, release)))) fail(`version ${release} introuvable`);
		await refreshStaticPool(app, flags.keep);
		await pointSlot(app, "b", release);
		await startSlot(app, "b");
		const probe = await probeSlot(app, "b");
		if (!probe.ok) {
			await stopSlot(app, "b");
			fail(`la version ${release} ne démarre pas — prévisualisation abandonnée`);
		}

		const token = crypto.randomUUID().replaceAll("-", "").slice(0, 16);
		const state = await readState(app);
		state.preview = { release, token, startedAt: new Date().toISOString() };
		state.slots.b.release = release;
		await writeState(app, state);
		await writeAsRoot(`${NGINX_UPSTREAM_DIR}/${app.previewUpstream}.conf`, previewContent(app, state.preview));
		await reloadNginxAndDrain();

		const url = `${app.publicUrl.replace(/\/$/u, "")}/__preview?token=${token}`;
		log(`✓ prévisualisation ${release} en ligne — ouvrir : ${url}`);
		log(`  (sortir : ${app.publicUrl.replace(/\/$/u, "")}/__preview?off=1 · arrêter : deploy.ts preview-off ${app.key})`);
		log("  la production n'a pas bougé ; `promote` publiera cette version.");
		return true;
	} finally {
		await unlock();
	}
}

async function commandPreviewOff(app: AppConfig): Promise<boolean> {
	const unlock = await acquireLock(app);
	try {
		await closePreview(app, "demande explicite");
		const state = await readState(app);
		if (state.live === "b") {
			log("⚠ la production tourne actuellement sur le slot B : il n'est pas arrêté.");
			return false;
		}
		await stopSlot(app, "b");
		log("✓ prévisualisation arrêtée, mémoire rendue");
		return true;
	} finally {
		await unlock();
	}
}

/** Passe en production la version actuellement en prévisualisation. */
async function commandPromote(app: AppConfig, flags: Flags): Promise<boolean> {
	const unlock = await acquireLock(app);
	try {
		const state = await readState(app);
		const release = flags.to ?? state.preview?.release ?? (await currentSlotRelease(app, "b"));
		if (!release) fail(`aucune prévisualisation à promouvoir pour ${app.key}`);
		if (!(await exists(releasePath(app, release)))) fail(`version ${release} introuvable sur disque`);
		log(`── promotion ${app.title} : ${release} ──`);
		await preflight(app, flags);
		await closePreview(app, "promotion en production");
		await refreshStaticPool(app, flags.keep);
		return await publishRelease(app, release, {
			drainSeconds: flags.drainSeconds,
			keep: flags.keep,
			allowLowMemory: flags.allowLowMemory,
			action: "promote",
		});
	} finally {
		await unlock();
	}
}

async function commandStatus(apps: AppConfig[], flags: Flags): Promise<boolean> {
	const memory = await readMemory();
	const rapport = await Promise.all(
		apps.map(async (app) => {
			const state = await readState(app);
			const slots: Record<string, unknown> = {};
			for (const slot of ["a", "b"] as SlotId[]) {
				const { unit, port } = app.slots[slot];
				slots[slot] = {
					unit,
					port,
					active: await unitActive(unit),
					memoryMiB: await unitMemoryMiB(unit),
					release: await currentSlotRelease(app, slot),
					live: state.live === slot,
				};
			}
			return {
				app: app.key,
				// Un verrou présent = publication en cours : c'est ce qui permet à un
				// pilote extérieur (CI, agent MCP) de suivre une publication détachée.
				running: await exists(lockPath(app)),
				live: state.live,
				release: state.release,
				previousRelease: state.previousRelease,
				preview: state.preview,
				updatedAt: state.updatedAt,
				releases: (await listReleases(app)).length,
				slots,
				publicUrl: app.publicUrl,
				publicStatus: await verifyPublic(app),
				lastActions: state.history.slice(-3),
			};
		}),
	);

	if (flags.json) {
		process.stdout.write(`${JSON.stringify({ memory, apps: rapport }, null, "\t")}\n`);
	} else {
		process.stdout.write(`mémoire : ${memory.availableMiB} Mio disponibles / ${memory.totalMiB} Mio\n`);
		for (const entry of rapport) {
			const slots = entry.slots as Record<
				string,
				{ unit: string; port: number; active: boolean; release: string | null; live: boolean; memoryMiB: number }
			>;
			process.stdout.write(`${entry.app} — version ${entry.release ?? "aucune"} (slot ${entry.live})\n`);
			for (const [id, slot] of Object.entries(slots)) {
				process.stdout.write(
					`  slot ${id} :${slot.port} ${slot.unit.padEnd(22)} ${(slot.active ? "actif" : "arrêté").padEnd(7)}` +
						`${slot.live ? "← trafic" : "        "} ${String(slot.memoryMiB).padStart(5)} Mio  ${slot.release ?? "-"}\n`,
				);
			}
			if (entry.running) process.stdout.write("  publication en cours\n");
			if (entry.preview) process.stdout.write(`  prévisualisation : ${entry.preview.release}\n`);
			process.stdout.write(`  public ${entry.publicUrl} → ${entry.publicStatus}\n`);
			process.stdout.write(
				`  versions sur disque : ${entry.releases} · précédente : ${entry.previousRelease ?? "-"}\n`,
			);
		}
	}
	return rapport.every((entry) => entry.publicStatus > 0 && entry.publicStatus < 400);
}

async function commandReleases(apps: AppConfig[]): Promise<boolean> {
	for (const app of apps) {
		const state = await readState(app);
		process.stdout.write(`${app.key} :\n`);
		for (const release of await listReleases(app)) {
			const marques = [
				release === state.release ? "production" : "",
				release === state.preview?.release ? "prévisualisation" : "",
				release === state.previousRelease ? "précédente" : "",
			].filter(Boolean);
			let meta = "";
			try {
				const info = JSON.parse(await readFile(`${releasePath(app, release)}/RELEASE.json`, "utf8"));
				meta = ` ${String(info.commit).slice(0, 8)} (${info.branch})`;
			} catch {
				/* version assemblée hors dépôt git */
			}
			process.stdout.write(`  ${release}${meta}${marques.length ? ` ← ${marques.join(", ")}` : ""}\n`);
		}
	}
	return true;
}

// ─────────────────────────────────────────────────────────────────────────────
// Installation (idempotente) : arborescence, unités systemd, amonts nginx
// ─────────────────────────────────────────────────────────────────────────────

async function installUnit(app: AppConfig, slot: SlotId): Promise<void> {
	const { unit } = app.slots[slot];
	const source = `${app.repoRoot}/${app.unitDir}/${unit}`;
	if (!(await exists(source))) fail(`unité manquante dans le dépôt : ${source}`);
	await writeAsRoot(`/etc/systemd/system/${unit}`, await readFile(source, "utf8"));
	await mustSudo(["systemctl", "daemon-reload"]);
	log(`unité ${unit} installée`);
}

async function installApp(app: AppConfig, flags: Flags): Promise<boolean> {
	const unlock = await acquireLock(app);
	try {
		log(`── installation de la mécanique bleu/vert pour ${app.title} ──`);
		await Promise.all(
			[releasesDir(app), `${appRoot(app)}/bun-cache`, `${appRoot(app)}/bun-cache-b`, `${appRoot(app)}/logs`].map(
				(path) => mkdir(path, { recursive: true }),
			),
		);
		await ensureNginxShutdownTimeout();

		// 1. Version d'amorçage à partir des artefacts déjà bâtis : les unités doivent
		//    trouver un `slot-a` valide dès leur installation (le watchdog peut
		//    redémarrer le service à n'importe quel moment).
		const initial = await readState(app);
		let release = initial.release;
		if (!release || !(await exists(releasePath(app, release)))) {
			release = await assembleRelease(app);
			initial.release = release;
			initial.slots.a.release = release;
			initial.slots.b.release = release;
			await writeState(app, initial);
		}
		await pointSlot(app, "a", release);
		await pointSlot(app, "b", release);
		await refreshStaticPool(app, flags.keep);

		// 2. La doublure d'abord : on prouve que la disposition par versions démarre AVANT
		//    de réécrire l'unité de production, qui continue de tourner sur l'ancienne.
		await installUnit(app, "b");
		await startSlot(app, "b");
		const probe = await probeSlot(app, "b");
		await stopSlot(app, "b");
		if (!probe.ok) fail("la disposition par versions ne démarre pas — unité de production laissée intacte");

		// 3. Unité de production. Le processus en cours n'est pas affecté : le nouveau
		//    fichier ne prendra effet qu'à son prochain démarrage, orchestré par `deploy`.
		await installUnit(app, "a");
		await mustSudo(["systemctl", "enable", app.slots.a.unit]);
		await retireLegacyDropIns(app);

		// 4. Amonts nginx : on garde le port actuellement servi, donc rien ne bouge pour
		//    les visiteurs ; seule la façon dont nginx apprend ce port change.
		const state = await readState(app);
		const port = app.slots[state.live].port;
		await writeAsRoot(`${NGINX_UPSTREAM_DIR}/${app.upstream}.conf`, upstreamContent(app, port));
		await writeAsRoot(`${NGINX_UPSTREAM_DIR}/${app.previewUpstream}.conf`, previewContent(app, state.preview));
		await patchSiteConf(app);
		await reloadNginxAndDrain();
		await verifyPublic(app);
		log(`✓ ${app.key} prêt pour le déploiement sans coupure (slot vivant : ${state.live}, :${port})`);
		return true;
	} finally {
		await unlock();
	}
}

/**
 * Sans `worker_shutdown_timeout`, un worker nginx qui draine peut vivre aussi longtemps
 * que sa connexion cliente (SSE, keepalive) : la bascule attendrait indéfiniment.
 */
async function ensureNginxShutdownTimeout(): Promise<void> {
	const content = await readFile(NGINX_MAIN_CONF, "utf8");
	if (content.includes("worker_shutdown_timeout")) return;
	const ligne = [
		"# Borne la survie des workers qui drainent après un `reload` : le déploiement",
		"# bleu/vert attend leur extinction avant de toucher au slot sortant (sinon un",
		"# ancien worker proxifie encore vers l'ancien port → 502).",
		"worker_shutdown_timeout 30s;",
		"",
	].join("\n");
	const modifié = content.replace(/^(worker_processes .*\n)/mu, `$1${ligne}`);
	if (modifié === content) {
		log("⚠ impossible de poser worker_shutdown_timeout automatiquement — à ajouter à la main");
		return;
	}
	await mustSudo(["cp", "-a", NGINX_MAIN_CONF, `${NGINX_MAIN_CONF}.bak-avant-bleu-vert`]);
	await writeAsRoot(NGINX_MAIN_CONF, modifié);
	const test = await sudo(["nginx", "-t"], { timeoutMs: 30_000 });
	if (test.code !== 0) {
		await mustSudo(["cp", "-a", `${NGINX_MAIN_CONF}.bak-avant-bleu-vert`, NGINX_MAIN_CONF]);
		fail(`nginx -t refuse worker_shutdown_timeout — configuration restaurée :\n${test.stderr}`);
	}
	log("worker_shutdown_timeout 30s posé dans nginx.conf");
}

/**
 * Instrumente le fichier de site : amont par inclusion, alias statique vers le
 * réservoir, aiguillage par cookie et point d'entrée `/__preview`. Idempotent.
 */
async function patchSiteConf(app: AppConfig): Promise<void> {
	if (!(await exists(app.siteConf))) fail(`fichier de site nginx introuvable : ${app.siteConf}`);
	const original = await readFile(app.siteConf, "utf8");
	let content = original;

	const inclusion = `include ${NGINX_UPSTREAM_DIR}/${app.upstream}.conf;`;
	const inclusionPreview = `include ${NGINX_UPSTREAM_DIR}/${app.previewUpstream}.conf;`;
	const declaration = new RegExp(`^\\s*upstream\\s+${app.upstream}\\s*\\{[^}]*\\}\\s*$`, "mu");
	if (declaration.test(content)) {
		content = content.replace(
			declaration,
			`# Amont piloté par scripts/ops/deploy.ts (bascule bleu/vert sans coupure).\n${inclusion}\n${inclusionPreview}`,
		);
	} else if (!content.includes(inclusion)) {
		fail(`amont ${app.upstream} introuvable dans ${app.siteConf} — instrumentation manuelle requise`);
	} else if (!content.includes(inclusionPreview)) {
		content = content.replace(inclusion, `${inclusion}\n${inclusionPreview}`);
	}

	const ancienAlias = `alias /home/ubuntu/rg/apps/${app.key}/.next/standalone/apps/${app.standaloneDir}/.next/static/;`;
	const nouvelAlias = `alias ${staticLink(app)}/;`;
	if (content.includes(ancienAlias)) content = content.replace(ancienAlias, nouvelAlias);

	// Aiguillage par cookie : `proxy_pass` sur une variable est résolu à chaque requête
	// contre les groupes d'amont déclarés. Sans cookie, la variable vaut l'amont de
	// production — le comportement par défaut est donc rigoureusement inchangé.
	content = content.replaceAll(`proxy_pass http://${app.upstream};`, `proxy_pass http://$${app.backendVariable};`);

	if (!content.includes("location = /__preview")) {
		const bloc = [
			"",
			"    # Prévisualisation : pose (ou retire) le cookie qui aiguille vers le slot B.",
			"    # Généré par scripts/ops/deploy.ts.",
			"    location = /__preview {",
			'        if ($arg_off) {',
			'            add_header Set-Cookie "rg_preview=; Path=/; Max-Age=0; SameSite=Lax" always;',
			"            return 302 /;",
			"        }",
			'        add_header Set-Cookie "rg_preview=$arg_token; Path=/; Secure; HttpOnly; SameSite=Lax" always;',
			'        add_header Cache-Control "no-store" always;',
			"        return 302 /;",
			"    }",
		].join("\n");
		content = content.replace(/\n(\s*)location \/ \{/u, `${bloc}\n$1location / {`);
	}

	if (content === original) {
		log(`${app.siteConf} déjà instrumenté`);
		return;
	}
	const sauvegarde = `${app.siteConf}.bak-bleu-vert-${stamp()}`;
	await mustSudo(["cp", "-a", app.siteConf, sauvegarde]);
	await writeAsRoot(app.siteConf, content);
	const test = await sudo(["nginx", "-t"], { timeoutMs: 30_000 });
	if (test.code !== 0) {
		await mustSudo(["cp", "-a", sauvegarde, app.siteConf]);
		fail(`nginx -t refuse l'instrumentation de ${app.siteConf} — configuration restaurée :\n${test.stderr}`);
	}
	log(`${app.siteConf} instrumenté (sauvegarde : ${sauvegarde})`);
}

/**
 * Les drop-ins historiques de /etc (mémoire, résilience, épinglage du miroir) sont
 * désormais fusionnés dans l'unité tenue en dépôt. Les laisser ferait vivre deux sources
 * de vérité qui divergeraient au premier réglage.
 */
async function retireLegacyDropIns(app: AppConfig): Promise<void> {
	const directory = `/etc/systemd/system/${app.slots.a.unit}.d`;
	if (!(await exists(directory))) return;
	const sauvegarde = `${appRoot(app)}/drop-ins-retires-${stamp()}`;
	await mustSudo(["cp", "-a", directory, sauvegarde]);
	await mustSudo(["rm", "-rf", directory]);
	await mustSudo(["systemctl", "daemon-reload"]);
	log(`drop-ins ${directory} retirés (fusionnés dans l'unité ; sauvegarde : ${sauvegarde})`);
}

// ─────────────────────────────────────────────────────────────────────────────
// Point d'entrée
// ─────────────────────────────────────────────────────────────────────────────

const USAGE = `Usage : bun scripts/ops/deploy.ts <commande> [azalee|website|all] [options]

Commandes
  deploy       type-check, build, publie et bascule sans coupure (défaut)
  preview      publie sur le slot B, joignable par cookie, sans toucher à la production
  promote      passe la prévisualisation en production
  preview-off  arrête la prévisualisation et rend la mémoire
  reload       republie la version courante sans coupure (après un swap de miroir)
  rollback     revient à la version précédente (--to=<version> pour viser)
  status       état des slots, versions, mémoire, santé publique
  releases     versions présentes sur disque
  install      (ré)installe unités systemd + amonts nginx (idempotent)

Options
  --no-build            publie les artefacts déjà bâtis
  --no-gate             saute le type-check préalable
  --drain=<secondes>    attente avant extinction de la doublure (défaut ${DEFAULT_DRAIN_SECONDS})
  --keep=<n>            versions conservées (défaut ${DEFAULT_KEEP})
  --to=<version>        cible d'un rollback / d'une promotion
  --allow-low-memory    passe outre le garde-fou mémoire
  --json                sortie JSON (status)
`;

async function main(): Promise<number> {
	const argv = process.argv.slice(2);
	const positional = argv.filter((argument) => !argument.startsWith("--"));
	const options = argv.filter((argument) => argument.startsWith("--"));

	const commands = new Set([
		"deploy",
		"preview",
		"promote",
		"preview-off",
		"reload",
		"rollback",
		"status",
		"releases",
		"install",
		"help",
	]);
	const command = commands.has(positional[0] ?? "") ? (positional.shift() as string) : "deploy";
	if (command === "help" || options.includes("--help")) {
		process.stdout.write(USAGE);
		return 0;
	}

	const cible = positional[0] ?? "all";
	const apps = cible === "all" ? Object.values(APPS) : [APPS[cible]];
	if (apps.some((app) => !app)) {
		process.stderr.write(`Application inconnue : ${cible}\n${USAGE}`);
		return 2;
	}

	const flag = (name: string): string | undefined =>
		options.find((option) => option.startsWith(`--${name}=`))?.split("=").slice(1).join("=");
	const flags: Flags = {
		build: !options.includes("--no-build"),
		gate: !options.includes("--no-gate"),
		drainSeconds: Number.parseInt(flag("drain") ?? String(DEFAULT_DRAIN_SECONDS), 10),
		keep: Number.parseInt(flag("keep") ?? String(DEFAULT_KEEP), 10),
		to: flag("to"),
		json: options.includes("--json"),
		allowLowMemory: options.includes("--allow-low-memory"),
	};

	logToStderr = flags.json;
	if (command === "status") return (await commandStatus(apps, flags)) ? 0 : 1;
	if (command === "releases") return (await commandReleases(apps)) ? 0 : 1;

	await mkdir(RELEASES_ROOT, { recursive: true });
	let ok = true;
	for (const app of apps) {
		await mkdir(`${appRoot(app)}/logs`, { recursive: true });
		logFile = `${appRoot(app)}/logs/${command}-${stamp()}.log`;
		try {
			if (command === "install") ok = (await installApp(app, flags)) && ok;
			else if (command === "reload") ok = (await commandReload(app, flags)) && ok;
			else if (command === "rollback") ok = (await commandRollback(app, flags)) && ok;
			else if (command === "preview") ok = (await commandPreview(app, flags)) && ok;
			else if (command === "preview-off") ok = (await commandPreviewOff(app)) && ok;
			else if (command === "promote") ok = (await commandPromote(app, flags)) && ok;
			else ok = (await commandDeploy(app, flags)) && ok;
			log(`journal : ${logFile}`);
		} catch (error) {
			log(`✗ ${app.key} : ${error instanceof Error ? error.message : String(error)}`);
			log(`journal : ${logFile}`);
			ok = false;
		} finally {
			await flushLog();
			logFile = null;
			// Rétention : 30 derniers journaux.
			const journaux = (await readdir(`${appRoot(app)}/logs`)).sort();
			await Promise.all(
				journaux
					.slice(0, Math.max(0, journaux.length - 30))
					.map((nom) => rm(`${appRoot(app)}/logs/${nom}`, { force: true })),
			);
		}
	}
	return ok ? 0 : 1;
}

process.exit(await main());
