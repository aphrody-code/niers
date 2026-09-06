// Donnée de jeu immuable entre deux dumps : rendue une fois, revalidée à l'heure, et les
// identifiants inconnus au build restent servis à la demande (`dynamicParams`). Sans
// `force-static`, le rendu repartait en dynamique dès qu'une dépendance lisait la requête.
export const revalidate = 3600;
export const dynamic = "force-static";
export const dynamicParams = true;

import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { CharacterSheetWithLevel } from "@/components/wiki/CharacterSheetWithLevel";
import { HowToObtain } from "@/components/wiki/HowToObtain";
import type { MovesetSkill } from "@/components/wiki/MovesetList";
import { AuraCard } from "@/components/wiki/AuraCard";
import { formatDescription, formatJapaneseName } from "@rosegriffon/azalee/text/format-description";
import { resolveCharaStats } from "@/lib/wiki/chara-stats";
import { wikiService } from "@/lib/wiki-service";
import { compareVariants } from "@rosegriffon/inagle";
import { CharaAssetsSection } from "@/components/wiki/CharaAssetsSection";
import { AphrodyDossierSection } from "@/components/wiki/AphrodyDossierSection";
import { CharaVariantsComparison } from "@/components/wiki/CharaVariantsComparison";

/**
 * Variante représentative d'un personnage (accès par slug de base) : celle dont la
 * position est la plus fréquente parmi les variantes. Les variantes étant déjà triées
 * par stat décroissante, on renvoie la meilleure de la position dominante. Corrige le
 * cas où une variante atypique (ex « Mark Evans » Milieu isolé parmi des Gardien)
 * devenait le défaut affiché à cause d'une stat plus haute.
 */
function pickDominantPositionVariant<T extends { position?: string }>(
	variants: T[]
): T | undefined {
	if (variants.length === 0) return undefined;
	const counts = new Map<string, number>();
	for (const v of variants) {
		const p = v.position || "";
		counts.set(p, (counts.get(p) ?? 0) + 1);
	}
	let dominant = "";
	let max = -1;
	for (const [p, c] of counts) {
		if (c > max) {
			max = c;
			dominant = p;
		}
	}
	return variants.find((v) => (v.position || "") === dominant) ?? variants[0];
}

export async function generateStaticParams() {
	try {
		const allParams = await wikiService.getAllBaseSlugParams();
		// Optimisation build speed : limiter à 50 personnages statiques, le reste en ISR dynamique
		return allParams.slice(0, 50);
	} catch (err) {
		console.error("Error generating static params for characters:", err);
		return [];
	}
}

