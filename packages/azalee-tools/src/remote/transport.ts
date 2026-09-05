/**
 * Transport HTTP du client Azalée : construction de la requête, délai
 * d'attente, annulation, nouvelles tentatives, décodage JSON.
 *
 * Ce module est **client-safe** : `fetch`, `AbortController`, `URL` et
 * `setTimeout` uniquement — il se bundle dans une webview Tauri ou un
 * navigateur. Aucun `node:*`, aucun `bun:*`, aucune référence nue à `process`.
 */

import { AzaleeRemoteError, isAzaleeRetriable } from "./errors";

/**
 * Base d'URL publique de l'API Azalée (passerelle nginx `api.rosegriffon.fr`,
 * qui route `/azalee/*` vers le service headless de cette bibliothèque).
 */
export const AZALEE_DEFAULT_API_URL = "https://api.rosegriffon.fr/azalee";

/**
 * Base d'URL du sidecar local (`azalee serve`), l'adresse qu'une application
 * Tauri interroge en priorité : mêmes routes, données du disque, zéro réseau.
 */
export const AZALEE_SIDECAR_API_URL = "http://127.0.0.1:3010";

/** En-tête d'identification par défaut (ignoré par les navigateurs, honoré par Bun/Node/Tauri). */
export const AZALEE_USER_AGENT = "@rosegriffon/azalee (+https://azalee.rosegriffon.fr)";

/**
 * Transport bas niveau : `Request` → `Response`. Par défaut le `fetch` global,
 * mais n'importe quelle fonction convient — `handleAzaleeRequest` de la lib
 * (mode local, zéro socket), le `fetch` du plugin HTTP de Tauri, ou un double
 * de test.
 */
export type AzaleeTransport = (request: Request) => Promise<Response>;

/** Options communes à tous les appels. */
export interface AzaleeRequestOptions {
	/** Annulation par l'appelant. Combinée au délai d'attente interne. */
	signal?: AbortSignal;
	/** Délai maximal par tentative, en millisecondes. Surcharge celui du client. */
	timeoutMs?: number;
	/** Nombre total de tentatives pour cet appel (1 = aucune reprise). */
	attempts?: number;
}

/** Configuration du client HTTP. */
export interface AzaleeClientOptions extends AzaleeRequestOptions {
	/**
	 * Base d'URL de l'API. Ordre de résolution : cette option → variable
	 * d'environnement `AZALEE_API_URL` → {@link AZALEE_DEFAULT_API_URL}.
	 */
	baseUrl?: string;
	/** Transport à utiliser. Par défaut `fetch` contre `baseUrl`. */
	transport?: AzaleeTransport;
	/** Implémentation de `fetch` à injecter (tests, plugin HTTP Tauri). */
	fetch?: typeof globalThis.fetch;
	/** En-têtes ajoutés à chaque requête. Voir la note CORS ci-dessous. */
	headers?: Record<string, string>;
	/** Valeur de `User-Agent`. `null` pour ne pas l'envoyer. */
	userAgent?: string | null;
	/** Délai de base entre deux tentatives, en millisecondes (défaut 300). */
	retryDelayMs?: number;
	/** Plafond du délai entre deux tentatives, en millisecondes (défaut 5000). */
	maxRetryDelayMs?: number;
}

/** Valeurs par défaut, exposées pour la documentation et les tests. */
export const AZALEE_CLIENT_DEFAULTS = {
	timeoutMs: 15_000,
	attempts: 3,
	retryDelayMs: 300,
	maxRetryDelayMs: 5_000,
} as const;

/**
 * Lit une variable d'environnement sans référencer `process` nu : dans un
 * navigateur ou une webview, `process` n'existe pas et une référence directe
 * ferait planter le module au chargement (ou serait remplacée en dur par un
 * bundler).
 */
export function readEnv(name: string): string | undefined {
	const host = globalThis as { process?: { env?: Record<string, string | undefined> } };
	return host.process?.env?.[name];
}

/**
 * Résout la base d'URL effective et retire le `/` final : toutes les routes de
 * `serve.ts` commencent par `/`, un double séparateur produirait un 404.
 */
export function resolveAzaleeApiUrl(explicit?: string): string {
	const raw = explicit ?? readEnv("AZALEE_API_URL") ?? AZALEE_DEFAULT_API_URL;
	return raw.replace(/\/+$/, "");
}

/** Valeur admissible dans une chaîne de requête. */
export type AzaleeQueryValue = string | number | boolean | undefined | null;

/**
 * Sérialise des paramètres en chaîne de requête. Les valeurs `undefined` et
 * `null` sont omises — passer `{ q: undefined }` doit produire la même URL que
 * `{}`, sinon le serveur reçoit `q=` et filtre sur la chaîne vide.
 */
