/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { runInaglePush, runSQLiteBackup } from "./tasks/db";
import { syncCdnAssets } from "./tasks/cdn";
import { runIeCrawl } from "./tasks/ie-crawl";
import { crawlTwitterSearch } from "./tasks/ie-crawl/harvest-search";
import { crawlTwitter } from "./tasks/ie-crawl/twitter";
import { crawlHashtagCampaigns } from "./tasks/ie-crawl/hashtag-harvest";
import { recolterCreationsDiscord } from "./tasks/campagnes-discord";
import { revaliderCreationsInstagram } from "./tasks/campagnes-instagram";
import { relayerCampagnesDiscord } from "./tasks/campagnes-relais-discord";
import { runRagSync, queryRag, ragGroundedQuery, ingestWikiCorpus } from "./tasks/ie-crawl/rag-index";
import {
	triggerPublishScheduled,
	triggerPatreonRefresh,
	triggerPatreonReminders,
	warmCaches,
	triggerGithubPublishWorkflow,
} from "./tasks/api";
import { runSeoIndexNow } from "./tasks/seo/indexnow";
import { runSeoLlmsTxt } from "./tasks/seo/llms-txt";
import { collecterAudienceAchillea } from "./tasks/stats/achillea";
import { rafraichirVideosTechniques } from "./tasks/zukan-videos";
import { initDiscord, getDiscordTelemetry, onTelemetryUpdate, getDiscordClient, GUILD_ID } from "./lib/discord";
import { runDiscordSync, runDiscordChannelScan } from "./tasks/discord";
import {
	armerSalonsDeCategories,
	armerSalonsDeDepot,
	runDiscordMessagesSync,
	runDiscordMessagesBackfill,
	brancherVeilleMessagesTempsReel,
} from "./tasks/discord-messages";
import { runDiscordPollsImport } from "./tasks/discord-polls";
import { runNoctalyImport } from "./tasks/discord-noctaly";
import { ServerWebSocket } from "bun";
import { CATALOGUE_TACHES, PLANIFICATIONS, TACHES_CRON } from "@rosegriffon/types/cron";
import { createSupabaseServiceClient } from "@rosegriffon/db/service";
import { resolveProfile, isAdmin } from "@rosegriffon/auth";
import { getOpenApiDocument, getSwaggerHtml } from "./lib/openapi";
import { demarrerPontBot } from "./lib/bot-signal";
import { registreExecutions, type OrigineExecution } from "./lib/executions";
import { enregistrerExecutionEnBase } from "./lib/executions-postgres";
import { signalerDecisionAlerte } from "./lib/alertes-cron";

// ─── LOGGER GLOBALE INTERCEPTION ET STREAMING WEBSOCKET ──────────────────────
const wsConnections = new Set<ServerWebSocket<any>>();

const originalLog = console.log;
const originalError = console.error;
const originalWarn = console.warn;

function broadcastLog(level: "info" | "warn" | "error", message: string) {
	const payload = JSON.stringify({
		type: "log",
		level,
		text: message,
		timestamp: Date.now(),
	});
	for (const ws of wsConnections) {
		try {
			ws.send(payload);
		} catch {
			wsConnections.delete(ws);
		}
	}
}

console.log = function (message?: any, ...optionalParams: any[]) {
	const str =
		String(message) +
		(optionalParams.length
			? " " +
				optionalParams.map((p) => (typeof p === "object" ? JSON.stringify(p) : String(p))).join(" ")
			: "");
	originalLog(message, ...optionalParams);
	broadcastLog("info", str);
};

console.error = function (message?: any, ...optionalParams: any[]) {
	const str =
		String(message) +
		(optionalParams.length
			? " " +
				optionalParams.map((p) => (typeof p === "object" ? JSON.stringify(p) : String(p))).join(" ")
			: "");
	originalError(message, ...optionalParams);
	broadcastLog("error", str);
};

console.warn = function (message?: any, ...optionalParams: any[]) {
	const str =
		String(message) +
		(optionalParams.length
			? " " +
				optionalParams.map((p) => (typeof p === "object" ? JSON.stringify(p) : String(p))).join(" ")
			: "");
	originalWarn(message, ...optionalParams);
	broadcastLog("warn", str);
};

console.log("==================================================================");
console.log("🚀 Initialisation du Daemon Cron Rose Griffon (Mode Autonome)");
console.log(`⏰ Heure actuelle : ${new Date().toISOString()}`);
console.log("==================================================================");

// ─── DIALOGUE CLI MANUEL — ARGUMENTS ─────────────────────────────────────────
// Lus AVANT la passerelle Discord : la connexion charge ~2000 membres et leurs
// présences, ce qui coûte plusieurs secondes et une bonne part du `MemoryMax=1G`
// de l'unité. Une exécution manuelle de `seo:indexnow` ou de `db` n'en a aucun
// besoin ; seules les tâches ci-dessous lisent réellement `getDiscordClient()`
// (`tasks/discord.ts` et `tasks/ie-crawl/discord.ts` sont les deux seuls
// appelants du dépôt). En mode démon, la passerelle est toujours montée.
const args = process.argv.slice(2);
const indexRun = args.indexOf("--run");
const tacheCli = indexRun >= 0 ? args[indexRun + 1] : undefined;

/** Une tâche a-t-elle besoin de la passerelle Discord connectée ? */
function exigeLaPasserelleDiscord(tache: string | undefined): boolean {
	if (!tache) return true; // mode démon : tout est planifié, la passerelle est requise.
	return tache.startsWith("discord") || tache === "crawl" || tache === "all";
}

// Initialisation du client Discord
const clientDiscord = exigeLaPasserelleDiscord(tacheCli) ? await initDiscord() : null;
if (!clientDiscord && tacheCli) {
	console.log(`[Cron Runner] Passerelle Discord non requise pour « ${tacheCli} » : démarrage direct.`);
}

// Veille des salons Discord suivis : les événements de la passerelle écrivent
// immédiatement en base, le trigger Postgres `rg_realtime_notify()` diffuse
// ensuite vers `rg-realtime` (SSE) et donc vers le site.
if (clientDiscord && !tacheCli) {
	brancherVeilleMessagesTempsReel(clientDiscord);
}

