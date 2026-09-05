/**
 * Surface publique du CLI `azalee`.
 *
 * Trois niveaux :
 *
 * 1. **Construction** — `createAzaleeProgram()` assemble le programme sans
 *    l'exécuter : on vérifie la liste des commandes, leur ordre (c'est l'ordre
 *    de `azalee --help`), leurs descriptions et leurs options.
 * 2. **Aide** — le `--help` de chaque commande est rendu sans exécuter d'action,
 *    grâce à `exitOverride()` + `configureOutput()`.
 * 3. **Exécution réelle** — quelques commandes sont lancées comme le ferait un
 *    utilisateur, sur les vraies données locales, et leur contrat de sortie
 *    (`--json` = un seul document JSON sur stdout) est vérifié.
 *
 * S'y ajoutent les helpers purs remontés du CLI vers la lib : leur contrat est
 * désormais public, il doit être testé comme tel.
 */

import { describe, expect, test } from "bun:test";
import path from "node:path";

import { CommanderError, type Command } from "commander";

import { CLI_NAME, CLI_VERSION, createAzaleeProgram } from "../src/cli/program";
import { parseReplArgs } from "../src/cli/commands/shell";
import { renderAsciiTable } from "../src/cli/context";
import { LEGACY_FORMATIONS } from "@rosegriffon/azalee/game/formations";
import { getItemCategoryLabel, ITEM_CATEGORY_LABELS_FR } from "@rosegriffon/azalee/game/item-categories";
import { getPersonalityName, PERSONALITY_NAMES } from "@rosegriffon/azalee/game/personality";
import { interpolateVariantStats } from "@rosegriffon/azalee/game/stats-interpolation";
import { base64ToUtf8, decodeTeamCode, encodeTeamCode, utf8ToBase64 } from "@rosegriffon/azalee/game/team-code";
import { containsJapanese, escapeRegExp, stripRubyAnnotations } from "@rosegriffon/azalee/text/japanese-detect";

/** Chemin du binaire CLI (exécuté tel quel par Bun). */
const CLI_ENTRY = path.resolve(import.meta.dir, "../src/cli.ts");
/** Racine du monorepo : cwd attendu par les commandes qui lisent des données. */
const REPO_ROOT = path.resolve(import.meta.dir, "../../..");

/**
 * Ordre d'enregistrement attendu. C'est un test de contrat : réordonner une
 * commande change l'aide vue par les utilisateurs et les agents.
 */
const EXPECTED_COMMANDS = [
	"translate",
	"search",
	"db",
	"redis",
	"glossary-rebuild",
	"audit",
	"test-variants",
	"rag",
	"status",
	"wave",
	"repair",
	"sync",
	"compare",
	"chara",
	"dialogue",
	"skill",
	"item",
	"team",
	"random-team",
	"team-builder",
	"test",
	"shell",
	"data",
	"serve",
] as const;

/** Rend l'aide d'une commande sans exécuter d'action ni quitter le process. */
function captureHelp(argv: string[]): string {
	const program = createAzaleeProgram();
	let out = "";
	program.exitOverride();
	program.configureOutput({
		writeOut: (str) => {
			out += str;
		},
		writeErr: (str) => {
			out += str;
		},
	});
	for (const cmd of allCommands(program)) {
		cmd.exitOverride();
		cmd.configureOutput({
			writeOut: (str) => {
				out += str;
			},
			writeErr: (str) => {
				out += str;
			},
		});
	}
	try {
		program.parse(["bun", "azalee", ...argv]);
	} catch (e) {
		// `--help` termine toujours par une CommanderError : c'est le chemin nominal.
		if (!(e instanceof CommanderError)) throw e;
	}
	return out;
}

/** Toutes les commandes du programme, sous-commandes comprises. */
function allCommands(program: Command): Command[] {
	const acc: Command[] = [];
	const walk = (cmd: Command) => {
		for (const sub of cmd.commands) {
			acc.push(sub);
			walk(sub);
		}
	};
	walk(program);
	return acc;
}

