/**
 * Prison de chemin partagée par les outils de lecture et d'écriture.
 *
 * Deux niveaux :
 *
 * 1. la **prison** — tout chemin est résolu (liens symboliques compris) et
 *    doit rester sous la racine du dépôt. Elle s'applique à TOUTES les
 *    portées, y compris `admin` : un serveur MCP n'a aucune raison légitime
 *    d'écrire hors du dépôt, et cela évite qu'un chemin mal formé aille
 *    toucher `/etc`.
 * 2. la **liste noire** — secrets, sauvegardes contenant des données
 *    personnelles, artefacts binaires et `node_modules`. Elle protège la
 *    portée `read` ; la portée `admin` peut la lever explicitement, parce que
 *    modifier un `.env` fait partie de l'administration du dépôt.
 */

export interface ResolvedPath {
	absolute: string;
	relative: string;
}

/**
 * Motifs interdits en lecture. Volontairement large : le coût d'un faux
 * positif est de devoir lire le fichier autrement, celui d'un faux négatif est
 * une fuite de secret ou de données personnelles sur un canal réseau.
 */
export const DENIED_PATTERNS = [
	/(^|\/)\.env(\.|$)/,
	/(^|\/)\.secrets(\/|$)/,
	/(^|\/)\.git\/(objects|logs)(\/|$)/,
	/(^|\/)node_modules(\/|$)/,
	/(^|\/)\.next(\/|$)/,
	/(^|\/)\.turbo(\/|$)/,
	/(^|\/)data\/backups(\/|$)/,
	/\.(sqlite|sqlite-wal|sqlite-shm|db)$/,
	/\.(pem|key|p12|pfx|keystore)$/,
	/(^|\/)auth\.json$/,
	/(^|\/)steam-cookies\.json$/,
	/(^|\/)bun\.lock$/,
	/\.(png|jpe?g|webp|gif|ico|woff2?|ttf|mp4|webm|zip|gz|tar|cpk|glb|wasm|so|dll|exe|node)$/i,
];

export function isDenied(relative: string): boolean {
	return DENIED_PATTERNS.some((pattern) => pattern.test(relative));
}

export interface ResolveOptions {
	/** Portée `admin` : lève la liste noire, jamais la prison de chemin. */
	allowDenied?: boolean;
}

/** Résout un chemin relatif dans la prison ; `undefined` si refusé. */
export async function resolveInside(
	root: string,
	relative: string,
	options: ResolveOptions = {},
): Promise<ResolvedPath | undefined> {
	const cleaned = toPosixPath(relative.trim());
	// Un chemin absolu est traité comme tel : le rendre relatif en retirant le
	// `/` initial ferait passer `/etc/passwd` pour `<dépôt>/etc/passwd`, une
	// réinterprétation silencieuse qui masque une erreur d'appel.
	// « Absolu » couvre les deux formes : `/etc/passwd` et `C:/Windows`.
	const candidat = isAbsolutePath(cleaned) ? cleaned : `${toPosixPath(root)}/${cleaned}`;
	const absolute = await realpathOrSelf(candidat);
	const rootReal = await realpathOrSelf(root);
	if (absolute !== rootReal && !absolute.startsWith(`${rootReal}/`)) return undefined;
	const inside = absolute === rootReal ? "" : absolute.slice(rootReal.length + 1);
	if (!options.allowDenied && isDenied(inside)) return undefined;
	return { absolute, relative: inside };
}

/**
 * Résolution des liens symboliques. Seul emprunt à `node:fs` de ce paquet :
 * Bun n'expose pas d'équivalent natif de `realpath`, et sans lui un lien
 * symbolique interne au dépôt suffirait à sortir de la prison de chemin.
 */
export async function realpathOrSelf(path: string): Promise<string> {
	try {
		const { realpath } = await import("node:fs/promises");
		// `realpath` rend des séparateurs `\` sous Windows : sans cette
		// conversion, la comparaison de la prison confronte un `C:\dépôt\x`
		// à un `C:/dépôt` et refuse tout — ou pire, laisse passer tout.
		return toPosixPath(await realpath(path));
	} catch {
		// Le chemin n'existe pas encore (cas d'une création) : on normalise
		// lexicalement, ce qui neutralise déjà les `..`.
		return normalizePath(path);
	}
}

/**
 * Sépateurs `\` ramenés à `/` : tout ce module raisonne en chemins POSIX, y
 * compris sous Windows où les deux formes désignent le même fichier.
 */
export function toPosixPath(path: string): string {
	return path.replace(/\\/g, "/");
}

/** Préfixe de lecteur Windows (`C:`) d'un chemin POSIXifié, sinon `""`. */
function driveOf(path: string): string {
	return /^[A-Za-z]:/.test(path) ? path.slice(0, 2) : "";
}

/** Un chemin est absolu s'il commence par `/` ou par un lecteur Windows. */
export function isAbsolutePath(path: string): boolean {
	const posix = toPosixPath(path);
	return posix.startsWith("/") || driveOf(posix) !== "";
}

/**
 * Normalisation `.`/`..` sans toucher au disque.
 *
 * Le préfixe de lecteur est mis de côté AVANT le découpage : sans cela
 * `C:\dépôt\paquet/..` se découpe en un seul segment `C:\dépôt\paquet` que le
 * `..` suivant efface — la racine de la prison devenait `/`, c'est-à-dire la
 * racine du système, et plus aucun chemin n'en sortait.
 */
export function normalizePath(path: string): string {
	const posix = toPosixPath(path);
	const drive = driveOf(posix);
	const parts: string[] = [];
	for (const segment of posix.slice(drive.length).split("/")) {
		if (segment === "" || segment === ".") continue;
		if (segment === "..") parts.pop();
		else parts.push(segment);
	}
	return `${drive}/${parts.join("/")}`;
}