// ─── METRICS & STATS D'EXÉCUTION ─────────────────────────────────────────────
// Cet objet reste la surface historique (`GET /metrics.json`, verbe IPC
// `metrics`, `task.status` du pont bot lisent `lastExecution`). Il n'est PAS
// remplacé : il est doublé par `registreExecutions` (compteurs par tâche,
// échecs consécutifs) et par l'historique persisté en base.
const metrics = {
	startedAt: new Date().toISOString(),
	tasksTriggered: 0,
	tasksSucceeded: 0,
	tasksFailed: 0,
	/** Lancements refusés parce que la même tâche tournait déjà. */
	tasksSkipped: 0,
	/** Exécutions abandonnées sur dépassement du délai maximal. */
	tasksTimedOut: 0,
	lastExecution: {} as Record<
		string,
		{ time: string; durationMs: number; success: boolean; error?: string }
	>,
};

/**
 * Délai maximal par défaut d'une tâche, en millisecondes.
 *
 * AUCUNE tâche n'en avait : un `fetch` sans `signal` ou un `ssh` bloqué
 * suspendait `executeTask` indéfiniment. Les conséquences sont mesurables — le
 * verrou du pont IPC (`lib/ipc-unix.ts`) n'est relâché qu'au retour de
 * `executeTask`, donc une tâche pendue interdisait DÉFINITIVEMENT tout nouveau
 * lancement de ce nom depuis Discord, et son échec restait invisible.
 *
 * HONNÊTETÉ SUR LA PORTÉE : le délai libère le SUIVI (métriques, historique,
 * alerte, verrou), il n'interrompt pas la promesse sous-jacente — JavaScript ne
 * le permet pas. Les annulations réelles sont posées au plus près des I/O :
 * `AbortSignal.timeout` sur les `fetch` des tâches et `kill()` sur le `ssh` de
 * `stats:achillea`.
 */
const DELAI_TACHE_DEFAUT_MS = 30 * 60_000;

/**
 * Délais spécifiques, calés sur la durée réellement observée de ces tâches :
 * elles parcourent un dump complet, une base entière ou tout un historique
 * Discord, et 30 minutes les couperaient en plein travail légitime.
 */
const DELAIS_TACHES_MS: Record<string, number> = {
	crawl: 4 * 3600_000,
	rag: 2 * 3600_000,
	"rag:wiki": 2 * 3600_000,
	"db:sync": 2 * 3600_000,
	"db:sqlite-backup": 2 * 3600_000,
	"discord:backfill": 4 * 3600_000,
	"discord:scan": 3600_000,
	cdn: 3600_000,
	"x:campagnes": 3600_000,
	"campagnes:discord": 3600_000,
	"campagnes:instagram": 3600_000,
	"campagnes:relais": 900_000,
	"seo:llms-txt": 3600_000,
	// Timelines des comptes officiels (`x-accounts.ts`). Elle EXISTAIT dans
	// `tasks/ie-crawl/twitter.ts` mais n'était câblée à aucune tâche : les
	// timelines n'étaient donc jamais collectées, et `@Azalee_IE` est resté figé
	// au 23/07/2026 pendant trois semaines pendant que `x-search` — qui collecte
	// par REQUÊTE, pas par compte — donnait l'illusion d'une veille à jour.
	// Toutes les heures, un seul compte, un seul post : le strict nécessaire pour
	// que l'annonce Discord d'Azalée parte, sans rejouer l'archive déjà en base.
	x: 3600_000,
	"x-search": 3600_000,
};

function delaiMaximalTache(nom: string): number {
	const surcharge = Number.parseInt(Bun.env.CRON_DELAI_TACHE_MS ?? "", 10);
	if (Number.isFinite(surcharge) && surcharge > 0) return surcharge;
	return DELAIS_TACHES_MS[nom] ?? DELAI_TACHE_DEFAUT_MS;
}

/** Levée quand une tâche dépasse son délai maximal. */
class ErreurDelaiDepasse extends Error {
	constructor(nom: string, delaiMs: number) {
		super(`Délai maximal dépassé (${Math.round(delaiMs / 1000)} s) pour la tâche « ${nom} ».`);
		this.name = "ErreurDelaiDepasse";
	}
}

/**
 * Exécutions en vol, nom de tâche → instant de départ.
 *
 * `lib/ipc-unix.ts` documentait déjà le trou : son verrou ne couvrait que les
 * lancements venus du pont, si bien que deux `POST /tasks/db/run` — ou un cron
 * qui repasse avant la fin du précédent (`discord:messages` toutes les 5 min) —
 * lançaient deux exécutions concurrentes sur les mêmes tables. Le verrou est
 * désormais posé ici, sur le seul point de passage commun.
 */
const tachesEnVol = new Map<string, number>();

/**
 * Enveloppe l'exécution d'une tâche : verrou d'unicité, délai maximal,
 * métriques, historique persisté et alerte sur échec répété.
 */