/** Exécute réellement le CLI et renvoie son code de sortie et sa sortie. */
async function runCli(args: string[]): Promise<{ code: number; stdout: string; stderr: string }> {
	const proc = Bun.spawn(["bun", CLI_ENTRY, ...args], {
		cwd: REPO_ROOT,
		stdout: "pipe",
		stderr: "pipe",
		env: { ...process.env, NO_COLOR: "1" },
	});
	const [stdout, stderr, code] = await Promise.all([
		new Response(proc.stdout).text(),
		new Response(proc.stderr).text(),
		proc.exited,
	]);
	return { code, stdout, stderr };
}

describe("createAzaleeProgram", () => {
	test("expose le nom, la description et la version du CLI", () => {
		const program = createAzaleeProgram();
		expect(program.name()).toBe(CLI_NAME);
		expect(program.name()).toBe("azalee");
		expect(program.description()).toBe("CLI d'Azalée pour Inazuma Eleven: Victory Road");
		expect(program.version()).toBe(CLI_VERSION);
	});

	test("enregistre exactement les 24 commandes attendues, dans l'ordre", () => {
		const program = createAzaleeProgram();
		expect(program.commands.map((c) => c.name())).toEqual([...EXPECTED_COMMANDS]);
	});

	test("`shell` conserve son alias `repl`", () => {
		const program = createAzaleeProgram();
		const shell = program.commands.find((c) => c.name() === "shell");
		expect(shell?.aliases()).toEqual(["repl"]);
	});

	test("`data` expose ses 7 sous-commandes de pipeline", () => {
		const program = createAzaleeProgram();
		const data = program.commands.find((c) => c.name() === "data");
		expect(data?.commands.map((c) => c.name())).toEqual([
			"push",
			"migrate",
			"load",
			"sync",
			"typecheck",
			"verify",
			"all",
		]);
	});

	test("chaque commande porte une description non vide", () => {
		const program = createAzaleeProgram();
		for (const cmd of allCommands(program)) {
			expect(cmd.description().length).toBeGreaterThan(0);
		}
	});

	test("construire le programme deux fois donne deux instances indépendantes", () => {
		const a = createAzaleeProgram();
		const b = createAzaleeProgram();
		expect(a).not.toBe(b);
		expect(a.commands.length).toBe(b.commands.length);
	});
});

describe("options déclarées", () => {
	/** Drapeaux attendus, par commande. */
	const EXPECTED_FLAGS: Record<string, string[]> = {
		translate: ["-j, --json"],
		search: ["-j, --json"],
		db: ["-s, --sqlite", "-j, --json"],
		redis: ["-j, --json"],
		"glossary-rebuild": ["-j, --json"],
		audit: ["-j, --json"],
		"test-variants": [],
		rag: ["-j, --json", "-l, --limit <limit>"],
		status: ["-j, --json"],
		wave: ["-c, --cycle", "-l, --loop"],
		repair: [],
		sync: ["-p, --push", "-j, --json"],
		compare: ["-l, --level <level>", "-j, --json"],
		chara: ["-j, --json"],
		dialogue: ["-s, --speaker <speaker>", "-j, --json", "-l, --limit <limit>"],
		skill: ["-j, --json"],
		item: ["-j, --json"],
		team: ["-j, --json"],
		"random-team": ["-f, --formation <formation>", "-e, --element <element>", "-p, --playstyle <playstyle>", "-j, --json"],
		"team-builder": ["-j, --json", "-f, --formation <formation>"],
		test: ["-p, --playwright"],
		shell: [],
		data: [],
		serve: ["-p, --port <port>", "-H, --host <host>", "--cors <origin>", "-j, --json"],
	};

	test("chaque commande déclare exactement ses drapeaux", () => {
		const program = createAzaleeProgram();
		for (const [name, flags] of Object.entries(EXPECTED_FLAGS)) {
			const cmd = program.commands.find((c) => c.name() === name);
			expect(cmd, `commande ${name} absente`).toBeDefined();
			expect(cmd!.options.map((o) => o.flags), `drapeaux de ${name}`).toEqual(flags);
		}
	});

	test("les valeurs par défaut documentées sont bien posées", () => {
		const program = createAzaleeProgram();
		const optionOf = (cmdName: string, long: string) =>
			program.commands.find((c) => c.name() === cmdName)?.options.find((o) => o.long === long);

		expect(optionOf("rag", "--limit")?.defaultValue).toBe("4");
		expect(optionOf("compare", "--level")?.defaultValue).toBe("99");
		expect(optionOf("dialogue", "--limit")?.defaultValue).toBe("10");
		expect(optionOf("random-team", "--formation")?.defaultValue).toBe("4-4-2");
		expect(optionOf("serve", "--host")?.defaultValue).toBe(process.env.AZALEE_HOST ?? "127.0.0.1");
		expect(optionOf("serve", "--port")?.defaultValue).toBe(process.env.AZALEE_PORT ?? "3010");
	});

	test("`data migrate` et `data sync` déclarent leurs drapeaux longs", () => {
		const program = createAzaleeProgram();
		const data = program.commands.find((c) => c.name() === "data");
		const migrate = data?.commands.find((c) => c.name() === "migrate");
		const sync = data?.commands.find((c) => c.name() === "sync");
		expect(migrate?.options.map((o) => o.flags)).toEqual(["--apply"]);
		expect(sync?.options.map((o) => o.flags)).toEqual(["--full", "--deletes"]);
	});

	test("les arguments positionnels sont conformes", () => {
		const program = createAzaleeProgram();
		const usageOf = (name: string) => program.commands.find((c) => c.name() === name)?.usage();

		expect(usageOf("redis")).toContain("<cmd> <key> [val]");
		expect(usageOf("compare")).toContain("<chara1> <chara2>");
		expect(usageOf("team-builder")).toContain("<action> [args...]");
		expect(usageOf("chara")).toContain("[query]");
	});
});

