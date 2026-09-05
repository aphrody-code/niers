/**
 * Page flagship `/demo` — DÉMO 3D COMPLÈTE d'un personnage (Aphrodi, c01001900).
 *
 * Server Component PUR : sérialise tout le dossier (`data/aphrody-dossier.json`) en un
 * {@link DemoData} via les helpers CLIENT-SAFE (`cpkAssetUrl`/`cpkAudioUrl`/`cpkThumbUrl`,
 * `getSkillCutin`/`cutinHasAssets`), puis délègue toute l'interactivité 3D à l'îlot
 * `<DemoExperience>` (`"use client"` + `next/dynamic { ssr:false }` pour la scène R3F).
 *
 * Aucune lecture SQLite / `server-only` : le dossier JSON suffit (frontière server/client nette).
 */
import type { Metadata } from "next";
import dossier from "@/data/aphrody-dossier.json";
import { cpkAssetUrl, cpkAudioUrl, cpkThumbUrl } from "@rosegriffon/azalee/cpk/shared";
import { cutinHasAssets, getSkillCutin } from "@rosegriffon/azalee/game/skills-cutin";
import { DemoExperience } from "@/components/wiki/demo/DemoExperience";
import type {
	DemoData,
	DemoDialogue,
	DemoFormData,
	DemoSkill,
	DemoTexture,
} from "@/components/wiki/demo/types";

const CDN = "https://cdn.rosegriffon.fr";

export const metadata: Metadata = {
	alternates: { canonical: "/demo" },
	description:
		"Démo 3D interactive d'Aphrodi (Byron Love Aphrody) dans Inazuma Eleven: Victory Road — modèles complets texturés des 3 ères, cut-ins jouables avec caméra scénarisée et export MP4, voix japonaise, calcul de statistiques byte-exact et galerie de textures téléchargeables.",
	openGraph: {
		description:
			"Toutes les ressources 3D d'un personnage réunies : modèles, cut-ins, voix, stats et textures — moteur niers, byte-exact.",
		images: ["/opengraph-image"],
		locale: "fr_FR",
		siteName: "Azalée - Inazuma Eleven Victory Road",
		title: "Démo 3D — Aphrodi | Azalée",
		type: "website",
		url: "/demo",
	},
	title: "Démo 3D complète — Aphrodi (Byron Love Aphrody) - Azalée",
};

/** Types minimaux du dossier (lecture locale, on ne dépend pas d'un schéma exporté). */
interface DossierAsset {
	code: string;
	face_g4tx?: string;
	icon_g4tx?: string;
	voice_acb?: string;
}
interface DossierSeries {
	code: string;
	label: string;
	face_subdir: string;
}
interface DossierTechInfo {
	skill_id_str: string;
	event_id_name: string;
	element: number;
	category: number;
	power_min: number;
	power_max: number;
	consume_tp: number;
	growth_type: number;
}
interface DossierTechCutin {
	event_id_name: string;
	texture_g4tx?: string;
	telop_by_lang?: string[][];
}
interface DossierVariant {
	code: string;
	is_primary: boolean;
	main_position: number;
	sub_position: number;
	growth_pattern: number;
	techniques?: { info: DossierTechInfo; cutin: DossierTechCutin | null; learn_level: number }[];
}

const d = dossier as unknown as {
	assets: DossierAsset[];
	series: DossierSeries[];
	variants: DossierVariant[];
	identity: Record<string, unknown>;
	profile: Record<string, unknown>;
	stats: { lv1: Record<string, number>; lv50: Record<string, number>; lv99: Record<string, number> };
	references: { title: string; detail: string }[];
	dialogues: { lines?: { fr?: string; ja?: string; mentions?: boolean }[] }[];
};

/** URL du telop FR d'une liste `[langue, chemin]` (repli en/1ère langue), ou `null`. */
function telopFrUrl(pairs: string[][] | undefined): string | null {
	if (!pairs?.length) return null;
	const chosen = pairs.find(([l]) => l === "fr") ?? pairs.find(([l]) => l === "en") ?? pairs[0];
	return chosen ? cpkAssetUrl(chosen[1]) : null;
}