export async function generateMetadata({
	params,
}: {
	params: Promise<{ id: string }>;
}): Promise<Metadata> {
	const { id } = await params;
	const baseChar =
		(await wikiService.getCharacterByBaseSlug(id)) ||
		(await wikiService.getCharacterBySlug(id)) ||
		(await wikiService.getCharacter(id));

	if (!baseChar) {
		return { title: "Personnage introuvable" };
	}

	const name = baseChar.names.fr || baseChar.names.en || "Personnage";
	const variant = pickDominantPositionVariant(baseChar.variants ?? []);
	const posMap: Record<string, string> = {
		DF: "Défenseur",
		FW: "Attaquant",
		GK: "Gardien",
		MF: "Milieu",
	};
	const elemMap: Record<string, string> = {
		Fire: "Feu",
		Forest: "Forêt",
		Mountain: "Montagne",
		Wind: "Vent",
	};
	const pos = variant?.position ? posMap[variant.position] || variant.position : "";
	const elem = variant?.element ? elemMap[variant.element] || variant.element : "";
	const team = (baseChar as any).teams?.[0]?.names?.fr || "";
	const rarity = variant?.rarity || baseChar.bestRarity || "";

	const parts = [name];
	if (pos) {
		parts.push(pos);
	}
	if (elem) {
		parts.push(elem);
	}
	if (team) {
		parts.push(team);
	}
	if (rarity) {
		parts.push(rarity);
	}

	const charDesc = baseChar.descriptions?.fr || baseChar.descriptions?.en || "";
	const shortDesc = charDesc
		? `${parts.join(" - ")}. ${charDesc.slice(0, 120)}${charDesc.length > 120 ? "..." : ""}`
		: `${parts.join(" - ")}. Stats, techniques et détails dans Inazuma Eleven: Victory Road.`;

	const ogImageUrl = new URL("/api/og/character", "https://azalee.rosegriffon.fr");
	ogImageUrl.searchParams.set("id", id);
	ogImageUrl.searchParams.set("name", name);
	ogImageUrl.searchParams.set("element", variant?.element || "neutral");
	ogImageUrl.searchParams.set("rarity", rarity);
	if (baseChar.image) {
		ogImageUrl.searchParams.set("image", baseChar.image);
	}

	return {
		alternates: {
			canonical: `/chara/${id}`,
		},
		description: shortDesc,
		openGraph: {
			description: shortDesc,
			images: [
				{
					url: ogImageUrl.toString(),
					width: 1200,
					height: 630,
					alt: `${name} - Fiche personnage Azalée`,
				},
			],
			locale: "fr_FR",
			siteName: "Azalée - Inazuma Eleven Victory Road",
			title: `${name} | Azalée`,
			type: "article",
			url: `/chara/${id}`,
		},
		title: `${name} | Inazuma Eleven Victory Road - Azalée`,
		twitter: {
			card: "summary_large_image",
			description: shortDesc,
			images: [ogImageUrl.toString()],
			title: `${name} | Azalée`,
		},
	};
}

