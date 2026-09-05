/**
 * Parseur **best-effort** du format `.g4cm` (caméra de cut-in Level-5, IEVR).
 *
 * Reversé depuis `data/common/event/ev60/ev60_00340/ev60_00340_camera.g4cm` (cut-in whs00340).
 * Le format fait partie de la famille de ressources Level-5 `G4MT/G4MA/G4TP/G4CM/G4VS/G4LA/G4BA`.
 *
 * CE QUI EST FIABLE (décodé byte-exact) :
 *  - en-tête + magic `G4CM` ;
 *  - la **table des plans** (« shots ») : N segments = les coupes caméra du cut-in
 *    (ex. c0010..c0050), chacun avec sa plage de frames `[startFrame, endFrame]` et son nom ;
 *  - la **table des pistes** (8 canaux × N plans) : 20 octets/enregistrement ;
 *  - les **timelines de keyframes** : tableaux de frames `u16` partagés (range 900..1500).
 *
 * CE QUI EST OPAQUE (NON récupérable de façon fiable) :
 *  - les **valeurs** des keyframes (position/rotation/FOV). Elles ne sont stockées NI en `f32`
 *    plain (scan: aucun run f32 « sain » contigu), NI en `fp16` (75% de valeurs aberrantes).
 *    Elles sont quantifiées/encodées en courbes (min+scale + échantillons compressés) ; sans le
 *    lecteur natif du jeu on ne peut pas les déquantifier sans risque d'hallucination.
 *    => `decodeTrackValues()` reste heuristique et renvoie `null` par défaut (cf. fallback).
 *
 * Conséquence pratique : on exploite la **structure temporelle réelle** (durée totale, nombre de
 * plans, points de coupe, quels canaux sont animés par plan) pour piloter une caméra **scénarisée
 * par plan** (cf. `buildCutinShots` dans `lib/cutin/camera.ts`), au lieu de positions brutes.
 *
 * Module **pur / client-safe** (aucun `bun:sqlite`, `fs`, `tls`) : utilisable dans un îlot
 * `"use client"` après `fetch('/raw/<vfs>')`.
 */

/** Identifiants de canaux observés (1 octet de tête par enregistrement de piste). */
export const G4CM_CHANNEL = {
	/** Position caméra (œil) X / Y / Z. */
	EYE_X: 0x16,
	EYE_Y: 0x17,
	EYE_Z: 0x18,
	/** Cible / point visé X / Y / Z. */
	TARGET_X: 0x1a,
	TARGET_Y: 0x1b,
	TARGET_Z: 0x1c,
	/** Optique : FOV et roll (twist), ordre observé 1e puis 1f. */
	FOV: 0x1e,
	ROLL: 0x1f,
} as const;

/** Étiquette lisible d'un canal (heuristique, voir note d'en-tête). */
export function g4cmChannelLabel(id: number): string {
	switch (id) {
		case 0x16:
			return "eyeX";
		case 0x17:
			return "eyeY";
		case 0x18:
			return "eyeZ";
		case 0x1a:
			return "targetX";
		case 0x1b:
			return "targetY";
		case 0x1c:
			return "targetZ";
		case 0x1e:
			return "fov";
		case 0x1f:
			return "roll";
		default:
			return `ch_0x${id.toString(16)}`;
	}
}

/** En-tête `.g4cm` (champs fiables uniquement). */
export interface G4cmHeader {
	/** Toujours `"G4CM"`. */
	magic: string;
	/** Offset (octets) du début de la table des plans (observé 0x40). */
	shotTableOffset: number;
	/** Taille déclarée de la section data utile (champ @0x0c). */
	dataSize: number;
	/** Nombre de plans (champ @0x20). */
	shotCount: number;
}

/** Un « plan » de caméra (coupe) du cut-in. */
export interface G4cmShot {
	/** Index 0-based du plan. */
	index: number;
	/** Nom interne (ex. `c0010`), résolu via la table de chaînes. */
	name: string;
	/** Première frame de jeu du plan (inclus). */
	startFrame: number;
	/** Dernière frame de jeu du plan (inclus). */
	endFrame: number;
	/** Durée en frames (= endFrame - startFrame + 1). */
	durationFrames: number;
}

