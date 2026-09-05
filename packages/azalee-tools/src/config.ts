/**
 * Configuration runtime de la bibliothèque Azalée.
 *
 * Deux familles de données coexistent :
 *
 * 1. Les **données figées** (manifestes d'assets, enrichissements, catalogue
 *    Cross…) vivent dans `src/data/*.json` et sont importées statiquement par
 *    les modules — elles suivent le package, y compris dans un bundle
 *    navigateur.
 * 2. Les **artefacts volumineux** (miroir SQLite du wiki, index CPK et index de
 *    texte en NDJSON gz) restent hors du package et sont localisés au runtime
 *    par ce module.
 *
 * L'ordre de résolution est explicite → variable d'environnement → chemins
 * conventionnels, ce qui permet à la lib de fonctionner à l'identique sous
 * `next build` (cwd = `apps/azalee`), sous le serveur standalone (cwd =
 * `.next/standalone/apps/azalee`), en CLI et en test (cwd = racine du dépôt).
 */

import { existsSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

/** Options de configuration explicites (priorité maximale). */
export interface AzaleeConfig {
	/** Dossier contenant les artefacts runtime (`cpk-index.ndjson.gz`, `game-text-*.ndjson.gz`, `backups/`). */
	dataDir?: string;
	/** Chemin du miroir SQLite des tables `inagle_*`. */
	mirrorPath?: string;
	/** Dossier de cache pour les SQLite matérialisés (défaut : `os.tmpdir()`). */
	cacheDir?: string;
}

const explicit: AzaleeConfig = {};

/**
 * Fixe la configuration runtime. Appelable plusieurs fois (fusion), typiquement
 * une seule fois au démarrage de l'application hôte ou du CLI.
 */
export function configureAzalee(config: AzaleeConfig): void {
	if (config.dataDir !== undefined) explicit.dataDir = path.resolve(config.dataDir);
	if (config.mirrorPath !== undefined) explicit.mirrorPath = path.resolve(config.mirrorPath);
	if (config.cacheDir !== undefined) explicit.cacheDir = path.resolve(config.cacheDir);
}

/** Renvoie la configuration explicite courante (lecture seule). */
export function getAzaleeConfig(): Readonly<AzaleeConfig> {
	return explicit;
}

/** Remet la configuration explicite à zéro (tests). */
export function resetAzaleeConfig(): void {
	delete explicit.dataDir;
	delete explicit.mirrorPath;
	delete explicit.cacheDir;
}

/**
 * Racine du package (`packages/azalee`), déduite de l'emplacement de ce module.
 * Sert de dernier recours pour remonter jusqu'à `apps/azalee/data` en monorepo.
 */
function packageRoot(): string {
	// `import.meta.dir` (Bun) / `import.meta.url` (Node) → `<pkg>/src`.
	const here =
		typeof import.meta.dir === "string" ? import.meta.dir : path.dirname(new URL(import.meta.url).pathname);
	return path.resolve(here, "..");
}

/** Candidats de dossier de données, du plus spécifique au plus générique. */
export function dataDirCandidates(): string[] {
	const root = packageRoot();
	return [
		explicit.dataDir,
		process.env.AZALEE_DATA_DIR,
		// Serveur standalone Next (cwd = `.next/standalone/apps/azalee`) et
		// `next build` (cwd = `apps/azalee`).
		path.resolve(process.cwd(), "data"),
		// Exécution depuis la racine du monorepo (CLI, tests, scripts).
		path.resolve(process.cwd(), "apps/azalee/data"),
		// Depuis `packages/azalee` : ../../apps/azalee/data.
		path.resolve(root, "../../apps/azalee/data"),
	].filter((p): p is string => Boolean(p));
}

/**
 * Marqueurs d'un vrai dossier de données Azalée. Sans ce filtre, un simple
 * `<cwd>/data` homonyme (il en existe un à la racine du monorepo, dédié aux
 * dumps Postgres) serait retenu — c'est exactement le type de « source
 * silencieuse » qu'on veut éviter.
 */
const DATA_DIR_MARKERS = [
	"cpk-index.ndjson.gz",
	"game-text-names.ndjson.gz",
	"backups/mirror.sqlite",
	"schema-snapshot",
];

/**
 * Résout le dossier des artefacts runtime : premier candidat existant
 * contenant au moins un marqueur Azalée. Renvoie `null` si aucun ne
 * correspond (la lib reste utilisable pour tout ce qui ne touche pas ces
 * artefacts).
 */
export function resolveDataDir(): string | null {
	for (const candidate of dataDirCandidates()) {
		if (!existsSync(candidate)) continue;
		if (DATA_DIR_MARKERS.some((marker) => existsSync(path.join(candidate, marker)))) {
			return candidate;
		}
	}
	return null;
}

/**
 * Résout un fichier d'artefact par son nom (ex. `cpk-index.ndjson.gz`). Chaque
 * dossier candidat est testé, le premier fichier existant gagne.
 */
export function resolveDataFile(name: string): string | null {
	for (const dir of dataDirCandidates()) {
		const candidate = path.join(dir, name);
		if (existsSync(candidate)) return candidate;
	}
	return null;
}

/**
 * Miroir du dépôt : `var/mirror.sqlite`, republié par
 * `scripts/donnees/miroir-inagle.sh`.
 *
 * Depuis la fusion, le miroir ne vit plus sous `apps/azalee/data/backups` mais à la racine du
 * dépôt, là où la CLI, le bot et `@niers/catalog` le trouvent aussi. On remonte les ancêtres
 * jusqu'au dossier qui porte `Cargo.toml` **et** `crates/` — la même signature que côté Rust,
 * pour qu'un `var/` homonyme rencontré en chemin ne soit jamais pris pour la racine.
 */
function miroirDuDepot(): string | null {
	let courant = packageRoot();
	for (;;) {
		if (existsSync(path.join(courant, "Cargo.toml")) && existsSync(path.join(courant, "crates"))) {
			const miroir = path.join(courant, "var", "mirror.sqlite");
			return existsSync(miroir) ? miroir : null;
		}
		const parent = path.dirname(courant);
		if (parent === courant) return null;
		courant = parent;
	}
}

/**
 * Résout le miroir SQLite des tables `inagle_*`.
 *
 * `SQLITE_DB_PATH` est épinglé en production (unités systemd) sur `var/mirror.sqlite` :
 * sans épinglage, le repli par `data/backups` prend le snapshot au nom
 * lexicographiquement le plus grand, ce qui est une source silencieuse.
 */
export function resolveMirrorPath(): string | null {
	if (explicit.mirrorPath) return explicit.mirrorPath;
	if (process.env.SQLITE_DB_PATH) return path.resolve(process.env.SQLITE_DB_PATH);

	// Le miroir du dépôt PRIME sur `data/backups`. C'est `nie-miroir.timer` qui le publie ;
	// l'ancien dossier existe encore sur les machines qui ont connu `azalee-mirror-sync`, mais
	// il ne se rafraîchit plus — le préférer ferait servir des données figées sans qu'aucune
	// erreur ne le dise.
	const miroir = miroirDuDepot();
	if (miroir) return miroir;

	for (const dir of dataDirCandidates()) {
		const backups = path.join(dir, "backups");
		const pinned = path.join(backups, "mirror.sqlite");
		if (existsSync(pinned)) return pinned;
		try {
			const snapshots = readdirSync(backups)
				.filter((f) => f.startsWith("supabase-") && f.endsWith(".sqlite"))
				.sort((a, b) => b.localeCompare(a));
			if (snapshots.length > 0) return path.join(backups, snapshots[0]);
		} catch {
			// dossier absent ou illisible — candidat suivant
		}
	}
	return null;
}

/**
 * Dossier de cache des SQLite matérialisés (index CPK, index de texte). Sur le
 * VPS `os.tmpdir()` est un tmpfs : matérialisation en RAM, reconstruite au
 * redémarrage si l'artefact source est plus récent.
 */
export function getCacheDir(sub?: string): string {
	const base = explicit.cacheDir ?? process.env.AZALEE_CACHE_DIR ?? tmpdir();
	return sub ? path.join(base, sub) : base;
}
