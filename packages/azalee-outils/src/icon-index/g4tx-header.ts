/**
 * Lecteur d'**en-tête** G4TX (conteneur de textures Level-5 « Graphics 4 Texture »).
 *
 * Port TypeScript **pur** de `nie_formats::g4tx::parse`
 * (`/home/ubuntu/niers/crates/engine/nie-formats/src/g4tx.rs`), limité à ce dont
 * l'indexation a besoin : la liste des **noms** de textures principales et des
 * **régions d'atlas**, plus leurs dimensions. Aucun décodage DDS/BC ici — le
 * pixel est l'affaire du service Rust `nie-model-serve`.
 *
 * ⚠ **Client-safe** : ne touche ni `node:fs`, ni `bun:sqlite`, ni le réseau. Il
 * prend un `Uint8Array` et rend des objets simples, donc il se bundle dans un
 * navigateur ou une webview Tauri.
 *
 * Un conteneur G4TX est de l'une des deux natures suivantes, et la distinction
 * commande la façon d'adresser une image :
 *
 * 1. **Multi-textures principales** (`subTextureCount === 0`) : chaque texture
 *    nommée porte son propre payload DDS complet. C'est le cas des icônes
 *    d'objets (`icon_item05.g4tx` = 80 textures 256×256). On **sélectionne** par
 *    nom, on ne rogne pas.
 * 2. **Atlas spatial** (`subTextureCount > 0`) : une (ou quelques) texture(s)
 *    principale(s) portant une table de rectangles nommés. On **rogne**.
 *
 * Disposition des tables (identique au port Rust, vérifiée octet à octet) :
 *
 * ```text
 * entryOffset     = 0x60
 * subEntryOffset  = entryOffset + textureCount * 0x30
 * hashOffset      = align16(subEntryOffset + subTextureCount * 0x18)
 * idOffset        = hashOffset + totalCount * 4
 * stringOffset    = align4(idOffset + totalCount)
 * payloadBase     = align16(headerSize + tableSize)
 * ```
 */

/** Magic « G4TX » en octets. */
const G4TX_MAGIC = [0x47, 0x34, 0x54, 0x58] as const; // "G4TX"

const HEADER_SIZE = 0x60;
const ENTRY_SIZE = 0x30;
const SUB_ENTRY_SIZE = 0x18;

/** Magic DDS (`"DDS "`). */
const DDS_MAGIC = 0x2053_4444;

/** En-tête G4TX (0x60 octets). */
export interface G4txHeader {
	/** Taille de l'en-tête (`0x60` observé). */
	headerSize: number;
	/** Type de fichier (`0x65` observé). */
	fileType: number;
	/** Taille de la table (sert au calcul de la base des payloads). */
	tableSize: number;
	/** Nombre de textures principales (entrées de 0x30 octets). */
	textureCount: number;
	/** Nombre total d'entrées nommées (principales + régions). */
	totalCount: number;
	/** Nombre de régions d'atlas (entrées de 0x18 octets). */
	subTextureCount: number;
}

/** Région d'atlas résolue (nom + rectangle). */
export interface G4txSubTexture {
	/** Identifiant (octet de la table d'ids). */
	id: number;
	/** Nom résolu depuis la table de chaînes. */
	name: string;
	/** Coin X du rectangle. */
	x: number;
	/** Coin Y du rectangle. */
	y: number;
	/** Largeur du rectangle. */
	width: number;
	/** Hauteur du rectangle. */
	height: number;
}

/** Texture principale d'un conteneur G4TX. */
export interface G4txTexture {
	/** Identifiant (octet de la table d'ids). */
	id: number;
	/** Nom de la texture (chaîne ASCII terminée par un zéro). */
	name: string;
	/** Largeur effective (en-tête DDS si présent, sinon champ d'entrée). */
	width: number;
	/** Hauteur effective. */
	height: number;
	/** Vrai si le payload commence par le magic DDS. */
	isDds: boolean;
	/** Offset absolu du payload dans le tampon. */
	dataOffset: number;
	/** Taille déclarée du payload. */
	dataSize: number;
	/** Régions d'atlas rattachées à cette texture principale. */
	subTextures: G4txSubTexture[];
}

/** Conteneur G4TX parsé. */
export interface G4tx {
	/** En-tête. */
	header: G4txHeader;
	/** Textures principales (avec leurs régions). */
	textures: G4txTexture[];
}

/** Vrai si `data` commence par le magic G4TX. */
export function isG4tx(data: Uint8Array): boolean {
	return (
		data.length >= 4 &&
		data[0] === G4TX_MAGIC[0] &&
		data[1] === G4TX_MAGIC[1] &&
		data[2] === G4TX_MAGIC[2] &&
		data[3] === G4TX_MAGIC[3]
	);
}

function align(value: number, to: number): number {
	return (value + (to - 1)) & ~(to - 1);
}