async function executeTask(
	name: string,
	fn: () => Promise<any>,
	origine: OrigineExecution = "planifie"
): Promise<{ success: boolean; error?: string }> {
	const depuis = tachesEnVol.get(name);
	if (depuis !== undefined) {
		const secondes = Math.round((Date.now() - depuis) / 1000);
		metrics.tasksSkipped++;
		registreExecutions.noterIgnoree(name);
		const message = `La tâche « ${name} » tourne déjà (depuis ${secondes} s) : lancement ignoré.`;
		console.warn(`[Cron Runner] ${message}`);
		return { success: false, error: message };
	}

	console.log(`[Cron Runner] Début de la tâche : ${name}`);
	metrics.tasksTriggered++;
	const start = Date.now();
	tachesEnVol.set(name, start);

	let minuteur: ReturnType<typeof setTimeout> | undefined;
	const delaiMs = delaiMaximalTache(name);

	try {
		const expiration = new Promise<never>((_, rejeter) => {
			minuteur = setTimeout(() => rejeter(new ErreurDelaiDepasse(name, delaiMs)), delaiMs);
		});
		const res = await Promise.race([fn(), expiration]);
		const duration = Date.now() - start;
		const isSuccess = res && typeof res === "object" && "success" in res ? res.success : true;
		const errorMsg = res && typeof res === "object" && "error" in res ? res.error : undefined;

		if (isSuccess) {
			metrics.tasksSucceeded++;
			metrics.lastExecution[name] = {
				time: new Date().toISOString(),
				durationMs: duration,
				success: true,
			};
			console.log(`[Cron Runner] Tâche complétée avec succès : ${name} en ${duration}ms`);
			consignerExecution({ nom: name, debutLe: start, dureeMs: duration, succes: true, origine });
			return { success: true };
		}

		metrics.tasksFailed++;
		metrics.lastExecution[name] = {
			time: new Date().toISOString(),
			durationMs: duration,
			success: false,
			error: errorMsg,
		};
		console.error(`[Cron Runner] Échec de la tâche : ${name} en ${duration}ms : ${errorMsg}`);
		consignerExecution({
			nom: name,
			debutLe: start,
			dureeMs: duration,
			succes: false,
			erreur: typeof errorMsg === "string" ? errorMsg : errorMsg ? String(errorMsg) : undefined,
			origine,
		});
		return { success: false, error: errorMsg };
	} catch (err: any) {
		const duration = Date.now() - start;
		const errorMsg = err?.message || String(err);
		const expiree = err instanceof ErreurDelaiDepasse;
		metrics.tasksFailed++;
		if (expiree) metrics.tasksTimedOut++;
		metrics.lastExecution[name] = {
			time: new Date().toISOString(),
			durationMs: duration,
			success: false,
			error: errorMsg,
		};
		console.error(
			`[Cron Runner] Erreur critique lors de la tâche : ${name} en ${duration}ms :`,
			err
		);
		consignerExecution({
			nom: name,
			debutLe: start,
			dureeMs: duration,
			succes: false,
			erreur: errorMsg,
			origine,
			expiree,
		});
		return { success: false, error: errorMsg };
	} finally {
		clearTimeout(minuteur);
		tachesEnVol.delete(name);
	}
}

/**
 * Consigne une exécution terminée : registre en mémoire, historique en base,
 * alerte Discord si la série d'échecs franchit le seuil.
 *
 * Volontairement NON attendue par `executeTask` : ni l'écriture d'historique ni
 * l'appel Discord ne doivent rallonger — ou faire échouer — une tâche de
 * production. Les deux chemins avalent déjà leurs propres erreurs ; le `catch`
 * final n'est qu'une ceinture.
 */
function consignerExecution(resultat: Parameters<typeof registreExecutions.noter>[0]): void {
	let decision: ReturnType<typeof registreExecutions.noter>;
	try {
		decision = registreExecutions.noter(resultat);
	} catch (err) {
		console.warn("[Cron Runner] registre d'exécutions indisponible :", err);
		return;
	}
	void enregistrerExecutionEnBase(resultat).catch((err) =>
		console.warn("[Cron Runner] historique non écrit :", err)
	);
	if (decision.type !== "aucune") {
		void signalerDecisionAlerte(decision).catch((err) =>
			console.warn("[Cron Runner] alerte non envoyée :", err)
		);
	}
}

