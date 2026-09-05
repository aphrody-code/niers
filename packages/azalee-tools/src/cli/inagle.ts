/**
 * Accès aux données de jeu pour le CLI : service inagle et résolution des
 * auras (Esprit Guerrier / Totem / Miximax) depuis le miroir SQLite.
 *
 * Les formes renvoyées par inagle sont hétérogènes (un « skill » peut être une
 * technique, une passive ou une aura) : elles restent volontairement souples
 * ici, mais toute la *présentation* est typée dans `render.ts`.
 */

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

import { createInagleService } from "@rosegriffon/inagle/service";

import { getSqlitePath, openReadonlyDatabase } from "./context";

/** Service inagle résolu (types réels du package, pas un `any` de façade). */
export type InagleService = Awaited<ReturnType<typeof createInagleService>>;

/** Type d'aura tel qu'affiché : dérivé du nom de table `inagle_*`. */
export type AuraKind = "keshin" | "soul" | "miximax" | "aura";

/** Aura résolue (identifiant, nom localisé, famille). */
export interface ResolvedAura {
	id: string;
	name: string;
	type: AuraKind | string;
}

/** Tables d'auras interrogées, dans l'ordre de priorité d'affichage. */
const AURA_TABLES = [
	{ table: "inagle_keshins", prefix: "keshin_" },
	{ table: "inagle_souls", prefix: "soul_" },
	{ table: "inagle_miximax", prefix: "miximax_" },
] as const;

/** Ligne d'aura telle que sélectionnée dans le miroir. */
interface AuraRow {
	id: string;
	name_fr: string | null;
	name_en: string | null;
	name_ja: string | null;
}

/**
 * Vue commune aux trois familles de « skills » d'inagle (technique, passive,
 * aura). Chaque famille a son propre type, mais un identifiant peut désigner
 * n'importe laquelle : cette vue décrit l'intersection réellement lue ici,
 * tous champs optionnels.
 */
interface SkillLike {
	displayName?: string;
	name_FR?: string;
	name_EN?: string;
	name_JA?: string;
	power_max?: number;
	power?: number;
	cost?: number;
	tp?: number;
	elementName?: { fr?: string; en?: string };
	categoryName?: { fr?: string; en?: string };
}

/** Résout un identifiant dans les trois familles, dans l'ordre historique. */
function lookupSkill(svc: InagleService, skillId: string): SkillLike | undefined {
	return (svc.skills.get(skillId) || svc.skills.passiveGet(skillId) || svc.skills.auraGet(skillId)) as
		| SkillLike
		| undefined;
}

/** Nom affichable d'une technique à partir de son identifiant. */
export function getSkillName(svc: InagleService, skillId: string): string {
	if (!skillId) return "N/A";
	const s = lookupSkill(svc, skillId);
	return s ? s.displayName || s.name_FR || s.name_EN || s.name_JA || skillId : skillId;
}

/** Détail d'une technique (nom, puissance, coût, élément, catégorie). */
export interface SkillDetails {
	id: string;
	name: string;
	power?: number;
	cost?: number;
	element?: string;
	category?: string;
}

export function getSkillDetails(svc: InagleService, skillId: string): SkillDetails {
	const s = lookupSkill(svc, skillId);
	if (!s) return { id: skillId, name: skillId };
	return {
		id: skillId,
		name: s.displayName || s.name_FR || s.name_EN || s.name_JA || skillId,
		power: s.power_max ?? s.power ?? undefined,
		cost: s.cost ?? s.tp ?? undefined,
		element: s.elementName?.fr || s.elementName?.en || undefined,
		category: s.categoryName?.fr || s.categoryName?.en || undefined,
	};
}

/** Convertit un nom de table `inagle_*` en libellé de famille d'aura. */
function tableToKind(table: string): string {
	return table.replace("inagle_", "").replace("s", "");
}

/**
 * Auras d'un personnage : union de `inagle_characters.data->auras` (miroir) et
 * du mapping `change_aura_skills.json` d'inagle. Renvoie `[]` si le miroir est
 * absent — une aura manquante ne doit jamais faire échouer une fiche.
 */
