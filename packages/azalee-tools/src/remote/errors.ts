/**
 * Erreurs typées du client HTTP Azalée.
 *
 * Le client ne rejette **jamais** avec une valeur non typée : tout échec —
 * réseau, délai dépassé, annulation, statut HTTP, corps illisible — est
 * enveloppé dans une {@link AzaleeRemoteError} porteuse d'un `kind`
 * discriminant et du statut HTTP quand il existe. C'est la convention retenue
 * par `@octokit/request-error` (`status` + `request` + `response` sur une
 * sous-classe d'`Error`) et par `stripe-node` (hiérarchie d'erreurs typées),
 * plutôt que le `{ data, error }` d'`openapi-fetch` : avec 41 routes, une
 * exception typée garde les signatures lisibles.
 *
 * Ce module est **client-safe** : aucun `node:*`, aucun `bun:*`.
 */

/** Nature d'un échec d'appel à l'API Azalée. */
export type AzaleeErrorKind =
	/** Le serveur a répondu avec un statut non-2xx. */
	| "http"
	/** La requête n'a pas abouti (DNS, connexion refusée, hors ligne…). */
	| "network"
	/** Le délai imparti a expiré avant la réponse. */
	| "timeout"
	/** L'appelant a annulé via son `AbortSignal`. */
	| "abort"
	/** Réponse reçue mais corps JSON illisible. */
	| "parse";

/** Détails de construction d'une {@link AzaleeRemoteError}. */
export interface AzaleeRemoteErrorInit {
	kind: AzaleeErrorKind;
	/** Statut HTTP, ou `0` quand l'échec est antérieur à la réponse. */
	status?: number;
	/** URL absolue appelée (sans en-tête ni secret : l'API est publique et anonyme). */
	url: string;
	/** Champ `error` du corps JSON renvoyé par `handleAzaleeRequest`, s'il existe. */
	detail?: string;
	/** Corps brut décodé, conservé pour le diagnostic. */
	body?: unknown;
	/** Erreur d'origine (`TypeError` de `fetch`, `DOMException` d'abandon…). */
	cause?: unknown;
}

/**
 * Erreur unique du client distant. Discriminer sur `kind`, et sur `status`
 * pour les erreurs HTTP (`404` = ressource absente, `5xx` = repli possible).
 *
 * ```ts
 * try {
 *   await client.character("mark-evans");
 * } catch (error) {
 *   if (isAzaleeRemoteError(error) && error.status === 404) return null;
 *   throw error;
 * }
 * ```
 */
export class AzaleeRemoteError extends Error {
	/** Nature de l'échec. */
	readonly kind: AzaleeErrorKind;
	/** Statut HTTP (`0` hors réponse). */
	readonly status: number;
	/** URL absolue appelée. */
	readonly url: string;
	/** Message d'erreur renvoyé par l'API, quand il est exploitable. */
	readonly detail?: string;
	/** Corps décodé de la réponse en échec. */
	readonly body?: unknown;

	constructor(message: string, init: AzaleeRemoteErrorInit) {
		super(message, init.cause === undefined ? undefined : { cause: init.cause });
		this.name = "AzaleeRemoteError";
		this.kind = init.kind;
		this.status = init.status ?? 0;
		this.url = init.url;
		if (init.detail !== undefined) this.detail = init.detail;
		if (init.body !== undefined) this.body = init.body;
	}
}

/**
 * Garde de type sur {@link AzaleeRemoteError}. Le repli sur la forme
 * structurelle couvre le cas multi-realm (module dupliqué par un bundler,
 * message passé entre la webview et un worker) où `instanceof` échoue.
 */
export function isAzaleeRemoteError(value: unknown): value is AzaleeRemoteError {
	if (value instanceof AzaleeRemoteError) return true;
	if (typeof value !== "object" || value === null) return false;
	const candidate = value as Partial<AzaleeRemoteError>;
	return candidate.name === "AzaleeRemoteError" && typeof candidate.kind === "string";
}

/** `true` si l'échec est un 404 de l'API (ressource absente, pas une panne). */
export function isAzaleeNotFound(value: unknown): boolean {
	return isAzaleeRemoteError(value) && value.status === 404;
}

/**
 * `true` si l'échec justifie une nouvelle tentative : coupure réseau, délai
 * dépassé, ou statut transitoire (`408`, `425`, `429`, `5xx`). Un `4xx` métier
 * n'est jamais réessayé — il serait rejoué à l'identique.
 */
export function isAzaleeRetriable(value: unknown): boolean {
	if (!isAzaleeRemoteError(value)) return false;
	if (value.kind === "network" || value.kind === "timeout") return true;
	if (value.kind !== "http") return false;
	return value.status === 408 || value.status === 425 || value.status === 429 || value.status >= 500;
}
