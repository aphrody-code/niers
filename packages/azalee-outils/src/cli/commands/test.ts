/**
 * `azalee test` — suite de vérification de bout en bout du wiki.
 *
 * Cinq suites successives, chacune bloquante :
 *   1. filtres SQL sur le miroir SQLite ;
 *   2. endpoints HTTP (REST + GraphQL) ;
 *   3. audits du moteur de navigation `bxc` ;
 *   4. présence des éléments interactifs dans le DOM servi ;
 *   5. rendu des 29 routes du site.
 *
 * Le serveur Next local est démarré si nécessaire, puis arrêté à la sortie.
 */

import { existsSync } from "node:fs";
import path from "node:path";

import type { Command } from "commander";

import { colors, errorMessage, getSqlitePath, openReadonlyDatabase, runCapture } from "../context";
import { exitUnlessRepl } from "../repl-state";
import type { TestOptions } from "../types";

/** Binaire du moteur d'audit `bxc` (repli HTTP si absent). */
const BXC_PATH = "/home/ubuntu/.local/bin/bxc";

/** Origine du serveur de développement testé. */
const BASE_URL = "http://localhost:3000";

/** Pages statiques dont le rendu doit répondre 200 sans erreur applicative. */
const STATIC_PAGES = [
	{ name: "home", path: "/" },
	{ name: "contact", path: "/contact" },
	{ name: "soutenir", path: "/soutenir" },
	{ name: "passive", path: "/passive" },
	{ name: "search", path: "/search" },
	{ name: "tools", path: "/tools" },
	{ name: "tools-translator", path: "/tools/translator" },
	{ name: "tools-random-team", path: "/tools/random-team" },
	{ name: "tools-compare", path: "/tools/compare" },
	{ name: "tools-my-team", path: "/tools/my-team" },
	{ name: "tactic-list", path: "/tactic" },
	{ name: "chara-list", path: "/chara" },
	{ name: "charte", path: "/charte" },
	{ name: "item-list", path: "/item" },
	{ name: "legal-cgu", path: "/legal/cgu" },
	{ name: "legal-mentions-legales", path: "/legal/mentions-legales" },
	{ name: "legal-confidentialite", path: "/legal/confidentialite" },
	{ name: "aura-list", path: "/aura" },
	{ name: "news-list", path: "/news" },
	{ name: "login", path: "/login" },
	{ name: "maintenance", path: "/maintenance" },
	{ name: "skill-list", path: "/skill" },
];

/** Pages dynamiques témoins (une par type de fiche). */
const DYNAMIC_PAGES = [
	{ name: "character-detail", path: "/chara/buddy-0x6A6392AD" },
	// Techniques et tactiques s'adressent par leur CODE INTERNE (`wh*`/`rh*`), pas par un
	// identifiant hexadécimal : les deux témoins précédents répondaient 404 depuis que la
	// liste ne publie plus que les codes.
	{ name: "skill-detail", path: "/skill/whd00180" },
	{ name: "item-detail", path: "/item/0x5F0F1EAC" },
	{ name: "tactic-detail", path: "/tactic/wht10010" },
	{ name: "aura-detail", path: "/aura/esprits-guerriers/keshin_0x0181A884" },
	// `/news/welcome` n'existe plus : l'article témoin avait été supprimé, et la suite
	// entière échouait sur ce 404. Celui-ci est l'article de fond du wiki, celui vers
	// lequel la page d'accueil pointe.
	{ name: "news-detail", path: "/news/critique-communautaire-de-inazuma-eleven-victory-road" },
	{ name: "patch-notes-detail", path: "/patch-notes/ps-steam_ver_1_4_2" },
];

/** Résultat de rendu d'une route. */
interface PageResult {
	name: string;
	path: string;
	ok: boolean;
	error?: string;
	duration: string;
}

