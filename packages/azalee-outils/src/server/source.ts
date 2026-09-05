/**
 * Résolution de la source de données, avec repli automatique sur l'API
 * distante.
 *
 * Le problème : le CLI et l'application de bureau doivent servir des données
 * **même quand la machine n'a ni miroir SQLite ni dump du jeu** — première
 * installation, poste d'un contributeur, GUI distribuée à un joueur. Sans
 * repli, chaque commande renverrait une liste vide sans expliquer pourquoi.
 *
 * La solution tient en une observation : `handleAzaleeRequest` est une fonction
 * pure `Request → Response`, et `createAzaleeClient` accepte n'importe quel
 * transport. Les deux modes partagent donc **exactement** le même code de
 * routage, de sérialisation et de typage ; seul le transport change.
 *
 *   - `local`  → `handleAzaleeRequest` en appel direct (zéro socket, zéro réseau) ;
 *   - `remote` → `fetch` vers `https://api.rosegriffon.fr/azalee` ;
 *   - `auto`   → `local` si les artefacts sont présents, sinon `remote`, avec
 *     bascule à chaud si une lecture locale échoue (base corrompue, miroir
 *     remplacé pendant une synchronisation).
 *
 * ⚠ Module **serveur** : il touche `node:fs` via `../config` et importe la
 * couche données. Dans une webview, utiliser `resolveAzaleeBaseUrl` du
 * sous-chemin `/remote`, qui joue le même rôle en HTTP pur.
 */

import { existsSync } from "node:fs";

import { hasDatabaseProvider } from "@rosegriffon/azalee/db";
import { resolveDataDir, resolveMirrorPath } from "../config";
import {
	type AzaleeClient,
	type AzaleeClientOptions,
	type AzaleeTransport,
	createAzaleeClient,
	resolveAzaleeApiUrl,
} from "../remote/index";
import { handleAzaleeRequest } from "./serve";

/** Provenance effective des données. */
export type AzaleeSourceKind = "local" | "remote";

/** Stratégie de sélection de la source. */
export type AzaleeSourceMode = "auto" | AzaleeSourceKind;

/** État des sources de données locales. */
export interface AzaleeLocalAvailability {
	/** Chemin du miroir SQLite des tables `inagle_*`, ou `null`. */
	mirror: string | null;
	/** Dossier des artefacts runtime (index CPK, index de texte), ou `null`. */
	dataDir: string | null;
	/** `true` si l'application hôte a injecté son propre client de base. */
	injectedProvider: boolean;
	/** `true` si la lecture locale du wiki est possible. */
	available: boolean;
	/** Explication lisible, reprise telle quelle dans `AzaleeData.reason`. */
	reason: string;
}

/**
 * Inspecte les sources locales sans rien ouvrir.
 *
 * `resolveMirrorPath()` accepte un chemin explicite ou `SQLITE_DB_PATH` sans
 * vérifier son existence (c'est voulu : l'appelant a été explicite). Ici on
 * vérifie, sinon `mode: "auto"` choisirait « local » pour un chemin épinglé qui
 * n'existe pas — exactement la panne silencieuse qu'on veut éviter.
 */
export function inspectLocalData(): AzaleeLocalAvailability {
	const rawMirror = resolveMirrorPath();
	const mirror = rawMirror !== null && existsSync(rawMirror) ? rawMirror : null;
	const dataDir = resolveDataDir();
	const injectedProvider = hasDatabaseProvider();
	const available = injectedProvider || mirror !== null;

	const reason = injectedProvider
		? "client de base injecté par l'application hôte"
		: mirror !== null
			? `miroir SQLite présent (${mirror})`
			: "aucun miroir SQLite ni client injecté sur cette machine";

	return { mirror, dataDir, injectedProvider, available, reason };
}

/** Options de `createAzaleeData`. */
export interface AzaleeDataOptions extends Omit<AzaleeClientOptions, "transport"> {
	/** Stratégie de sélection (défaut : `"auto"`). */
	mode?: AzaleeSourceMode;
	/**
	 * Autoriser la bascule à chaud vers l'API distante quand une lecture locale
	 * échoue (défaut : `true` en mode `auto`, `false` sinon). Un `404` ne
	 * déclenche jamais de bascule : c'est une réponse, pas une panne.
	 */
	fallbackOnError?: boolean;
	/** CORS de la réponse locale — sans objet hors navigateur, laissé à `false`. */
	cors?: boolean | string;
}