/** Construit le {@link DemoData} sérialisable depuis le dossier (exécuté côté serveur). */
function buildDemoData(): DemoData {
	// --- Formes : 1 entrée par ère (series), enrichie par asset + variante de même code. ---
	const forms: DemoFormData[] = d.series.map((s) => {
		const asset = d.assets.find((a) => a.code === s.code);
		// Variante canonique de cette ère (préférer is_primary, sinon 1ère de même code).
		const variant =
			d.variants.find((v) => v.code === s.code && v.is_primary) ??
			d.variants.find((v) => v.code === s.code) ??
			d.variants[0];
		return {
			code: s.code,
			label: s.label,
			faceSubdir: s.face_subdir,
			isPrimary: variant?.is_primary ?? false,
			modelGlbUrl: `${CDN}/model-full/${s.code}.glb`,
			voiceUrl: cpkAudioUrl(asset?.voice_acb ?? `data/common/sound_asset/ja/${s.code}.acb`),
			iconUrl: asset?.icon_g4tx ? cpkAssetUrl(asset.icon_g4tx) : null,
			faceTextureUrl: asset?.face_g4tx ? cpkAssetUrl(asset.face_g4tx) : null,
			growth: {
				mainPosition: variant?.main_position ?? 3,
				subPosition: variant?.sub_position ?? 0,
				growthPattern: variant?.growth_pattern ?? 0,
			},
		};
	});

	// --- Techniques : flatten des variants[].techniques, dédupliquées par skill_id_str. ---
	const seen = new Set<string>();
	const skills: DemoSkill[] = [];
	const pushSkill = (
		info: DossierTechInfo,
		cutin: DossierTechCutin | null,
		learnLevel: number,
	): void => {
		if (!info?.skill_id_str || seen.has(info.skill_id_str)) return;
		seen.add(info.skill_id_str);
		// Noms localisés via le manifeste skills-cutin (info brut n'a que des codes numériques).
		const entry = getSkillCutin(info.skill_id_str);
		const fallbackName = { fr: "?", en: "?", ja: "?" };
		const event = info.event_id_name;
		skills.push({
			skillIdStr: info.skill_id_str,
			eventIdName: event,
			elementName: entry?.element_name ?? fallbackName,
			categoryName: entry?.category_name ?? fallbackName,
			powerMin: info.power_min,
			powerMax: info.power_max,
			consumeTp: info.consume_tp,
			growthType: info.growth_type ?? entry?.growth_type ?? 0,
			learnLevel,
			telopUrl: telopFrUrl(cutin?.telop_by_lang),
			textureUrl: cutin?.texture_g4tx ? cpkAssetUrl(cutin.texture_g4tx) : null,
			wazaGlbUrl: `${CDN}/model-chr/waza/${event}.glb`,
			has3d: cutinHasAssets(event),
		});
	};
	for (const v of d.variants) {
		for (const t of v.techniques ?? []) pushSkill(t.info, t.cutin, t.learn_level);
	}
	// Lignée vedette explicite (au cas où une ère manquerait dans le dossier).
	for (const code of ["whs00340", "whs00900", "whs02900"]) {
		const entry = getSkillCutin(code);
		if (!entry?.cutin || seen.has(code)) continue;
		pushSkill(
			{
				skill_id_str: entry.skill_id_str,
				event_id_name: entry.event_id_name,
				element: entry.element,
				category: entry.category,
				power_min: entry.power_min,
				power_max: entry.power_max,
				consume_tp: entry.consume_tp,
				growth_type: entry.growth_type,
			},
			{
				event_id_name: entry.cutin.event_id_name,
				texture_g4tx: entry.cutin.texture_g4tx,
				telop_by_lang: entry.cutin.telop_by_lang,
			},
			0,
		);
	}

	// --- Textures : visage + icône par forme ; texture + telop par cut-in 3D. ---
	const textures: DemoTexture[] = [];
	for (const f of forms) {
		if (f.faceTextureUrl) {
			textures.push({
				label: `Visage — ${f.label}`,
				url: f.faceTextureUrl,
				thumbUrl: cpkThumbUrl(`data/dx11/chr/_face/${f.faceSubdir}/${f.code}/${f.code}.g4tx`),
				group: "face",
			});
		}
		if (f.iconUrl) {
			textures.push({
				label: `Portrait — ${f.label}`,
				url: f.iconUrl,
				thumbUrl: cpkThumbUrl(
					`data/dx11/menu/200_icon/10_icon_chr/face/${f.code}_l.g4tx`,
				),
				group: "icon",
			});
		}
	}
	for (const sk of skills) {
		if (!sk.has3d) continue;
		if (sk.textureUrl) {
			textures.push({
				label: `Cut-in — ${sk.elementName.fr} ${sk.categoryName.fr}`,
				url: sk.textureUrl,
				thumbUrl: cpkThumbUrl(`data/dx11/chr/_waza/${sk.eventIdName}/${sk.eventIdName}.g4tx`),
				group: "waza",
			});
		}
		if (sk.telopUrl) {
			textures.push({
				label: `Telop — ${sk.elementName.fr} ${sk.categoryName.fr}`,
				url: sk.telopUrl,
				thumbUrl: null,
				group: "telop",
			});
		}
	}

	// --- Dialogues : répliques où le perso est mentionné (dédupe FR, max 8). ---
	const dialogues: DemoDialogue[] = [];
	const seenLines = new Set<string>();
	for (const ev of d.dialogues ?? []) {
		for (const line of ev.lines ?? []) {
			if (!line.mentions || !line.fr || seenLines.has(line.fr)) continue;
			seenLines.add(line.fr);
			dialogues.push({ lineFr: line.fr, lineJa: line.ja ?? "" });
			if (dialogues.length >= 8) break;
		}
		if (dialogues.length >= 8) break;
	}

	const id = d.identity as Record<string, string | number>;
	const pr = d.profile as Record<string, string>;

	return {
		identity: {
			nameFr: String(id.name_fr ?? ""),
			nameEn: String(id.name_en ?? ""),
			nameJa: String(id.name_ja ?? ""),
			nicknameEn: String(id.nickname_en ?? pr.nickname_en ?? ""),
			nicknameJa: String(id.nickname_ja ?? pr.nickname_ja ?? ""),
			epithetFr: String(pr.epithet_fr ?? ""),
			epithetEn: String(pr.epithet_en ?? ""),
			epithetJa: String(pr.epithet_ja ?? ""),
			kana: String(pr.kana ?? ""),
			teamNameEn: String(id.team_name_en ?? ""),
			mainPosition: Number(id.main_position ?? 0),
			subPosition: Number(id.sub_position ?? 0),
			elementId: Number(id.element_id ?? 0),
			constellationFr: String(id.constellation_fr ?? ""),
			constellationEn: String(id.constellation_en ?? ""),
			zukanOrder: Number(id.zukan_order ?? 0),
		},
		forms,
		skills,
		textures,
		precomputedStats: { lv1: d.stats.lv1, lv50: d.stats.lv50, lv99: d.stats.lv99 },
		references: d.references ?? [],
		dialogues,
	};
}

