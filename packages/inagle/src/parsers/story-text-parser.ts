import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { parseAllCharaBase } from "./chara-base.js";
import { buildAllNameMaps, getLocalizedNames } from "./chara-text.js";
import { type Locale, sanitizeText, toHex } from "../core/data-loader.js";
import { DATA_ROOT, resolveGameDataFile } from "../core/paths.js";

export interface ResolvedSpeaker {
	charaId: string;
	names: { en?: string; fr?: string; ja?: string };
}

export interface StoryDialogueLine {
	dialogueId: string; // e.g. "ev01_00010_010_010"
	hashId: string;     // e.g. "0xDD9B831F"
	speaker: ResolvedSpeaker;
	text: { en?: string; fr?: string; ja?: string };
}

export interface StoryEvent {
	eventId: string; // e.g. "ev01_00010"
	dialogues: StoryDialogueLine[];
}

export interface MapNpcDialogue {
	zoneId: string; // e.g. "w10i000"
	dialogues: Array<{
		hashId: string;
		text: { en?: string; fr?: string; ja?: string };
	}>;
}

export interface StoryGoal {
	chapterId: string; // e.g. "c01"
	goals: Array<{
		hashId: string;
		text: { en?: string; fr?: string; ja?: string };
	}>;
}

export interface ChapterPhase {
	chapterId: string; // e.g. "c01"
	phases: Array<{
		hashId: string;
		text: { en?: string; fr?: string; ja?: string };
	}>;
}

/**
 * Load name tags config to resolve speaker names
 */
function loadNameTagLookup(): Map<string, string[]> {
	const lookup = new Map<string, string[]>();
	const filePath = resolveGameDataFile("character", "chara_name_tag");
	if (!filePath || !existsSync(filePath)) return lookup;

	try {
		const data = JSON.parse(readFileSync(filePath, "utf-8"));
		const list = data.lists?.[0]?.values || [];
		for (const tag of list) {
			const charaId = tag.charaId.toUpperCase();
			const overloadNameId = tag.overloadNameId.toUpperCase();
			if (overloadNameId && overloadNameId !== "0X00000000") {
				const existing = lookup.get(charaId) || [];
				if (!existing.includes(overloadNameId)) {
					existing.push(overloadNameId);
					lookup.set(charaId, existing);
				}
			}
		}
	} catch (e) {
		console.error("[StoryTextParser] Failed to load name tags config", e);
	}
	return lookup;
}

/**
 * Resolve speaker names from speaker ID
 */
function buildSpeakerResolver() {
	const bases = parseAllCharaBase();
	const nameTags = loadNameTagLookup();
	const { names: nameMaps, romanized: romaMaps } = buildAllNameMaps();

	const baseCharaMap = new Map<string, typeof bases[0]>();
	for (const b of bases) {
		baseCharaMap.set(b.charaId.toUpperCase(), b);
	}

	return (speakerIdInt: number): ResolvedSpeaker => {
		const charaId = toHex(speakerIdInt).toUpperCase();
		const result: ResolvedSpeaker = { charaId, names: {} };

		// 1. Try name tags lookup first (overrides / NPC names)
		const tagNameHashes = nameTags.get(charaId);
		if (tagNameHashes && tagNameHashes.length > 0) {
			// Find the first valid name tag with translations
			for (const tagHash of tagNameHashes) {
				const localized = getLocalizedNames(tagHash, nameMaps, romaMaps);
				if (localized.ja || localized.en || localized.fr) {
					result.names = {
						ja: localized.ja,
						en: localized.en,
						fr: localized.fr,
					};
					return result;
				}
			}
		}

		// 2. Try playable characters base map
		const baseChara = baseCharaMap.get(charaId);
		if (baseChara) {
			const localized = getLocalizedNames(baseChara.nameHash, nameMaps, romaMaps);
			result.names = {
				ja: localized.ja,
				en: localized.en,
				fr: localized.fr,
			};
			return result;
		}

		// 3. Fallback: Check if the speaker is Unmei Sasanami test protagonist
		if (charaId === "0XED912800" || charaId === "0X686BE87E") {
			result.names = {
				ja: "笹波 雲明",
				en: "Destin Billows",
				fr: "Destin Billows",
			};
			return result;
		}

		// 4. Default fallback names if not found
		result.names = {
			ja: `NPC (${charaId})`,
			en: `NPC (${charaId})`,
			fr: `NPC (${charaId})`,
		};
		return result;
	};
}

