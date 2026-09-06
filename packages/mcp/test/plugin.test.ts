/**
 * Plugin Claude `niers` : cohérence de la configuration livrée.
 *
 * Ces tests remplacent une inspection à l'œil sur la machine cliente. Ils
 * reproduisent ce que fait Claude Code au chargement — lecture du manifeste,
 * expansion des variables d'environnement, découverte des composants — et
 * échouent sur les modes de panne réellement rencontrés : entrée MCP sans
 * `type` (silencieusement ignorée), variable non définie envoyée telle quelle,
 * composant rangé sous `.claude-plugin/` (jamais découvert), outil MCP cité
 * sous un préfixe que le serveur déclaré ne porte pas.
 *
 * ATTENTION aux constantes de ce fichier. Elles sont écrites EN DUR, et c'est
 * délibéré : des attentes relues sur le disque feraient un test incapable
 * d'échouer, qui rassurerait pendant qu'une skill disparaît. La suppression
 * d'une skill, d'un agent ou le renommage du plugin DOIT rougir ici.
 *
 * Le plugin s'appelait `rose-griffon` et vivait en `plugins/rose-griffon`.
 * Il a été renommé `niers` (débranding : niers est un projet `aphrody-dev`),
 * et ce fichier était resté derrière — il testait un plugin absent du disque.
 */

import { describe, expect, test } from "bun:test";

const RACINE = `${import.meta.dir}/../../..`;
const PLUGIN = `${RACINE}/plugins/niers-plugin`;

/** Nom du plugin, du serveur MCP qu'il fournit, et de sa source marketplace. */
const NOM_PLUGIN = "niers";
const NOM_SERVEUR = "niers-game";
const SOURCE_MARKETPLACE = "./niers-plugin";

/** Les 16 skills livrées. Une disparition — comme un ajout — doit rougir. */
const SKILLS = [
	"aphrody-pet",
	"assembler-modeles-textures",
	"bun-ffi",
	"creer-assets-3d",
	"formats-level5",
	"ievr-terminologie",
	"jouer-ievr",
	"napi-rs",
	"niers-architecture",
	"niers-autonome",
	"niers-monorepo",
	"peaufiner-rendu-3d",
	"pixel-perfect",
	"rs-to-ts",
	"rust-bun",
	"wasm-bun",
];

/** Les 6 agents livrés. */
const AGENTS = ["build-doctor", "bun-rs", "forge-analyst", "port-scout", "re-lookup", "vfs-scout"];

async function lireJson<T>(chemin: string): Promise<T> {
	return (await Bun.file(chemin).json()) as T;
}

/** Frontmatter YAML brut d'un fichier Markdown (entre les deux `---`). */
function frontmatter(contenu: string): string {
	return contenu.split("---")[1] ?? "";
}

/**
 * Valeur du champ `description` d'un frontmatter, les deux formes YAML
 * confondues : `description: texte` sur une ligne, et le scalaire replié
 * `description: >-` suivi de lignes indentées. Un test qui ne connaît que la
 * première mesure une description repliée à ZÉRO caractère et rougit sur cinq
 * skills parfaitement décrites — l'inverse exact de ce qu'il cherche.
 */
function description(entete: string): string {
	const lignes = entete.split(/\r?\n/);
	const depart = lignes.findIndex((ligne) => /^description:/.test(ligne));
	if (depart < 0) return "";
	const premiere = lignes[depart]!.replace(/^description: */, "");
	if (!/^[>|][-+]?$/.test(premiere.trim())) return premiere.trim();
	const repliees: string[] = [];
	for (const ligne of lignes.slice(depart + 1)) {
		if (!/^\s/.test(ligne) || ligne.trim() === "") break;
		repliees.push(ligne.trim());
	}
	return repliees.join(" ");
}

/** Expansion `${VAR}` / `${VAR:-défaut}`, comme le fait Claude Code. */
function etendre(valeur: string, env: Record<string, string | undefined>): string {
	return valeur.replaceAll(/\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}/g, (_t, nom: string, defaut?: string) => {
		return env[nom] ?? defaut ?? `\${${nom}}`;
	});
}

