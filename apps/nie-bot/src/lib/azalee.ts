/**
 * Accès du bot aux données de jeu IEVR — via `@rosegriffon/azalee`.
 *
 * ── POURQUOI L'API LOCALE ET PAS LE MIROIR SQLITE ──────────────────────────
 * La bibliothèque sait lire le miroir `apps/azalee/data/backups/mirror.sqlite`
 * sans injection de client. Le bot ne le fait PAS, et ce n'est pas un choix de
 * confort :
 *
 *  1. `rg-bot.service` tourne sous `ProtectHome=read-only` avec
 *     `ReadWritePaths=/home/ubuntu/rg/apps/bot /tmp`. Le dossier du miroir est
 *     donc en LECTURE SEULE pour lui.
 *  2. Le miroir est en journal WAL. Ouvrir une base WAL — même en lecture seule —
 *     exige que le fichier `-shm` EXISTE, et donc de pouvoir le créer quand il
 *     manque. Mesuré sous le bac à sable exact du service : `-shm` présent →
 *     lecture OK (même sans aucun écrivain vivant) ; `-shm` absent →
 *     `SQLITE_CANTOPEN`. Aujourd'hui il existe parce que le script de dump le
 *     crée à 04:05 ; rien ne garantit qu'il survivra à toutes les manipulations
 *     du miroir, et le bot n'a aucun moyen de le recréer.
 *  3. `file:…?immutable=1` n'est pas une échappatoire : `bun:sqlite` 1.4 n'honore
 *     pas les noms de fichier URI, même avec `SQLITE_OPEN_URI` en `flags`.
 *
 * On passe donc par `@niers/azalee-tools/remote`, un client HTTP typé (41 routes)
 * pointé sur `azalee-api.service` en loopback. Zéro descripteur SQLite, zéro
 * `-shm`, zéro modification du durcissement systemd — et la rotation nocturne
 * redémarre elle-même `azalee-api.service`, donc il n'y a aucun descripteur à
 * faire survivre.
 *
 * ⚠ Ce que ce choix ne résout PAS, et qu'il faut savoir : `azalee-api.service`
 * porte le même durcissement. Si SA base est illisible, ses routes de LISTE
 * répondent 200 avec `{"data":[],"total":0}` (les routes de fiche, elles,
 * répondent 500). Une autocomplétion vide n'est donc pas la preuve que la
 * donnée n'existe pas : `ops_status azalee-api` reste le juge.
 *
 * ⚠ Ne JAMAIS importer `@niers/azalee-tools/server/index` ici : c'est le sous-chemin
 * qui ouvre `bun:sqlite`, précisément ce qu'on évite.
 */
import {
	createAzaleeClient,
	isAzaleeNotFound,
	isAzaleeRemoteError,
	type AzaleeClient,
} from "@niers/azalee-tools/remote";

/**
 * API Azalée en loopback. 3010 est déjà pris par un autre service du VPS : le
 * défaut de la bibliothèque est 8807, servi aussi sur `api.rosegriffon.fr/azalee`.
 */
export const URL_API_AZALEE = Bun.env.AZALEE_API_URL ?? "http://127.0.0.1:8807";

/** Base publique du wiki, pour les liens des embeds. */
export const URL_WIKI_AZALEE = Bun.env.AZALEE_URL ?? "https://azalee.rosegriffon.fr";

let client: AzaleeClient | null = null;

/**
 * Client partagé. Créé à la première commande utilisée, pas à l'import : un
 * démarrage du bot ne doit pas dépendre de la disponibilité d'un autre service.
 */
export function azalee(): AzaleeClient {
	// Réessais calibrés sur le TROU DE LA ROTATION NOCTURNE : `miroir-inagle.sh`
	// redémarre `azalee-api.service`, qui met ~2,4 s à répondre 200 (mesuré).
	// Les défauts de la bibliothèque (3 tentatives, 300 ms de base) couvraient
	// ~0,9 s : une commande `/ievr` échouait une fois par nuit pour rien.
	// 800 ms → 1,6 s → 3,2 s couvre le redémarrage avec de la marge, et
	// `ECONNREFUSED` est bien classé réessayable par le transport.
	client ??= createAzaleeClient({
		baseUrl: URL_API_AZALEE,
		timeoutMs: 8_000,
		attempts: 4,
		retryDelayMs: 800,
	});
	return client;
}

export { isAzaleeNotFound, isAzaleeRemoteError };

/** Lien vers une page du wiki. */
export function urlWiki(chemin: string): string {
	return `${URL_WIKI_AZALEE}${chemin.startsWith("/") ? chemin : `/${chemin}`}`;
}

/**
 * Corrige les noms de fichier DOUBLÉS hérités de l'ancien dump sur disque.
 *
 * La bibliothèque forge encore `…/{code}_{code}.png` (`packages/azalee/src/
 * images/utils.ts`) pour les télops de techniques et les emblèmes d'équipe. Le
 * dump `dx11/menu` a été archivé : le CDN décode désormais les CPK EN DIRECT, et
 * le décodeur ne connaît que `{code}.png`. Mesuré le 13/8/2026 sur 15 codes
 * tirés au hasard plus 6 emblèmes : la forme doublée renvoie 404 à tous les
 * coups, la forme simple renvoie 200 partout où l'asset existe.
 *
 * La ré-écriture est donc strictement gagnante : soit l'image apparaît, soit on
 * est dans l'état d'avant (404, embed sans image). Le site, lui, a un `onError`
 * pour rattraper ; un embed Discord n'en a pas.
 */
export function corrigerNomDouble(url: string): string {
	return url.replace(/\/([^/]+)_\1(\.[a-z0-9]+)$/i, "/$1$2");
}

/**
 * Ne garde une URL d'illustration que si Discord saura aller la chercher.
 *
 * L'API renvoie parfois un chemin local de repli (`/ievr.webp`) quand aucun
 * visuel n'existe : le poser dans un embed produirait une image cassée.
 */
export function illustrationAbsolue(url: string | null | undefined): string | null {
	if (typeof url !== "string") {
		return null;
	}
	const propre = url.trim();
	return propre.startsWith("https://") || propre.startsWith("http://")
		? corrigerNomDouble(propre)
		: null;
}

/** Message d'erreur lisible pour un salon, sans pile d'appels. */
export function messageErreurAzalee(err: unknown): string {
	if (isAzaleeNotFound(err)) {
		return "Introuvable dans le wiki.";
	}
	if (isAzaleeRemoteError(err)) {
		return `API Azalée injoignable (${URL_API_AZALEE}) — ${err.message}`;
	}
	return err instanceof Error ? err.message : String(err);
}