export default function DemoPage() {
	const data = buildDemoData();
	return (
		<div className="w-full space-y-10">
			<header className="space-y-2">
				<p className="text-xs font-semibold uppercase tracking-wider text-primary">
					Démo 3D · #{data.identity.zukanOrder} · {data.identity.constellationFr}
				</p>
				<h1 className="text-fluid-headline-md font-extrabold text-on-surface">
					{data.identity.nameFr}{" "}
					<span className="text-on-surface-variant">« {data.identity.epithetFr} »</span>
				</h1>
				<p className="max-w-3xl text-sm text-on-surface-variant">
					{data.identity.nameJa} · {data.identity.kana} — équipe {data.identity.teamNameEn}. Toutes les
					ressources 3D du personnage réunies : modèles complets texturés des 3 ères, cut-ins jouables
					(caméra scénarisée, export MP4), voix japonaise, statistiques byte-exact (moteur niers) et
					textures téléchargeables. Une seule scène, pilotée à la demande.
				</p>
			</header>

			{/* Toute l'interactivité 3D (scène R3F lazy + sélecteur + stats + galerie). */}
			<DemoExperience data={data} />

			{/* Références culturelles (RSC pur). */}
			{data.references.length > 0 && (
				<section className="space-y-3">
					<h2 className="text-fluid-headline-sm font-bold text-on-surface">Références &amp; étymologie</h2>
					<div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
						{data.references.map((r) => (
							<article
								key={r.title}
								className="rounded-2xl border border-outline-variant/20 bg-surface-container-low/40 p-4"
							>
								<h3 className="mb-1 text-sm font-bold text-on-surface">{r.title}</h3>
								<p className="text-sm text-on-surface-variant">{r.detail}</p>
							</article>
						))}
					</div>
				</section>
			)}

			{/* Dialogues (RSC pur). */}
			{data.dialogues.length > 0 && (
				<section className="space-y-3">
					<h2 className="text-fluid-headline-sm font-bold text-on-surface">Répliques</h2>
					<ul className="space-y-2">
						{data.dialogues.map((line) => (
							<li
								key={line.lineFr}
								className="rounded-xl border border-outline-variant/20 bg-surface-container/60 p-3"
							>
								<p className="text-sm text-on-surface">« {line.lineFr} »</p>
								{line.lineJa && (
									<p className="mt-0.5 text-xs text-on-surface-variant">{line.lineJa}</p>
								)}
							</li>
						))}
					</ul>
				</section>
			)}
		</div>
	);
}