/** Client de lecture augmenté de sa provenance. */
export interface AzaleeData extends AzaleeClient {
	/** Source retenue au démarrage. */
	readonly source: AzaleeSourceKind;
	/** Explication du choix, affichable dans un CLI ou une barre d'état. */
	readonly reason: string;
	/** État des sources locales au moment de la construction. */
	readonly local: AzaleeLocalAvailability;
	/** `true` si une lecture locale a échoué et que les appels partent à distance. */
	isDegraded(): boolean;
}

/**
 * Préfixe de chemin de la base d'URL distante (`/azalee` pour
 * `https://api.rosegriffon.fr/azalee`). La passerelle nginx le retire avant de
 * transmettre au service ; le transport local doit faire la même chose, sinon
 * il chercherait la route `/azalee/api/characters` qui n'existe pas.
 */
function basePathPrefix(baseUrl: string): string {
	try {
		return new URL(baseUrl).pathname.replace(/\/+$/, "");
	} catch {
		return "";
	}
}

/**
 * Transport « local » : `handleAzaleeRequest`, sans socket ni sérialisation
 * réseau. L'hôte est ignoré ; seuls le chemin (dépréfixé) et la chaîne de
 * requête comptent.
 */
function localTransport(prefix: string, cors: boolean | string): AzaleeTransport {
	return (request) => {
		const url = new URL(request.url);
		if (prefix && url.pathname.startsWith(prefix)) {
			url.pathname = url.pathname.slice(prefix.length) || "/";
		}
		return handleAzaleeRequest(new Request(`http://azalee.local${url.pathname}${url.search}`, request), { cors });
	};
}

/**
 * Transport de repli : tente le local, bascule sur le distant si la lecture
 * locale lève ou renvoie un `5xx`. Un `4xx` (dont `404`) est une réponse
 * légitime et remonte telle quelle — replier dessus ferait apparaître dans le
 * GUI des fiches absentes du miroir local, ce qui rendrait la source
 * imprévisible.
 *
 * La requête transmise au distant est l'originale : elle porte déjà l'URL
 * publique complète, préfixe inclus.
 */
function fallbackTransport(
	local: AzaleeTransport,
	remote: AzaleeTransport,
	state: { degraded: boolean },
): AzaleeTransport {
	return async (request) => {
		if (state.degraded) return remote(request);
		let response: Response;
		try {
			response = await local(request);
		} catch {
			state.degraded = true;
			return remote(request);
		}
		if (response.status >= 500) {
			state.degraded = true;
			return remote(request);
		}
		return response;
	};
}

/**
 * Crée une source de lecture Azalée qui fonctionne avec ou sans données
 * locales.
 *
 * ```ts
 * import { createAzaleeData } from "./";
 *
 * const data = createAzaleeData();          // auto
 * console.log(data.source, data.reason);    // "local" | "remote"
 * const { data: rows } = await data.characters({ q: "Mark", limit: 5 });
 * ```
 *
 * En mode `local`, `baseUrl` ne sert qu'à former des URL lisibles dans les
 * messages d'erreur : aucune requête réseau n'est émise.
 */
export function createAzaleeData(options: AzaleeDataOptions = {}): AzaleeData {
	const { mode = "auto", fallbackOnError, cors = false, ...clientOptions } = options;
	const local = inspectLocalData();

	const source: AzaleeSourceKind = mode === "auto" ? (local.available ? "local" : "remote") : mode;
	const reason =
		mode === "auto"
			? local.available
				? `source locale retenue : ${local.reason}`
				: `repli distant : ${local.reason}`
			: `source ${source} imposée par l'appelant`;

	const state = { degraded: false };
	const allowFallback = fallbackOnError ?? mode === "auto";
	// La base d'URL reste toujours la distante : elle sert d'adresse de repli
	// et rend les messages d'erreur lisibles même en mode local.
	const baseUrl = resolveAzaleeApiUrl(clientOptions.baseUrl);

	let transport: AzaleeTransport | undefined;
	if (source === "local") {
		const direct = localTransport(basePathPrefix(baseUrl), cors);
		transport = allowFallback
			? fallbackTransport(direct, (request) => (clientOptions.fetch ?? globalThis.fetch)(request), state)
			: direct;
	}

	const client = createAzaleeClient({
		...clientOptions,
		baseUrl,
		...(transport ? { transport } : {}),
	});

	return {
		...client,
		source,
		reason,
		local,
		isDegraded: () => state.degraded,
	};
}