describe("manifeste et marketplace", () => {
	test("le manifeste déclare un nom en kebab-case et une version", async () => {
		const manifeste = await lireJson<{ name: string; version: string; description: string }>(
			`${PLUGIN}/.claude-plugin/plugin.json`,
		);
		expect(manifeste.name).toBe(NOM_PLUGIN);
		expect(manifeste.name).toMatch(/^[a-z][a-z0-9-]*$/);
		expect(manifeste.version).toMatch(/^\d+\.\d+\.\d+$/);
		expect(manifeste.description.length).toBeGreaterThan(30);
	});

	test("la marketplace pointe le plugin, et la source existe vraiment", async () => {
		// La marketplace vit dans `plugins/`, pas à la racine : ses `source` sont
		// donc relatives à `plugins/`. Une source résolue depuis la racine du
		// dépôt viserait `<dépôt>/niers-plugin`, qui n'existe pas — panne muette,
		// le plugin est simplement absent du catalogue côté client.
		const marketplace = await lireJson<{
			plugins: { name: string; source: string; description: string }[];
		}>(`${RACINE}/plugins/.claude-plugin/marketplace.json`);
		const entree = marketplace.plugins.find((p) => p.name === NOM_PLUGIN);
		expect(entree).toBeDefined();
		expect(entree!.source).toBe(SOURCE_MARKETPLACE);
		expect(entree!.description.length).toBeGreaterThan(30);
		expect(await Bun.file(`${RACINE}/plugins/${entree!.source.slice(2)}/.claude-plugin/plugin.json`).exists()).toBe(
			true,
		);
	});

	test("les composants sont à la racine du plugin, pas dans .claude-plugin", async () => {
		// `Bun.file(...).exists()` répond `false` sur un répertoire : on passe
		// donc par `stat()` pour les dossiers, `exists()` pour les fichiers.
		for (const dossier of ["skills", "agents"]) {
			expect((await Bun.file(`${PLUGIN}/${dossier}`).stat()).isDirectory()).toBe(true);
		}
		for (const fichier of ["README.md", ".claude-plugin/plugin.json"]) {
			expect(await Bun.file(`${PLUGIN}/${fichier}`).exists()).toBe(true);
		}
		// Les composants dans `.claude-plugin/` ne seraient pas découverts.
		expect(await Bun.file(`${PLUGIN}/.claude-plugin/skills/${SKILLS[0]}/SKILL.md`).exists()).toBe(false);
		expect(await Bun.file(`${PLUGIN}/.claude-plugin/.mcp.json`).exists()).toBe(false);
	});
});

// Le serveur `niers-game` est déclaré à la RACINE du dépôt, pas dans le
// plugin : c'est ce que dit son manifeste, et `plugins/niers-plugin/.mcp.json`
// a été retiré en conséquence. Ces tests suivent le fichier réellement lu.
describe("configuration MCP du dépôt", () => {
	test("l'entrée déclare `type` — sans lui elle est lue comme stdio par défaut", async () => {
		const config = await lireJson<{
			mcpServers: Record<string, { type?: string; command?: string; args?: string[] }>;
		}>(`${RACINE}/.mcp.json`);
		const serveur = config.mcpServers[NOM_SERVEUR];
		expect(serveur).toBeDefined();
		expect(serveur!.type).toBe("stdio");
		expect(serveur!.command).toBeString();
		expect(serveur!.args).toBeArray();
	});

	test("aucun chemin de machine n'est figé dans la déclaration", async () => {
		// La déclaration est lue sur une machine cliente quelconque : aucun
		// chemin absolu de CETTE machine-ci n'a le droit d'y être figé. Le cas a
		// été payé : `NIERS_REPO=/home/ubuntu/niers`, la valeur du VPS, hérité
		// sur le poste Windows où il se résout en
		// `C:\Program Files\Git\home\ubuntu\niers` — un dossier inexistant, et
		// `repo_read` répond alors « introuvable » sur tout le dépôt.
		const brut = await Bun.file(`${RACINE}/.mcp.json`).text();
		expect(brut).not.toContain("/home/ubuntu");
		expect(brut).not.toContain("C:\\");

		// Les arguments sont relatifs au dépôt, donc portables tels quels.
		const config = await lireJson<{
			mcpServers: Record<string, { args: string[]; env?: Record<string, string> }>;
		}>(`${RACINE}/.mcp.json`);
		const serveur = config.mcpServers[NOM_SERVEUR]!;
		const entree = serveur.args.find((argument) => argument.includes("apps/nie-mcp"));
		expect(entree).toBeDefined();
		expect(entree!.startsWith("/")).toBe(false);
		expect(await Bun.file(`${RACINE}/${entree!}`).exists()).toBe(true);

		// Et si un jour une variable revient, elle doit rester VISIBLE non
		// définie plutôt que de partir en chemin vide.
		expect(etendre("${NIERS_REPO}/x", { NIERS_REPO: undefined })).toContain("${NIERS_REPO}");
	});

	test("le plugin ne livre pas de hook", async () => {
		// Le hook `restore-lockfile.sh` a disparu avec le débranding, et sa
		// couverture avec lui. Cette assertion tient la place : si un `hooks/`
		// revient, ce test rougit et réclame les tests qui vont avec (objet
		// racine `hooks`, commandes en `${CLAUDE_PLUGIN_ROOT}`, script valide
		// pour bash) plutôt que de le laisser arriver sans filet.
		expect(await Bun.file(`${PLUGIN}/hooks`).exists()).toBe(false);
	});
});