/** Un enregistrement de piste (20 octets) : un canal animé sur un plan. */
export interface G4cmTrack {
	/** Décalage de l'enregistrement dans le fichier (debug). */
	recordOffset: number;
	/** Id de canal (cf. `G4CM_CHANNEL`). */
	channel: number;
	/** Libellé heuristique du canal. */
	channelLabel: string;
	/** `cnt` (1 = constant, 2 = linéaire, 3 = courbe/tangentes — interprétation heuristique). */
	kind: number;
	/** `true` si la piste est constante (une seule valeur, pas de timeline). */
	constant: boolean;
	/** Index de courbe (croissant pour les pistes animées ; 0 pour les constantes). */
	curveIndex: number;
	/** Décalage (en u16) dans le tableau de frames partagé. */
	frameOffset: number;
	/** Nombre de keyframes de la piste. */
	keyCount: number;
	/** Champ auxiliaire `a` (index présumé vers les valeurs ; usage exact non confirmé). */
	aux: number;
	/** Frames de la piste (résolues depuis le tableau partagé), ou `null` si constante. */
	frames: number[] | null;
}

/** Résultat complet du parse best-effort. */
export interface G4cmParsed {
	header: G4cmHeader;
	shots: G4cmShot[];
	tracks: G4cmTrack[];
	/** Tableau de frames partagé (u16) servant de timeline aux pistes. */
	frameTable: number[];
	/** Durée totale du cut-in (frames), du 1er au dernier plan. */
	totalFrames: number;
	/** Cadence supposée (le jeu tourne les cut-in à 60 fps). */
	assumedFps: number;
}

const REC_SIZE = 20;
const DEFAULT_FPS = 60;

/** Lit une chaîne C (NUL-terminée) à `off`. */
function readCString(view: DataView, off: number): string {
	let s = "";
	for (let i = off; i < view.byteLength; i++) {
		const c = view.getUint8(i);
		if (c === 0) break;
		s += String.fromCharCode(c);
	}
	return s;
}

/**
 * Parse un `.g4cm` (octets bruts, ex. via `fetch('/raw/<vfs>')`). **Best-effort** : décode la
 * structure temporelle fiable et ignore les valeurs de keyframes (cf. note d'en-tête).
 *
 * @throws si le magic n'est pas `G4CM`.
 */
export function parseG4cm(bytes: Uint8Array): G4cmParsed {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const magic = String.fromCharCode(bytes[0], bytes[1], bytes[2], bytes[3]);
	if (magic !== "G4CM") {
		throw new Error(`g4cm: magic inattendu "${magic}" (attendu G4CM)`);
	}

	const header: G4cmHeader = {
		magic,
		shotTableOffset: view.getUint16(0x04, true),
		dataSize: view.getUint32(0x0c, true),
		shotCount: view.getUint16(0x20, true),
	};

	// --- Table des plans : shotCount × 16 octets à shotTableOffset (start,end,idx u16…). ---
	const shots: G4cmShot[] = [];
	for (let i = 0; i < header.shotCount; i++) {
		const o = header.shotTableOffset + i * 16;
		if (o + 6 > view.byteLength) break;
		const startFrame = view.getUint16(o, true);
		const endFrame = view.getUint16(o + 2, true);
		shots.push({
			index: view.getUint16(o + 4, true),
			name: "",
			startFrame,
			endFrame,
			durationFrames: endFrame - startFrame + 1,
		});
	}

	// --- Table de chaînes des noms de plans (offsets u16 puis blob NUL-séparé). ---
	// Située juste après le pool de floats de config ; repérée par la 1re table d'offsets
	// croissants suivie d'octets ASCII imprimables. Implémentation tolérante : on cherche le
	// 1er motif "cXXXX\0" à partir de l'offset 0x130 (observé) et on remplit séquentiellement.
	const names = findShotNames(bytes, shots.length);
	shots.forEach((s, i) => {
		s.name = names[i] ?? `shot${i}`;
	});

	// --- Tableau de frames partagé : u16 dans [900..1500] à partir de la fin de la table de
	// pistes. On le localise après avoir lu les enregistrements (le bloc data suit). ---
	const { tracks, dataStart } = readTracks(view, bytes);
	const frameTable = readFrameTable(view, dataStart);

	// Résolution des frames de chaque piste animée depuis le tableau partagé.
	for (const t of tracks) {
		if (t.constant || t.keyCount <= 0) {
			t.frames = null;
			continue;
		}
		const out: number[] = [];
		for (let i = 0; i < t.keyCount; i++) {
			const idx = t.frameOffset + i;
			if (idx < frameTable.length) out.push(frameTable[idx]);
		}
		t.frames = out.length ? out : null;
	}

	const first = shots.length ? Math.min(...shots.map((s) => s.startFrame)) : 0;
	const last = shots.length ? Math.max(...shots.map((s) => s.endFrame)) : 0;

	return {
		header,
		shots,
		tracks,
		frameTable,
		totalFrames: last - first,
		assumedFps: DEFAULT_FPS,
	};
}