export default async function PlayerDetailPage({ params }: { params: Promise<{ id: string }> }) {
	const { id } = await params;
	// 1. base_slug (name-c0xxxxx) — canonical static URL
	let baseChar = await wikiService.getCharacterByBaseSlug(id);
	// 2. variant slug fallback (legacy: gamma-0xABCD)
	if (!baseChar) {
		baseChar = await wikiService.getCharacterBySlug(id);
	}
	// 3. raw ID fallback
	if (!baseChar) {
		baseChar = await wikiService.getCharacter(id);
	}

	if (!baseChar) {
		notFound();
	}

	// Variante spécifique si l'URL cible un id/slug de variante ; sinon (accès par slug de
	// base) on prend la variante de position DOMINANTE — évite qu'une variante atypique
	// (ex un « Mark Evans » Milieu isolé parmi 15 Gardien) devienne le défaut affiché.
	const variant =
		baseChar.variants.find((v) => v.charaParamId === id || v.slug === id) ||
		pickDominantPositionVariant(baseChar.variants);

	if (!variant) {
		notFound();
	}

	// Merge Base and Variant data
	const player = {
		...variant,
		constellation: baseChar.constellation || variant.constellation,
		descriptions: baseChar.descriptions,
		internalCode: baseChar.internalCode,
		names: baseChar.names,
		teamId: (baseChar as any).teams?.[0]?.id as string | undefined,
		teamName: (baseChar as any).teams?.[0]?.names?.fr || (baseChar as any).teams?.[0]?.names?.en,
		zukanHash: baseChar.zukanHash || variant.zukanHash,
		// Portées par le personnage de base, pas par la variante : sans ce report,
		// la section encyclopédique et le numéro de maillot restaient invisibles.
		uniformNumber: (baseChar as any).uniformNumber,
		wikiSections: (baseChar as any).wikiSections,
	};

	// Fetch all distinct forms of this character (same baseSlug/chara_id, different pos/elem/rarity)
	const sheetId = (player as any).sheetData?.sheetId;
	const forms = await wikiService.getCharacterForms(
		baseChar.charaId,
		sheetId,
		baseChar.baseSlug || undefined
	);
	const auras = await wikiService.getCharacterAuras(baseChar.charaId, variant.charaParamId);

	// Build a map of skillId -> skillName for variant comparisons
	const variantSkillIds = new Set<string>();
	for (const v of baseChar.variants) {
		for (const sk of v.skills || []) {
			if (sk.skillId) variantSkillIds.add(sk.skillId);
		}
	}
	// Résolution groupée (1-2 requêtes) au lieu d'un `getSkill` par technique
	// distincte — jusqu'à plusieurs dizaines par personnage entre toutes ses
	// variantes.
	const variantSkillsById = await wikiService.getSkillsByIds(Array.from(variantSkillIds));
	const skillsMap = new Map<string, string>();
	for (const skillId of variantSkillIds) {
		const sk = variantSkillsById.get(skillId);
		skillsMap.set(skillId, sk?.displayName || sk?.name_FR || sk?.name_EN || skillId);
	}

	// Calculate comparisons against the youngest base variant
	const baseVariant = baseChar.variants[0];
	const comparisons = baseChar.variants.map((v) => {
		return compareVariants(baseVariant as any, v as any, skillsMap);
	});

	// Map position to UI expected format
	const positionMap: Record<string, string> = {
		Coach: "COA",
		DF: "DEF",
		FW: "ATT",
		GK: "GAR",
		MF: "MIL",
	};
	const displayPosition = positionMap[player.position] || player.position;

	// Stats brutes du Google Sheet (pas de calcul d'interpolation)
	const stats = player.stats.lv99;

	// Stats RÉELLES décodées LIVE depuis la gamedata (table de croissance cfg.bin
	// via CDN /cfg). La DB ne porte que Lv1/Lv99 dans des colonnes scalaires ; les
	// paliers Lv30/Lv50 (et le total) sont résolus ici par
	// (position × growthPattern × rang de rareté). Repli silencieux si CDN KO.
	const growthStats = await resolveCharaStats(
		(variant as any).charaParamId || id,
		player.position,
		player.rarity
	);

	// --- Detect BASARA & Hero ---
	const _isBasara = player.rarity === "BASARA" || (variant as any).rarityCode >= 20;
	const _isHero = player.rarity === "Héros";

	// --- Hero variant forms (Fire / Black / Pink) ---
	const currentFormEntry = forms?.find((f) => f.id === ((variant as any).charaParamId || id));
	const currentHeroType = currentFormEntry?.heroType;
	const pos = currentFormEntry?.position || player.position;
	const elem = currentFormEntry?.element || player.element;
	const heroForms =
		forms
			?.filter(
				(f) => f.rarity === "Héros" && f.heroType && f.position === pos && f.element === elem
			)
			.map((f) => ({ slug: f.slug, type: f.heroType! }))
			.toSorted((a, b) => {
				const order = ["fire", "black", "pink"];
				return order.indexOf(a.type) - order.indexOf(b.type);
			}) || [];

	// --- Helper: resolve skill names from a string (newline or comma separated) ---
	async function resolveSkillNames(raw: string): Promise<MovesetSkill[]> {
		const separator = raw.includes("\n") ? "\n" : ",";
		const moveNames = raw
			.split(separator)
			.map((s) => s.trim())
			.filter((s) => s && !s.startsWith("#N/A") && !s.startsWith("Mix "));

		// Résolution groupée : un moveset compte rarement plus d'une poignée de
		// noms distincts (souvent répétés pour les paliers d'évolution) — 1-2
		// requêtes au lieu d'une par nom, y compris les doublons.
		const skillsByName = await wikiService.getSkillsByIds(moveNames);
		const resolvedSkills = moveNames.map((name) => skillsByName.get(name));

		// Parallelize any aura searches for missing skills
		const auraPromises = resolvedSkills.map((skill, index) => {
			if (skill) return Promise.resolve(null);
			return wikiService.findAuraByName(moveNames[index]);
		});
		const resolvedAuras = await Promise.all(auraPromises);

		const result: MovesetSkill[] = [];
		for (let index = 0; index < resolvedSkills.length; index++) {
			const skill = resolvedSkills[index];
			const isPassive = skill?.categoryName?.fr?.toLowerCase() === "talent";
			if (skill) {
				result.push({
					category: skill.categoryName?.fr || "Spécial",
					element: skill.elementName?.en?.toLowerCase(),
					growthType: (skill as any).growthType,
					id: (skill as any).skillId || skill.skillID || skill.skillIDStr,
					imageUrl: skill.image,
					isPassive,
					name: skill.displayName || skill.name_FR || skill.name_EN || "???",
					power: skill.power_min ? `${skill.power_min}` : undefined,
					slotNumber: index + 1,
					tension: (skill as any).consumeTp || (skill as any).tension_cost,
					videoUrl: (skill as any).videoUrl,
				});
			} else {
				const aura = resolvedAuras[index];
				if (aura) {
					result.push({
						category: "Hyper Technique",
						href: `/aura/${aura.categorySlug || "autres"}/${aura.id}`,
						id: aura.id,
						isPassive: false,
						name: aura.name_fr || aura.name_en || moveNames[index],
						slotNumber: index + 1,
					});
				} else {
					result.push({
						category: "Hyper Technique",
						id: `unresolved-${index}`,
						name: moveNames[index],
						slotNumber: index + 1,
					});
				}
			}
		}
		return result;
	}

	// --- Helper: déduplique les skills et affiche le niveau d'évolution final ---
	// Si "Tornade de Feu" apparaît 3x → une seule ligne "Tornade de Feu 3"
	// 3 types : nombres (2,3), V (V2,V3), N (N2,N3)
	// GrowthType 1,2 → nombres | growthType 3 → N | growthType 7 → V
	function collapseEvolutions(skills: MovesetSkill[]): MovesetSkill[] {
		const countMap = new Map<string, number>();
		const lastMap = new Map<string, MovesetSkill>();
		for (const skill of skills) {
			const count = (countMap.get(skill.name) || 0) + 1;
			countMap.set(skill.name, count);
			lastMap.set(skill.name, skill);
		}
		const seen = new Set<string>();
		const result: MovesetSkill[] = [];
		let slot = 1;
		for (const skill of skills) {
			if (seen.has(skill.name)) {
				continue;
			}
			seen.add(skill.name);
			const total = countMap.get(skill.name) || 1;
			const entry = { ...lastMap.get(skill.name)!, slotNumber: slot++ };
			if (total > 1) {
				entry.evolutionLevel = total;
				const gt = entry.growthType ?? 0;
				if (gt === 7) {
					entry.evolutionSuffix = `V${total}`;
				} else if (gt === 3) {
					entry.evolutionSuffix = `N${total}`;
				} else {
					entry.evolutionSuffix = `${total}`;
				}
			}
			result.push(entry);
		}
		return result;
	}

	// --- Fetch Skills & Passives ---
	// Approche unifiée : sheetData.moveset/altMoveset pour TOUS (BASARA, Héros, normaux)
	const moveset: MovesetSkill[] = [];
	const altMoveset: MovesetSkill[] = [];

	const movesetRaw = ((player as any).sheetData?.moveset ||
		(baseChar as any).sheetData?.moveset) as string | undefined;
	const altMovesetRaw = ((player as any).sheetData?.altMoveset ||
		(baseChar as any).sheetData?.altMoveset) as string | undefined;

	if (movesetRaw) {
		const resolved = await resolveSkillNames(movesetRaw);
		moveset.push(...collapseEvolutions(resolved));
	}
	if (altMovesetRaw) {
		const resolved = await resolveSkillNames(altMovesetRaw);
		altMoveset.push(...collapseEvolutions(resolved));
	}

	// Fallback: resolve binary skill IDs from chara_param
	// NOTE: mapDbCharacterToBase corrige déjà le mapping inversé du parser :
	//   SkillId = hash hex du skill (ex: "0xF9B80F86")
	//   LearnLevel = niveau d'apprentissage réel (ex: 1, 13, 20...)
	if (moveset.length === 0) {
		const vAny = variant as any;
		const rawSkills: Array<{ learnLevel: number; skillId: string }> | string[] =
			vAny.skills || vAny.moves || [];

		// Filter out phantom skill IDs (aura-dependent special slot)
		const PHANTOM_IDS = new Set(["0xDBEDB6B8"]);
		const skillList = rawSkills
			.map((s) => {
				if (typeof s === "string") {
					return { skillId: s, learnLevel: 0 };
				}
				return s;
			})
			.filter((s) => !PHANTOM_IDS.has(s.skillId));

		// Résolution groupée : c'est ce chemin (moveset binaire de secours) qui
		// émettait une requête `inagle_skills` par technique — jusqu'à ~600 sur
		// une seule fiche en repli Postgres (build Vercel). 1-2 requêtes ici.
		const skillsById = await wikiService.getSkillsByIds(skillList.map((s) => s.skillId));
		const fetchedSkills = skillList.map((s) => skillsById.get(s.skillId));

		const unsorted: MovesetSkill[] = [];
		fetchedSkills.forEach((skill, index) => {
			if (!skill) {
				return;
			}
			const learnLevel = skillList[index]?.learnLevel || 0;
			const isPassive =
				skill.categoryName?.en?.toLowerCase() === "passive" ||
				skill.categoryName?.fr?.toLowerCase() === "talent" ||
				(skill as any).skillType === "passive";

			unsorted.push({
				category: skill.categoryName?.fr || "Spécial",
				element: skill.elementName?.en?.toLowerCase(),
				id: (skill as any).skillId || skill.skillID || skill.skillIDStr,
				imageUrl: skill.image,
				isPassive,
				learnLevel,
				name: skill.displayName || skill.name_FR || skill.name_EN || "???",
				power: skill.power_min ? `${skill.power_min}` : undefined,
				slotNumber: 0,
				tension: (skill as any).consumeTp || (skill as any).tension_cost,
				videoUrl: (skill as any).videoUrl,
			});
		});

		// Trier par niveau d'apprentissage (ordre du zukan)
		unsorted.sort((a, b) => (a.learnLevel || 0) - (b.learnLevel || 0));
		unsorted.forEach((s, i) => {
			s.slotNumber = i + 1;
		});
		moveset.push(...unsorted);

		// Fallback alt moveset from firstMoves
		const firstMovesRaw = (player as any).sheetData?.firstMoves as string | undefined;
		if (firstMovesRaw) {
			const resolved = await resolveSkillNames(firstMovesRaw);
			altMoveset.push(...resolved);
		}

		// Last resort: derive alt moveset from first 3 non-passive binary skills
		if (altMoveset.length === 0 && moveset.length > 0) {
			const nonPassive = moveset.filter((s) => !s.isPassive);
			const first3 = nonPassive.slice(0, 3);
			for (let i = 0; i < first3.length; i++) {
				altMoveset.push({ ...first3[i], slotNumber: i + 1 });
			}
		}
	}

	// If moveset has less than 6 skills, and we have an aura, append the first aura as the 6th moveset slot
	if (moveset.length < 6 && auras && auras.length > 0) {
		const aura = auras[0];
		moveset.push({
			category: "Hyper Technique",
			href: `/aura/${aura.categorySlug || "autres"}/${aura.id}`,
			id: aura.id,
			isPassive: false,
			name: aura.name || "Aura",
			slotNumber: moveset.length + 1,
			element: aura.element?.en?.toLowerCase(),
			imageUrl: aura.imageUrl || undefined,
		});
	}

	// If altMoveset has less than 6 skills, and we have an aura, append the first aura as the 6th altMoveset slot
	if (altMoveset.length < 6 && auras && auras.length > 0) {
		const aura = auras[0];
		altMoveset.push({
			category: "Hyper Technique",
			href: `/aura/${aura.categorySlug || "autres"}/${aura.id}`,
			id: aura.id,
			isPassive: false,
			name: aura.name || "Aura",
			slotNumber: altMoveset.length + 1,
			element: aura.element?.en?.toLowerCase(),
			imageUrl: aura.imageUrl || undefined,
		});
	}

	// IEVR : toujours 6 techniques par moveset
	const moveLimit = 6;
	const limitedMoveset = moveset.slice(0, moveLimit);
	const limitedAltMoveset = altMoveset.slice(0, moveLimit);

	const charName = player.names.fr || player.names.en || player.names.ja || "Personnage";

	const POSITION_FR: Record<string, string> = {
		Coach: "Entraîneur",
		DF: "Défenseur",
		FW: "Attaquant",
		GK: "Gardien",
		MF: "Milieu de terrain",
	};
	const ELEMENT_FR: Record<string, string> = {
		Fire: "Feu",
		Forest: "Forêt",
		Mountain: "Montagne",
		Wind: "Vent",
	};

	const skillNames = limitedMoveset.filter((s) => !s.isPassive).map((s) => s.name);

	const jsonLd = {
		"@context": "https://schema.org",
		"@type": "Person",
		additionalType: "https://schema.org/FictionalCharacter",
		alternateName: [player.names.en, player.names.ja].filter(Boolean),
		description:
			player.descriptions?.fr ||
			player.descriptions?.en ||
			`${charName} est un joueur ${POSITION_FR[player.position] || player.position} de type ${ELEMENT_FR[player.element] || player.element} dans Inazuma Eleven: Victory Road.`,
		genre: (baseChar as any).series || undefined,
		image: baseChar.image || undefined,
		isPartOf: {
			"@type": "VideoGame",
			name: "Inazuma Eleven: Victory Road",
			publisher: { "@type": "Organization", name: "Level-5" },
		},
		jobTitle: POSITION_FR[player.position] || player.position,
		knowsAbout: skillNames.length > 0 ? skillNames : undefined,
		memberOf: player.teamName ? { "@type": "SportsTeam", name: player.teamName } : undefined,
		name: charName,
		url: `https://azalee.rosegriffon.fr/chara/${baseChar.slug || id}`,
	};

	const breadcrumbJsonLd = {
		"@context": "https://schema.org",
		"@type": "BreadcrumbList",
		itemListElement: [
			{ "@type": "ListItem", item: "https://azalee.rosegriffon.fr", name: "Accueil", position: 1 },
			{
				"@type": "ListItem",
				item: "https://azalee.rosegriffon.fr/chara",
				name: "Joueurs",
				position: 2,
			},
			{ "@type": "ListItem", name: charName, position: 3 },
		],
	};

	return (
		<div className="w-full space-y-4 sm:space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
			<script
				type="application/ld+json"
				dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
			/>
			<script
				type="application/ld+json"
				dangerouslySetInnerHTML={{ __html: JSON.stringify(breadcrumbJsonLd) }}
			/>

			{/* Breadcrumb + Compare link */}
			<div className="flex items-center justify-between">
				<nav
					aria-label="Fil d'Ariane"
					className="flex items-center gap-2 text-sm text-on-surface-variant"
				>
					<Link href="/chara" className="hover:underline">
						Joueurs
					</Link>
					<span aria-hidden="true">/</span>
					<span className="text-on-surface" aria-current="page">
						{charName}
					</span>
				</nav>
			</div>

			{/* Character Sheet - Game-style UI with level toggle */}
			<CharacterSheetWithLevel
				name={player.names.fr || player.names.en || "Nom inconnu"}
				names={player.names}
				nickname={formatJapaneseName(player.names.ja) || undefined}
				description={formatDescription(player.descriptions?.fr || player.descriptions?.en, "fr")}
				descriptions={player.descriptions}
				position={displayPosition as "ATT" | "MIL" | "DEF" | "GAR"}
				jerseyNumber={(player as any).uniformNumber ?? 10}
				rarity={player.rarity}
				element={player.element}
				series={(baseChar as any).series}
				gender={
					(baseChar as any).gender === 1 ? "F" : (baseChar as any).gender === 2 ? "X" : "M"
				}
				stats={{
					agility: stats.agility,
					control: stats.control,
					intelligence: stats.intelligence || 0,
					kick: stats.kick,
					physical: stats.physical,
					pressure: stats.pressure,
					technique: stats.technique,
				}}
				skills={limitedMoveset}
				altSkills={_isBasara && limitedAltMoveset.length > 0 ? limitedAltMoveset : undefined}
				avatarUrl={(player as any).image || player.internalCode}
				zukanHash={player.zukanHash}
				teamName={player.teamName}
				teamId={player.teamId}
				prevCharacterId={undefined}
				nextCharacterId={undefined}
				sheetData={(player as any).sheetData}
				constellation={player.constellation}
				heroConstellations={(baseChar as any).heroConstellations}
				forms={forms}
				currentFormId={(variant as any).charaParamId || id}
				slug={(variant as any).slug || baseChar.slug || id}
				basaraFormSlug={
					!_isBasara && forms?.find((f) => f.rarity === "BASARA")
						? forms.find((f) => f.rarity === "BASARA")!.slug
						: undefined
				}
				normalFormSlug={
					_isBasara && forms?.find((f) => f.rarity !== "BASARA" && f.rarity !== "Héros")
						? forms.find((f) => f.rarity !== "BASARA" && f.rarity !== "Héros")!.slug
						: undefined
				}
				heroForms={heroForms.length > 1 ? heroForms : undefined}
				currentHeroType={currentHeroType}
				isControllable={(player as any).isControllable || (baseChar as any).isControllable}
				statsLv1={
					player.stats.lv1
						? {
								agility: player.stats.lv1.agility,
								control: player.stats.lv1.control,
								intelligence: player.stats.lv1.intelligence || 0,
								kick: player.stats.lv1.kick,
								physical: player.stats.lv1.physical,
								pressure: player.stats.lv1.pressure,
								technique: player.stats.lv1.technique,
							}
						: undefined
				}
				statsLv30={
					// Priorité aux stats RÉELLES de la table de croissance (gamedata) ;
					// repli sur le JSONB historique (souvent vide) puis undefined.
					growthStats
						? growthStats.lv30
						: player.stats.lv30 && player.stats.lv30.kick
							? {
									agility: player.stats.lv30.agility,
									control: player.stats.lv30.control,
									intelligence: player.stats.lv30.intelligence || 0,
									kick: player.stats.lv30.kick,
									physical: player.stats.lv30.physical,
									pressure: player.stats.lv30.pressure,
									technique: player.stats.lv30.technique,
								}
							: undefined
				}
				statsLv50={
					growthStats
						? growthStats.lv50
						: player.stats.lv50 && player.stats.lv50.kick
							? {
									agility: player.stats.lv50.agility,
									control: player.stats.lv50.control,
									intelligence: player.stats.lv50.intelligence || 0,
									kick: player.stats.lv50.kick,
									physical: player.stats.lv50.physical,
									pressure: player.stats.lv50.pressure,
									technique: player.stats.lv50.technique,
								}
							: undefined
				}
			/>

			{player.internalCode && (
				<div className="mt-6">
					<CharaAssetsSection internalCode={player.internalCode} displayName={charName} />
				</div>
			)}

			{player.internalCode && (
				<div className="mt-6">
					<AphrodyDossierSection internalCode={player.internalCode} />
				</div>
			)}

			<CharaVariantsComparison comparisons={comparisons} baseName={charName} />

			{/* Info Panel — Series, Apparitions */}
			<div className="bg-surface-container-low rounded-[20px] sm:rounded-[24px] border border-outline-variant/30 px-4 py-3 sm:px-6 sm:py-5 space-y-3 sm:space-y-5">
				{/* Series + Control Type */}
				<div className="flex flex-wrap items-center gap-2 sm:gap-3">
					{(baseChar as any).series && (
						<span className="inline-flex items-center px-2.5 py-1 sm:px-3 sm:py-1.5 rounded-full text-[10px] sm:text-xs font-bold bg-secondary-container text-on-secondary-container">
							{(baseChar as any).series}
						</span>
					)}
					{(baseChar as any).controlType && (
						<span className="inline-flex items-center px-2.5 py-1 sm:px-3 sm:py-1.5 rounded-full text-[10px] sm:text-xs font-medium bg-tertiary-container text-on-tertiary-container">
							{(baseChar as any).controlType}
						</span>
					)}
					{(baseChar as any).isControllable && (
						<span className="inline-flex items-center px-2.5 py-1 sm:px-3 sm:py-1.5 rounded-full text-[10px] sm:text-xs font-medium bg-primary/10 text-primary border border-primary/20">
							Jouable
						</span>
					)}
				</div>

				{/* Game Appearances */}
				{(baseChar as any).gameAppearances && (baseChar as any).gameAppearances.length > 0 && (
					<div>
						<h3 className="text-[10px] sm:text-xs font-bold text-on-surface-variant uppercase tracking-wider mb-1.5 sm:mb-2">
							Apparitions
						</h3>
						<div className="flex flex-wrap gap-1 sm:gap-1.5">
							{(baseChar as any).gameAppearances.map((g: string) => {
								const labels: Record<string, string> = {
									ars: "Ares",
									go1: "GO",
									go2: "Chrono Stone",
									go3: "Galaxy",
									ie1: "IE 1",
									ie2: "IE 2",
									ie3: "IE 3",
									ori: "Orion",
									vic: "Victory Road",
								};
								return (
									<span
										key={g}
										className="inline-flex items-center px-2 py-0.5 sm:px-2.5 rounded-full text-[10px] sm:text-[11px] font-bold bg-surface-container-high text-on-surface-variant"
									>
										{labels[g] || g}
									</span>
								);
							})}
						</div>
					</div>
				)}
			</div>

			{/* Wiki Sections (History, Recruitment, etc.) */}
			{(player as any).wikiSections && (player as any).wikiSections.length > 0 && (
				<div className="bg-surface-container-low rounded-[20px] sm:rounded-[24px] border border-outline-variant/30 px-4 py-5 sm:px-8 sm:py-8 space-y-6 sm:space-y-10">
					{(player as any).wikiSections.map((section: any, i: number) => (
						<div key={i} className="space-y-2 sm:space-y-4">
							<h3 className="text-lg sm:text-2xl font-bold font-grade-high text-on-surface border-b-2 border-primary/20 pb-1.5 sm:pb-2 inline-block">
								{section.title}
							</h3>
							<div className="prose prose-sm sm:prose-lg dark:prose-invert max-w-none text-on-surface-variant leading-relaxed whitespace-pre-wrap font-medium">
								{section.content}
							</div>
						</div>
					))}
				</div>
			)}

			{/* Auras & Esprits Guerriers Section */}
			{auras && auras.length > 0 && (
				<div className="bg-surface-container-low rounded-[20px] sm:rounded-[24px] border border-outline-variant/30 px-4 py-5 sm:px-8 sm:py-8 space-y-4">
					<h3 className="text-lg sm:text-2xl font-bold font-grade-high text-on-surface border-b-2 border-primary/20 pb-1.5 sm:pb-2 inline-block">
						Auras & Invocations associées
					</h3>
					<p className="text-sm text-on-surface-variant/80 font-medium">
						Ces Esprits Guerriers, Totems ou Miximax sont spécifiquement liés à {charName} dans les
						données du jeu.
					</p>
					<div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-4 pt-2">
						{auras.map((aura) => (
							<AuraCard
								key={aura.id}
								id={aura.id}
								name={aura.name}
								category={aura.categorySlug}
								subType={aura.subType}
								element={aura.element}
								assetCode={aura.assetCode || undefined}
								image={aura.imageUrl || undefined}
								passiveEffect={aura.passiveEffect}
								hissatsuName={aura.hissatsuName}
							/>
						))}
					</div>
				</div>
			)}

			{/* How to Obtain Section */}
			<HowToObtain
				constellation={player.constellation}
				heroConstellations={(baseChar as any).heroConstellations}
				spiritExchange={(baseChar as any).sheetData?.spiritExchange}
			/>
		</div>
	);
}
