/** `azalee random-team` — génération d'une équipe aléatoire complète. */

import type { Command } from "commander";

import {
	colors,
	errorMessage,
	getSqlitePath,
	openReadonlyDatabase,
	reportError,
	restoreLogs,
	suppressLogs,
} from "../context";
import type { RandomTeamOptions } from "../types";

/**
 * Répartitions jouables : `df`/`mf`/`fw` sont les effectifs par ligne (le
 * gardien est implicite). `def`/`off` sont les indices tactiques du jeu.
 * Table locale au tirage : les `FORMATIONS` de la lib décrivent des
 * *coordonnées* d'éditeur, pas des effectifs par ligne.
 */
interface RandomTeamLayout {
	name: string;
	layout: string;
	df: number;
	mf: number;
	fw: number;
	def: number;
	off: number;
}

const LAYOUTS: readonly RandomTeamLayout[] = [
	{ def: 3, df: 4, fw: 2, layout: "4-4-2", mf: 4, name: "4-4-2", off: 3 },
	{ def: 2, df: 4, fw: 3, layout: "4-3-3", mf: 3, name: "4-3-3", off: 4 },
	{ def: 2, df: 3, fw: 3, layout: "3-4-3", mf: 4, name: "3-4-3", off: 4 },
	{ def: 3, df: 3, fw: 2, layout: "3-5-2", mf: 5, name: "3-5-2", off: 3 },
	{ def: 4, df: 4, fw: 1, layout: "4-5-1", mf: 5, name: "4-5-1", off: 2 },
	{ def: 3, df: 3, fw: 1, layout: "3-6-1", mf: 6, name: "3-6-1", off: 3 },
	{ def: 4, df: 5, fw: 1, layout: "5-4-1", mf: 4, name: "5-4-1", off: 2 },
	{ def: 1, df: 2, fw: 4, layout: "2-4-4", mf: 4, name: "2-4-4", off: 5 },
	{ def: 2, df: 2, fw: 3, layout: "2-5-3", mf: 5, name: "2-5-3", off: 4 },
	{ def: 4, df: 5, fw: 4, layout: "5-1-4", mf: 1, name: "5-1-4", off: 2 },
	{ def: 5, df: 3, fw: 4, layout: "3-3-4", mf: 3, name: "3-3-4", off: 1 },
	{ def: 3, df: 5, fw: 2, layout: "5-3-2", mf: 3, name: "5-3-2", off: 3 },
];

/** Alias FR/EN acceptés par `--element`, vers le libellé canonique FR. */
const ELEMENT_ALIASES: Record<string, string> = {
	fire: "Feu",
	feu: "Feu",
	wind: "Vent",
	vent: "Vent",
	forest: "Forêt",
	forêt: "Forêt",
	foret: "Forêt",
	mountain: "Montagne",
	montagne: "Montagne",
	void: "Néant",
	néant: "Néant",
	neant: "Néant",
};

/** Alias FR/EN acceptés par `--playstyle`, vers le libellé canonique EN. */
const PLAYSTYLE_ALIASES: Record<string, string> = {
	bond: "Bond",
	lien: "Bond",
	justice: "Justice",
	breach: "Breach",
	percée: "Breach",
	percee: "Breach",
	tension: "Tension",
	"rough play": "Rough Play",
	roughplay: "Rough Play",
	"jeu violent": "Rough Play",
	counter: "Counter",
	contre: "Counter",
};

/** Abréviations d'élément affichées sur le terrain ASCII. */
const ELEMENT_ABBR: Record<string, string> = {
	Feu: "Fe",
	Vent: "Ve",
	Forêt: "Fo",
	Montagne: "Mo",
	Néant: "Ne",
};

/** Ligne `inagle_characters` telle que sélectionnée pour le tirage. */
interface PlayerRow {
	id: string;
	name_fr: string | null;
	name_en: string | null;
	element: string | null;
	sheet_data: string | null;
	stat_frappe: number | null;
	stat_controle: number | null;
	stat_technique: number | null;
	stat_pression: number | null;
	stat_physique: number | null;
	stat_agilite: number | null;
	stat_intelligence: number | null;
}

