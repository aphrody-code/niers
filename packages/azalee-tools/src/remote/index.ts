/**
 * `@rosegriffon/azalee/remote` — client HTTP typé de l'API Azalée.
 *
 * **Client-safe** : ce sous-chemin ne contient que du `fetch` standard et des
 * types effacés à la compilation. Il se bundle dans une webview Tauri, un
 * navigateur ou un worker (`bun build --target=browser` passe), là où
 * `@rosegriffon/azalee/server` reste réservé à un runtime Bun/Node.
 *
 * Il répond à un besoin précis : **le CLI et l'application de bureau doivent
 * servir des données même sans miroir SQLite ni dump du jeu sur la machine**.
 * Les mêmes 41 routes sont alors lues à distance au lieu du disque.
 *
 * ```ts
 * import { createAzaleeClient, isAzaleeNotFound } from "./";
 *
 * const api = createAzaleeClient(); // https://api.rosegriffon.fr/azalee
 * const mark = await api.character("mark-evans");
 * ```
 *
 * Côté serveur, `createAzaleeData` (dans `@rosegriffon/azalee/server`) choisit
 * automatiquement entre le disque local et ce client.
 */

export {
	AzaleeRemoteError,
	isAzaleeNotFound,
	isAzaleeRemoteError,
	isAzaleeRetriable,
	type AzaleeErrorKind,
	type AzaleeRemoteErrorInit,
} from "./errors";

export {
	AZALEE_CLIENT_DEFAULTS,
	AZALEE_DEFAULT_API_URL,
	AZALEE_SIDECAR_API_URL,
	AZALEE_USER_AGENT,
	buildQuery,
	createTransportContext,
	readEnv,
	requestJson,
	resolveAzaleeApiUrl,
	type AzaleeClientOptions,
	type AzaleeQueryValue,
	type AzaleeRequestOptions,
	type AzaleeTransport,
	type AzaleeTransportContext,
} from "./transport";

export {
	createAzaleeClient,
	defaultAzaleeCandidates,
	probeAzaleeApi,
	resolveAzaleeBaseUrl,
	type AzaleeClient,
} from "./client";

export type * from "./types";