describe("--help", () => {
	test("l'aide racine liste les 24 commandes", () => {
		const help = captureHelp(["--help"]);
		expect(help).toContain("Usage: azalee [options] [command]");
		for (const name of EXPECTED_COMMANDS) {
			expect(help, `commande ${name} absente de l'aide`).toContain(name);
		}
	});

	test.each([...EXPECTED_COMMANDS])("`azalee %s --help` s'affiche sans exécuter la commande", (name) => {
		const help = captureHelp([name, "--help"]);
		expect(help).toContain(`Usage: azalee ${name}`);
		expect(help).toContain("display help for command");
	});

	test.each(["push", "migrate", "load", "sync", "typecheck", "verify", "all"])(
		"`azalee data %s --help` s'affiche",
		(name) => {
			const help = captureHelp(["data", name, "--help"]);
			expect(help).toContain(`Usage: azalee data ${name}`);
		},
	);

	test("une option inconnue est refusée avec un code d'erreur commander", () => {
		const program = createAzaleeProgram();
		program.exitOverride();
		program.configureOutput({ writeOut: () => {}, writeErr: () => {} });
		for (const cmd of allCommands(program)) {
			cmd.exitOverride();
			cmd.configureOutput({ writeOut: () => {}, writeErr: () => {} });
		}
		expect(() => program.parse(["bun", "azalee", "status", "--nawak"])).toThrow(CommanderError);
	});
});

describe("exécution réelle", () => {
	test("`db --sqlite --json` renvoie un tableau JSON depuis le miroir", async () => {
		const { code, stdout } = await runCli(["db", "select count(*) as n from inagle_teams", "--sqlite", "--json"]);
		expect(code).toBe(0);
		const rows = JSON.parse(stdout) as Array<{ n: number }>;
		expect(Array.isArray(rows)).toBe(true);
		expect(rows[0].n).toBeGreaterThan(0);
	}, 30_000);

	test("`db --sqlite` sans --json rend une table ASCII", async () => {
		const { code, stdout } = await runCli(["db", "select count(*) as n from inagle_teams", "--sqlite"]);
		expect(code).toBe(0);
		expect(stdout).toContain("SQLite DB utilisé :");
		expect(stdout).toContain("┌");
		expect(stdout).toContain("│ n ");
	}, 30_000);

	test("`serve --json` liste les routes de l'API headless et quitte", async () => {
		const { code, stdout } = await runCli(["serve", "--json"]);
		expect(code).toBe(0);
		const payload = JSON.parse(stdout) as { routes: string[] };
		expect(payload.routes.length).toBeGreaterThan(10);
		expect(payload.routes).toContain("/api/characters");
	}, 30_000);

	test("`search --json` renvoie des entités du jeu", async () => {
		const { code, stdout } = await runCli(["search", "mark", "--json"]);
		expect(code).toBe(0);
		const results = JSON.parse(stdout) as Array<{ id: string; name: string; type: string }>;
		expect(results.length).toBeGreaterThan(0);
		expect(results[0]).toHaveProperty("name");
		expect(results[0]).toHaveProperty("type");
	}, 120_000);

	test("une entrée vide produit un objet d'erreur JSON, pas une trace", async () => {
		const { stdout } = await runCli(["search", "", "--json"]);
		expect(JSON.parse(stdout)).toEqual({ error: "Requête de recherche vide" });
	}, 30_000);

	test("`--version` renvoie la version déclarée", async () => {
		const { code, stdout } = await runCli(["--version"]);
		expect(code).toBe(0);
		expect(stdout.trim()).toBe(CLI_VERSION);
	}, 30_000);
});