/**
 * Parse all dialogues from story mode events
 */
export function parseAllEventDialogues(): StoryEvent[] {
	const eventDir = join(DATA_ROOT, "common/text/event");
	if (!existsSync(eventDir)) return [];

	const resolveSpeaker = buildSpeakerResolver();
	const files = readdirSync(eventDir).filter((f) => f.endsWith("_map.cfg.bin.json"));
	const events: StoryEvent[] = [];

	const locales: Locale[] = ["ja", "en", "fr"];

	for (const file of files) {
		const eventId = file.replace("_map.cfg.bin.json", "");
		const mapPath = join(eventDir, file);

		try {
			const mapData = JSON.parse(readFileSync(mapPath, "utf-8"));
			const washaBeg = mapData.entries.find((e: any) => e.name.startsWith("TEXT_WASHA_MAP_BEGIN_"));
			if (!washaBeg?.children) continue;

			// Load localized translations for this event
			const localizedMaps = new Map<Locale, Map<string, string>>();
			for (const loc of locales) {
				const locPath = join(DATA_ROOT, "common/text", loc, "event", `${eventId}.cfg.bin.json`);
				if (existsSync(locPath)) {
					const locData = JSON.parse(readFileSync(locPath, "utf-8"));
					const textBeg = locData.entries.find((e: any) => e.name.startsWith("TEXT_INFO_BEGIN_"));
					if (textBeg?.children) {
						const textMap = new Map<string, string>();
						for (const child of textBeg.children) {
							if (child.name.startsWith("TEXT_INFO_") && child.variables?.length >= 3) {
								const hashVal = parseInt(child.variables[0].value, 10);
								const textVal = child.variables[2].value;
								textMap.set(toHex(hashVal), sanitizeText(textVal));
							}
						}
						localizedMaps.set(loc, textMap);
					}
				}
			}

			const dialogues: StoryDialogueLine[] = [];

			for (const child of washaBeg.children) {
				if (child.name.startsWith("TEXT_WASHA_MAP_") && child.variables?.length >= 16) {
					const hashVal = parseInt(child.variables[0].value, 10);
					const hashId = toHex(hashVal);
					const speakerIdInt = parseInt(child.variables[2].value, 10);
					const dialogueId = child.variables[15].value;

					const text: { en?: string; fr?: string; ja?: string } = {
						ja: localizedMaps.get("ja")?.get(hashId),
						en: localizedMaps.get("en")?.get(hashId),
						fr: localizedMaps.get("fr")?.get(hashId),
					};

					// Fallback to JA if EN or FR translations are missing
					if (!text.en) text.en = text.ja;
					if (!text.fr) text.fr = text.ja;

					if (dialogueId && (text.ja || text.en || text.fr)) {
						dialogues.push({
							dialogueId,
							hashId,
							speaker: resolveSpeaker(speakerIdInt),
							text,
						});
					}
				}
			}

			if (dialogues.length > 0) {
				events.push({
					eventId,
					dialogues,
				});
			}
		} catch (e) {
			console.error(`[StoryTextParser] Failed to parse event ${eventId}`, e);
		}
	}

	return events;
}

/**
 * Helper to parse a folder of simple localized files
 */
