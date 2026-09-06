/**
 * Résolution des quatre gisements Inazuma Eleven, à l'exécution.
 *
 * Le dépôt interdit d'inscrire un chemin de machine dans un binaire : la racine du jeu se résout
 * déjà ainsi côté Rust (`nie_formats::vfs::resolve_game_dir`). Même règle ici, pour les quatre
 * sources qu'un même personnage traverse :
 *
 * | Gisement | Ce qu'il porte | Où il vit |
 * |---|---|---|
 * | `jeu` | les fichiers du jeu, décodés à la volée | HTTP — `nie-model-serve` (CDN ou local) |
 * | `extrait` | 68 tables `inagle_*` tirées du jeu | miroir SQLite en lecture seule |
 * | `re` | le reverse de `nie.exe` | `var/niers.sqlite` |
 * | `anime` | les épisodes de la série | SQLite du crawler IETV |
 *
 * Aucune source n'est obligatoire : chacune se résout à `null` quand elle est absente, et la
 * façade dit alors *pourquoi* elle ne peut pas répondre, au lieu de rendre une liste vide qu'on
 * prendrait pour une vérité.
 */
import { existsSync, readlinkSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { BASE_JEU_DEFAUT, baseJeu } from "./jeu.ts";

/** Une source résolue : son chemin (ou son URL) et la raison de son absence, jamais les deux. */
export interface Source {
	/** Chemin absolu ou URL de base. `null` si la source est introuvable. */
	readonly emplacement: string | null;
	/** Ce qui a été essayé, pour que l'absence soit diagnosticable et non muette. */
	readonly essais: readonly string[];
}

/** Les quatre gisements, résolus une fois par processus. */
export interface Sources {
	readonly racine: string;
	readonly jeu: Source;
	readonly extrait: Source;
	readonly re: Source;
	readonly anime: Source;
}

/** Vrai si le chemin désigne un fichier lisible — un lien symbolique est suivi. */
function fichierLisible(chemin: string): boolean {
	try {
		return statSync(chemin).isFile();
	} catch {
		return false;
	}
}

/**
 * Racine du dépôt : l'ancêtre qui porte `Cargo.toml` **et** `crates/`.
 *
 * `NIE_REPO_DIR` la force. Une variable posée mais vide est ignorée — une chaîne vide n'est pas
 * une racine, elle renverrait un chemin où rien n'est jamais trouvé (même piège que `NIE_GAME_DIR`).
 */
export function racineDepot(depart = process.cwd()): string {
	const forcee = process.env.NIE_REPO_DIR?.trim();
	if (forcee) {
		return resolve(forcee);
	}
	let courant = resolve(depart);
	for (;;) {
		if (existsSync(join(courant, "Cargo.toml")) && existsSync(join(courant, "crates"))) {
			return courant;
		}
		const parent = dirname(courant);
		if (parent === courant) {
			return resolve(depart);
		}
		courant = parent;
	}
}

/** Premier chemin lisible de la liste, avec la trace de ce qui a été tenté. */
function premierLisible(candidats: readonly (string | undefined)[]): Source {
	const essais: string[] = [];
	for (const c of candidats) {
		if (!c) {
			continue;
		}
		const chemin = isAbsolute(c) ? c : resolve(c);
		essais.push(chemin);
		if (fichierLisible(chemin)) {
			return { emplacement: chemin, essais };
		}
	}
	return { emplacement: null, essais };
}

/**
 * Le miroir des tables `inagle_*`.
 *
 * Il est republié par `nie-miroir` sous la forme d'un **lien symbolique daté**
 * (`mirror.sqlite -> supabase-<horodatage>.sqlite`) que le script bascule atomiquement. On suit
 * donc le lien : ouvrir le lien lui-même marcherait, mais laisserait croire que le fichier ne
 * change jamais, alors qu'il change à chaque synchronisation.
 */
function sourceExtrait(racine: string): Source {
	const s = premierLisible([
		process.env.NIE_MIROIR_SQLITE?.trim(),
		join(racine, "var", "mirror.sqlite"),
		join(dirname(racine), "rg", "apps", "azalee", "data", "backups", "mirror.sqlite"),
	]);
	if (!s.emplacement) {
		return s;
	}
	try {
		const cible = readlinkSync(s.emplacement);
		const resolu = isAbsolute(cible) ? cible : join(dirname(s.emplacement), cible);
		return fichierLisible(resolu) ? { emplacement: resolu, essais: s.essais } : s;
	} catch {
		return s; // Ce n'est pas un lien : c'est déjà le fichier.
	}
}

/**
 * Le cache du crawler IETV, dans le répertoire personnel — `undefined` s'il n'y en a pas.
 *
 * `process.env.HOME` n'existe PAS sous Windows (c'est `USERPROFILE`) : `HOME ?? ""` y produisait
 * `join("", ".cache", …)`, donc un chemin RELATIF, que `premierLisible` résolvait ensuite contre
 * le cwd. Le repli visait alors `<répertoire courant>/.cache/ietv/episodes.db` au lieu du
 * répertoire personnel — sans rien dire, exactement le piège de la variable posée mais vide que
 * `racineDepot` évite plus haut. `homedir()` répond sur les deux plateformes.
 */
function cacheIetvPersonnel(): string | undefined {
	const maison = homedir();
	return maison ? join(maison, ".cache", "ietv", "episodes.db") : undefined;
}

/**
 * Résout les quatre gisements. Le résultat est mémorisé : ces chemins ne bougent pas sous nos
 * pieds pendant la vie d'un processus, et une résolution refaite à chaque requête coûterait
 * quatre `stat` par appel.
 */
let cache: Sources | undefined;

export function sources(depart?: string): Sources {
	if (cache && depart === undefined) {
		return cache;
	}
	const racine = racineDepot(depart);
	const resolues: Sources = {
		anime: premierLisible([
			process.env.NIE_ANIME_SQLITE?.trim(),
			join(racine, "data", "anime", "episodes.db"),
			cacheIetvPersonnel(),
		]),
		extrait: sourceExtrait(racine),
		// La base du CDN n'est PAS recalculée ici : elle vient de `./jeu.ts`, qui porte les
		// conventions d'URL du serveur. La dupliquer laisserait `sources()` et les constructeurs
		// d'URL diverger en silence — chacun visant une origine différente.
		jeu: {
			emplacement: baseJeu(),
			essais: ["NIE_CDN_URL", BASE_JEU_DEFAUT],
		},
		racine,
		re: premierLisible([process.env.NIE_KB_SQLITE?.trim(), join(racine, "var", "niers.sqlite")]),
	};
	if (depart === undefined) {
		cache = resolues;
	}
	return resolues;
}

/** Oublie la résolution mémorisée — pour les tests, et après une bascule du miroir. */
export function oublierSources(): void {
	cache = undefined;
}