describe("rendu de table ASCII", () => {
	test("encadre les colonnes et signale l'absence de résultat", () => {
		expect(renderAsciiTable([])).toBe("Aucun résultat (0 ligne).");
		const table = renderAsciiTable([{ id: 1, nom: "Raimon" }]);
		expect(table.split("\n")).toHaveLength(5);
		expect(table).toContain("│ id │ nom    │");
	});

	test("sérialise les objets et tronque au-delà de 45 caractères", () => {
		const table = renderAsciiTable([{ data: { a: 1 }, long: "x".repeat(60) }]);
		expect(table).toContain('{"a":1}');
		expect(table).toContain("x".repeat(42) + "...");
	});

	test("affiche NULL pour les valeurs absentes", () => {
		expect(renderAsciiTable([{ v: null }])).toContain("NULL");
	});
});

describe("découpage des lignes du shell", () => {
	test("respecte les guillemets simples et doubles", () => {
		expect(parseReplArgs('chara "Mark Evans"')).toEqual(["chara", "Mark Evans"]);
		expect(parseReplArgs("chara 'Axel Blaze'")).toEqual(["chara", "Axel Blaze"]);
		expect(parseReplArgs("  search   mark  ")).toEqual(["search", "mark"]);
		expect(parseReplArgs("")).toEqual([]);
	});
});

describe("règles de jeu remontées dans la lib", () => {
	const variant = {
		stats: {
			lv1: { kick: 10, control: 10, technique: 10, pressure: 10, physical: 10, agility: 10, intelligence: 10 },
			lv30: { kick: 40, control: 40, technique: 40, pressure: 40, physical: 40, agility: 40, intelligence: 40 },
			lv50: { kick: 60, control: 60, technique: 60, pressure: 60, physical: 60, agility: 60, intelligence: 60 },
			lv99: { kick: 100, control: 100, technique: 100, pressure: 100, physical: 100, agility: 100, intelligence: 100 },
		},
	};

	test("les paliers connus sont renvoyés tels quels", () => {
		expect(interpolateVariantStats(variant, 99)).toEqual(variant.stats.lv99);
		expect(interpolateVariantStats(variant, 1)).toEqual(variant.stats.lv1);
	});

	test("l'interpolation est croissante et bornée par les paliers", () => {
		const lv15 = interpolateVariantStats(variant, 15);
		const lv40 = interpolateVariantStats(variant, 40);
		const lv75 = interpolateVariantStats(variant, 75);
		expect(lv15.kick).toBeGreaterThan(variant.stats.lv1.kick);
		expect(lv15.kick).toBeLessThan(variant.stats.lv30.kick);
		expect(lv40.kick).toBeGreaterThan(variant.stats.lv30.kick);
		expect(lv40.kick).toBeLessThan(variant.stats.lv50.kick);
		expect(lv75.kick).toBeGreaterThan(variant.stats.lv50.kick);
		expect(lv75.kick).toBeLessThan(variant.stats.lv99.kick);
	});

	test("sans paliers intermédiaires, l'interpolation reste linéaire lv1→lv99", () => {
		const partial = { stats: { lv1: variant.stats.lv1, lv99: variant.stats.lv99 } };
		const mid = interpolateVariantStats(partial, 50);
		expect(mid.kick).toBe(Math.floor(10 + 90 * (49 / 98)));
	});

	test("une variante sans stats renvoie des zéros plutôt que de lever", () => {
		expect(interpolateVariantStats({}, 50)).toEqual({
			kick: 0,
			control: 0,
			technique: 0,
			pressure: 0,
			physical: 0,
			agility: 0,
			intelligence: 0,
		});
	});

	test("les libellés de personnalité couvrent la table du jeu", () => {
		expect(Object.keys(PERSONALITY_NAMES)).toHaveLength(12);
		expect(getPersonalityName(3)).toBe("Passionné");
		expect(getPersonalityName(undefined)).toBe("Inconnu");
		expect(getPersonalityName(99)).toBe("Type 99");
	});

	test("les catégories d'objet sont traduites, l'inconnue est conservée", () => {
		expect(getItemCategoryLabel("craft_obj")).toBe("Matériau");
		expect(getItemCategoryLabel("inconnue")).toBe("inconnue");
		expect(getItemCategoryLabel(undefined)).toBeUndefined();
		expect(Object.keys(ITEM_CATEGORY_LABELS_FR).length).toBeGreaterThan(15);
	});
});