// ─── DIALOGUE CLI MANUEL ─────────────────────────────────────────────────────
if (indexRun >= 0) {
	const taskIdx = indexRun + 1;
	const task = tacheCli;
	console.log(`🏃 Exécution manuelle de la tâche : ${task}`);

	if (task === "db") {
		await executeTask("db:sync", runInaglePush);
		await executeTask("db:sqlite-backup", runSQLiteBackup);
	} else if (task === "cdn") {
		await executeTask("cdn", syncCdnAssets);
	} else if (task === "crawl") {
		await executeTask("crawl", runIeCrawl);
	} else if (task === "rag") {
		await executeTask("rag", runRagSync);
	} else if (task === "x" || task === "twitter" || task === "x:timelines") {
		// UNIQUEMENT le dernier post d'`@Azalee_IE`. C'est la seule veille qui doit
		// être fraîche : c'est elle qui alimente l'annonce Discord du bot Azalée.
		await executeTask("x", () => crawlTwitter({ comptes: ["Azalee_IE"], parCompte: 1 }));
	} else if (task === "x-search" || task === "x:search") {
		await executeTask("x-search", crawlTwitterSearch);
	} else if (task === "rag:wiki" || task === "rag-wiki") {
		await executeTask("rag:wiki", () => ingestWikiCorpus());
	} else if (task === "query") {
		const question = args.slice(taskIdx + 1).join(" ");
		if (!question) {
			console.error("❌ Veuillez spécifier une question sémantique.");
		} else {
			const results = await queryRag(question);
			console.log(`\n🔍 Résultats RAG pour : "${question}"\n`);
			for (const res of results) {
				console.log(`[Score: ${res.score.toFixed(4)}] ${res.title} (${res.type})`);
				console.log(`URL: ${res.url}`);
				console.log(`Date: ${res.date}`);
				console.log(`Extrait: ${res.text.slice(0, 300)}...\n`);
				console.log("------------------------------------------------------------------");
			}
		}
	} else if (task === "publish") {
		await executeTask("publish", triggerPublishScheduled);
	} else if (task === "github-publish" || task === "gh-publish") {
		await executeTask("github-publish", triggerGithubPublishWorkflow);
	} else if (task === "patreon") {
		await executeTask("patreon", triggerPatreonRefresh);
	} else if (task === "reminders") {
		const stage = args[taskIdx + 1] || "announce";
		await executeTask(`reminders:${stage}`, () => triggerPatreonReminders(stage));
	} else if (task === "warm") {
		await executeTask("warm", warmCaches);
	} else if (task === "discord") {
		await executeTask("discord:sync", runDiscordSync);
	} else if (task === "discord:scan") {
		await executeTask("discord:scan", runDiscordChannelScan);
	} else if (task === "discord:messages" || task === "discord-messages") {
		await executeTask("discord:messages", runDiscordMessagesSync);
	} else if (task === "discord:archives" || task === "discord-archives") {
		// Armement seul : utile pour vérifier ce que la veille prendra en charge
		// (elle l'exécute déjà à chaque passe) sans attendre cinq minutes.
		await executeTask("discord:archives", async () => {
			const categories = await armerSalonsDeCategories();
			const depots = await armerSalonsDeDepot();
			return {
				success: true,
				stats: {
					salonsExamines: categories.examines,
					salonsArmes: categories.armes,
					salonsDejaSuivis: categories.deja,
					salonsDepotArmes: depots,
				},
			};
		});
	} else if (task === "campagnes:relais" || task === "campagnes-relais" || task === "relais") {
		// `--run campagnes:relais <slug>` restreint le relais à une seule campagne.
		const slug = Bun.argv[indexRun + 2];
		await executeTask("campagnes:relais", () =>
			relayerCampagnesDiscord(slug && !slug.startsWith("-") ? { slug } : {})
		);
	} else if (task === "discord:backfill" || task === "discord-backfill") {
		await executeTask("discord:backfill", runDiscordMessagesBackfill);
	} else if (task === "discord:polls" || task === "discord-polls" || task === "sondages") {
		await executeTask("discord:polls", () => runDiscordPollsImport());
	} else if (task === "noctaly:import" || task === "noctaly-import" || task === "noctaly") {
		await executeTask("noctaly:import", runNoctalyImport);
	} else if (task === "x:campagnes" || task === "x-campagnes" || task === "campagnes") {
		// `--run x:campagnes <slug>` restreint la récolte à une seule campagne.
		const slug = args[taskIdx + 1];
		await executeTask("x:campagnes", () =>
			crawlHashtagCampaigns(slug && !slug.startsWith("-") ? { slug } : {})
		);
	} else if (
		task === "campagnes:discord" ||
		task === "campagnes-discord" ||
		task === "discord:campagnes"
	) {
		// `--run campagnes:discord <slug>` restreint la récolte à une seule campagne.
		const slugDiscord = args[taskIdx + 1];
		await executeTask("campagnes:discord", () =>
			recolterCreationsDiscord(
				slugDiscord && !slugDiscord.startsWith("-") ? { slug: slugDiscord } : {}
			)
		);
	} else if (
		task === "campagnes:instagram" ||
		task === "campagnes-instagram" ||
		task === "instagram"
	) {
		// `--run campagnes:instagram <slug>` restreint la revalidation à une campagne.
		const slugInstagram = args[taskIdx + 1];
		await executeTask("campagnes:instagram", () =>
			revaliderCreationsInstagram(
				slugInstagram && !slugInstagram.startsWith("-") ? { slug: slugInstagram } : {}
			)
		);
	} else if (task === "seo:indexnow") {
		await executeTask("seo:indexnow", runSeoIndexNow);
	} else if (task === "seo:llms-txt" || task === "seo:llms") {
		await executeTask("seo:llms-txt", runSeoLlmsTxt);
	} else if (task === "zukan:videos" || task === "zukan-videos") {
		await executeTask("zukan:videos", rafraichirVideosTechniques);
	} else if (task === "stats:achillea" || task === "stats") {
		await executeTask("stats:achillea", collecterAudienceAchillea);
	} else if (task === "seo") {
		await executeTask("seo:llms-txt", runSeoLlmsTxt);
		await executeTask("seo:indexnow", runSeoIndexNow);
	} else if (task === "all") {
		console.log("🏃 Exécution de toutes les tâches...");
		await executeTask("publish", triggerPublishScheduled);
		await executeTask("warm", warmCaches);
		await executeTask("db:sync", runInaglePush);
		await executeTask("db:sqlite-backup", runSQLiteBackup);
		await executeTask("patreon", triggerPatreonRefresh);
		await executeTask("cdn", syncCdnAssets);
		await executeTask("crawl", runIeCrawl);
		await executeTask("reminders", () => triggerPatreonReminders("announce"));
		await executeTask("discord:sync", runDiscordSync);
		await executeTask("discord:scan", runDiscordChannelScan);
		await executeTask("discord:messages", runDiscordMessagesSync);
		await executeTask("discord:polls", () => runDiscordPollsImport());
		await executeTask("x:campagnes", () => crawlHashtagCampaigns());
		await executeTask("campagnes:discord", () => recolterCreationsDiscord());
		await executeTask("campagnes:instagram", () => revaliderCreationsInstagram());
		await executeTask("campagnes:relais", () => relayerCampagnesDiscord());
		await executeTask("seo:llms-txt", runSeoLlmsTxt);
		await executeTask("seo:indexnow", runSeoIndexNow);
		await executeTask("stats:achillea", collecterAudienceAchillea);
	} else {
		console.error(
			`❌ Tâche inconnue : ${task}. Choisissez parmi : db, cdn, crawl, rag, x, x-search, x:campagnes, query, publish, github-publish, patreon, reminders, warm, discord, discord:scan, discord:archives, discord:messages, discord:backfill, discord:polls, noctaly:import, campagnes:discord, campagnes:instagram, campagnes:relais, seo, seo:indexnow, seo:llms-txt, stats:achillea, backup:postgres, backup:pg, all.`
		);
	}
	process.exit(0);
}

// ─── ENREGISTREMENT DES SGHEDULES CRON DE PRODUCTION (BUN.CRON) ───────────────

// 1. Publication programmée Azalée : toutes les 15 minutes
Bun.cron("*/15 * * * *", async () => {
	await executeTask("publish", triggerPublishScheduled);
});

// 2. Pré-chauffage des caches ISR : toutes les 30 minutes
Bun.cron("*/30 * * * *", async () => {
	await executeTask("warm", warmCaches);
});

// 2b. Synchronisation Discord (membres, rôles, staff) : toutes les 30 minutes
Bun.cron("*/30 * * * *", async () => {
	await executeTask("discord:sync", runDiscordSync);
});

// 2c. Veille des salons Discord suivis : toutes les 5 minutes.
// Filet de sécurité du temps réel : les événements de la passerelle sont perdus
// pendant un redémarrage du démon ou une coupure de la connexion Discord, et le
// repli sans intents privilégiés n'en reçoit aucun. Ce rattrapage REST rejoue
// tout ce qui a été manqué (messages, éditions, réactions).
Bun.cron("*/5 * * * *", async () => {
	await executeTask("discord:messages", runDiscordMessagesSync);
});

// 2d. Extraction des sondages EasyPoll : toutes les heures, à la minute 10.
// L'import relit les encarts déjà archivés — il ne rattrape donc jamais plus que
// ce que la veille (2c) a écrit, d'où le décalage volontaire de 5 minutes.
// EasyPoll RÉÉCRIT son encart quand un sondage se clôt : sans repasse régulière,
// la page publique resterait bloquée sur « résultats masqués » indéfiniment.
Bun.cron("10 * * * *", async () => {
	await executeTask("discord:polls", () => runDiscordPollsImport());
});