/** Ligne `inagle_coordinators` (coach, manager ou coordinateur). */
interface StaffRow {
	id: string;
	name_localised: string | null;
	name_romaji: string | null;
	element: string | null;
	playstyle: string | null;
	buff: string | null;
	role: string | null;
}

export function registerRandomTeamCommand(program: Command): void {
	program
		.command("random-team")
		.description("Génère une équipe aléatoire complète de 11 joueurs, coach et managers")
		.option("-f, --formation <formation>", "Configuration de la formation (ex: 4-4-2, 4-3-3, 3-5-2)", "4-4-2")
		.option("-e, --element <element>", "Filtrer de préférence par élément (Feu, Vent, Forêt, Montagne)")
		.option(
			"-p, --playstyle <playstyle>",
			"Filtrer de préférence par style (Lien, Justice, Percée, Tension, Jeu violent, Contre)",
		)
		.option("-j, --json", "Format de sortie en JSON brute")
		.action(async (options: RandomTeamOptions) => {
			suppressLogs(!!options.json);
			try {
				const dbPath = getSqlitePath();
				if (!dbPath) {
					reportError(
						options.json,
						"Base de données SQLite introuvable",
						`${colors.red}Erreur: Base de données SQLite introuvable.${colors.reset}`,
					);
					return;
				}

				const db = openReadonlyDatabase(dbPath);

				const elVal = options.element ? ELEMENT_ALIASES[options.element.toLowerCase()] : undefined;
				const psVal = options.playstyle ? PLAYSTYLE_ALIASES[options.playstyle.toLowerCase()] : undefined;

				const layout = options.formation || "4-4-2";
				const formation = LAYOUTS.find((f) => f.name === layout || f.layout === layout) || LAYOUTS[0];

				function pickRandom<T>(arr: T[], n: number): T[] {
					const copy = [...arr];
					const result: T[] = [];
					for (let i = 0; i < n && copy.length > 0; i++) {
						const idx = Math.floor(Math.random() * copy.length);
						result.push(copy.splice(idx, 1)[0]);
					}
					return result;
				}

				/**
				 * Vivier d'un poste. Les filtres sont **préférentiels** : si le
				 * style de jeu ne laisse pas assez de joueurs on le relâche, puis
				 * l'élément — une équipe incomplète serait pire qu'un filtre ignoré.
				 */
				function fetchPool(position: string, requiredCount: number): PlayerRow[] {
					const query = `SELECT id, chara_id, internal_code, name_fr, name_en, element, position, stat_frappe, stat_controle, stat_technique, stat_pression, stat_physique, stat_agilite, stat_intelligence, sheet_data, zukan_hash FROM inagle_characters WHERE position = ? AND stat_frappe IS NOT NULL AND zukan_hash IS NOT NULL`;
					const params: string[] = [position];

					let filterQuery = query;
					const filterParams = [...params];

					if (elVal) {
						filterQuery += " AND element = ?";
						filterParams.push(elVal);
					}
					if (psVal) {
						filterQuery += " AND json_extract(sheet_data, '$.playstyle') = ?";
						filterParams.push(psVal);
					}

					let rows = db.prepare(filterQuery).all(...filterParams) as PlayerRow[];

					if (rows.length < requiredCount && psVal) {
						let fbQuery = query;
						const fbParams = [...params];
						if (elVal) {
							fbQuery += " AND element = ?";
							fbParams.push(elVal);
						}
						rows = db.prepare(fbQuery).all(...fbParams) as PlayerRow[];
					}

					if (rows.length < requiredCount) {
						rows = db.prepare(query).all(...params) as PlayerRow[];
					}

					return rows;
				}

				const gk = pickRandom(fetchPool("Gardien", 1), 1);
				const df = pickRandom(fetchPool("Défenseur", formation.df), formation.df);
				const mf = pickRandom(fetchPool("Milieu", formation.mf), formation.mf);
				const fw = pickRandom(fetchPool("Attaquant", formation.fw), formation.fw);

				const coachesPool = db
					.prepare(
						"SELECT id, name_localised, name_romaji, element, playstyle, buff, role FROM inagle_coordinators WHERE role = 'Coach' OR role = 'Manager'",
					)
					.all() as StaffRow[];
				const managersPool = db
					.prepare(
						"SELECT id, name_localised, name_romaji, element, playstyle, buff, role FROM inagle_coordinators WHERE role = 'Coordinator'",
					)
					.all() as StaffRow[];

				const coach = pickRandom(coachesPool, 1)[0] || null;
				const managers = pickRandom(managersPool, 3);

				db.close();

				const allPlayers = [...gk, ...df, ...mf, ...fw];

				if (options.json) {
					restoreLogs(true);
					const teamJson = {
						formation: formation.layout,
						gk: gk.map((p) => ({ id: p.id, name: p.name_fr || p.name_en, element: p.element })),
						df: df.map((p) => ({ id: p.id, name: p.name_fr || p.name_en, element: p.element })),
						mf: mf.map((p) => ({ id: p.id, name: p.name_fr || p.name_en, element: p.element })),
						fw: fw.map((p) => ({ id: p.id, name: p.name_fr || p.name_en, element: p.element })),
						coach: coach
							? {
									id: coach.id,
									name: coach.name_localised || coach.name_romaji,
									element: coach.element,
									playstyle: coach.playstyle,
									buff: coach.buff,
								}
							: null,
						managers: managers.map((m) => ({
							id: m.id,
							name: m.name_localised || m.name_romaji,
							element: m.element,
							playstyle: m.playstyle,
							buff: m.buff,
						})),
					};
					console.log(JSON.stringify(teamJson, null, 2));
					return;
				}

				restoreLogs(true);

				// Agrégats : moyennes de stats, répartition d'éléments et de styles.
				const statsTotal = {
					frappe: 0,
					controle: 0,
					technique: 0,
					pression: 0,
					physique: 0,
					agilite: 0,
					intelligence: 0,
				};
				const elementCounts: Record<string, number> = {};
				const playstyleCounts: Record<string, number> = {};

				for (const p of allPlayers) {
					let kick = p.stat_frappe || 0;
					let ctrl = p.stat_controle || 0;
					let tech = p.stat_technique || 0;
					let pres = p.stat_pression || 0;
					let phys = p.stat_physique || 0;
					let agil = p.stat_agilite || 0;
					let intel = p.stat_intelligence || 0;

					// `sheet_data` (JSON) prime sur les colonnes plates quand il existe.
					try {
						const sd = typeof p.sheet_data === "string" ? JSON.parse(p.sheet_data) : p.sheet_data;
						if (sd?.stats) {
							kick = sd.stats.kick || kick;
							ctrl = sd.stats.control || ctrl;
							tech = sd.stats.technique || tech;
							pres = sd.stats.pressure || pres;
							phys = sd.stats.physical || phys;
							agil = sd.stats.agility || agil;
							intel = sd.stats.intelligence || intel;
						}
					} catch {}

					statsTotal.frappe += kick;
					statsTotal.controle += ctrl;
					statsTotal.technique += tech;
					statsTotal.pression += pres;
					statsTotal.physique += phys;
					statsTotal.agilite += agil;
					statsTotal.intelligence += intel;

					const el = p.element || "Néant";
					elementCounts[el] = (elementCounts[el] || 0) + 1;

					let ps = "Aucun";
					try {
						const sd = typeof p.sheet_data === "string" ? JSON.parse(p.sheet_data) : p.sheet_data;
						ps = sd?.playstyle || "Aucun";
					} catch {}
					playstyleCounts[ps] = (playstyleCounts[ps] || 0) + 1;
				}

				const pCount = allPlayers.length || 1;
				const statsAvg = {
					frappe: Math.round(statsTotal.frappe / pCount),
					controle: Math.round(statsTotal.controle / pCount),
					technique: Math.round(statsTotal.technique / pCount),
					pression: Math.round(statsTotal.pression / pCount),
					physique: Math.round(statsTotal.physique / pCount),
					agilite: Math.round(statsTotal.agilite / pCount),
					intelligence: Math.round(statsTotal.intelligence / pCount),
				};

				// Synergie d'élément : 3 joueurs minimum, bonus croissant.
				const synergies: string[] = [];
				for (const [el, num] of Object.entries(elementCounts)) {
					if (num >= 3) {
						const boost = num >= 6 ? 15 : num >= 4 ? 10 : 5;
						synergies.push(`${el} x${num} (+${boost}% boost)`);
					}
				}

				function centerText(text: string, width: number): string {
					const padTotal = width - text.length;
					if (padTotal <= 0) return text.substring(0, width);
					const padLeft = Math.floor(padTotal / 2);
					const padRight = padTotal - padLeft;
					return " ".repeat(padLeft) + text + " ".repeat(padRight);
				}

				/** « Mark Evans » → « Mark E. » quand la case est trop étroite. */
				function shortenName(name: string, maxLen: number = 14): string {
					if (name.length <= maxLen) return name;
					const parts = name.split(" ");
					if (parts.length > 1) {
						const first = parts[0];
						const last = parts[parts.length - 1];
						const shortened = `${first} ${last[0]}.`;
						if (shortened.length <= maxLen) return shortened;
					}
					return name.substring(0, maxLen - 1) + "…";
				}

				function formatPlayerForPitch(
					player: PlayerRow,
					positionCode: string,
					maxNameLen: number,
				): { display: string; length: number } {
					const name = player.name_fr || player.name_en || player.id;
					const el = player.element || "Néant";
					const shortName = shortenName(name, maxNameLen);

					const elAbbr = ELEMENT_ABBR[el] || "Ne";
					const raw = `${shortName} (${elAbbr})`;

					const colorMap: Record<string, string> = {
						GK: colors.yellow,
						DF: colors.blue,
						MF: colors.green,
						FW: colors.red,
					};
					const pColor = colorMap[positionCode] || colors.reset;
					const elColor =
						el === "Feu"
							? colors.red
							: el === "Vent"
								? colors.cyan
								: el === "Forêt"
									? colors.green
									: el === "Montagne"
										? colors.yellow
										: colors.reset;

					const display = `${colors.bold}${pColor}${shortName}${colors.reset} (${elColor}${elAbbr}${colors.reset})`;
					return { display, length: raw.length };
				}

				function centerRowOfPlayers(players: PlayerRow[], positionCode: string, totalWidth: number = 78): string {
					const n = players.length;
					if (n === 0) return "";
					const colWidth = Math.floor(totalWidth / n);
					const maxNameLen = Math.max(5, colWidth - 6);

					let rowText = "";
					for (const p of players) {
						const { display, length } = formatPlayerForPitch(p, positionCode, maxNameLen);
						const padTotal = colWidth - length;
						const padLeft = Math.floor(Math.max(0, padTotal) / 2);
						const padRight = Math.max(0, padTotal - padLeft);
						rowText += " ".repeat(padLeft) + display + " ".repeat(padRight);
					}
					const leftOver = totalWidth - n * colWidth;
					if (leftOver > 0) {
						rowText += " ".repeat(leftOver);
					}
					return rowText;
				}

				const fwRowText = centerRowOfPlayers(fw, "FW", 78);
				const mfRowText = centerRowOfPlayers(mf, "MF", 78);
				const dfRowText = centerRowOfPlayers(df, "DF", 78);
				const gkRowText = centerRowOfPlayers(gk, "GK", 78);

				console.log(`\n${colors.green}╔${"═".repeat(78)}╗${colors.reset}`);
				const titleRaw = `TERRAIN DE JEU - VICTORY ROAD (${formation.name})`;
				const titleColored = `${colors.bold}${colors.yellow}TERRAIN DE JEU - VICTORY ROAD (${formation.name})${colors.reset}`;
				const padTotal = 78 - titleRaw.length;
				const padLeft = Math.floor(padTotal / 2);
				const padRight = padTotal - padLeft;
				const titleRowText = " ".repeat(padLeft) + titleColored + " ".repeat(padRight);

				console.log(`${colors.green}║${colors.reset}${titleRowText}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}╠${"═".repeat(78)}╣${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${" ".repeat(78)}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${fwRowText}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${" ".repeat(78)}${colors.green}║${colors.reset}`);
				const lineSepRaw = "─".repeat(60);
				const lineSepPad = centerText(lineSepRaw, 78);
				console.log(`${colors.green}║${colors.reset}${lineSepPad}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${" ".repeat(78)}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${mfRowText}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${" ".repeat(78)}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${lineSepPad}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${" ".repeat(78)}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${dfRowText}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${" ".repeat(78)}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${lineSepPad}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${" ".repeat(78)}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${gkRowText}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}║${colors.reset}${" ".repeat(78)}${colors.green}║${colors.reset}`);
				console.log(`${colors.green}╚${"═".repeat(78)}╝${colors.reset}`);

				console.log(`\n${colors.bold}${colors.cyan}⚙️  CARACTÉRISTIQUES DE L'ÉQUIPE :${colors.reset}`);
				console.log(`  · ${colors.bold}Formation :${colors.reset} ${formation.name} (${formation.layout})`);
				console.log(`  · ${colors.bold}Moyennes de stats (Lv 99) :${colors.reset}`);
				console.log(
					`      Frappe: ${colors.yellow}${statsAvg.frappe}${colors.reset} | Contrôle: ${colors.yellow}${statsAvg.controle}${colors.reset} | Technique: ${colors.yellow}${statsAvg.technique}${colors.reset}`,
				);
				console.log(
					`      Physique: ${colors.yellow}${statsAvg.physique}${colors.reset} | Pression: ${colors.yellow}${statsAvg.pression}${colors.reset} | Agilité: ${colors.yellow}${statsAvg.agilite}${colors.reset} | Intel.: ${colors.yellow}${statsAvg.intelligence}${colors.reset}`,
				);

				console.log(
					`  · ${colors.bold}Synergies d'éléments :${colors.reset} ${synergies.length > 0 ? synergies.map((s) => `${colors.green}${s}${colors.reset}`).join(", ") : "Aucune synergie active (3+ du même élément)"}`,
				);

				console.log(`\n${colors.bold}${colors.cyan}👔 STAFF TECHNIQUE :${colors.reset}`);
				if (coach) {
					console.log(
						`  · [${colors.yellow}COACH${colors.reset}] ${colors.bold}${coach.name_localised || coach.name_romaji}${colors.reset} (${coach.element} | ${coach.playstyle || "N/A"})`,
					);
					if (coach.buff) console.log(`      Buff: ${colors.green}${coach.buff}${colors.reset}`);
				} else {
					console.log(`  · [${colors.yellow}COACH${colors.reset}] Aucun`);
				}
				console.log(`  · [${colors.yellow}MANAGERS${colors.reset}]`);
				for (const m of managers) {
					console.log(
						`      - ${colors.bold}${m.name_localised || m.name_romaji}${colors.reset} (${m.element} | ${m.playstyle || "N/A"})${m.buff ? ` -> Buff: ${colors.green}${m.buff}${colors.reset}` : ""}`,
					);
				}
				console.log();
			} catch (e) {
				reportError(
					options.json,
					errorMessage(e),
					`${colors.red}Erreur random-team : ${errorMessage(e)}${colors.reset}`,
				);
			}
		});
}