describe("code de partage d'équipe", () => {
	test("base64 UTF-8 aller-retour, y compris accents et kana", () => {
		for (const s of ["Raimon", "Forêt & Montagne", "円堂 守"]) {
			expect(base64ToUtf8(utf8ToBase64(s))).toBe(s);
		}
	});

	test("l'encodage reste byte-identique à l'encodage historique (Buffer)", () => {
		const sample = "diamond442|field-0:0x1234|field-1:0xABCD";
		expect(utf8ToBase64(sample)).toBe(Buffer.from(sample).toString("base64"));
	});

	test("encode puis décode restitue formation et emplacements", () => {
		const slots = [
			{ slot: "field-0", charaId: "0x1111" },
			{ slot: "reserve-1", charaId: "0x2222" },
		];
		const decoded = decodeTeamCode(encodeTeamCode("box442", slots));
		expect(decoded.formationId).toBe("box442");
		expect(decoded.slots).toEqual(slots);
	});

	test("un segment malformé est ignoré sans casser le décodage", () => {
		const decoded = decodeTeamCode(utf8ToBase64("box442|field-0:0x1111|corrompu"));
		expect(decoded.formationId).toBe("box442");
		expect(decoded.slots).toEqual([{ slot: "field-0", charaId: "0x1111" }]);
	});
});

describe("détection de texte japonais", () => {
	test("distingue le japonais du latin", () => {
		expect(containsJapanese("円堂 守")).toBe(true);
		expect(containsJapanese("ファイアトルネード")).toBe(true);
		expect(containsJapanese("Fire Tornado")).toBe(false);
	});

	test("retire les annotations furigana en gardant la base", () => {
		expect(stripRubyAnnotations("[円堂/えんどう] [守/まもる]")).toBe("円堂 守");
		expect(stripRubyAnnotations("  Raimon  ")).toBe("Raimon");
	});

	test("échappe les métacaractères d'expression régulière", () => {
		expect(new RegExp(escapeRegExp("a.b*c")).test("a.b*c")).toBe(true);
		expect(new RegExp(escapeRegExp("a.b*c")).test("axbbc")).toBe(false);
	});
});

describe("formations partagées avec l'éditeur web", () => {
	test("les 8 formations héritées portent les identifiants persistés", () => {
		expect(LEGACY_FORMATIONS.map((f) => f.id)).toEqual([
			"diamond442",
			"box442",
			"freedom352",
			"triangle433",
			"delta433",
			"balance451",
			"hexa361",
			"double541",
		]);
	});

	test("chacune décrit 11 emplacements dont un gardien", () => {
		for (const f of LEGACY_FORMATIONS) {
			expect(f.positions, f.id).toHaveLength(11);
			expect(f.positions.filter((p) => p.role === "GK"), f.id).toHaveLength(1);
			expect(f.positions.map((p) => p.index), f.id).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
		}
	});
});