// 2e. Récolte des campagnes à hashtag sur X : toutes les heures, à la minute 20.
// Cadence choisie sur une contrainte mesurée, pas par confort : l'API renvoie des
// 429 dès 6 à 8 pages consécutives, et la récolte doit rester le SEUL scan X en
// cours. La minute 20 évite les créneaux chargés (:00, :15, :30) et le crawl
// officiel de 5h00. La reprise est incrémentale (curseur persisté par campagne).
Bun.cron("20 * * * *", async () => {
	await executeTask("x:campagnes", () => crawlHashtagCampaigns());
});

// 2f. Récolte des créations Discord des campagnes : toutes les heures, minute 25.
// Cinq minutes APRÈS la récolte X (minute 20), et vingt minutes après la veille
// des salons : la tâche relit `discord_messages`, elle ne peut donc jamais
// remonter plus que ce que la veille a déjà archivé. Elle n'appelle Discord que
// pour recopier une image jamais vue — une passe sans nouveauté ne fait qu'une
// requête SQL.
Bun.cron("25 * * * *", async () => {
	await executeTask("campagnes:discord", () => recolterCreationsDiscord());
});

// 2f-bis. Revalidation des créations Instagram : toutes les heures, minute 35.
//
// PAS UNE RÉCOLTE — LE CONTRAIRE. Instagram n'a aucun collecteur possible
// (`docs/insta.md`) : la table est remplie par le formulaire de /iergday. Cette
// passe repose la seule question que Meta accepte encore sans compte — « ce post
// existe-t-il toujours, et est-il public ? » — parce que rien d'autre ne nous le
// dirait : un post supprimé chez Instagram laisserait sinon une carte morte dans
// la galerie pour toujours.
//
// Dix minutes APRÈS la récolte Discord et dix AVANT le relais de la minute 45 :
// une création masquée dans cette passe ne part donc jamais dans le salon de
// suivi comme si elle était encore en ligne. La fraîcheur de 12 h et le plafond
// de 60 lignes font qu'une passe sans changement ne coûte qu'une requête SQL.
Bun.cron("35 * * * *", async () => {
	await executeTask("campagnes:instagram", () => revaliderCreationsInstagram());
});

// 2g. Relais des créations vers le salon de suivi : tous les quarts d'heure.
//
// PLUS SOUVENT QUE LES DEUX RÉCOLTES, ET C'EST VOULU. Le relais ne va chercher
// nulle part : il lit ce que les récoltes ont déjà écrit et poste ce qui manque.
// Une passe à vide coûte deux requêtes SQL. Le caler sur l'heure comme les
// récoltes ferait attendre jusqu'à soixante minutes une création déjà en base —
// pendant une campagne, c'est la différence entre un fil vivant et un digest.
Bun.cron("*/15 * * * *", async () => {
	await executeTask("campagnes:relais", () => relayerCampagnesDiscord());
});

// 3. Synchronisation Inagle DB & SQLite Backup : tous les jours à 2h00 UTC
//
// L'ORDRE DES TROIS TÂCHES EST LE CORRECTIF, PAS UN DÉTAIL. `db:sync` vide puis
// réécrit `inagle_skills` : le push préserve désormais les vidéos et vignettes
// zukan, mais c'est `zukan:videos` qui va les rechercher à la source. Il doit
// donc tourner APRÈS le push (sinon il écrit dans une table qui sera vidée dans
// la foulée) et AVANT `db:sqlite-backup` (sinon le miroir servi par azalée
// embarque un instantané sans vidéos). Le 17/8/2026, `zukan:videos` n'était
// planifiée nulle part : le push a effacé les 1211 variantes de la nuit, et
// plus rien ne les remettait — /skill a perdu vidéos et aperçus d'un coup.
Bun.cron("0 2 * * *", async () => {
	await executeTask("db:sync", runInaglePush);
	await executeTask("zukan:videos", rafraichirVideosTechniques);
	await executeTask("db:sqlite-backup", runSQLiteBackup);
});

// 3b. Sauvegarde logique PostgreSQL : elle N'EST PLUS ICI.
//
// `db:postgres-backup` appelait `scripts/ops/backup-postgres.sh`, hérité de
// l'époque Supabase Cloud. Depuis la bascule en auto-hébergement du 11/8/2026,
// son `pg_dumpall --globals-only` tourne sous le rôle `rg`, qui n'est pas
// superutilisateur : « permission denied for table pg_authid ». Le `set -e` du
// script coupait alors avant le dump du schéma et des données, en laissant un
// dossier ne contenant qu'un `roles.sql` vide — seize jours d'échec quotidien
// pour une sauvegarde qui n'en était plus une.
// `rg-sauvegarde.timer` (`scripts/ops/sauvegarde.ts`) fait déjà le même travail
// en mieux : dump de CHAQUE base sous le rôle `postgres`, restauration de
// contrôle, rotation à 7 copies, dans `/var/backups/postgres`. Deux
// sauvegardes dont une muette valent moins qu'une seule qui se vérifie.

// 4. Rafraîchissement Patreon Website : tous les jours à 3h00 UTC
Bun.cron("0 3 * * *", async () => {
	await executeTask("patreon", triggerPatreonRefresh);
});

// 5. Synchronisation CDN : tous les jours à 4h00 UTC
Bun.cron("0 4 * * *", async () => {
	await executeTask("cdn", syncCdnAssets);
});

// 5b. Publication automatique des packages sur GitHub Packages : tous les jours à 4h30 UTC
Bun.cron("30 4 * * *", async () => {
	await executeTask("github-publish", triggerGithubPublishWorkflow);
});

// 5c. Crawl officiel Inazuma Eleven & RAG Sync : tous les jours à 5h00 UTC
Bun.cron("0 5 * * *", async () => {
	await executeTask("crawl", runIeCrawl);
});

// 6. Rappels Patreon Website : tous les jours à 8h00 UTC
Bun.cron("0 8 * * *", async () => {
	await executeTask("reminders", () => triggerPatreonReminders("announce"));
});

