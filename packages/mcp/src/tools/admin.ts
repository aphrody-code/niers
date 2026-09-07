/**
 * Outils d'**administration** — portée `admin` uniquement.
 *
 * Ils écrivent, suppriment et exécutent. Un client qui présente le jeton de
 * lecture ne les voit même pas dans `tools/list` : la portée est décidée par
 * le transport d'après le jeton, jamais par le client.
 *
 * Garde-fous conservés malgré la portée totale :
 *
 * - la **prison de chemin** reste active pour tous les outils de fichiers :
 *   écrire hors du dépôt n'a aucun usage légitime ici, et un chemin mal formé
 *   irait sinon toucher le système ;
 * - la liste noire est en revanche **levée** : modifier un `.env` ou un
 *   fichier de configuration fait partie de l'administration du dépôt ;
 * - chaque opération est tracée sur stderr (donc dans journald), avec le
 *   chemin ou la commande concernée.
 *
 * `shell_run` est l'exception assumée : une commande shell peut sortir du
 * dépôt. C'est précisément ce qu'on attend d'un accès d'administration —
 * c'est aussi pourquoi son jeton doit être traité comme un accès SSH.
 */

import { z } from "zod";
import { structured, text, toolError } from "../protocol/types.ts";
import { defineTool, type RegisteredTool } from "../registry.ts";
import { KNOWN_SERVICES } from "./ops.ts";
import { resolveInside } from "./paths.ts";
import { ensureDir, deleteRecursive, movePath } from "./fs-native.ts";

export interface AdminToolsOptions {
	/** Racine du dépôt : prison de chemin des outils de fichiers. */
	root: string;
	/** Journalisation des opérations sensibles (stderr par défaut). */
	onAudit?: (line: string) => void;
	/** Délai maximal d'une commande, en millisecondes. */
	commandTimeoutMs?: number;
}

const DEFAULT_COMMAND_TIMEOUT_MS = 120_000;
const MAX_OUTPUT_CHARS = 20_000;

/** Actions systemd autorisées : rien qui masque ou désactive une unité. */
const SERVICE_ACTIONS = ["start", "stop", "restart", "reload"] as const;

function tronquer(value: string): string {
	return value.length > MAX_OUTPUT_CHARS
		? `${value.slice(0, MAX_OUTPUT_CHARS)}\n… (sortie tronquée à ${MAX_OUTPUT_CHARS} caractères)`
		: value;
}

interface Execution {
	exitCode: number;
	stdout: string;
	stderr: string;
	ms: number;
}

/** Code de sortie d'une commande interrompue — convention de `timeout(1)`. */
const EXIT_CODE_DELAI = 124;

async function executer(command: string[], cwd: string, timeoutMs: number): Promise<Execution> {
	const started = performance.now();
	const proc = Bun.spawn(command, { cwd, stdout: "pipe", stderr: "pipe", stdin: "ignore" });

	// Le délai doit être TENU, pas seulement signalé. Tuer le shell ne suffit
	// pas : un petit-fils (`bash -lc "sleep 30"`) lui survit et garde les tubes
	// ouverts, si bien que la lecture de `stdout` ne se termine jamais et que
	// l'appel dépasse indéfiniment son propre délai. On court donc la lecture
	// CONTRE l'échéance, et c'est l'échéance qui tranche.
	let minuteur: ReturnType<typeof setTimeout> | undefined;
	const echeance = new Promise<"delai">((resolve) => {
		minuteur = setTimeout(() => {
			proc.kill(9);
			resolve("delai");
		}, timeoutMs);
	});

	const lecture = (async (): Promise<Execution> => {
		const [stdout, stderr] = await Promise.all([
			new Response(proc.stdout).text(),
			new Response(proc.stderr).text(),
		]);
		const exitCode = await proc.exited;
		return { exitCode, stdout: tronquer(stdout), stderr: tronquer(stderr), ms: Math.round(performance.now() - started) };
	})();
	// Sans ce filet, la lecture abandonnée par la course rejette dans le vide.
	lecture.catch(() => {});

	const issue = await Promise.race([lecture, echeance]);
	clearTimeout(minuteur);
	if (issue === "delai") {
		return {
			exitCode: EXIT_CODE_DELAI,
			stdout: "",
			stderr: `Commande interrompue après ${timeoutMs} ms.`,
			ms: Math.round(performance.now() - started),
		};
	}
	return issue;
}

