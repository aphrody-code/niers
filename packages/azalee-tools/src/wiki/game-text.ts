/**
 * Couche de résolution de texte de jeu pour les pages wiki (RSC).
 *
 * Façade serveur au-dessus de `game-text/index.ts` (index SQLite des
 * 259 899 entrées texte décodées, matérialisé sur /tmp en singleton) + du mapping
 * `data/chr-model-names.json` (code de modèle 3D → vrai nom) produit à l'étape
 * index. But : les pages serveur résolvent un `hashId` ou un code de modèle vers
 * le VRAI texte du jeu au lieu d'afficher des IDs/codes bruts.
 *
 * ⚠ module **serveur** : touche `bun:sqlite` + `node:fs`. NE JAMAIS importer
 * depuis un composant `"use client"`. Les types + helpers purs (résolution de nom
 * à partir d'un mapping déjà chargé) vivent dans `game-text-shared.ts`, client-safe.
 */

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import {
	getText,
	getTextLocale,
	getTexts,
} from "../game-text/index";
import type { GameTextLocale, GameTextLocalized } from "../game-text/shared";
import { type ChrModelNames, resolveModelName } from "./game-text-shared";

// --- Réexports de la couche index (un seul point d'entrée « game-text » wiki) ---

export {
	categoryStats,
	getText,
	getTextLocale,
	getTexts,
	listCategory,
	searchText,
} from "../game-text/index";
export type { GameText, GameTextLocalized } from "../game-text/shared";

// --- Résolution texte (hashId) ---------------------------------------------

/**
 * Résout un `hashId` vers son texte dans une langue (repli fr → en → ja).
 * `undefined` si le hashId est inconnu de l'index. Alias clair `resolveText`
 * demandé par les pages wiki (sucre sur `getTextLocale`).
 */
export function resolveText(
	hashId: string | number,
	locale: GameTextLocale = "fr",
): string | undefined {
	return getTextLocale(hashId, locale);
}

/** Résout un `hashId` dans les 3 langues (+ catégorie). `undefined` si inconnu. */
export function resolveTextAll(hashId: string | number): GameTextLocalized | undefined {
	return getText(hashId);
}

/** Résout un lot de `hashId` d'un coup (lookup groupé, repli par langue côté caller). */
export function resolveTexts(
	hashIds: Array<string | number>,
): Map<string, GameTextLocalized> {
	return getTexts(hashIds);
}

// --- Résolution de NOM par code de modèle 3D (waza/item/animal) -------------

const CHR_MODEL_NAMES_FILES = [
	process.env.GAME_TEXT_DATA_DIR
		? path.join(process.env.GAME_TEXT_DATA_DIR, "chr-model-names.json")
		: undefined,
	path.resolve(process.cwd(), "data", "chr-model-names.json"),
	path.resolve(process.cwd(), "apps/azalee/data", "chr-model-names.json"),
].filter((p): p is string => Boolean(p));

let _modelNames: ChrModelNames | null = null;

/** Charge (et met en cache) le mapping code-modèle → nom. Singleton process-local. */
function getModelNames(): ChrModelNames {
	if (_modelNames) return _modelNames;
	for (const fp of CHR_MODEL_NAMES_FILES) {
		if (!existsSync(fp)) continue;
		try {
			const raw = JSON.parse(readFileSync(fp, "utf8")) as Partial<ChrModelNames>;
			_modelNames = {
				decorative: Array.isArray(raw.decorative) ? raw.decorative : [],
				names: raw.names && typeof raw.names === "object" ? raw.names : {},
			};
			return _modelNames;
		} catch {
			// Fichier corrompu : on tente la source suivante.
		}
	}
	// Artefact absent : repli vide (les codes restent en libellé, honnête).
	_modelNames = { decorative: [], names: {} };
	return _modelNames;
}

/**
 * Résout un code de modèle 3D (`i*`, `kt*`, `an*`, `b*`, `d*`, `ev*`) vers son
 * vrai nom. Renvoie `{ name, resolved }` : `resolved=true` si nom décodé réel,
 * sinon `name = code` (repli honnête pour les modèles décoratifs sans entrée texte).
 */
export function resolveModelCodeName(code: string | undefined | null): {
	name: string;
	resolved: boolean;
} {
	return resolveModelName(getModelNames(), code);
}

/** Renvoie le mapping code-modèle complet (pour câbler une galerie d'un coup). */
export function getChrModelNames(): ChrModelNames {
	return getModelNames();
}