// 7. SEO — Régénération des fichiers llms.txt/llm.txt (fraîcheur) : tous les jours à 5h45 UTC.
// NB : régénère les fichiers publics dans le checkout local (canonique). La mise en
// ligne effective dépend d'un redéploiement (commit du repo) ; sur le VPS, cela
// rafraîchit la copie servie localement si applicable.
Bun.cron("45 5 * * *", async () => {
	await executeTask("seo:llms-txt", runSeoLlmsTxt);
});

// 8. SEO — Soumission IndexNow des sitemaps (Bing/Yandex/Seznam…) : tous les jours à 6h00 UTC.
// Cadence quotidienne raisonnable pour éviter tout abus/bannissement de la clé.
Bun.cron("0 6 * * *", async () => {
	await executeTask("seo:indexnow", runSeoIndexNow);
});

// 9. Statistiques — rapatriement de l'audience du second serveur, toutes les 15 min.
// `achillea.rosegriffon.fr` et `ranked.rosegriffon.fr` sont servis par un autre VPS :
// leur journal nginx n'est pas lisible d'ici. L'onglet Statistiques affiche une
// fenêtre de 24 h — un décalage plus long rendrait « les dernières heures » faux
// de façon visible pour ces deux domaines.
Bun.cron("*/15 * * * *", async () => {
	await executeTask("stats:achillea", collecterAudienceAchillea);
});

console.log("✅ Toutes les tâches cron ont été planifiées avec succès.");

// ─── SERVEUR HTTP + WEBSOCKETS (BUN.SERVE) ───────────────────────────────────
const PORT = Number(process.env.CRON_PORT) || 3005;

Bun.serve({
	// Boucle locale : sans `hostname`, Bun écoute sur toutes les interfaces et le tableau
	// de bord du cron se retrouvait joignable en HTTP clair depuis l'Internet public.
	// L'accès depuis le VPN reste assuré par `wg-forward@3005.service`, qui relaie
	// 10.8.0.1:3005 vers cette même boucle — le motif déjà utilisé par treize autres ports.
	hostname: "127.0.0.1",
	port: PORT,
	async fetch(req, server) {
		const url = new URL(req.url);

		// 1. WebSocket upgrade
		if (url.pathname === "/ws") {
			const success = server.upgrade(req);
			if (success) return undefined;
			return new Response("Upgrade WebSocket échoué", { status: 400 });
		}

		// 2. Health check
		if (url.pathname === "/health") {
			return Response.json({
				status: "healthy",
				uptime: process.uptime(),
				memory: process.memoryUsage(),
				time: new Date().toISOString(),
				wsConnections: wsConnections.size,
			});
		}

		// 3. Prometheus / Custom Metrics
		if (url.pathname === "/metrics") {
			return Response.json(metrics);
		}

		// 3b. OpenAPI JSON Spec
		if (url.pathname === "/openapi.json") {
			return Response.json(getOpenApiDocument());
		}

		// 3c. Swagger UI HTML
		if (url.pathname === "/swagger") {
			return new Response(getSwaggerHtml(), {
				headers: { "Content-Type": "text/html; charset=utf-8" },
			});
		}

		// 3d. Télémétrie Discord
		if (url.pathname === "/discord-telemetry") {
			const telemetry = getDiscordTelemetry();
			if (!telemetry) {
				return new Response(JSON.stringify({ error: "Télémétrie Discord indisponible (non connecté ou désactivé)" }), {
					status: 503,
					headers: { "Content-Type": "application/json" }
				});
			}
			return Response.json(telemetry);
		}

		// Helper pour extraire les cookies de la requête
		function getCookie(request: Request, name: string): string | null {
			const cookieHeader = request.headers.get("Cookie");
			if (!cookieHeader) return null;
			const cookies = cookieHeader.split(";");
			for (const cookie of cookies) {
				const [key, value] = cookie.trim().split("=");
				if (key === name) return value ? decodeURIComponent(value) : "";
			}
			return null;
		}

		// Sécurisation par CRON_SECRET ou authentification par Cookie Admin (Better Auth)
		const authHeader = req.headers.get("Authorization");
		const cronSecret = process.env.CRON_SECRET;
		let isAuthorized = false;

		// 1. Autorisation via Token d'automatisation
		if (cronSecret && authHeader === `Bearer ${cronSecret}`) {
			isAuthorized = true;
		}

		// 2. Autorisation via Session Cookie Administrateur
		if (!isAuthorized) {
			const sessionToken = getCookie(req, "better-auth.session-token");
			if (sessionToken) {
				try {
					const supabase = createSupabaseServiceClient();
					const { data: session } = await supabase
						.from("session")
						.select("user_id, expires_at")
						.eq("token", sessionToken)
						.maybeSingle();

					if (session && new Date(session.expires_at) > new Date()) {
						const profile = await resolveProfile(supabase, { id: session.user_id });
						if (profile && isAdmin(profile.role)) {
							isAuthorized = true;
							console.log(`[Cron Auth] Accès autorisé pour l'administrateur : ${profile.username}`);
						}
					}
				} catch (authErr) {
					console.warn("[Cron Auth] Échec de l'authentification par session cookie :", authErr);
				}
			}
		}

		if (!isAuthorized) {
			return new Response("Non autorisé", { status: 401 });
		}

		// 4. Liste des tâches — servie depuis le CATALOGUE PARTAGÉ.
		//
		// Elle était recopiée ici à la main, et c'est ainsi qu'elle a divergé du
		// catalogue du bot (dix-huit noms d'un côté, vingt-deux de l'autre). Une
		// seule source désormais : `@rosegriffon/types/cron`, que le bot et le
		// site lisent aussi. Le catalogue porte en plus le libellé, la famille,
		// la durée indicative et le niveau d'accès — d'où `catalogue`, à côté des
		// deux champs historiques que d'anciens clients attendent encore.
		if (url.pathname === "/tasks" && req.method === "GET") {
			return Response.json({
				tasks: TACHES_CRON,
				schedules: PLANIFICATIONS,
				catalogue: CATALOGUE_TACHES,
			});
		}

		// 5. Déclenchement manuel d'une tâche à la demande
		if (
			url.pathname.startsWith("/tasks/") &&
			url.pathname.endsWith("/run") &&
			req.method === "POST"
		) {
			const taskName = url.pathname.slice(7, -4);

			const runnerMap: Record<string, () => Promise<any>> = {
				db: runInaglePush,
				cdn: syncCdnAssets,
				crawl: runIeCrawl,
				rag: runRagSync,
				publish: triggerPublishScheduled,
				"github-publish": triggerGithubPublishWorkflow,
				patreon: triggerPatreonRefresh,
				reminders: () => triggerPatreonReminders("announce"),
				warm: warmCaches,
				discord: runDiscordSync,
				"discord:scan": runDiscordChannelScan,
				"discord-scan": runDiscordChannelScan,
				"discord:messages": runDiscordMessagesSync,
				"discord-messages": runDiscordMessagesSync,
				"discord:archives": async () => ({
					success: true,
					stats: {
						categories: await armerSalonsDeCategories(),
						depots: await armerSalonsDeDepot(),
					},
				}),
				"discord:backfill": runDiscordMessagesBackfill,
				"discord-backfill": runDiscordMessagesBackfill,
				"discord:polls": () => runDiscordPollsImport(),
				"discord-polls": () => runDiscordPollsImport(),
				"noctaly:import": runNoctalyImport,
				"noctaly-import": runNoctalyImport,
				"x:campagnes": () => crawlHashtagCampaigns(),
				"x-campagnes": () => crawlHashtagCampaigns(),
				"campagnes:discord": () => recolterCreationsDiscord(),
				"campagnes-discord": () => recolterCreationsDiscord(),
				"campagnes:instagram": () => revaliderCreationsInstagram(),
				"campagnes-instagram": () => revaliderCreationsInstagram(),
				"campagnes:relais": () => relayerCampagnesDiscord(),
				"campagnes-relais": () => relayerCampagnesDiscord(),
				"seo:indexnow": runSeoIndexNow,
				"seo-indexnow": runSeoIndexNow,
				"seo:llms-txt": runSeoLlmsTxt,
				"seo-llms-txt": runSeoLlmsTxt,
				// Annoncée par `GET /tasks` et planifiée toutes les 15 min depuis sa
				// création, mais absente d'ici : `POST /tasks/stats:achillea/run`
				// répondait 400 « Tâche inconnue » sur une tâche qui tourne toute la
				// journée. Relevé par `catalogue.test.ts`, qui relie désormais les
				// trois listes.
				"stats:achillea": collecterAudienceAchillea,
				"stats-achillea": collecterAudienceAchillea,
				"zukan:videos": rafraichirVideosTechniques,
				"zukan-videos": rafraichirVideosTechniques,
			};

			const taskFn = runnerMap[taskName];
			if (!taskFn) {
				return new Response(JSON.stringify({ error: `Tâche "${taskName}" inconnue.` }), {
					status: 400,
					headers: { "Content-Type": "application/json" },
				});
			}

			// Lancer la tâche en tâche de fond pour libérer la requête HTTP instantanément
			executeTask(taskName, taskFn);

			return Response.json(
				{ success: true, message: `Tâche "${taskName}" démarrée en tâche de fond.` },
				{ status: 202 }
			);
		}

		return new Response("Non trouvé", { status: 404 });
	},
	websocket: {
		open(ws) {
			wsConnections.add(ws);
			ws.send(
				JSON.stringify({ type: "system", text: "Connexion établie avec le flux de logs rg-cron." })
			);
			const telemetry = getDiscordTelemetry();
			if (telemetry) {
				ws.send(JSON.stringify({ type: "discord-telemetry", data: telemetry }));
			}
		},
		message(ws, msg) {
			originalLog(`[WS Message] reçu : ${msg}`);
		},
		close(ws) {
			wsConnections.delete(ws);
		},
	},
});