function readCString(data: Uint8Array, at: number): string {
	if (at < 0 || at >= data.length) return "";
	let end = at;
	while (end < data.length && data[end] !== 0) end += 1;
	return new TextDecoder("utf-8").decode(data.subarray(at, end));
}

/**
 * Parse un conteneur G4TX.
 *
 * Le payload des textures n'a pas besoin d'être présent : sur un tampon tronqué
 * on rend quand même noms, ids, dimensions d'entrée et régions (`isDds` faux).
 *
 * @throws {Error} magic absent, tampon plus court que l'en-tête, ou table hors limites.
 */
export function parseG4tx(data: Uint8Array): G4tx {
	if (data.length < HEADER_SIZE) {
		throw new Error(`G4TX : tampon trop court (${data.length} < ${HEADER_SIZE})`);
	}
	if (!isG4tx(data)) throw new Error("G4TX : magic absent");

	const view = new DataView(data.buffer, data.byteOffset, data.byteLength);

	const header: G4txHeader = {
		headerSize: view.getUint16(4, true),
		fileType: view.getUint16(6, true),
		tableSize: view.getUint32(0x0c, true),
		textureCount: view.getUint16(0x20, true),
		totalCount: view.getUint16(0x22, true),
		subTextureCount: data[0x25] ?? 0,
	};

	const { textureCount, totalCount, subTextureCount } = header;

	const entryOffset = HEADER_SIZE;
	const subEntryOffset = entryOffset + textureCount * ENTRY_SIZE;
	const hashOffset = align(subEntryOffset + subTextureCount * SUB_ENTRY_SIZE, 16);
	const idOffset = hashOffset + totalCount * 4;
	const stringOffset = align(idOffset + totalCount, 4);

	if (idOffset + totalCount > data.length) {
		throw new Error("G4TX : table d'ids hors limites");
	}
	if (stringOffset + totalCount * 2 > data.length) {
		throw new Error("G4TX : table d'offsets de chaînes hors limites");
	}

	// Offsets de chaînes : i16 signé par entrée, relatif à `stringOffset`.
	const stringOffsets: number[] = new Array(totalCount);
	for (let i = 0; i < totalCount; i += 1) {
		stringOffsets[i] = view.getInt16(stringOffset + i * 2, true);
	}
	const nameAt = (index: number): string => {
		const rel = stringOffsets[index];
		if (rel === undefined || rel < 0) return "";
		return readCString(data, stringOffset + rel);
	};

	const payloadBase = align(header.headerSize + header.tableSize, 16);

	// Régions d'atlas brutes (leur nom/id vit après les textures principales
	// dans les tables globales : index absolu = textureCount + rang).
	const subEntries: Array<{ entryId: number; x: number; y: number; width: number; height: number }> =
		new Array(subTextureCount);
	for (let i = 0; i < subTextureCount; i += 1) {
		const at = subEntryOffset + i * SUB_ENTRY_SIZE;
		subEntries[i] = {
			entryId: view.getInt16(at, true),
			// +0x02 : inconnu.
			x: view.getInt16(at + 4, true),
			y: view.getInt16(at + 6, true),
			width: view.getInt16(at + 8, true),
			height: view.getInt16(at + 10, true),
		};
	}

	const textures: G4txTexture[] = new Array(textureCount);
	for (let i = 0; i < textureCount; i += 1) {
		const at = entryOffset + i * ENTRY_SIZE;
		const chunkOffset = view.getUint32(at + 4, true);
		const chunkSize = view.getUint32(at + 8, true);
		const entryWidth = view.getInt16(at + 0x18, true);
		const entryHeight = view.getInt16(at + 0x1a, true);

		const dataOffset = payloadBase + chunkOffset;
		let isDds = false;
		let width = entryWidth;
		let height = entryHeight;
		if (dataOffset + 0x14 <= data.length && view.getUint32(dataOffset, true) === DDS_MAGIC) {
			isDds = true;
			// DDS_HEADER : height @+0x0C, width @+0x10.
			height = view.getInt32(dataOffset + 0x0c, true);
			width = view.getInt32(dataOffset + 0x10, true);
		}

		const subTextures: G4txSubTexture[] = [];
		for (let s = 0; s < subEntries.length; s += 1) {
			const sub = subEntries[s];
			if (!sub || sub.entryId !== i) continue;
			const absolute = textureCount + s;
			subTextures.push({
				id: data[idOffset + absolute] ?? 0,
				name: nameAt(absolute),
				x: sub.x,
				y: sub.y,
				width: sub.width,
				height: sub.height,
			});
		}

		textures[i] = {
			id: data[idOffset + i] ?? 0,
			name: nameAt(i),
			width,
			height,
			isDds,
			dataOffset,
			dataSize: chunkSize,
			subTextures,
		};
	}

	return { header, textures };
}