export function adminTools(options: AdminToolsOptions): RegisteredTool[] {
	const root = options.root.replace(/\/+$/, "");
	const timeout = options.commandTimeoutMs ?? DEFAULT_COMMAND_TIMEOUT_MS;
	const audit = options.onAudit ?? ((line: string) => process.stderr.write(`${line}\n`));

	return [
		defineTool({
			name: "repo_write",
			title: "Écrire un fichier du monorepo",
			description:
				"Crée ou remplace intégralement un fichier du dépôt sur le VPS. Le chemin doit rester dans le dépôt. Pour une modification ponctuelle, préférer repo_edit : écraser un fichier entier fait perdre le reste de son contenu.",
			scope: "admin",
			inputSchema: z.object({
				path: z.string().min(1).describe("Chemin relatif à la racine du dépôt."),
				content: z.string().describe("Contenu complet du fichier."),
				createDirs: z.boolean().default(true).describe("Créer les répertoires parents manquants."),
			}),
			annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: false },
			handler: async ({ path, content, createDirs }) => {
				const target = await resolveInside(root, path, { allowDenied: true });
				if (!target) return toolError(`Chemin refusé ou hors du dépôt : ${path}`);
				const existait = await Bun.file(target.absolute).exists();
				if (createDirs) {
					const parent = target.absolute.slice(0, target.absolute.lastIndexOf("/"));
					await ensureDir(parent);
				}
				const octets = await Bun.write(target.absolute, content);
				audit(`mcp-admin repo_write path=${target.relative} bytes=${octets} nouveau=${existait ? 0 : 1}`);
				return structured({ path: target.relative, bytes: octets, created: !existait });
			},
		}),

		defineTool({
			name: "repo_edit",
			title: "Remplacer une chaîne dans un fichier",
			description:
				"Remplace une chaîne exacte par une autre dans un fichier du dépôt. Échoue si la chaîne est absente, ou si elle apparaît plusieurs fois sans `replaceAll` — ce qui évite de modifier la mauvaise occurrence.",
			scope: "admin",
			inputSchema: z.object({
				path: z.string().min(1).describe("Chemin relatif du fichier."),
				oldString: z.string().min(1).describe("Texte à remplacer, exactement tel qu'il figure dans le fichier."),
				newString: z.string().describe("Texte de remplacement."),
				replaceAll: z.boolean().default(false).describe("Remplacer toutes les occurrences."),
			}),
			annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: false },
			handler: async ({ path, oldString, newString, replaceAll }) => {
				const target = await resolveInside(root, path, { allowDenied: true });
				if (!target) return toolError(`Chemin refusé ou hors du dépôt : ${path}`);
				const file = Bun.file(target.absolute);
				if (!(await file.exists())) return toolError(`Fichier introuvable : ${target.relative}`);
				const avant = await file.text();
				const occurrences = avant.split(oldString).length - 1;
				if (occurrences === 0) return toolError(`Chaîne introuvable dans ${target.relative}.`);
				if (occurrences > 1 && !replaceAll) {
					return toolError(
						`${occurrences} occurrences dans ${target.relative} : préciser un contexte plus large ou activer replaceAll.`,
					);
				}
				const apres = replaceAll ? avant.replaceAll(oldString, newString) : avant.replace(oldString, newString);
				await Bun.write(target.absolute, apres);
				audit(`mcp-admin repo_edit path=${target.relative} occurrences=${replaceAll ? occurrences : 1}`);
				return structured({
					path: target.relative,
					replaced: replaceAll ? occurrences : 1,
					bytes: apres.length,
				});
			},
		}),

		defineTool({
			name: "repo_delete",
			title: "Supprimer un fichier ou un dossier",
			description:
				"Supprime définitivement un fichier du dépôt, ou un dossier avec `recursive`. Opération irréversible : vérifier le chemin avec repo_list avant d'appeler.",
			scope: "admin",
			inputSchema: z.object({
				path: z.string().min(1).describe("Chemin relatif à supprimer."),
				recursive: z.boolean().default(false).describe("Requis pour supprimer un dossier et son contenu."),
			}),
			annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: false },
			handler: async ({ path, recursive }) => {
				const target = await resolveInside(root, path, { allowDenied: true });
				if (!target) return toolError(`Chemin refusé ou hors du dépôt : ${path}`);
				if (target.relative === "") return toolError("Refus de supprimer la racine du dépôt.");
				let info: { isDirectory(): boolean };
				try {
					info = await Bun.file(target.absolute).stat();
				} catch {
					return toolError(`Chemin introuvable : ${target.relative}`);
				}
				if (info.isDirectory()) {
					if (!recursive) return toolError(`${target.relative} est un dossier : passer recursive: true.`);
					await deleteRecursive(target.absolute);
				} else {
					await Bun.file(target.absolute).delete();
				}
				audit(`mcp-admin repo_delete path=${target.relative} dossier=${info.isDirectory() ? 1 : 0}`);
				return structured({ path: target.relative, deleted: true, directory: info.isDirectory() });
			},
		}),

		defineTool({
			name: "repo_move",
			title: "Déplacer ou renommer",
			description:
				"Déplace ou renomme un fichier ou un dossier à l'intérieur du dépôt. Les deux chemins doivent rester dans le dépôt.",
			scope: "admin",
			inputSchema: z.object({
				from: z.string().min(1).describe("Chemin source, relatif au dépôt."),
				to: z.string().min(1).describe("Chemin destination, relatif au dépôt."),
			}),
			annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: false },
			handler: async ({ from, to }) => {
				const source = await resolveInside(root, from, { allowDenied: true });
				const destination = await resolveInside(root, to, { allowDenied: true });
				if (!source) return toolError(`Source refusée ou hors du dépôt : ${from}`);
				if (!destination) return toolError(`Destination refusée ou hors du dépôt : ${to}`);
				if (!(await Bun.file(source.absolute).exists())) {
					const stat = await Bun.file(source.absolute)
						.stat()
						.catch(() => undefined);
					if (!stat) return toolError(`Source introuvable : ${source.relative}`);
				}
				try {
					await movePath(source.absolute, destination.absolute);
				} catch (e: any) {
					return toolError(e?.message || "échec du déplacement");
				}
				audit(`mcp-admin repo_move from=${source.relative} to=${destination.relative}`);
				return structured({ from: source.relative, to: destination.relative, moved: true });
			},
		}),

		defineTool({
			name: "shell_run",
			title: "Exécuter une commande sur le VPS",
			description:
				"Exécute une commande shell sur le VPS et renvoie sa sortie, son code de retour et sa durée. Sert à ce que les outils typés ne couvrent pas : git commit, build, déploiement, installation. Le répertoire de travail est le dépôt par défaut. Accès complet — à n'utiliser qu'à bon escient.",
			scope: "admin",
			inputSchema: z.object({
				command: z.string().min(1).describe("Commande complète, interprétée par bash."),
				cwd: z.string().default("").describe("Répertoire de travail, relatif au dépôt."),
				timeoutMs: z
					.int()
					.min(1000)
					.max(600_000)
					.default(DEFAULT_COMMAND_TIMEOUT_MS)
					.describe("Délai maximal avant interruption."),
			}),
			annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: true },
			handler: async ({ command, cwd, timeoutMs }, context) => {
				const dossier = await resolveInside(root, cwd, { allowDenied: true });
				if (!dossier) return toolError(`Répertoire de travail hors du dépôt : ${cwd}`);
				audit(`mcp-admin shell_run cwd=${dossier.relative || "."} cmd=${command.slice(0, 200)}`);
				context.log("notice", { outil: "shell_run", commande: command });
				const resultat = await executer(["bash", "-lc", command], dossier.absolute, timeoutMs);
				const corps = [
					resultat.stdout.trim(),
					resultat.stderr.trim() ? `--- stderr ---\n${resultat.stderr.trim()}` : "",
				]
					.filter(Boolean)
					.join("\n");
				return {
					content: [text(corps || "(aucune sortie)")],
					structuredContent: {
						command,
						cwd: dossier.relative || ".",
						exitCode: resultat.exitCode,
						ms: resultat.ms,
						stdout: resultat.stdout,
						stderr: resultat.stderr,
					},
					isError: resultat.exitCode !== 0,
				};
			},
		}),

		defineTool({
			name: "ops_service",
			title: "Agir sur un service systemd",
			description:
				"Démarre, arrête, redémarre ou recharge une unité systemd du périmètre Rose Griffon. Redémarrer le wiki ou le site coupe le service quelques secondes : vérifier ops_status ensuite.",
			scope: "admin",
			inputSchema: z.object({
				service: z.enum(KNOWN_SERVICES).describe("Unité systemd du périmètre."),
				action: z.enum(SERVICE_ACTIONS).describe("Action à appliquer."),
			}),
			annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: true },
			handler: async ({ service, action }) => {
				audit(`mcp-admin ops_service unit=${service} action=${action}`);
				const resultat = await executer(["sudo", "-n", "systemctl", action, service], root, 60_000);
				if (resultat.exitCode !== 0) {
					return toolError(
						`systemctl ${action} ${service} a échoué (code ${resultat.exitCode}) : ${resultat.stderr || resultat.stdout}`,
					);
				}
				const etat = await executer(["systemctl", "is-active", service], root, 10_000);
				return structured({ service, action, active: etat.stdout.trim(), ms: resultat.ms });
			},
		}),

		defineTool({
			name: "access_info",
			title: "Portée d'accès de la connexion",
			description:
				"Indique la portée accordée à la connexion courante (`read` ou `admin`) et les outils correspondants. Utile pour savoir si l'écriture est disponible avant de la tenter.",
			inputSchema: z.object({}),
			annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
			handler: (_args, context) =>
				structured({
					scope: context.scope,
					writable: context.scope === "admin",
					protocolVersion: context.meta.protocolVersion,
					era: context.meta.era,
					client: context.meta.clientInfo ?? null,
				}),
		}),
	];
}
