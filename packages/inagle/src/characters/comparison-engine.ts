import type { CharacterVariant, BaseCharacter } from "../core/types.js";

export interface StatComparison {
	stat: string;
	baseValue: number;
	variantValue: number;
	difference: number;
}

export interface VariantComparisonResult {
	variantId: string;
	rarity: string;
	element: string;
	position: string;
	classification: "Base Version" | "Pure Upgrade" | "Element Shift" | "Position Shift" | "Tactical Variation" | "Series Evolution" | "Hybrid Evolution";
	elementChanged: boolean;
	positionChanged: boolean;
	statChanges: StatComparison[];
	totalStatDiff: number;
	addedSkills: string[];
	removedSkills: string[];
	explanation: string;
}

/**
 * Compare a variant against the base (youngest) variant of a character
 */
export function compareVariants(
	baseVariant: CharacterVariant,
	variant: CharacterVariant,
	skillsMap: Map<string, string> = new Map()
): VariantComparisonResult {
	if (baseVariant.charaParamId === variant.charaParamId) {
		return {
			variantId: variant.charaParamId,
			rarity: variant.rarity,
			element: variant.element,
			position: variant.position,
			classification: "Base Version",
			elementChanged: false,
			positionChanged: false,
			statChanges: [],
			totalStatDiff: 0,
			addedSkills: [],
			removedSkills: [],
			explanation: "Il s'agit de la version d'origine (la plus jeune).",
		};
	}

	const elementChanged = baseVariant.element !== variant.element;
	const positionChanged = baseVariant.position !== variant.position;

	// Compare stats (lv99)
	const statsKeys = ["kick", "control", "technique", "physical", "pressure", "agility", "intelligence"] as const;
	const statChanges: StatComparison[] = [];
	let totalStatDiff = 0;
	let allBetterOrEqual = true;
	let atLeastOneBetter = false;

	for (const key of statsKeys) {
		const baseVal = baseVariant.stats.lv99[key] || 0;
		const varVal = variant.stats.lv99[key] || 0;
		const diff = varVal - baseVal;
		totalStatDiff += diff;

		if (diff < 0) {
			allBetterOrEqual = false;
		}
		if (diff > 0) {
			atLeastOneBetter = true;
		}

		statChanges.push({
			stat: key,
			baseValue: baseVal,
			variantValue: varVal,
			difference: diff,
		});
	}

	// Compare movesets (skills)
	const getSkillName = (id: string) => skillsMap.get(id) || id;
	const baseSkills = new Set((baseVariant.skills || []).map((s) => s.skillId));
	const varSkills = new Set((variant.skills || []).map((s) => s.skillId));

	const addedSkills: string[] = [];
	const removedSkills: string[] = [];

	for (const id of varSkills) {
		if (!baseSkills.has(id)) {
			addedSkills.push(getSkillName(id));
		}
	}

	for (const id of baseSkills) {
		if (!varSkills.has(id)) {
			removedSkills.push(getSkillName(id));
		}
	}

	// Determine classification
	let classification: VariantComparisonResult["classification"] = "Tactical Variation";
	const details: string[] = [];

	if (elementChanged && positionChanged) {
		classification = "Hybrid Evolution";
		details.push(`Changement de poste (${baseVariant.position} ➔ ${variant.position}) et d'élément (${baseVariant.element} ➔ ${variant.element})`);
	} else if (positionChanged) {
		classification = "Position Shift";
		details.push(`Changement de poste de jeu : ${baseVariant.position} ➔ ${variant.position}`);
	} else if (elementChanged) {
		classification = "Element Shift";
		details.push(`Changement d'élément élémentaire : ${baseVariant.element} ➔ ${variant.element}`);
	} else if (allBetterOrEqual && atLeastOneBetter) {
		classification = "Pure Upgrade";
		details.push("Amélioration pure des statistiques globales");
	} else {
		classification = "Tactical Variation";
		details.push("Variante tactique avec ajustement de moveset et stats équilibrées");
	}

	if (totalStatDiff !== 0) {
		details.push(`Différence totale de statistiques de ${totalStatDiff > 0 ? "+" : ""}${totalStatDiff}`);
	}

	if (addedSkills.length > 0) {
		details.push(`Nouvelles techniques apprises : ${addedSkills.join(", ")}`);
	}

	return {
		variantId: variant.charaParamId,
		rarity: variant.rarity,
		element: variant.element,
		position: variant.position,
		classification,
		elementChanged,
		positionChanged,
		statChanges,
		totalStatDiff,
		addedSkills,
		removedSkills,
		explanation: details.join(". ") + ".",
	};
}

/**
 * Classifies all variants of a base character
 */
export function analyzeCharacterVariants(
	baseChar: BaseCharacter,
	skillsMap: Map<string, string> = new Map()
): VariantComparisonResult[] {
	if (!baseChar.variants || baseChar.variants.length === 0) return [];

	// Assume variants are pre-sorted youngest first (variants[0] is the base)
	const baseVariant = baseChar.variants[0];
	return baseChar.variants.map((v) => compareVariants(baseVariant, v, skillsMap));
}