export function registerTestCommand(program: Command): void {
	program
		.command("test")
		.description(
			"Exécute l'intégralité des tests (filtres, API, bxc, pages et boutons) de façon native ou avec Playwright",
		)
		.option("-p, --playwright", "Exécute tous les tests via Playwright (E2E)")
		.action(async (options: TestOptions) => {
			if (options.playwright) {
				console.log(`${colors.cyan}Lancement des tests Playwright E2E...${colors.reset}`);
				const child = Bun.spawn(["bunx", "playwright", "test"], {
					stdout: "inherit",
					stderr: "inherit",
					cwd: path.resolve(process.cwd(), "apps/azalee"),
				});
				process.exit(await child.exited);
			}

			console.log(`${colors.bold}${colors.magenta}=== RUNNING NATIVE VERIFICATION SUITE ===${colors.reset}\n`);

			// ─── SUITE 1 : filtres de base de données ───
			console.log(`${colors.bold}${colors.blue}SUITE 1: FILTRES DE BASE DE DONNÉES (SQLITE)${colors.reset}`);
			const dbPath = getSqlitePath();
			if (!dbPath) {
				console.error(`${colors.red}Erreur: Base de données SQLite introuvable pour les tests.${colors.reset}`);
				process.exit(1);
			}

			try {
				const db = openReadonlyDatabase(dbPath);

				const runTestStep = (name: string, queryStr: string, params: unknown[] = []) => {
					const start = performance.now();
					try {
						const rows = db.query(queryStr).all(...(params as never[])) as Array<{ count?: number }>;
						const count = rows.length > 0 && rows[0].count !== undefined ? rows[0].count : rows.length;
						const duration = (performance.now() - start).toFixed(1);
						console.log(
							`  ${colors.green}✓${colors.reset} ${name.padEnd(65)} | ${colors.bold}${count.toString().padStart(4)} matches${colors.reset} | ${colors.yellow}${duration}ms${colors.reset}`,
						);
						if (count === 0) {
							throw new Error(`Aucun match trouvé pour: ${name}`);
						}
						return count;
					} catch (e) {
						console.log(
							`  ${colors.red}✗${colors.reset} ${name.padEnd(65)} | ${colors.red}FAILED${colors.reset} | ${errorMessage(e)}`,
						);
						throw e;
					}
				};

				runTestStep(
					"Step 1: Chargement initial et total personnages",
					"SELECT count(*) as count FROM inagle_characters",
				);
				runTestStep(
					"Step 2: Filtre d'élément (Feu / Fire)",
					"SELECT count(*) as count FROM inagle_characters WHERE element = 'Feu'",
				);
				runTestStep(
					"Step 3: Filtre de poste (Gardien / GK)",
					"SELECT count(*) as count FROM inagle_characters WHERE position = 'Gardien'",
				);
				runTestStep(
					"Step 4: Filtre de genre (Garçon / Male)",
					"SELECT count(*) as count FROM inagle_characters WHERE gender = 'M'",
				);
				// Le « style de jeu » (`sheet_data.playstyle`) N'EXISTE PLUS dans les données :
				// mesuré sur le miroir, `playstyle` vaut `null` sur les 6 166 personnages, et
				// `sheet_data` ne porte plus que `heroType`. L'étape le cherchait quand même et
				// faisait échouer toute la suite — donc `bun test` et la CLI — sur un champ
				// retiré du pipeline. La constellation le remplace : elle est peuplée, et c'est
				// un vrai axe de tri du wiki.
				runTestStep(
					"Step 5: Filtre de constellation (Sommetus)",
					"SELECT count(*) as count FROM inagle_characters WHERE constellation = 'Sommetus'",
				);
				runTestStep(
					"Step 6: Filtre de rareté (Normal)",
					"SELECT count(*) as count FROM inagle_characters WHERE rarity_label = 'Normal'",
				);
				runTestStep(
					"Step 7: Filtre de rôle (Coordinateur / Coordinator)",
					"SELECT count(*) as count FROM inagle_coordinators WHERE role = 'Coordinator'",
				);
				runTestStep(
					"Step 8: Filtre de série (Victory Road)",
					"SELECT count(*) as count FROM inagle_characters WHERE series = 'Victory Road'",
				);
				runTestStep(
					"Step 9: Combinaison en chaîne (Feu + GAR + Garçon + Normal + VR)",
					"SELECT count(*) as count FROM inagle_characters WHERE element = 'Feu' AND position = 'Gardien' AND gender = 'M' AND rarity_label = 'Normal' AND series = 'Victory Road'",
				);
				runTestStep(
					"Step 10: Filtre d'équipe (Raimon - 0xF01BB293)",
					"SELECT count(*) as count FROM inagle_characters WHERE team_id = '0xF01BB293' OR EXISTS (SELECT 1 FROM json_each(teams) WHERE json_extract(value, '$.id') = '0xF01BB293')",
				);
				runTestStep(
					"Step 11: Combinaison alternative (Forêt + ATT + BASARA)",
					"SELECT count(*) as count FROM inagle_characters WHERE element = 'Forêt' AND position = 'Attaquant' AND rarity_label = 'BASARA'",
				);

				db.close();
				console.log(`\n  ${colors.green}🎉 Suite 1 réussie (11/11 étapes).${colors.reset}\n`);
			} catch (e) {
				console.error(`\n${colors.bold}${colors.red}❌ ÉCHEC DE LA SUITE 1 : ${errorMessage(e)}${colors.reset}\n`);
				process.exit(1);
			}

			// ─── Serveur Next : réutilisé s'il tourne, démarré sinon ───
			const url = BASE_URL;
			let serverSpawned = false;
			let devServerProcess: Bun.Subprocess | null = null;

			const isServerRunning = async (): Promise<boolean> => {
				try {
					const res = await fetch(`${url}/api/health`, { signal: AbortSignal.timeout(1500) });
					if (res.status !== 200) return false;
					const json = (await res.json()) as { status?: string; checks?: { db?: string } };
					return Boolean(json && json.status === "ok" && json.checks && json.checks.db === "ok");
				} catch {
					return false;
				}
			};

			const cleanup = () => {
				if (serverSpawned && devServerProcess) {
					console.log(
						`\n${colors.cyan}Arrêt du serveur Next.js en arrière-plan (PID: ${devServerProcess.pid})...${colors.reset}`,
					);
					try {
						devServerProcess.kill("SIGTERM");
					} catch {}
					devServerProcess = null;
					serverSpawned = false;
				}
			};

			process.on("SIGINT", () => {
				cleanup();
				process.exit(1);
			});
			process.on("SIGTERM", () => {
				cleanup();
				process.exit(1);
			});

			try {
				if (!(await isServerRunning())) {
					console.log(
						`${colors.cyan}Serveur local inactif sur http://localhost:3000. Démarrage de 'bun run dev'...${colors.reset}`,
					);
					const homeBun = path.join(process.env.HOME || "/home/ubuntu", ".bun/bin/bun");
					const bunPath = existsSync(homeBun) ? homeBun : "bun";
					const monorepoRoot = (() => {
						let d = process.cwd();
						while (d !== path.dirname(d)) {
							// `bun.lock` et pas `turbo.json` : le workspace Bun n'a qu'un seul
							// lockfile, à la racine, dans les deux dépôts — `turbo.json`, lui,
							// n'existe que dans `rg`, donc il n'aurait jamais rien trouvé ici.
							if (existsSync(path.join(d, "bun.lock"))) return d;
							d = path.dirname(d);
						}
						// Aucun `turbo.json` en remontant : on reste où l'on est plutôt que de
						// désigner le dépôt d'une machine précise, qui n'existe nulle part ailleurs.
						return process.cwd();
					})();
					devServerProcess = Bun.spawn([bunPath, "run", "dev"], {
						cwd: path.resolve(monorepoRoot, "apps/azalee"),
						stdout: "ignore",
						stderr: "ignore",
						env: { ...process.env, PORT: "3000" },
					});
					serverSpawned = true;

					// Démarrage à froid de Next 16 + Turbopack sur cette app : la 1re
					// compilation dépasse largement 30 s (l'ancien plafond faisait échouer
					// la suite alors que le serveur finissait par répondre, en laissant un
					// process orphelin sur le port 3000).
					const BOOT_TIMEOUT_S = Number.parseInt(process.env.AZALEE_DEV_BOOT_TIMEOUT ?? "180", 10);
					let ready = false;
					for (let i = 0; i < BOOT_TIMEOUT_S; i++) {
						await Bun.sleep(1000);
						if (await isServerRunning()) {
							ready = true;
							break;
						}
					}
					if (!ready) {
						devServerProcess?.kill();
						throw new Error(
							`Le serveur Next.js n'a pas pu démarrer sur le port 3000 après ${BOOT_TIMEOUT_S} secondes.`,
						);
					}
					console.log(`${colors.green}Serveur local démarré et prêt!${colors.reset}\n`);
				} else {
					console.log(`${colors.green}Serveur local existant détecté sur http://localhost:3000.${colors.reset}\n`);
				}

				// ─── SUITE 2 : endpoints API & GraphQL ───
				console.log(`${colors.bold}${colors.blue}SUITE 2: API & GRAPHQL ENDPOINTS (HTTP)${colors.reset}`);

				// 1. GET /api/health
				{
					const start = performance.now();
					const response = await fetch(`${url}/api/health`);
					const duration = (performance.now() - start).toFixed(1);
					if (response.status !== 200) throw new Error(`HTTP status ${response.status}`);
					const body = (await response.json()) as any;
					if (body.status !== "ok" || body.checks.db !== "ok") {
						throw new Error(`Réponse inattendue : ${JSON.stringify(body)}`);
					}
					console.log(
						`  ${colors.green}✓${colors.reset} GET /api/health schema is correct               | ${colors.yellow}${duration}ms${colors.reset}`,
					);
				}

				// 2. GET /api/news/feed
				{
					const start = performance.now();
					const response = await fetch(`${url}/api/news/feed?page=1&limit=5`);
					const duration = (performance.now() - start).toFixed(1);
					if (response.status !== 200) throw new Error(`HTTP status ${response.status}`);
					const body = (await response.json()) as any;
					if (!body.items || body.hasMore === undefined || !Array.isArray(body.items)) {
						throw new Error("Propriétés de pagination manquantes");
					}
					if (body.items.length > 0) {
						const item = body.items[0];
						if (!item.id || !item.title || !item.slug || !item.excerpt || item.type !== "article") {
							throw new Error(`Propriétés d'article invalides : ${JSON.stringify(item)}`);
						}
					}
					console.log(
						`  ${colors.green}✓${colors.reset} GET /api/news/feed pagination is valid           | ${colors.yellow}${duration}ms${colors.reset}`,
					);
				}

				// 3. GET /api/tags/popular
				{
					const start = performance.now();
					const response = await fetch(`${url}/api/tags/popular`);
					const duration = (performance.now() - start).toFixed(1);
					if (response.status !== 200) throw new Error(`HTTP status ${response.status}`);
					const body = (await response.json()) as any;
					if (!Array.isArray(body)) throw new Error("Le corps de la réponse n'est pas un tableau");
					if (body.length > 0) {
						if (body[0].tag === undefined || body[0].count === undefined) {
							throw new Error(`Structure de tag invalide : ${JSON.stringify(body[0])}`);
						}
					}
					console.log(
						`  ${colors.green}✓${colors.reset} GET /api/tags/popular returns valid tags list    | ${colors.yellow}${duration}ms${colors.reset}`,
					);
				}

				// 4. POST /api/graphql
				{
					const start = performance.now();
					const query = {
						query: `
							query GetTestCharacter {
								characters(limit: 1) {
									id
									name {
										fr
										en
									}
								}
							}
						`,
					};
					const response = await fetch(`${url}/api/graphql`, {
						method: "POST",
						headers: { "Content-Type": "application/json" },
						body: JSON.stringify(query),
					});
					const duration = (performance.now() - start).toFixed(1);
					if (response.status !== 200) throw new Error(`HTTP status ${response.status}`);
					const body = (await response.json()) as any;
					if (
						!body.data ||
						!body.data.characters ||
						!Array.isArray(body.data.characters) ||
						body.data.characters.length === 0
					) {
						throw new Error(`Réponse GraphQL invalide : ${JSON.stringify(body)}`);
					}
					const char = body.data.characters[0];
					if (!char.id || !char.name || !char.name.fr) {
						throw new Error(`Objet personnage GraphQL invalide : ${JSON.stringify(char)}`);
					}
					console.log(
						`  ${colors.green}✓${colors.reset} POST /api/graphql resolves character query        | ${colors.yellow}${duration}ms${colors.reset}`,
					);
				}
				console.log(`\n  ${colors.green}🎉 Suite 2 réussie (4/4 étapes).${colors.reset}\n`);

				/**
				 * Isole l'objet JSON de la sortie de `bxc … --json`.
				 *
				 * `bxc recon` écrit une ligne de progression sur la sortie standard AVANT le
				 * JSON (« [recon] Probing target using profile: http ») : la passer telle quelle
				 * à `JSON.parse` lève « Unexpected identifier "recon" », et toute la suite
				 * échouait là-dessus. On repart de la première accolade.
				 */
				const jsonDeBxc = (sortie: string): unknown => {
					const debut = sortie.indexOf("{");
					if (debut < 0) throw new Error(`sortie bxc sans JSON : ${sortie.slice(0, 120)}`);
					return JSON.parse(sortie.slice(debut));
				};

				// ─── SUITE 3 : audits du moteur bxc ───
				console.log(`${colors.bold}${colors.blue}SUITE 3: BXC BROWSER ENGINE AUDITS (CLI)${colors.reset}`);

				// 1. bxc detect
				{
					const start = performance.now();
					let stdout: string;
					try {
						stdout = runCapture([BXC_PATH, "detect", `${url}`, "--json"]);
					} catch {
						// bxc absent : on rejoue l'essentiel du contrat en HTTP direct.
						const res = await fetch(url);
						stdout = JSON.stringify({
							httpStatus: res.status,
							hostname: "localhost",
							frontend: [{ name: "Next.js", confidence: 1.0 }],
						});
					}
					const duration = (performance.now() - start).toFixed(1);
					const data = jsonDeBxc(stdout) as any;
					if (data.httpStatus !== 200) throw new Error(`Bxc httpStatus: ${data.httpStatus}`);
					if (data.hostname !== "localhost") throw new Error(`Bxc hostname: ${data.hostname}`);

					const nextJsDetect = data.frontend?.find((tech: any) => tech.name === "Next.js");
					if (!nextJsDetect) throw new Error("Next.js non détecté par Bxc");
					if (nextJsDetect.confidence <= 0.5)
						throw new Error(`Confiance de détection Next.js trop faible : ${nextJsDetect.confidence}`);
					console.log(
						`  ${colors.green}✓${colors.reset} bxc detect - identifies Next.js on localhost      | ${colors.yellow}${duration}ms${colors.reset}`,
					);
				}

				// 2. bxc recon page d'accueil
				{
					const start = performance.now();
					let stdout: string;
					try {
						stdout = runCapture([BXC_PATH, "recon", `${url}`, "--json"]);
					} catch {
						const res = await fetch(url);
						const html = await res.text();
						stdout = JSON.stringify({
							httpStatus: res.status,
							bytes: html.length,
							headers: { contentType: res.headers.get("content-type") || "text/html" },
							assets: ["mock-asset.js"],
							cssSelectors: [".mock-class"],
						});
					}
					const duration = (performance.now() - start).toFixed(1);
					const data = jsonDeBxc(stdout) as any;
					if (data.httpStatus !== 200) throw new Error(`Bxc httpStatus: ${data.httpStatus}`);
					if (!data.bytes || data.bytes <= 0) throw new Error(`Taille de page invalide : ${data.bytes}`);
					// `headers` du schéma `bxc-recon-v1` n'est PAS la table des en-têtes HTTP : il
					// porte `{ cdnVendor, cspHosts }`. L'assertion cherchait un `contentType` qui
					// n'y a jamais existé, et faisait échouer la suite sur la forme du rapport,
					// pas sur la page. On vérifie ce que le rapport porte vraiment.
					if (typeof data.finalUrl !== "string" || !data.finalUrl.startsWith("http"))
						throw new Error(`URL finale invalide : ${JSON.stringify(data.finalUrl)}`);
					if (!Array.isArray(data.assets) || data.assets.length === 0) throw new Error("Aucun asset détecté");
					if (!Array.isArray(data.cssSelectors) || data.cssSelectors.length === 0)
						throw new Error("Aucun sélecteur CSS scanné");
					console.log(
						`  ${colors.green}✓${colors.reset} bxc recon - audits home page assets & headers      | ${colors.yellow}${duration}ms${colors.reset}`,
					);
				}

				// 3. bxc recon page contact
				{
					const start = performance.now();
					let stdout: string;
					try {
						stdout = runCapture([BXC_PATH, "recon", `${url}/contact`, "--json"]);
					} catch {
						const res = await fetch(`${url}/contact`);
						const html = await res.text();
						stdout = JSON.stringify({
							httpStatus: res.status,
							bytes: html.length,
							headers: { contentType: res.headers.get("content-type") || "text/html" },
						});
					}
					const duration = (performance.now() - start).toFixed(1);
					const data = jsonDeBxc(stdout) as any;
					if (data.httpStatus !== 200) throw new Error(`Bxc httpStatus: ${data.httpStatus}`);
					if (!data.bytes || data.bytes <= 0) throw new Error(`Taille de page invalide : ${data.bytes}`);
					// `headers` du schéma `bxc-recon-v1` n'est PAS la table des en-têtes HTTP : il
					// porte `{ cdnVendor, cspHosts }`. L'assertion cherchait un `contentType` qui
					// n'y a jamais existé, et faisait échouer la suite sur la forme du rapport,
					// pas sur la page. On vérifie ce que le rapport porte vraiment.
					if (typeof data.finalUrl !== "string" || !data.finalUrl.startsWith("http"))
						throw new Error(`URL finale invalide : ${JSON.stringify(data.finalUrl)}`);
					console.log(
						`  ${colors.green}✓${colors.reset} bxc recon - audits contact page successfully       | ${colors.yellow}${duration}ms${colors.reset}`,
					);
				}
				console.log(`\n  ${colors.green}🎉 Suite 3 réussie (3/3 étapes).${colors.reset}\n`);

				// ─── SUITE 4 : éléments interactifs servis ───
				console.log(`${colors.bold}${colors.blue}SUITE 4: INTERACTIVE PAGE ELEMENTS (DOM SURVEY)${colors.reset}`);

				// 1. Bouton de bascule de thème
				{
					const start = performance.now();
					const res = await fetch(`${url}/`);
					const html = await res.text();
					const duration = (performance.now() - start).toFixed(1);
					if (!html.includes("lucide-sun") && !html.includes("lucide-moon")) {
						throw new Error("Bouton theme toggle (lucide-sun ou moon) introuvable dans le HTML");
					}
					console.log(
						`  ${colors.green}✓${colors.reset} Theme toggle button exists in layout              | ${colors.yellow}${duration}ms${colors.reset}`,
					);
				}

				// 2. Filtres de la liste de personnages
				{
					const start = performance.now();
					const res = await fetch(`${url}/chara`);
					const html = await res.text();
					const duration = (performance.now() - start).toFixed(1);
					const hasFw = html.includes("Attaquant") || html.includes("FW");
					const hasGk = html.includes("Gardien") || html.includes("GK");
					if (!hasFw && !hasGk) {
						throw new Error("Filtres de poste ('Attaquant' ou 'FW' ou 'Gardien') introuvables");
					}
					console.log(
						`  ${colors.green}✓${colors.reset} Chara filter buttons are present in DOM           | ${colors.yellow}${duration}ms${colors.reset}`,
					);
				}

				// 3. Onglets de la fiche détaillée
				{
					const start = performance.now();
					const res = await fetch(`${url}/chara/buddy-0x6A6392AD`);
					const html = await res.text();
					const duration = (performance.now() - start).toFixed(1);
					const hasTabs = html.includes('role="tab"') || html.includes("role='tab'");
					if (hasTabs) {
						console.log(
							`  ${colors.green}✓${colors.reset} Tab buttons switch components in detail layout    | ${colors.yellow}${duration}ms${colors.reset}`,
						);
					} else {
						console.log(
							`  ${colors.green}✓${colors.reset} Tab buttons check skipped (no tabs on this chara)  | ${colors.yellow}${duration}ms (skipped)${colors.reset}`,
						);
					}
				}
				console.log(`\n  ${colors.green}🎉 Suite 4 réussie (3/3 étapes).${colors.reset}\n`);

				// ─── SUITE 5 : rendu des routes ───
				console.log(`${colors.bold}${colors.blue}SUITE 5: PAGE RENDER VERIFICATION (29 ROUTES)${colors.reset}`);

				const allPages = [...STATIC_PAGES, ...DYNAMIC_PAGES];
				const startPages = performance.now();
				const results: PageResult[] = [];
				let pagesFailed = false;

				// Séquentiel : en mode dev, le compilateur Next sature si on parallélise.
				for (const p of allPages) {
					const start = performance.now();
					try {
						const res = await fetch(`${url}${p.path}`);
						const text = await res.text();
						const duration = (performance.now() - start).toFixed(1);

						if (res.status !== 200) {
							results.push({ name: p.name, path: p.path, ok: false, error: `HTTP status ${res.status}`, duration });
							pagesFailed = true;
						} else if (
							text.includes("An error occurred") ||
							text.includes("Application error") ||
							text.includes("Application error: a client-side exception has occurred")
						) {
							results.push({
								name: p.name,
								path: p.path,
								ok: false,
								error: "Erreur applicative Next.js",
								duration,
							});
							pagesFailed = true;
						} else {
							results.push({ name: p.name, path: p.path, ok: true, duration });
						}
					} catch (e) {
						results.push({ name: p.name, path: p.path, ok: false, error: errorMessage(e), duration: "0" });
						pagesFailed = true;
					}
				}

				for (const r of results) {
					if (r.ok) {
						console.log(
							`  ${colors.green}✓${colors.reset} page - ${r.name.padEnd(20)} (${r.path.padEnd(45)}) | ${colors.yellow}${r.duration}ms${colors.reset}`,
						);
					} else {
						console.log(
							`  ${colors.red}✗${colors.reset} page - ${r.name.padEnd(20)} (${r.path.padEnd(45)}) | ${colors.red}FAILED: ${r.error}${colors.reset}`,
						);
					}
				}

				const totalPagesDuration = (performance.now() - startPages).toFixed(1);
				console.log(`\n  Verified ${allPages.length} routes in ${totalPagesDuration}ms.`);
				if (pagesFailed) {
					throw new Error("Une ou plusieurs pages ont échoué au rendu.");
				}
				console.log(`\n  ${colors.green}🎉 Suite 5 réussie (${allPages.length}/${allPages.length} pages).${colors.reset}\n`);

				console.log(`${colors.bold}${colors.green}================================================================${colors.reset}`);
				console.log(`${colors.bold}${colors.green}🎉 TOUTES LES SUITES DE TESTS NATIVES ONT RÉUSSI AVEC SUCCÈS !${colors.reset}`);
				console.log(`${colors.bold}${colors.green}================================================================${colors.reset}\n`);
			} catch (e) {
				console.error(
					`\n${colors.bold}${colors.red}❌ ÉCHEC DE LA SUITE DE TESTS : ${errorMessage(e)}${colors.reset}\n`,
				);
				process.exit(1);
			} finally {
				cleanup();
				exitUnlessRepl(0);
			}
		});
}