export function getAurasForChara(charaId: string, charaParamId: string): ResolvedAura[] {
	const dbPath = getSqlitePath();
	if (!dbPath) return [];

	try {
		const db = openReadonlyDatabase(dbPath);

		const hexIds = new Set<string>();

		// 1. Miroir SQLite : inagle_characters (data->'auras')
		const rows = db
			.query("SELECT data FROM inagle_characters WHERE chara_id = ? OR id = ?")
			.all(charaId, charaParamId) as Array<{ data?: string | null }>;

		for (const row of rows) {
			if (row.data) {
				try {
					const dataObj = JSON.parse(row.data) as { auras?: Array<{ skillId?: string }> };
					const dbAuras = dataObj.auras || [];
					for (const a of dbAuras) {
						if (a.skillId) hexIds.add(a.skillId.toLowerCase());
					}
				} catch {}
			}
		}

		// 2. Mapping change_aura_skills d'inagle (source hors miroir).
		const changeAuraSkillsPath = path.resolve(
			process.cwd(),
			"packages/inagle/src/entries/change_aura_skills.json",
		);
		if (existsSync(changeAuraSkillsPath)) {
			try {
				const changeAuraSkills = JSON.parse(readFileSync(changeAuraSkillsPath, "utf-8")) as Array<{
					id?: string;
					charaParamId?: string;
				}>;
				const jsonAuras = changeAuraSkills.filter(
					(c) =>
						c.charaParamId &&
						(c.charaParamId.toLowerCase() === charaId.toLowerCase() ||
							c.charaParamId.toLowerCase() === charaParamId.toLowerCase()),
				);
				for (const a of jsonAuras) {
					if (a.id) hexIds.add(a.id.toLowerCase());
				}
			} catch {}
		}

		const results: ResolvedAura[] = [];

		for (const hexId of hexIds) {
			const cleanHexPart = hexId.replace(/^0x/i, "").toUpperCase();
			const formattedId = "0x" + cleanHexPart;

			for (const t of AURA_TABLES) {
				const prefixedId = t.prefix + formattedId;
				const possibleIds = [prefixedId, formattedId, hexId];

				const placeHolders = possibleIds.map(() => "?").join(",");
				const auraRows = db
					.query(`SELECT id, name_fr, name_en, name_ja FROM ${t.table} WHERE id IN (${placeHolders})`)
					.all(...possibleIds) as AuraRow[];

				if (auraRows && auraRows.length > 0) {
					const a = auraRows[0];
					results.push({
						id: a.id,
						name: a.name_fr || a.name_en || a.name_ja || "Inconnu",
						type: tableToKind(t.table),
					});
					break;
				}
			}
		}

		db.close();
		return results;
	} catch {
		return [];
	}
}

/** Métadonnées d'une aura à partir de son identifiant brut, ou `null`. */
export function getAuraMetadataFromDb(auraId: string): ResolvedAura | null {
	const dbPath = getSqlitePath();
	if (!dbPath) return null;

	try {
		const db = openReadonlyDatabase(dbPath);

		const cleanHexPart = auraId.replace(/^0x/i, "").toUpperCase();
		const formattedId = "0x" + cleanHexPart;

		for (const t of AURA_TABLES) {
			const prefixedId = t.prefix + formattedId;
			const possibleIds = [prefixedId, formattedId, auraId];
			const placeHolders = possibleIds.map(() => "?").join(",");

			const auraRows = db
				.query(`SELECT id, name_fr, name_en, name_ja FROM ${t.table} WHERE id IN (${placeHolders})`)
				.all(...possibleIds) as AuraRow[];

			if (auraRows && auraRows.length > 0) {
				const a = auraRows[0];
				db.close();
				return {
					id: a.id,
					name: a.name_fr || a.name_en || a.name_ja || "Inconnu",
					type: tableToKind(t.table),
				};
			}
		}

		db.close();
		return null;
	} catch {
		return null;
	}
}

export { createInagleService };