export function buildQuery(params?: Record<string, AzaleeQueryValue>): string {
	if (!params) return "";
	const search = new URLSearchParams();
	for (const [key, value] of Object.entries(params)) {
		if (value === undefined || value === null) continue;
		search.set(key, String(value));
	}
	const serialized = search.toString();
	return serialized ? `?${serialized}` : "";
}

/**
 * Combine plusieurs signaux d'annulation et un délai d'attente en un seul
 * signal. `AbortSignal.any` n'est pas universellement disponible (WebKitGTK
 * ancien embarqué par Tauri sous Linux) : on retombe sur un `AbortController`
 * relayant le premier signal déclenché.
 */
function linkSignals(timeoutMs: number, external?: AbortSignal): { signal: AbortSignal; dispose: () => void } {
	const timeout = AbortSignal.timeout(timeoutMs);
	if (!external) return { signal: timeout, dispose: () => {} };

	// `AbortSignal.any` est une méthode STATIQUE qui construit via `new this` :
	// la détacher de son porteur produit « undefined is not a constructor ».
	const holder = AbortSignal as unknown as { any?: (signals: AbortSignal[]) => AbortSignal };
	if (typeof holder.any === "function") return { signal: holder.any([timeout, external]), dispose: () => {} };

	const controller = new AbortController();
	const relay = (source: AbortSignal) => () => controller.abort(source.reason);
	const onTimeout = relay(timeout);
	const onExternal = relay(external);
	if (timeout.aborted) onTimeout();
	else if (external.aborted) onExternal();
	else {
		timeout.addEventListener("abort", onTimeout, { once: true });
		external.addEventListener("abort", onExternal, { once: true });
	}
	return {
		signal: controller.signal,
		dispose: () => {
			timeout.removeEventListener("abort", onTimeout);
			external.removeEventListener("abort", onExternal);
		},
	};
}

/** Pause interruptible entre deux tentatives. */
function sleep(ms: number, signal?: AbortSignal): Promise<void> {
	return new Promise((resolve, reject) => {
		if (signal?.aborted) {
			reject(signal.reason);
			return;
		}
		const timer = setTimeout(() => {
			signal?.removeEventListener("abort", onAbort);
			resolve();
		}, ms);
		function onAbort() {
			clearTimeout(timer);
			reject(signal?.reason);
		}
		signal?.addEventListener("abort", onAbort, { once: true });
	});
}

/**
 * Convertit un en-tête `Retry-After` (secondes ou date HTTP) en millisecondes.
 * Renvoie `null` si l'en-tête est absent ou illisible.
 */
function retryAfterMs(response: Response): number | null {
	const raw = response.headers.get("Retry-After");
	if (!raw) return null;
	const seconds = Number.parseFloat(raw);
	if (Number.isFinite(seconds)) return Math.max(0, seconds * 1000);
	const date = Date.parse(raw);
	return Number.isFinite(date) ? Math.max(0, date - Date.now()) : null;
}

/** Délai exponentiel avec bruit, pour ne pas synchroniser plusieurs clients. */
function backoffMs(attempt: number, base: number, max: number): number {
	const exponential = Math.min(max, base * 2 ** attempt);
	return exponential * (0.5 + Math.random() * 0.5);
}

/** Distingue une annulation demandée par l'appelant, un délai dépassé et une panne réseau. */
function abortKind(
	signal: AbortSignal | undefined,
	external: AbortSignal | undefined,
): "abort" | "timeout" | "network" {
	if (external?.aborted) return "abort";
	return signal?.aborted ? "timeout" : "network";
}

/** Décode le corps d'une réponse en échec, sans jamais rejeter. */
async function readErrorBody(response: Response): Promise<{ detail?: string; body?: unknown }> {
	try {
		const text = await response.text();
		if (!text) return {};
		try {
			const parsed: unknown = JSON.parse(text);
			const detail =
				typeof parsed === "object" && parsed !== null && typeof (parsed as { error?: unknown }).error === "string"
					? (parsed as { error: string }).error
					: undefined;
			return detail === undefined ? { body: parsed } : { detail, body: parsed };
		} catch {
			return { detail: text.slice(0, 500), body: text };
		}
	} catch {
		return {};
	}
}

/** Contexte figé d'un client, partagé par tous les appels. */
export interface AzaleeTransportContext {
	baseUrl: string;
	transport: AzaleeTransport;
	headers: Record<string, string>;
	timeoutMs: number;
	attempts: number;
	retryDelayMs: number;
	maxRetryDelayMs: number;
	signal?: AbortSignal;
}

/**
 * Construit le contexte de transport d'un client à partir de ses options.
 *
 * Note CORS : seuls `Accept` et `User-Agent` sont envoyés par défaut. `Accept`
 * fait partie des en-têtes CORS sûrs et `User-Agent` est un en-tête interdit
 * (donc retiré) côté navigateur — aucun des deux ne déclenche de pré-vol
 * `OPTIONS`. Ajouter un en-tête personnalisé en déclencherait un, que la
 * réponse `Access-Control-Allow-Headers: Content-Type` de `serve.ts` refuserait.
 */