describe("skills et agents du plugin", () => {
	test("chaque skill a un SKILL.md avec nom et description", async () => {
		for (const nom of SKILLS) {
			const chemin = `${PLUGIN}/skills/${nom}/SKILL.md`;
			expect(await Bun.file(chemin).exists()).toBe(true);
			const contenu = await Bun.file(chemin).text();
			// `\r?\n` : le dépôt est travaillé depuis un poste Windows, où ces
			// fichiers sont en CRLF. Ce qui est testé est la présence du bloc de
			// frontmatter, pas la fin de ligne — figer `\n` faisait rougir la
			// suite sur une seule machine, pour une raison sans rapport.
			expect(/^---\r?\n/.test(contenu)).toBe(true);
			expect(frontmatter(contenu)).toContain(`name: ${nom}`);
			// Une description courte ne déclenche pas : c'est le seul texte que
			// Claude Code lit pour décider de charger la skill.
			expect(description(frontmatter(contenu)).length).toBeGreaterThanOrEqual(40);
		}
	});

	test("aucune skill livrée n'est absente de la liste attendue", async () => {
		// Le pendant du test précédent : sans lui, une skill AJOUTÉE puis oubliée
		// passerait inaperçue, et la liste en dur cesserait de décrire le plugin.
		const glob = new Bun.Glob("*/SKILL.md");
		const trouvees: string[] = [];
		for await (const relatif of glob.scan({ cwd: `${PLUGIN}/skills`, onlyFiles: true })) {
			trouvees.push(relatif.replaceAll("\\", "/").split("/")[0]!);
		}
		expect(trouvees.toSorted()).toEqual([...SKILLS].toSorted());
	});

	test("chaque agent déclare des exemples de déclenchement", async () => {
		// Sans `<example>`, le modèle n'a que la description pour décider de
		// déléguer, et ne délègue pas.
		//
		// `bun-rs` en est dépourvu — un manque RÉEL du plugin, mesuré, pas une
		// tolérance de confort. Il est donc nommé, et l'assertion est un
		// cliquet à double sens : si quelqu'un lui écrit ses exemples, ce test
		// rougit et demande qu'on le retire d'ici ; si un autre agent perd les
		// siens, il rougit aussi. Une simple exclusion, elle, se serait tue
		// dans les deux cas.
		const SANS_EXEMPLE = ["bun-rs"];
		const manquants: string[] = [];
		for (const nom of AGENTS) {
			const contenu = await Bun.file(`${PLUGIN}/agents/${nom}.md`).text();
			expect(frontmatter(contenu)).toContain(`name: ${nom}`);
			if (!contenu.includes("<example>")) manquants.push(nom);
		}
		expect(manquants.toSorted()).toEqual([...SANS_EXEMPLE].toSorted());
	});

	test("un agent qui fige ses outils n'y met aucun outil MCP", async () => {
		// `tools:` REMPLACE l'héritage : l'agent n'a plus que ce qu'il liste.
		// Y lister un outil MCP le priverait de tout sur la machine cliente, car
		// le préfixe dépend de l'enregistrement (`mcp__niers-game__` en projet,
		// `mcp__plugin_niers_niers-game__` en plugin). Les outils intégrés
		// (Bash, Read, Grep…) portent le même nom partout : eux sont portables.
		for (const nom of AGENTS) {
			const declares = /^tools: *(.*)$/m.exec(frontmatter(await Bun.file(`${PLUGIN}/agents/${nom}.md`).text()));
			if (!declares) continue;
			expect(declares[1]).not.toContain("mcp__");
		}
	});

	test("tout outil MCP cité par une skill ou un agent porte le préfixe du serveur déclaré", async () => {
		// Un outil cité sous un autre préfixe que celui du serveur livré ne
		// sera jamais résolu : la citation reste lisible, et n'appelle rien.
		const prefixes = [`mcp__${NOM_SERVEUR}__`, `mcp__plugin_${NOM_PLUGIN}_${NOM_SERVEUR}__`];
		const glob = new Bun.Glob("**/*.md");
		for await (const relatif of glob.scan({ cwd: PLUGIN, onlyFiles: true })) {
			const contenu = await Bun.file(`${PLUGIN}/${relatif}`).text();
			for (const [cite] of contenu.matchAll(/mcp__[A-Za-z0-9_-]+__/g)) {
				expect(prefixes).toContain(cite);
			}
		}
	});

	test("aucune skill ne pré-autorise un outil d'écriture ou de shell", async () => {
		// `allowed-tools` ACCORDE la permission sans invite. Une skill se
		// déclenche sur une simple question : y pré-autoriser `Bash`, `Write` ou
		// un `repo_delete` ouvrirait un accès équivalent SSH sans confirmation.
		const interdits = ["Bash", "Write", "Edit", "shell_run", "repo_write", "repo_edit", "repo_delete", "repo_move"];
		for (const nom of SKILLS) {
			const entete = frontmatter(await Bun.file(`${PLUGIN}/skills/${nom}/SKILL.md`).text());
			const declares = /^allowed-tools: *(.*)$/m.exec(entete);
			if (!declares) continue;
			for (const outil of interdits) expect(declares[1]).not.toContain(outil);
		}
	});
});