/** Lit les enregistrements de pistes (20 o.) jusqu'au premier mot nul ; renvoie le début du data. */
function readTracks(
	view: DataView,
	bytes: Uint8Array
): { tracks: G4cmTrack[]; dataStart: number } {
	// La table de pistes commence après la table de chaînes + une petite table d'index.
	// Observé : début à 0x190. On scanne depuis 0x190 par pas de 20 jusqu'au mot de tête nul.
	const START = 0x190;
	const tracks: G4cmTrack[] = [];
	let o = START;
	while (o + REC_SIZE <= view.byteLength) {
		const channel = bytes[o];
		const kind = bytes[o + 1];
		if (channel === 0 && kind === 0) break; // fin de table
		tracks.push({
			recordOffset: o,
			channel,
			channelLabel: g4cmChannelLabel(channel),
			kind,
			constant: kind === 1,
			curveIndex: view.getUint16(o + 6, true),
			aux: view.getUint32(o + 8, true),
			frameOffset: view.getUint32(o + 12, true),
			keyCount: view.getUint32(o + 16, true),
			frames: null,
		});
		o += REC_SIZE;
	}
	// Le bloc data (tableau de frames) suit, aligné 16 octets après le dernier enregistrement.
	const dataStart = (o + 15) & ~15;
	return { tracks, dataStart };
}

/** Lit le tableau de frames partagé (u16) tant que les valeurs restent dans [900..1500]. */
function readFrameTable(view: DataView, dataStart: number): number[] {
	const frames: number[] = [];
	for (let o = dataStart; o + 2 <= view.byteLength; o += 2) {
		const v = view.getUint16(o, true);
		// Le 1er u16 est souvent un en-tête (= dernière frame) ; on l'inclut tel quel, l'indexation
		// des pistes (frameOffset) en tient compte (offset 1 = 1re vraie frame).
		if (v < 800 || v > 1600) {
			if (frames.length > 4) break; // sortie de la zone de frames
			continue;
		}
		frames.push(v);
	}
	return frames;
}

/** Repère les noms de plans (`cXXXX`) dans le blob de chaînes. */
function findShotNames(bytes: Uint8Array, count: number): string[] {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const names: string[] = [];
	// Recherche du 1er octet ASCII 'c' (0x63) suivi de 4 chiffres, à partir de 0x130.
	for (let i = 0x130; i < Math.min(bytes.byteLength - 6, 0x400) && names.length < count; i++) {
		if (
			bytes[i] === 0x63 && // 'c'
			bytes[i + 1] >= 0x30 &&
			bytes[i + 1] <= 0x39
		) {
			const s = readCString(view, i);
			if (/^c\d{4,}$/.test(s)) {
				names.push(s);
				i += s.length; // saute la chaîne + son NUL
			}
		}
	}
	return names;
}

/**
 * Tente de déquantifier les valeurs d'une piste. **Non implémenté de façon fiable** : l'encodage
 * des valeurs est opaque (ni f32 ni fp16). Renvoie toujours `null` — utiliser le fallback
 * scénarisé par plan (`buildCutinShots`). Conservé comme point d'extension si le lecteur natif
 * (nie-engine) expose un jour le décodeur de courbes.
 */
export function decodeTrackValues(_track: G4cmTrack, _parsed: G4cmParsed): number[] | null {
	return null;
}