export function createTransportContext(options: AzaleeClientOptions = {}): AzaleeTransportContext {
	const baseUrl = resolveAzaleeApiUrl(options.baseUrl);
	const fetchImpl = options.fetch ?? globalThis.fetch;
	const transport: AzaleeTransport = options.transport ?? ((request) => fetchImpl(request));

	const headers: Record<string, string> = { Accept: "application/json", ...options.headers };
	const userAgent = options.userAgent === undefined ? AZALEE_USER_AGENT : options.userAgent;
	if (userAgent !== null) headers["User-Agent"] = userAgent;

	const context: AzaleeTransportContext = {
		baseUrl,
		transport,
		headers,
		timeoutMs: options.timeoutMs ?? AZALEE_CLIENT_DEFAULTS.timeoutMs,
		attempts: Math.max(1, options.attempts ?? AZALEE_CLIENT_DEFAULTS.attempts),
		retryDelayMs: options.retryDelayMs ?? AZALEE_CLIENT_DEFAULTS.retryDelayMs,
		maxRetryDelayMs: options.maxRetryDelayMs ?? AZALEE_CLIENT_DEFAULTS.maxRetryDelayMs,
	};
	if (options.signal) context.signal = options.signal;
	return context;
}

/** Exécute une tentative unique et renvoie la réponse, ou lève une erreur typée. */
async function attemptOnce(
	context: AzaleeTransportContext,
	url: string,
	options: AzaleeRequestOptions,
): Promise<Response> {
	const external = options.signal ?? context.signal;
	const { signal, dispose } = linkSignals(options.timeoutMs ?? context.timeoutMs, external);
	try {
		return await context.transport(new Request(url, { method: "GET", headers: context.headers, signal }));
	} catch (cause) {
		const kind = abortKind(signal, external);
		const message =
			kind === "abort"
				? `appel annulé : ${url}`
				: kind === "timeout"
					? `délai dépassé (${options.timeoutMs ?? context.timeoutMs} ms) : ${url}`
					: `API Azalée injoignable : ${url}`;
		throw new AzaleeRemoteError(message, { kind, url, cause });
	} finally {
		dispose();
	}
}

/**
 * Exécute une requête GET JSON avec reprise sur échec transitoire.
 *
 * Seuls les échecs réseau, les délais dépassés et les statuts `408`/`425`/
 * `429`/`5xx` sont réessayés ; un `404` remonte immédiatement. Un
 * `Retry-After` renvoyé par le serveur prime sur le délai exponentiel.
 */
export async function requestJson<T>(
	context: AzaleeTransportContext,
	path: string,
	query?: Record<string, AzaleeQueryValue>,
	options: AzaleeRequestOptions = {},
): Promise<T> {
	const url = `${context.baseUrl}${path.startsWith("/") ? path : `/${path}`}${buildQuery(query)}`;
	const attempts = Math.max(1, options.attempts ?? context.attempts);
	let lastError: unknown;
	/** Délai imposé par un `Retry-After` de la tentative précédente. */
	let serverDelayMs: number | null = null;

	for (let attempt = 0; attempt < attempts; attempt++) {
		if (attempt > 0) {
			const delay = serverDelayMs ?? backoffMs(attempt - 1, context.retryDelayMs, context.maxRetryDelayMs);
			serverDelayMs = null;
			await sleep(Math.min(delay, context.maxRetryDelayMs), options.signal ?? context.signal);
		}

		let response: Response;
		try {
			response = await attemptOnce(context, url, options);
		} catch (error) {
			lastError = error;
			if (isAzaleeRetriable(error) && attempt < attempts - 1) continue;
			throw error;
		}

		if (!response.ok) {
			serverDelayMs = retryAfterMs(response);
			const { detail, body } = await readErrorBody(response);
			const error = new AzaleeRemoteError(detail ?? `HTTP ${response.status} sur ${url}`, {
				kind: "http",
				status: response.status,
				url,
				...(detail === undefined ? {} : { detail }),
				...(body === undefined ? {} : { body }),
			});
			lastError = error;
			if (isAzaleeRetriable(error) && attempt < attempts - 1) continue;
			throw error;
		}

		try {
			return (await response.json()) as T;
		} catch (cause) {
			throw new AzaleeRemoteError(`réponse JSON illisible : ${url}`, {
				kind: "parse",
				status: response.status,
				url,
				cause,
			});
		}
	}

	/* c8 ignore next 2 — la boucle sort toujours par `return` ou `throw`. */
	// `lastError` est de type `unknown` : on ne relance jamais un littéral nu.
	throw lastError instanceof Error ? lastError : new Error(String(lastError));
}