console.log(`🌐 Serveur HTTP et WebSocket à l'écoute sur le port ${PORT}`);

// ─── SERVEUR TCP IPC LOCAL (BUN.LISTEN) ──────────────────────────────────────
const IPC_PORT = 4001;

Bun.listen({
	hostname: "127.0.0.1",
	port: IPC_PORT,
	socket: {
		async data(socket, data) {
			try {
				const payload = JSON.parse(data.toString());
				if (payload.cmd === "query_rag" && payload.question) {
					if (payload.web) {
						// Réponse unifiée : vectoriel local + grounding web live (bxc).
						const grounded = await ragGroundedQuery(payload.question, {
							web: true,
							webMaxResults: payload.webMaxResults ?? 3,
						});
						socket.write(
							JSON.stringify({ success: true, results: grounded.sources, grounded }) + "\n"
						);
					} else {
						const results = await queryRag(payload.question);
						socket.write(JSON.stringify({ success: true, results }) + "\n");
					}
				} else if (payload.cmd === "health") {
					socket.write(JSON.stringify({ success: true, uptime: process.uptime(), metrics }) + "\n");
				} else {
					socket.write(JSON.stringify({ success: false, error: "Commande inconnue" }) + "\n");
				}
			} catch (err: any) {
				socket.write(JSON.stringify({ success: false, error: err.message }) + "\n");
			}
		},
	},
});

// ─── STREAMING DE LA TÉLÉMÉTRIE EN TEMPS RÉEL SUR LES WEBSOCKETS ─────────────
onTelemetryUpdate((telemetry) => {
	const payload = JSON.stringify({
		type: "discord-telemetry",
		data: telemetry,
		timestamp: Date.now(),
	});
	for (const ws of wsConnections) {
		try {
			ws.send(payload);
		} catch {
			wsConnections.delete(ws);
		}
	}
});