function parseSimpleLocalizedTextFiles(subDir: string): Array<{ id: string; items: Array<{ hashId: string; text: { en?: string; fr?: string; ja?: string } }> }> {
	const locales = ["ja", "en", "fr"] as const;
	const results: Map<string, Map<string, { en?: string; fr?: string; ja?: string }>> = new Map();

	for (const loc of locales) {
		const dirPath = join(DATA_ROOT, "common/text", loc, subDir);
		if (!existsSync(dirPath)) continue;

		try {
			const files = readdirSync(dirPath).filter((f) => f.endsWith(".cfg.bin.json"));
			for (const file of files) {
				const fileId = file.replace(".cfg.bin.json", "");
				const filePath = join(dirPath, file);
				const data = JSON.parse(readFileSync(filePath, "utf-8"));
				const textBeg = data.entries.find((e: any) => e.name.startsWith("TEXT_INFO_BEGIN_") || e.name.startsWith("NOUN_INFO_BEGIN_"));
				if (!textBeg?.children) continue;

				const fileMap = results.get(fileId) || new Map<string, { en?: string; fr?: string; ja?: string }>();

				for (const child of textBeg.children) {
					if ((child.name.startsWith("TEXT_INFO_") || child.name.startsWith("NOUN_INFO_")) && child.variables?.length >= 3) {
						const hashVal = parseInt(child.variables[0].value, 10);
						const hashId = toHex(hashVal);
						
						// In NOUN_INFO, text is at index 5. In TEXT_INFO, index 2
						const textIdx = child.name.startsWith("NOUN_INFO_") ? 5 : 2;
						if (child.variables[textIdx]) {
							const textVal = child.variables[textIdx].value;
							const cleaned = sanitizeText(textVal);
							if (cleaned) {
								const existing = fileMap.get(hashId) || {};
								existing[loc] = cleaned;
								fileMap.set(hashId, existing);
							}
						}
					}
				}
				results.set(fileId, fileMap);
			}
		} catch (e) {
			console.error(`[StoryTextParser] Failed to parse ساده files under ${subDir}/${loc}`, e);
		}
	}

	return Array.from(results.entries()).map(([id, fileMap]) => {
		const items = Array.from(fileMap.entries()).map(([hashId, text]) => {
			if (!text.en) text.en = text.ja;
			if (!text.fr) text.fr = text.ja;
			return { hashId, text };
		});
		return { id, items };
	});
}

/**
 * Parse all NPC dialogues by zone
 */
export function parseAllMapNpcDialogues(): MapNpcDialogue[] {
	const parsed = parseSimpleLocalizedTextFiles("map");
	return parsed.map((p) => ({
		zoneId: p.id.replace("_npc_text", ""),
		dialogues: p.items,
	}));
}

/**
 * Parse all story goals / purposes
 */
export function parseAllStoryGoals(): StoryGoal[] {
	const parsed = parseSimpleLocalizedTextFiles("purpose");
	return parsed.map((p) => ({
		chapterId: p.id.replace("_purpose_text", ""),
		goals: p.items,
	}));
}

/**
 * Parse all chapter phases
 */
export function parseAllChapterPhases(): ChapterPhase[] {
	const parsed = parseSimpleLocalizedTextFiles("phase");
	return parsed.map((p) => ({
		chapterId: p.id.replace("_phase_text", ""),
		phases: p.items,
	}));
}

/**
 * Build complete story text database containing dialogues, maps, goals, and phases
 */
export function buildStoryTextDatabase() {
	console.log("[StoryTextParser] Generating event dialogues...");
	const events = parseAllEventDialogues();
	console.log(`  - Events parsed: ${events.length}`);

	console.log("[StoryTextParser] Generating map NPC dialogues...");
	const mapNpc = parseAllMapNpcDialogues();
	console.log(`  - Zones parsed: ${mapNpc.length}`);

	console.log("[StoryTextParser] Generating story goals/purposes...");
	const goals = parseAllStoryGoals();
	console.log(`  - Purpose chapters parsed: ${goals.length}`);

	console.log("[StoryTextParser] Generating chapter phases...");
	const phases = parseAllChapterPhases();
	console.log(`  - Phase chapters parsed: ${phases.length}`);

	return {
		generatedAt: new Date().toISOString(),
		events,
		mapNpc,
		goals,
		phases,
	};
}
