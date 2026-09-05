/**
 * Code de partage d'une composition d'équipe.
 *
 * Format : `<formationId>|<slot>:<charaId>|<slot>:<charaId>…`, encodé en
 * base64 **UTF-8**. C'est le format persisté dans les URLs partagées du wiki
 * et accepté par `azalee team-builder save` : la même implémentation doit donc
 * servir le navigateur, le CLI et un sidecar Tauri.
 *
 * 100 % API Web (`btoa`/`atob` + `TextEncoder`/`TextDecoder`) : aucun `Buffer`,
 * donc bundlable en webview. Byte-identique à
 * `Buffer.from(x).toString("base64")` pour tout texte UTF-8.
 */

/** Encode une chaîne UTF-8 en base64 (équivalent Web de `Buffer.from`). */
export function utf8ToBase64(text: string): string {
	const bytes = new TextEncoder().encode(text);
	let binary = "";
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary);
}

/** Décode une base64 vers la chaîne UTF-8 correspondante. */
export function base64ToUtf8(encoded: string): string {
	const binary = atob(encoded);
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	return new TextDecoder().decode(bytes);
}

/** Emplacement occupé dans une composition (`field-0`, `reserve-2`…). */
export interface TeamCodeSlot {
	slot: string;
	charaId: string;
}

/** Contenu décodé d'un code de partage d'équipe. */
export interface DecodedTeamCode {
	formationId: string;
	slots: TeamCodeSlot[];
}

/**
 * Construit un code de partage à partir d'une formation et des emplacements
 * occupés. L'ordre des emplacements est conservé tel quel.
 */
export function encodeTeamCode(formationId: string, slots: readonly TeamCodeSlot[]): string {
	const parts = [formationId, ...slots.map((s) => `${s.slot}:${s.charaId}`)];
	return utf8ToBase64(parts.join("|"));
}

/**
 * Décode un code de partage. Les segments malformés (sans `:`) sont ignorés,
 * comme le fait le wiki : un code tronqué reste exploitable partiellement.
 */
export function decodeTeamCode(encoded: string): DecodedTeamCode {
	const parts = base64ToUtf8(encoded).split("|");
	const slots: TeamCodeSlot[] = [];
	for (let i = 1; i < parts.length; i++) {
		const [slot, charaId] = parts[i].split(":");
		if (slot && charaId) slots.push({ slot, charaId });
	}
	return { formationId: parts[0] ?? "", slots };
}