// ─── SERVEUR HTTP D'ANNONCES DISCORD (PORT 3006) ─────────────────────────────
//
// ── CE SERVICE PUBLIE DANS UN SALON PUBLIC ──────────────────────────────────
// Il en découle trois règles, et elles ont toutes corrigé un défaut réel
// (relevé le 14/8/2026) :
//
//  1. **AUCUN SECRET PAR DÉFAUT.** Il valait `default-secret-key-12345` quand
//     `DISCORD_ANNOUNCE_SECRET` n'était pas posé — et il ne l'était pas. Toute
//     personne pouvant atteindre le port publiait donc un embed arbitraire
//     (titre, lien, image) dans le salon d'annonces. Sans secret, le service
//     refuse maintenant TOUT : il vaut mieux une annonce qui ne part pas qu'une
//     annonce que personne n'a demandée.
//  2. **BOUCLE LOCALE.** Le seul appelant légitime est le site, sur ce VPS.
//     Écouter sur `*` exposait la surface à tout ce qui atteint la machine — le
//     pare-feu tenait, mais il n'a pas à être la seule barrière.
//  3. **AUCUNE DEVINETTE DE SALON.** Sans salon configuré, la version
//     précédente en cherchait un par NOM (`annonces`, `news`, `general`) et
//     publiait dedans. Un salon d'annonces se DÉSIGNE par un administrateur ;
//     il ne se devine pas.
const ANNOUNCE_PORT = 3006;
const announceSecret = (Bun.env.DISCORD_ANNOUNCE_SECRET ?? "").trim();

if (announceSecret === "") {
	console.warn(
		"⚠ [announce-server] DISCORD_ANNOUNCE_SECRET absent : le service d'annonces refusera TOUTES les requêtes. " +
			"Pose la variable pour l'activer — une publication publique ne s'ouvre pas toute seule."
	);
}

Bun.serve({
	// Boucle locale : le site tourne sur la même machine.
	hostname: "127.0.0.1",
	port: ANNOUNCE_PORT,
	async fetch(req) {
		const url = new URL(req.url);
		if (req.method !== "POST" || url.pathname !== "/announce") {
			return new Response("Not Found", { status: 404 });
		}

		// Fail closed : pas de secret configuré, pas d'annonce. Jamais de valeur
		// de repli — un secret par défaut est un secret public.
		if (announceSecret === "") {
			console.error(
				"[announce-server] requête refusée : DISCORD_ANNOUNCE_SECRET n'est pas configuré."
			);
			return new Response("Announce service not configured", { status: 503 });
		}

		// Check Authorization
		const authHeader = req.headers.get("Authorization");
		if (!authHeader || authHeader !== `Bearer ${announceSecret}`) {
			return new Response("Unauthorized", { status: 401 });
		}

		try {
			const body = (await req.json()) as {
				type?: string;
				title?: string;
				slug?: string;
				excerpt?: string;
				featured_image_url?: string;
			};
			const { type, title, slug, excerpt, featured_image_url } = body;

			if (!title || !slug) {
				return new Response("Missing title or slug", { status: 400 });
			}

			const client = getDiscordClient();
			if (!client || !client.isReady()) {
				console.error("[announce-server] Client Discord non prêt ou non initialisé.");
				return new Response("Discord client not ready", { status: 500 });
			}

			// Find Guild
			const guild = client.guilds.cache.get(GUILD_ID || "");
			if (!guild) {
				console.error("[announce-server] Guild non trouvé:", GUILD_ID);
				return new Response("Guild not found", { status: 500 });
			}

			// Resolve Channel ID
			const channelId = type === "wiki"
				? (Bun.env.DISCORD_WIKI_ANNOUNCEMENTS_CHANNEL_ID || Bun.env.DISCORD_NEWS_CHANNEL_ID)
				: (Bun.env.DISCORD_WEBSITE_ANNOUNCEMENTS_CHANNEL_ID || Bun.env.DISCORD_NEWS_CHANNEL_ID);

			// Le salon est DÉSIGNÉ, jamais deviné. La version précédente cherchait
			// un salon nommé `annonces`, `news` ou `general` quand la variable
			// n'était pas posée : un salon de discussion générale pouvait recevoir
			// des annonces automatiques sans que personne ne l'ait décidé.
			if (!channelId) {
				console.error(
					"[announce-server] refusé : aucun salon d'annonces configuré. " +
						"Pose DISCORD_WIKI_ANNOUNCEMENTS_CHANNEL_ID / DISCORD_WEBSITE_ANNOUNCEMENTS_CHANNEL_ID " +
						"(ou DISCORD_NEWS_CHANNEL_ID). Un salon public ne se devine pas."
				);
				return new Response("Announce channel not configured", { status: 503 });
			}
			const channel = guild.channels.cache.get(channelId);
			if (!channel || !channel.isTextBased()) {
				console.error(
					`[announce-server] refusé : le salon configuré (${channelId}) est introuvable ou n'est pas textuel.`
				);
				return new Response("Channel not found", { status: 500 });
			}

			// Build Embed
			const articleUrl = type === "wiki"
				? `https://azalee.rosegriffon.fr/news/${slug}`
				: `https://rosegriffon.fr/chroniques/${slug}`;

			const embedColor = type === "wiki" ? 0xf2a93b : 0xa14b3f; // Azalée orange vs Rose Griffon red

			const embed = {
				title,
				url: articleUrl,
				description: excerpt || undefined,
				color: embedColor,
				image: featured_image_url ? { url: featured_image_url } : undefined,
				footer: { text: type === "wiki" ? "Azalée — Rose Griffon" : "Rose Griffon" },
				timestamp: new Date().toISOString()
			};

			await channel.send({ embeds: [embed] });
			console.log(`[announce-server] Annonce envoyée pour : "${title}" dans le salon #${channel.name}`);
			return new Response(JSON.stringify({ success: true }), {
				headers: { "Content-Type": "application/json" }
			});
		} catch (err) {
			console.error("[announce-server] Erreur lors de l'annonce:", err);
			return new Response(JSON.stringify({ error: err instanceof Error ? err.message : String(err) }), {
				status: 500,
				headers: { "Content-Type": "application/json" }
			});
		}
	}
});

// Pont natif vers rg-bot : socket UNIX (`/var/lib/rg/cron.sock`), verbes de
// lecture + `task.run` gardé, et poussée `tweets.signal` au bot. Aucune surface
// HTTP n'est modifiée et aucun port n'est ouvert. Cf. `lib/ipc-unix.ts`.
demarrerPontBot({
	metriques: () => metrics,
	telemetrie: getDiscordTelemetry,
	executer: executeTask,
	queryRag,
});

console.log(`📣 Serveur HTTP d'annonces Discord à l'écoute sur le port ${ANNOUNCE_PORT}`);
console.log(`🔌 Serveur TCP IPC local à l'écoute sur le port ${IPC_PORT}`);
console.log("💤 Le daemon est en cours d'exécution et écoute...");
