/**
 * Rendu des fiches détaillées du CLI (personnage, technique, objet, équipe).
 *
 * Couche **présentation pure** : chaque fonction reçoit une entité inagle et
 * renvoie un bloc de texte encadré, prêt à être écrit sur stdout. Aucune
 * décision métier ici — l'interpolation des stats, les libellés de
 * personnalité et de catégorie d'objet viennent de `@rosegriffon/azalee/game`.
 *
 * Largeur de référence : 78 colonnes utiles dans un cadre de 80.
 */

import { buildAuraHashSet } from "@rosegriffon/inagle/skills/mapper-aura";

import { PERSONALITY_NAMES } from "@rosegriffon/azalee/game/personality";
import { getItemCategoryLabel } from "@rosegriffon/azalee/game/item-categories";
import { interpolateVariantStats } from "@rosegriffon/azalee/game/stats-interpolation";
import { colors } from "./context";
import { getAuraMetadataFromDb, getAurasForChara, getSkillName, type InagleService, type ResolvedAura } from "./inagle";

/**
 * Longueur d'affichage d'une chaîne dans un terminal : les séquences ANSI ne
 * comptent pas, les idéogrammes pleine chasse comptent double.
 */
export function getVisibleLength(str: string): number {
	// Retire les codes d'échappement ANSI.
	const stripped = str.replace(/\x1b\[[0-9;]*m/g, "");
	// Les caractères pleine chasse (CJK) occupent 2 cellules.
	let len = 0;
	for (let i = 0; i < stripped.length; i++) {
		const code = stripped.charCodeAt(i);
		if (
			(code >= 0x3000 && code <= 0x9fff) ||
			(code >= 0xf900 && code <= 0xfaff) ||
			(code >= 0xff00 && code <= 0xffef)
		) {
			len += 2;
		} else {
			len += 1;
		}
	}
	return len;
}

/** Ligne de cadre à une seule colonne (76 cellules utiles). */
export function renderOneColumn(text: string): string {
	const visible = getVisibleLength(text);
	const padding = " ".repeat(Math.max(0, 76 - visible));
	return `│ ${text}${padding} │`;
}

/** Ligne de cadre à deux colonnes (39 + 34 cellules par défaut). */
export function renderTwoColumns(left: string, right: string, colWidthLeft = 39, colWidthRight = 34): string {
	const visibleLeft = getVisibleLength(left);
	const visibleRight = getVisibleLength(right);

	const padLeft = " ".repeat(Math.max(0, colWidthLeft - visibleLeft));
	const padRight = " ".repeat(Math.max(0, colWidthRight - visibleRight));

	return `│ ${left}${padLeft} │ ${right}${padRight} │`;
}

/**
 * Fiche complète d'un personnage : identité, affiliation, description,
 * puis une section par variante (courbe de stats lv1/50/99, techniques, auras).
 */
export function renderCharaProfile(chara: any, svc: InagleService): string {
	const lines: string[] = [];
	const nameFr = chara.names?.fr || "N/A";
	const nameEn = chara.names?.en || "N/A";
	const nameJa = chara.names?.ja || "N/A";
	const roma = chara.romanized?.full || "N/A";
	const genderStr = chara.gender === 1 ? "Féminin" : "Masculin";
	const controllableStr = chara.isControllable ? "Oui" : "Non";

	const personality =
		chara.personalityType !== undefined
			? PERSONALITY_NAMES[chara.personalityType] || `Type ${chara.personalityType}`
			: "Inconnu";

	const firstPerson = chara.firstPerson?.ja || chara.firstPerson?.fr || "N/A";
	const secondPerson = chara.secondPersonMale?.ja || chara.secondPersonMale?.fr || "N/A";

	lines.push(`┌──────────────────────────────────────────────────────────────────────────────┐`);
	lines.push(
		renderTwoColumns(
			`${colors.bold}${colors.green}${nameFr}${colors.reset}`,
			`${colors.cyan}ID: ${chara.charaId}${colors.reset}`,
		),
	);
	lines.push(
		renderTwoColumns(
			`${colors.yellow}Anglais: ${nameEn}${colors.reset}`,
			`${colors.yellow}Japonais: ${nameJa}${colors.reset}`,
		),
	);
	lines.push(
		renderTwoColumns(
			`${colors.yellow}Romanisé: ${roma}${colors.reset}`,
			`${colors.yellow}Genre: ${genderStr}${colors.reset}`,
		),
	);
	lines.push(`├──────────────────────────────────────────────────────────────────────────────┤`);
	lines.push(renderOneColumn(`${colors.bold}Affiliation :${colors.reset} ${chara.teamName || "Aucune"}`));
	lines.push(
		renderTwoColumns(
			`${colors.bold}Contrôlable :${colors.reset} ${controllableStr}`,
			`${colors.bold}Personnalité :${colors.reset} ${personality}`,
		),
	);
	lines.push(
		renderTwoColumns(
			`${colors.bold}Je :${colors.reset} ${firstPerson}`,
			`${colors.bold}Tu (Masc) :${colors.reset} ${secondPerson}`,
		),
	);
	lines.push(`├──────────────────────────────────────────────────────────────────────────────┤`);

	if (chara.descriptions?.fr || chara.descriptions?.en) {
		const desc = chara.descriptions.fr || chara.descriptions.en;
		lines.push(renderOneColumn(`${colors.bold}Description :${colors.reset}`));
		const words = desc.split(" ");
		let currentLine = "  ";
		for (const word of words) {
			if (getVisibleLength(currentLine + word) > 72) {
				lines.push(renderOneColumn(currentLine));
				currentLine = "  " + word + " ";
			} else {
				currentLine += word + " ";
			}
		}
		if (currentLine !== "  ") {
			lines.push(renderOneColumn(currentLine));
		}
		lines.push(`├──────────────────────────────────────────────────────────────────────────────┤`);
	}

	if (chara.variants && chara.variants.length > 0) {
		const varHeader = `Variantes de cartes (${chara.variants.length}) :`;
		const varPadding = " ".repeat(Math.max(0, 76 - varHeader.length));
		lines.push(`│ ${colors.bold}${colors.magenta}${varHeader}${colors.reset}${varPadding} │`);
		for (const [idx, v] of (chara.variants as any[]).entries()) {
			const el = v.element || "N/A";
			const pos = v.position || "N/A";
			const rar = v.rarity || "N/A";

			lines.push(
				renderOneColumn(
					`  [${idx + 1}] Element: ${el.padEnd(10)} | Position: ${pos.padEnd(4)} | Rareté: ${rar}`,
				),
			);

			if (v.stats) {
				const s1 = interpolateVariantStats(v, 1);
				const s50 = interpolateVariantStats(v, 50);
				const s99 = interpolateVariantStats(v, 99);

				const t1 =
					s1.kick + s1.control + s1.physical + s1.pressure + s1.technique + s1.agility + (s1.intelligence || 0);
				const t50 =
					s50.kick +
					s50.control +
					s50.physical +
					s50.pressure +
					s50.technique +
					s50.agility +
					(s50.intelligence || 0);
				const t99 =
					s99.kick +
					s99.control +
					s99.physical +
					s99.pressure +
					s99.technique +
					s99.agility +
					(s99.intelligence || 0);

				const renderRow = (label: string, v1: number, v2: number, v3: number) => {
					const col1 = ` ${label.padEnd(12)} `;
					const col2 = ` ${v1.toString().padStart(7)} `;
					const col3 = ` ${v2.toString().padStart(7)} `;
					const col4 = ` ${v3.toString().padStart(7)} `;
					return `│       │${col1}│${col2}│${col3}│${col4}│                         │`;
				};

				const renderTotalRow = () => {
					const col1 = ` ${colors.bold}${"TOTAL".padEnd(12)}${colors.reset} `;
					const col2 = ` ${colors.green}${t1.toString().padStart(7)}${colors.reset} `;
					const col3 = ` ${colors.green}${t50.toString().padStart(7)}${colors.reset} `;
					const col4 = ` ${colors.green}${t99.toString().padStart(7)}${colors.reset} `;
					return `│       │${col1}│${col2}│${col3}│${col4}│                         │`;
				};

				lines.push(
					`│       ${colors.bold}Courbe de progression des statistiques :${colors.reset}${" ".repeat(30)} │`,
				);
				lines.push(`│       ┌──────────────┬─────────┬─────────┬─────────┐                         │`);
				lines.push(`│       │ Stat         │ Niv. 1  │ Niv. 50 │ Niv. 99 │                         │`);
				lines.push(`│       ├──────────────┼─────────┼─────────┼─────────┤                         │`);
				lines.push(renderRow("Frappe", s1.kick, s50.kick, s99.kick));
				lines.push(renderRow("Contrôle", s1.control, s50.control, s99.control));
				lines.push(renderRow("Physique", s1.physical, s50.physical, s99.physical));
				lines.push(renderRow("Pression", s1.pressure, s50.pressure, s99.pressure));
				lines.push(renderRow("Technique", s1.technique, s50.technique, s99.technique));
				lines.push(renderRow("Agilité", s1.agility, s50.agility, s99.agility));
				lines.push(
					renderRow("Intelligence", s1.intelligence || 0, s50.intelligence || 0, s99.intelligence || 0),
				);
				lines.push(`│       ├──────────────┼─────────┼─────────┼─────────┤                         │`);
				lines.push(renderTotalRow());
				lines.push(`│       └──────────────┴─────────┴─────────┴─────────┘                         │`);
			}

			const auraHashes = buildAuraHashSet();
			const trueSkills = (v.skills || []).filter(
				(s: any) => s.skillId !== "0x00000002" && !auraHashes.has(s.skillId),
			);
			const rawAuraIds = (v.skills || [])
				.filter((s: any) => auraHashes.has(s.skillId))
				.map((s: any) => s.skillId);

			const renderColoredLine = (prefix: string, content: string, colorCode: string) => {
				const visibleLength = prefix.length + content.length;
				const padding = " ".repeat(Math.max(0, 70 - visibleLength));
				return `│       ${colorCode}${prefix}${content}${colors.reset}${padding} │`;
			};

			if (trueSkills.length > 0) {
				lines.push(
					renderColoredLine("Techniques : ", `(${trueSkills.length})`, colors.bold + colors.blue),
				);
				for (const sk of trueSkills) {
					const name = getSkillName(svc, sk.skillId);
					const label = ` - ${name} (Niv. ${sk.learnLevel})`;
					lines.push(`│       ${label.padEnd(70)} │`);
				}
			}

			// Auras (Esprit Guerrier, Totem, Miximax) : miroir SQLite + IDs bruts.
			const sqliteAuras = getAurasForChara(chara.charaId, v.charaParamId);
			const allAuras: ResolvedAura[] = [...sqliteAuras];
			for (const auraId of rawAuraIds) {
				if (!allAuras.some((a) => a.id.toLowerCase().includes(auraId.toLowerCase()))) {
					const auraDetails = getAuraMetadataFromDb(auraId);
					if (auraDetails) {
						allAuras.push(auraDetails);
					} else {
						const name = getSkillName(svc, auraId);
						allAuras.push({
							id: auraId,
							name,
							type: "aura",
						});
					}
				}
			}

			if (allAuras.length > 0) {
				lines.push(
					renderColoredLine(
						"Auras / Miximax / Keshin : ",
						`(${allAuras.length})`,
						colors.bold + colors.magenta,
					),
				);
				for (const aura of allAuras) {
					const typeLabel =
						aura.type === "keshin"
							? "Esprit Guerrier"
							: aura.type === "soul"
								? "Totem"
								: aura.type === "miximax"
									? "Miximax"
									: "Aura";
					const label = ` - ${aura.name} [${typeLabel}]`;
					lines.push(`│       ${label.padEnd(70)} │`);
				}
			}

			if (idx < chara.variants.length - 1) {
				lines.push(`│                                                                              │`);
			}
		}
	}

	lines.push(`└──────────────────────────────────────────────────────────────────────────────┘`);
	return lines.join("\n");
}

/** Fiche d'une technique / passive / aura : nom, description, puissance. */
export function renderSkillProfile(skill: any, _svc: InagleService): string {
	const lines: string[] = [];
	const displayName = skill.displayName || skill.name_FR || skill.name_EN || "N/A";
	const typeStr = (skill.skillType || "Technique").toUpperCase();
	const idStr = skill.skillIDStr || skill.auraId || skill.passiveId || "N/A";

	lines.push(`┌──────────────────────────────────────────────────────────────────────────────┐`);
	lines.push(
		`│ [${colors.bold}${colors.green}${typeStr}${colors.reset}] ${colors.bold}${displayName.padEnd(50)}${colors.reset} │ ${colors.cyan}ID: ${idStr.padEnd(10)}${colors.reset} │`,
	);

	if (skill.desc_FR || skill.description_FR || skill.desc_EN) {
		const desc = skill.desc_FR || skill.description_FR || skill.desc_EN;
		lines.push(`├──────────────────────────────────────────────────────────────────────────────┤`);
		lines.push(`│ ${colors.bold}Description :${colors.reset}${" ".repeat(63)} │`);
		const words = desc.split(" ");
		let currentLine = "│   ";
		for (const word of words) {
			if ((currentLine + word).length > 74) {
				lines.push(currentLine.padEnd(79) + "│");
				currentLine = "│   " + word + " ";
			} else {
				currentLine += word + " ";
			}
		}
		if (currentLine !== "│   ") {
			lines.push(currentLine.padEnd(79) + "│");
		}
	}

	if (skill.power_max !== undefined) {
		lines.push(`├──────────────────────────────────────────────────────────────────────────────┤`);
		const power = skill.power_max;
		const cost = skill.cost || 0;
		const element = skill.elementName?.fr || "N/A";
		const category = skill.categoryName?.fr || "N/A";
		lines.push(
			`│ Puissance Max : ${power.toString().padEnd(10)} | Coût : ${cost.toString().padEnd(8)} | Élément : ${element.padEnd(10)} | Cat : ${category.padEnd(8)} │`,
		);
	}
	lines.push(`└──────────────────────────────────────────────────────────────────────────────┘`);
	return lines.join("\n");
}

/** Fiche d'un objet : identité, catégorie, prix, stats, boutiques, description. */
export function renderItemProfile(item: any): string {
	const lines: string[] = [];
	const nameFr = item.names?.fr || item.name || "N/A";
	const nameEn = item.names?.en || "N/A";
	const nameJa = item.names?.ja || "N/A";
	const code = item.internalCode || "N/A";
	const cat = getItemCategoryLabel(item.category) || "N/A";
	const priceStr = item.price ? `${item.price} Ptz` : "N/A";

	lines.push(`┌──────────────────────────────────────────────────────────────────────────────┐`);
	lines.push(
		`│ ${colors.bold}${colors.green}${nameFr.padEnd(40)}${colors.reset} │ ${colors.cyan}ID: ${item.itemId.padEnd(28)}${colors.reset} │`,
	);
	lines.push(
		`│ ${colors.yellow}Anglais: ${nameEn.padEnd(31)}${colors.reset} │ ${colors.yellow}Japonais: ${nameJa.padEnd(29)}${colors.reset} │`,
	);
	lines.push(
		`│ ${colors.yellow}Code: ${code.padEnd(34)}${colors.reset} │ ${colors.yellow}Catégorie: ${cat.padEnd(28)}${colors.reset} │`,
	);
	lines.push(`├──────────────────────────────────────────────────────────────────────────────┤`);
	lines.push(`│ Prix d'achat: ${priceStr.padEnd(64)} │`);

	if (item.stats) {
		lines.push(`│ ${colors.bold}Statistiques octroyées :${colors.reset}${" ".repeat(53)} │`);
		lines.push(
			`│   Stat 1: ${String(item.stats.stat1).padEnd(10)} | Stat 2: ${String(item.stats.stat2).padEnd(44)} │`,
		);
	}

	if (item.shops && (item.shops.fr?.length > 0 || item.shops.en?.length > 0)) {
		const shops = item.shops.fr || item.shops.en;
		lines.push(`│ Disponibilité : ${shops.join(", ").substring(0, 58).padEnd(60)} │`);
	}

	if (item.descriptions?.fr || item.descriptions?.en) {
		const desc = item.descriptions.fr || item.descriptions.en;
		lines.push(`├──────────────────────────────────────────────────────────────────────────────┤`);
		lines.push(`│ ${colors.bold}Description :${colors.reset}${" ".repeat(63)} │`);
		const words = desc.split(" ");
		let currentLine = "│   ";
		for (const word of words) {
			if ((currentLine + word).length > 74) {
				lines.push(currentLine.padEnd(79) + "│");
				currentLine = "│   " + word + " ";
			} else {
				currentLine += word + " ";
			}
		}
		if (currentLine !== "│   ") {
			lines.push(currentLine.padEnd(79) + "│");
		}
	}

	lines.push(`└──────────────────────────────────────────────────────────────────────────────┘`);
	return lines.join("\n");
}

/** Fiche d'une équipe : identité, uniformes (5 max), saisons. */
export function renderTeamProfile(team: any): string {
	const lines: string[] = [];
	const nameFr = team.name_FR || team.displayName || team.name || "N/A";
	const nameEn = team.name_EN || "N/A";
	const nameJa = team.name_JA || "N/A";
	const id = team.teamId || "N/A";
	const code = team.teamIdStr || "N/A";

	lines.push(`┌──────────────────────────────────────────────────────────────────────────────┐`);
	lines.push(
		`│ ${colors.bold}${colors.green}${nameFr.padEnd(40)}${colors.reset} │ ${colors.cyan}ID: ${id.padEnd(28)}${colors.reset} │`,
	);
	lines.push(
		`│ ${colors.yellow}Anglais: ${nameEn.padEnd(31)}${colors.reset} │ ${colors.yellow}Japonais: ${nameJa.padEnd(29)}${colors.reset} │`,
	);
	lines.push(`│ ${colors.yellow}Code: ${code.padEnd(34)}${colors.reset} │${" ".repeat(30)} │`);
	lines.push(`├──────────────────────────────────────────────────────────────────────────────┤`);

	if (team.uniforms && team.uniforms.length > 0) {
		lines.push(`│ ${colors.bold}Kits / Uniformes (${team.uniforms.length}) :${colors.reset}${" ".repeat(53)} │`);
		for (const [idx, u] of (team.uniforms as any[]).slice(0, 5).entries()) {
			const kitId = u.uniformId || "N/A";
			const modelName = u.modelName || "N/A";
			lines.push(`│   [${idx + 1}] ID Uniforme: ${String(kitId).padEnd(10)} | Modèle: ${modelName.padEnd(34)} │`);
		}
		if (team.uniforms.length > 5) {
			lines.push(`│   ... et ${team.uniforms.length - 5} autres kits.                                              │`);
		}
	}

	if (team.seasons && team.seasons.length > 0) {
		lines.push(`│ Saisons / Événements: ${team.seasons.join(", ").substring(0, 56).padEnd(58)} │`);
	}

	lines.push(`└──────────────────────────────────────────────────────────────────────────────┘`);
	return lines.join("\n");
}
