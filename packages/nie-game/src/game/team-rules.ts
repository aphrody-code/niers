import type { TeamMember, TeamMemberStats } from "./team-types";
import type { Formation } from "./formations";

export interface RecalculatedStats extends TeamMemberStats {
	combatPower: number;
}

export function getPositionMatchFactor(playerPos: string, slotId: string, formation: Formation): { factor: number; status: "match" | "adjacent" | "mismatch" | "none" } {
	if (!playerPos || !slotId) return { factor: 1.0, status: "none" };
	if (slotId.startsWith("reserve-") || slotId.startsWith("manager-") || slotId.startsWith("support-")) {
		return { factor: 1.0, status: "match" };
	}
	
	const slotIndex = parseInt(slotId.replace("field-", ""), 10);
	const posCoord = formation.positions.find(p => p.index === slotIndex);
	if (!posCoord) return { factor: 1.0, status: "none" };

	const p = playerPos.toUpperCase();
	const s = posCoord.role.toUpperCase();

	if (p === s) {
		return { factor: 1.0, status: "match" };
	}

	// Adjacent check:
	// MF is adjacent to FW and DF.
	// FW is adjacent to MF.
	// DF is adjacent to MF.
	// GK is adjacent to nothing.
	if (
		(p === "MF" && (s === "FW" || s === "DF")) ||
		(s === "MF" && (p === "FW" || p === "DF"))
	) {
		return { factor: 0.85, status: "adjacent" };
	}

	return { factor: 0.65, status: "mismatch" };
}

export function recalculateMemberStats(
	member: TeamMember,
	level: number,
	slotId: string,
	formation: Formation,
	dominantElement?: string | null,
	hasHarmony?: boolean
): RecalculatedStats {
	const baseStats = member.stats || {
		kick: 0,
		control: 0,
		technique: 0,
		pressure: 0,
		physical: 0,
		agility: 0,
		intelligence: 0
	};

	const levelFactor = 0.2 + 0.8 * (level - 1) / 98;
	const { factor: posFactor } = getPositionMatchFactor(member.position, slotId, formation);

	// Dominant element boost (+5% to stats if member shares element)
	const isDominant = dominantElement && member.element === dominantElement;
	const elementMultiplier = isDominant ? 1.05 : 1.0;

	// Harmony boost (+3% to all stats if active)
	const harmonyMultiplier = hasHarmony ? 1.03 : 1.0;

	const multiplier = posFactor * elementMultiplier * harmonyMultiplier;

	const recalculate = (val: number) => {
		return Math.round(val * levelFactor * multiplier);
	};

	const stats = {
		kick: recalculate(baseStats.kick),
		control: recalculate(baseStats.control),
		technique: recalculate(baseStats.technique),
		pressure: recalculate(baseStats.pressure),
		physical: recalculate(baseStats.physical),
		agility: recalculate(baseStats.agility),
		intelligence: recalculate(baseStats.intelligence)
	};

	const combatPower = Object.values(stats).reduce((a, b) => a + b, 0);

	return { ...stats, combatPower };
}

export interface ElementLink {
	slotA: string;
	slotB: string;
	element: string;
	coordA: { top: number; left: number };
	coordB: { top: number; left: number };
}

export interface ElementSynergyInfo {
	dominantElement: string | null;
	hasHarmony: boolean;
	links: ElementLink[];
}

export function calculateElementSynergies(
	members: Record<string, TeamMember>,
	formation: Formation
): ElementSynergyInfo {
	const fieldMembers = Object.entries(members).filter(([slotId]) => slotId.startsWith("field-"));
	
	const elementsCount: Record<string, number> = { Fire: 0, Forest: 0, Mountain: 0, Wind: 0 };
	for (const [_, m] of fieldMembers) {
		if (elementsCount[m.element] !== undefined) {
			elementsCount[m.element]++;
		}
	}

	// Dominant: most frequent element, must have at least 4 players
	let dominantElement: string | null = null;
	let maxCount = 0;
	for (const [el, count] of Object.entries(elementsCount)) {
		if (count >= 4 && count > maxCount) {
			dominantElement = el;
			maxCount = count;
		}
	}

	// Harmony: all 4 elements represented on field
	const hasHarmony = Object.values(elementsCount).every(count => count > 0);

	// Links: same element, distance < 25 between field slots (excluding GK since GK doesn't link)
	const links: ElementLink[] = [];
	
	for (let i = 0; i < fieldMembers.length; i++) {
		const [slotIdA, mA] = fieldMembers[i];
		const idxA = parseInt(slotIdA.replace("field-", ""), 10);
		const coordA = formation.positions.find(p => p.index === idxA);
		if (!coordA || coordA.role === "GK") continue;

		for (let j = i + 1; j < fieldMembers.length; j++) {
			const [slotIdB, mB] = fieldMembers[j];
			if (mA.element !== mB.element) continue;

			const idxB = parseInt(slotIdB.replace("field-", ""), 10);
			const coordB = formation.positions.find(p => p.index === idxB);
			if (!coordB || coordB.role === "GK") continue;

			const distance = Math.sqrt(
				Math.pow(coordA.top - coordB.top, 2) + Math.pow(coordA.left - coordB.left, 2)
			);
			
			if (distance < 25) {
				links.push({
					slotA: slotIdA,
					slotB: slotIdB,
					element: mA.element,
					coordA: { top: coordA.top, left: coordA.left },
					coordB: { top: coordB.top, left: coordB.left }
				});
			}
		}
	}

	return { dominantElement, hasHarmony, links };
}
