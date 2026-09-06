import { existsSync, mkdirSync, readFileSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { gunzipSync } from "node:zlib";
import audioManifest from "../../data/audio-manifest.json";
import {
	type CharacterAssetLinks,
	CharacterAssetLinksSchema,
	type CrossAsset,
	CrossAssetSchema,
} from "../schemas/zod-schemas";

// Nom de l'index SQLite généré
const SQLITE_DB_NAME = "catalog-index-v2.sqlite";
const CACHE_DIR_NAME = "azalee-cross-catalog-v2";

/**
 * Résout le chemin absolu vers le fichier d'index NDJSON compressé.
 */
function resolveSourcePath(): string | null {
	const candidates = [
		process.env.AZALEE_DATA_DIR
			? path.join(process.env.AZALEE_DATA_DIR, "cross/catalog-index.ndjson.gz")
			: undefined,
		path.resolve(process.cwd(), "data/cross/catalog-index.ndjson.gz"),
		path.resolve(process.cwd(), "apps/azalee/data/cross/catalog-index.ndjson.gz"),
		path.resolve(process.cwd(), "packages/inagle-cross/data/catalog-index.ndjson.gz"),
	].filter((c): c is string => Boolean(c));
	for (const c of candidates) {
		if (existsSync(c)) return c;
	}
	return null;
}

let _db: any = null;

/**
 * Initialise et retourne la base de données SQLite en cache.
 */
export function getDb(): any {
	if (_db) return _db;
	const src = resolveSourcePath();
	if (!src) {
		throw new Error(
			"[inagle-cross-catalog] Fichier catalog-index.ndjson.gz introuvable dans les chemins standards."
		);
	}

	const cacheDir = path.join(tmpdir(), CACHE_DIR_NAME);
	mkdirSync(cacheDir, { recursive: true });
	const cachePath = path.join(cacheDir, SQLITE_DB_NAME);

	const needsBuild =
		!existsSync(cachePath) || statSync(cachePath).mtimeMs < statSync(src).mtimeMs;

	if (typeof Bun === "undefined") {
		throw new Error("[inagle-cross-catalog] Bun est requis pour utiliser bun:sqlite.");
	}

	// eslint-disable-next-line @typescript-eslint/no-require-imports
	const { Database: DBConstructor } = require("bun:sqlite");

	if (needsBuild) {
		const db = new DBConstructor(cachePath, { create: true });
		buildSqlite(db, src);
		_db = db;
	} else {
		_db = new DBConstructor(cachePath, { readonly: true });
		_db.exec("PRAGMA temp_store = MEMORY");
	}
	return _db;
}

function buildSqlite(db: any, srcPath: string): void {
	console.log("[inagle-cross-catalog] Matérialisation de l'index SQLite...");
	db.exec("PRAGMA journal_mode = WAL");
	db.exec("DROP TABLE IF EXISTS cross_catalog");
	db.exec(`
		CREATE TABLE cross_catalog (
			guid   TEXT NOT NULL PRIMARY KEY,
			key    TEXT NOT NULL,
			type   TEXT NOT NULL,
			bundle TEXT NOT NULL,
			size   INTEGER NOT NULL,
			n_deps INTEGER NOT NULL,
			deps   TEXT NOT NULL
		)
	`);

	const gz = readFileSync(srcPath);
	const ndjson = gunzipSync(gz).toString("utf8");

	const insert = db.prepare(
		"INSERT OR REPLACE INTO cross_catalog (guid, key, type, bundle, size, n_deps, deps) VALUES (?, ?, ?, ?, ?, ?, ?)"
	);

	db.transaction(() => {
		for (const line of ndjson.split("\n")) {
			if (!line) continue;
			try {
				const entry = JSON.parse(line);
				if (entry.kind === "asset") {
					insert.run(
						entry.guid,
						entry.key,
						entry.type || "Unknown",
						entry.bundle || "",
						entry.size || 0,
						entry.n_deps || 0,
						JSON.stringify(entry.deps || [])
					);
				}
			} catch {
				// Ignorer les lignes corrompues
			}
		}
	})();

	db.exec("CREATE INDEX idx_catalog_type ON cross_catalog (type)");
	db.exec("CREATE INDEX idx_catalog_key ON cross_catalog (key)");
	db.exec("PRAGMA wal_checkpoint(TRUNCATE)");
	db.exec("ANALYZE");
	console.log("[inagle-cross-catalog] Matérialisation terminée avec succès.");
}

/**
 * Recherche paginée dans le catalogue d'assets.
 */
export function searchCatalog(
	q?: string,
	type?: string,
	limit = 50,
	offset = 0
): { assets: CrossAsset[]; total: number } {
	const db = getDb();
	const cleanQ = (q ?? "").trim().toLowerCase();
	const cleanType = (type ?? "").trim();

	let query = "SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE 1=1";
	let countQuery = "SELECT COUNT(*) AS total FROM cross_catalog WHERE 1=1";
	const params: any[] = [];

	if (cleanQ) {
		const filter = `%${cleanQ}%`;
		query += " AND (lower(key) LIKE ? OR lower(bundle) LIKE ?)";
		countQuery += " AND (lower(key) LIKE ? OR lower(bundle) LIKE ?)";
		params.push(filter, filter);
	}

	if (cleanType) {
		query += " AND type = ?";
		countQuery += " AND type = ?";
		params.push(cleanType);
	}

	const totalRow = db.query(countQuery).get(...params) as { total: number } | null;
	const total = totalRow?.total ?? 0;

	query += " ORDER BY key LIMIT ? OFFSET ?";
	const rows = db.query(query).all(...params, limit, offset) as any[];

	const assets = rows.map((r) => {
		let deps: string[] = [];
		try {
			deps = JSON.parse(r.deps);
		} catch {}
		return CrossAssetSchema.parse({
			guid: r.guid,
			key: r.key,
			type: r.type,
			bundle: r.bundle,
			size: r.size,
			n_deps: r.n_deps,
			deps,
		});
	});

	return { assets, total };
}

/**
 * Récupère un asset unique par son GUID.
 */
export function getAssetByGuid(guid: string): CrossAsset | null {
	const db = getDb();
	const row = db
		.query("SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE guid = ?")
		.get(guid) as any;
	if (!row) return null;

	let deps: string[] = [];
	try {
		deps = JSON.parse(row.deps);
	} catch {}

	return CrossAssetSchema.parse({
		guid: row.guid,
		key: row.key,
		type: row.type,
		bundle: row.bundle,
		size: row.size,
		n_deps: row.n_deps,
		deps,
	});
}

/**
 * Récupère un asset unique par sa clé exacte.
 */
export function getAssetByKey(key: string): CrossAsset | null {
	const db = getDb();
	const row = db
		.query("SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE key = ?")
		.get(key) as any;
	if (!row) return null;

	let deps: string[] = [];
	try {
		deps = JSON.parse(row.deps);
	} catch {}

	return CrossAssetSchema.parse({
		guid: row.guid,
		key: row.key,
		type: row.type,
		bundle: row.bundle,
		size: row.size,
		n_deps: row.n_deps,
		deps,
	});
}

/**
 * Liste les types distincts d'assets.
 */
export function getCatalogTypes(): string[] {
	const db = getDb();
	const rows = db.query("SELECT DISTINCT type FROM cross_catalog ORDER BY type").all() as {
		type: string;
	}[];
	return rows.map((r) => r.type);
}

/**
 * Mappe le code de silhouette (Shape) vers le squelette/corps partagé.
 */
export function getBodyCodeFromShape(shape: number | string | undefined): string {
	if (!shape) return "c000101";
	const val = typeof shape === "number" ? shape : parseInt(shape);
	if (isNaN(val)) {
		const str = shape.toString().toLowerCase();
		if (str.includes("female")) {
			if (str.includes("large") || str.includes("tall") || str.includes("muscular")) return "c000401";
			return "c000201";
		}
		if (str.includes("large") || str.includes("tall") || str.includes("muscular")) return "c000301";
		return "c000101";
	}
	// 1-3 : Hommes standards, petits, ronds
	if (val >= 1 && val <= 3) return "c000101";
	// 4-9 : Hommes grands, athlétiques, costauds
	if (val >= 4 && val <= 9) return "c000301";
	// 10-12 : Femmes standards, petites, rondes
	if (val >= 10 && val <= 12) return "c000201";
	// 13-18 : Femmes grandes, athlétiques, costaudes
	if (val >= 13 && val <= 18) return "c000401";
	return "c000101";
}

/**
 * Cibles d'entrée pour la classification d'assets
 */
export interface CharacterAssetInput {
	code: string | number;
	characterIconCode?: number;
	faceCode?: number;
	voiceCode?: number;
	shape?: number | string;
	defaultModelSet?: {
		uniformCode?: number;
		skinCode?: number;
		gloveCode?: number;
		shoesCode?: number;
		captainMarkCode?: number;
		uniformNumberCode?: number;
		accessoryCodes?: number[];
	};
	shapeCorrectionCode?: number | null;
}

/**
 * Classifie et lie les assets d'un personnage à partir du catalogue d'assets.
 */
export function classifyCharacterAssets(input: CharacterAssetInput): CharacterAssetLinks {
	const rawCode = input.code.toString();
	const codeClean = rawCode.startsWith("c") ? rawCode : `c${rawCode.padStart(8, "0")}`;
	const codeNumberStr = codeClean.slice(1);
	const codeNumber = parseInt(codeNumberStr) || 0;

	// Déduction des codes annexes s'ils ne sont pas fournis
	const iconVal = input.characterIconCode ?? codeNumber;
	const faceVal = input.faceCode ?? codeNumber;
	const voiceVal = input.voiceCode ?? codeNumber;

	const iconCodeStr = `c${iconVal.toString().padStart(8, "0")}`;
	const faceCodeStr = `c${faceVal.toString().padStart(8, "0")}`;
	const voiceCodeStr = `c${voiceVal.toString().padStart(8, "0")}`;

	const bodyCode = getBodyCodeFromShape(input.shape);

	// 1. Icone
	const iconKey = `Icons/Character/${iconCodeStr}.png`;
	const iconAsset = getAssetByKey(iconKey);

	// 2. Voix ACB/AWB
	const acbKey = `Sound/CharacterVoice/${voiceCodeStr}.acb`;
	const awbKey = `Sound/CharacterVoice/${voiceCodeStr}.awb`;
	const voiceAcbAsset = getAssetByKey(acbKey);
	const voiceAwbAsset = getAssetByKey(awbKey);

	// 3. Audio WAV cues issus du manifest
	const voiceWavs: { cueName: string; fileName: string; url: string }[] = [];
	const characters = (audioManifest as any).characters || {};
	const cues = characters[voiceCodeStr] as string[] | undefined;
	if (cues) {
		for (const cue of cues) {
			const cleanCue = cue.replace(/;.*$/, "").trim();
			const fileName = `${voiceCodeStr}_${cleanCue}.wav`;
			const url = `https://cdn.rosegriffon.fr/cross/audio/${fileName}`;
			const labelMatch = cleanCue.match(/_([A-Za-z]+)$/);
			const cueName = labelMatch ? labelMatch[1] : cleanCue;
			voiceWavs.push({ cueName, fileName, url });
		}
	}

	// 4. Visages in-game / director
	const faceInGameKey = `CharacterParts/Face/${faceCodeStr}/Prefabs/${faceCodeStr}.prefab`;
	const faceDirectorKey = `DirectorParts/Face/${faceCodeStr}/Prefabs/face_${faceCodeStr}.prefab`;
	const faceInGameAsset = getAssetByKey(faceInGameKey);
	const faceDirectorAsset = getAssetByKey(faceDirectorKey);

	// 5. Corps
	const bodyMeshKey = `CharacterParts/Base/Fbx/${bodyCode}.fbx`;
	const bodyMeshAsset = getAssetByKey(bodyMeshKey);

	// 6. Animations / Motions
	const db = getDb();
	const motionsRows = db
		.query(
			"SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE key LIKE ? AND type = 'AnimationClip'"
		)
		.all(`Motions/Battle/${bodyCode}/%`) as any[];
	const motions = motionsRows.map((r) => {
		let deps: string[] = [];
		try {
			deps = JSON.parse(r.deps);
		} catch {}
		return {
			key: r.key,
			asset: CrossAssetSchema.parse({
				guid: r.guid,
				key: r.key,
				type: r.type,
				bundle: r.bundle,
				size: r.size,
				n_deps: r.n_deps,
				deps,
			}),
		};
	});

	// 7. Uniformes
	const uniforms: any[] = [];
	const ms = input.defaultModelSet;
	if (ms && ms.uniformCode) {
		const uCode = ms.uniformCode;
		const uGroup = uCode.toString().padStart(4, "0");
		const shapeNum = bodyCode.slice(5); // ex: c000101 -> "01"

		// Mesh
		const meshKey = `CharacterParts/Uniform/Fbx/u${uGroup}${shapeNum}/u${uGroup}${shapeNum}.fbx`;
		const meshAsset = getAssetByKey(meshKey);

		// Matériaux / Variantes
		const uVariantPrefix = `CharacterParts/Uniform/u${uCode.toString().padStart(7, "0")}`;
		const matRows = db
			.query(
				"SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE key LIKE ? AND (type = 'Material' OR type = 'Texture2D')"
			)
			.all(`${uVariantPrefix}%`) as any[];

		// On extrait les Home (_H) et Away (_A)
		const homeRows = matRows.filter((r) => r.key.includes("_H"));
		const awayRows = matRows.filter((r) => r.key.includes("_A"));

		if (homeRows.length > 0) {
			const firstMat = homeRows.find((r) => r.type === "Material");
			uniforms.push({
				variant: `u${uCode.toString().padStart(7, "0")}`,
				isHome: true,
				mesh: meshAsset ? meshAsset.key : null,
				meshAsset,
				material: firstMat ? firstMat.key : homeRows[0].key,
				materialAsset: firstMat
					? CrossAssetSchema.parse({
							guid: firstMat.guid,
							key: firstMat.key,
							type: firstMat.type,
							bundle: firstMat.bundle,
							size: firstMat.size,
							n_deps: firstMat.n_deps,
							deps: JSON.parse(firstMat.deps),
						})
					: null,
			});
		}

		if (awayRows.length > 0) {
			const firstMat = awayRows.find((r) => r.type === "Material");
			uniforms.push({
				variant: `u${uCode.toString().padStart(7, "0")}`,
				isHome: false,
				mesh: meshAsset ? meshAsset.key : null,
				meshAsset,
				material: firstMat ? firstMat.key : awayRows[0].key,
				materialAsset: firstMat
					? CrossAssetSchema.parse({
							guid: firstMat.guid,
							key: firstMat.key,
							type: firstMat.type,
							bundle: firstMat.bundle,
							size: firstMat.size,
							n_deps: firstMat.n_deps,
							deps: JSON.parse(firstMat.deps),
						})
					: null,
			});
		}
	}

	// 8. Peau (skin)
	let skinAsset = null;
	if (ms && ms.skinCode) {
		const skinKey = `CharacterParts/Skin/Fbx/sk${ms.skinCode.toString().padStart(6, "0")}/`;
		const row = db
			.query(
				"SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE key LIKE ? LIMIT 1"
			)
			.get(`${skinKey}%`) as any;
		if (row) {
			skinAsset = CrossAssetSchema.parse({
				guid: row.guid,
				key: row.key,
				type: row.type,
				bundle: row.bundle,
				size: row.size,
				n_deps: row.n_deps,
				deps: JSON.parse(row.deps),
			});
		}
	}

	// 9. Gants (GK glove)
	let gloveAsset = null;
	if (ms && ms.gloveCode) {
		const gloveKey = `CharacterParts/Glove/Fbx/g${ms.gloveCode.toString().padStart(6, "0")}/`;
		const row = db
			.query(
				"SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE key LIKE ? LIMIT 1"
			)
			.get(`${gloveKey}%`) as any;
		if (row) {
			gloveAsset = CrossAssetSchema.parse({
				guid: row.guid,
				key: row.key,
				type: row.type,
				bundle: row.bundle,
				size: row.size,
				n_deps: row.n_deps,
				deps: JSON.parse(row.deps),
			});
		}
	}

	// 10. Chaussures (shoes)
	let shoesAsset = null;
	if (ms && ms.shoesCode) {
		const shoesKey = `CharacterParts/Shoes/Fbx/s${ms.shoesCode.toString().padStart(6, "0")}/`;
		const row = db
			.query(
				"SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE key LIKE ? LIMIT 1"
			)
			.get(`${shoesKey}%`) as any;
		if (row) {
			shoesAsset = CrossAssetSchema.parse({
				guid: row.guid,
				key: row.key,
				type: row.type,
				bundle: row.bundle,
				size: row.size,
				n_deps: row.n_deps,
				deps: JSON.parse(row.deps),
			});
		}
	}

	// 11. Brassard de capitaine (armband)
	let captainMarkAsset = null;
	if (ms && ms.captainMarkCode) {
		const markKey = `CharacterParts/Mark/Fbx/m${ms.captainMarkCode.toString().padStart(6, "0")}/`;
		const row = db
			.query(
				"SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE key LIKE ? LIMIT 1"
			)
			.get(`${markKey}%`) as any;
		if (row) {
			captainMarkAsset = CrossAssetSchema.parse({
				guid: row.guid,
				key: row.key,
				type: row.type,
				bundle: row.bundle,
				size: row.size,
				n_deps: row.n_deps,
				deps: JSON.parse(row.deps),
			});
		}
	}

	// 12. Numéros d'uniforme (uniformNumber)
	let uniformNumberAsset = null;
	if (ms && ms.uniformNumberCode) {
		const numKey = `CharacterParts/Number/Fbx/n${ms.uniformNumberCode.toString().padStart(6, "0")}/`;
		const row = db
			.query(
				"SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE key LIKE ? LIMIT 1"
			)
			.get(`${numKey}%`) as any;
		if (row) {
			uniformNumberAsset = CrossAssetSchema.parse({
				guid: row.guid,
				key: row.key,
				type: row.type,
				bundle: row.bundle,
				size: row.size,
				n_deps: row.n_deps,
				deps: JSON.parse(row.deps),
			});
		}
	}

	// 13. Shape correction
	let shapeCorrectionAsset = null;
	if (input.shapeCorrectionCode) {
		const shapeCorrKey = `CharacterParts/ShapeCorrection/${bodyCode}_`;
		const row = db
			.query(
				"SELECT guid, key, type, bundle, size, n_deps, deps FROM cross_catalog WHERE key LIKE ? LIMIT 1"
			)
			.get(`${shapeCorrKey}%`) as any;
		if (row) {
			shapeCorrectionAsset = CrossAssetSchema.parse({
				guid: row.guid,
				key: row.key,
				type: row.type,
				bundle: row.bundle,
				size: row.size,
				n_deps: row.n_deps,
				deps: JSON.parse(row.deps),
			});
		}
	}

	return CharacterAssetLinksSchema.parse({
		code: codeClean,
		icon: iconAsset ? iconAsset.key : null,
		iconAsset,
		voiceAcb: voiceAcbAsset ? voiceAcbAsset.key : null,
		voiceAcbAsset,
		voiceAwb: voiceAwbAsset ? voiceAwbAsset.key : null,
		voiceAwbAsset,
		voiceWavs,
		faceInGame: faceInGameAsset ? faceInGameAsset.key : null,
		faceInGameAsset,
		faceDirector: faceDirectorAsset ? faceDirectorAsset.key : null,
		faceDirectorAsset,
		bodyMesh: bodyMeshAsset ? bodyMeshAsset.key : null,
		bodyMeshAsset,
		motions,
		uniforms,
		skin: skinAsset ? skinAsset.key : null,
		skinAsset,
		glove: gloveAsset ? gloveAsset.key : null,
		gloveAsset,
		shoes: shoesAsset ? shoesAsset.key : null,
		shoesAsset,
		captainMark: captainMarkAsset ? captainMarkAsset.key : null,
		captainMarkAsset,
		uniformNumber: uniformNumberAsset ? uniformNumberAsset.key : null,
		uniformNumberAsset,
		shapeCorrection: shapeCorrectionAsset ? shapeCorrectionAsset.key : null,
		shapeCorrectionAsset,
	});
}
