export const dynamic = "force-dynamic";

import { getSupabaseClient } from "@/lib/api/supabase";
import type { MetadataRoute } from "next";
import { getCoachesList } from "@/lib/wiki/coaches";
import { getCapsuleList } from "@/lib/wiki/gacha";
import { getAllQuests } from "@/lib/wiki/quests";
import { getShopsList } from "@/lib/wiki/shops";
import { getStadiumsList } from "@/lib/wiki/stadiums";
import { getAllTeamIds } from "@/lib/wiki/teams";
import { getAllTrophies } from "@/lib/wiki/trophies";
import { wikiService } from "@/lib/wiki-service";

const BASE_URL = "https://azalee.rosegriffon.fr";

const AURA_CATEGORIES = ["esprits-guerriers", "totems", "miximax", "eveil", "changement-mode"];

export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
	const supabase = getSupabaseClient();

	// Fetch all IDs in parallel — using correct column names
	const [characters, skills, items, articles, tags] = await Promise.all([
		// `base_slug` et non `slug` : la forme canonique est celle que le site lie
		// lui-même (5 204 valeurs), pas la variante suffixée du hash (6 148) — publier
		// la seconde donnait un plan de site dont aucune URL n'était celle du maillage.
		supabase
			.from("inagle_characters")
			.select("base_slug, updated_at")
			.not("base_slug", "is", null)
			.order("updated_at", { ascending: false }),
		// `internal_code` en plus de `id` : c'est lui qui dit si la ligne est une
		// vraie technique (`wh*`/`rh*`) ou une entrée de service (`swap_skill_waza_*`).
		supabase
			.from("inagle_skills")
			.select("id, internal_code, created_at")
			.order("created_at", { ascending: false }),
		supabase
			.from("inagle_items")
			.select("id, created_at")
			.order("created_at", { ascending: false }),
		supabase
			.from("articles")
			.select("slug, updated_at, published_at")
			.eq("status", "published")
			.eq("app", "azalee")
			.order("published_at", { ascending: false }),
		supabase
			.from("articles")
			.select("tags")
			.eq("status", "published")
			.eq("app", "azalee")
			.not("tags", "is", null),
	]);

	const entries: MetadataRoute.Sitemap = [];

	// Static pages
	const staticPages = [
		{ changeFrequency: "daily" as const, priority: 1.0, url: "/" },
		{ changeFrequency: "weekly" as const, priority: 0.9, url: "/chara" },
		{ changeFrequency: "weekly" as const, priority: 0.9, url: "/skill" },
		{ changeFrequency: "weekly" as const, priority: 0.8, url: "/item" },
		{ changeFrequency: "weekly" as const, priority: 0.8, url: "/aura" },
		{ changeFrequency: "weekly" as const, priority: 0.7, url: "/passive" },
		{ changeFrequency: "weekly" as const, priority: 0.6, url: "/search" },
		{ changeFrequency: "daily" as const, priority: 0.8, url: "/news" },
		{ changeFrequency: "weekly" as const, priority: 0.7, url: "/patch-notes" },
		{ changeFrequency: "weekly" as const, priority: 0.7, url: "/tactic" },
		// Sections du wiki absentes jusqu'ici du plan de site alors que leurs
		// pages existent et répondent 200 : elles n'étaient découvrables que par
		// le maillage interne.
		{ changeFrequency: "weekly" as const, priority: 0.7, url: "/equipe" },
		{ changeFrequency: "weekly" as const, priority: 0.7, url: "/quete" },
		{ changeFrequency: "weekly" as const, priority: 0.6, url: "/boutique" },
		{ changeFrequency: "weekly" as const, priority: 0.6, url: "/entraineur" },
		{ changeFrequency: "weekly" as const, priority: 0.6, url: "/stade" },
		{ changeFrequency: "weekly" as const, priority: 0.6, url: "/succes" },
		{ changeFrequency: "weekly" as const, priority: 0.6, url: "/capsule" },
		{ changeFrequency: "weekly" as const, priority: 0.6, url: "/drops" },
		{ changeFrequency: "weekly" as const, priority: 0.6, url: "/invocation" },
		{ changeFrequency: "monthly" as const, priority: 0.6, url: "/niveau" },
		{ changeFrequency: "monthly" as const, priority: 0.5, url: "/cross" },
		// Les collections média (`/gallery`, `/textures`, `/sons`, `/videos`, `/modeles`,
		// `/mode`) et les cinq outils (`/tools/{stats,compare,random-team,my-team,translator}`)
		// ont migré vers l'explorateur de bureau — cf. `docs/MIGRATION-EXPLORATEUR.md`. Un plan
		// de site n'annonce que des URLs servies : ne pas les remettre ici.
		{ changeFrequency: "monthly" as const, priority: 0.4, url: "/tools/niers" },
		{ changeFrequency: "yearly" as const, priority: 0.3, url: "/contact" },
		{ changeFrequency: "yearly" as const, priority: 0.3, url: "/soutenir" },
		{ changeFrequency: "yearly" as const, priority: 0.2, url: "/charte" },
		{
			changeFrequency: "yearly" as const,
			priority: 0.1,
			url: "/legal/cgu",
		},
		{
			changeFrequency: "yearly" as const,
			priority: 0.1,
			url: "/legal/confidentialite",
		},
		{
			changeFrequency: "yearly" as const,
			priority: 0.1,
			url: "/legal/mentions-legales",
		},
	];

	// Aura category pages
	for (const cat of AURA_CATEGORIES) {
		staticPages.push({
			changeFrequency: "weekly" as const,
			priority: 0.7,
			url: `/aura/${cat}`,
		});
	}

	for (const page of staticPages) {
		entries.push({
			changeFrequency: page.changeFrequency,
			lastModified: new Date(),
			priority: page.priority,
			url: `${BASE_URL}${page.url}`,
		});
	}

	// Characters
	if (characters.data) {
		// Un `base_slug` est partagé par toutes les variantes d'un même personnage
		// (326 slugs en regroupent plusieurs, « unknown » à lui seul en réunit 53) :
		// sans déduplication, la même URL serait annoncée jusqu'à 58 fois. On garde
		// la ligne la plus récemment mise à jour, les lignes arrivant déjà triées.
		const vus = new Set<string>();
		for (const c of characters.data) {
			const slug = c.base_slug;
			if (!slug || vus.has(slug)) continue;
			vus.add(slug);
			entries.push({
				changeFrequency: "monthly",
				lastModified: c.updated_at ? new Date(c.updated_at) : new Date(),
				priority: 0.8,
				url: `${BASE_URL}/chara/${slug}`,
			});
		}
	}

	// Skills
	if (skills.data) {
		for (const s of skills.data) {
			// Le plan de site doit annoncer EXACTEMENT ce que la liste publie. Le
			// service (`getSkillsList`) ne retient que les codes `wh*`/`rh*` non
			// suffixés `_or` : reproduire ici la seule moitié `_or` du filtre laissait
			// **9 URL orphelines** — les `swap_skill_waza_*`, qui répondent 200 avec un
			// code pour titre et une puissance de 0, et qu'aucune page ne lie.
			// Reproduire le filtre en entier plutôt que d'en recopier un morceau.
			if (typeof s.id !== "string" || s.id.endsWith("_or")) continue;
			if (!/^(wh|rh)/.test(s.internal_code ?? s.id)) continue;
			entries.push({
				changeFrequency: "monthly",
				lastModified: s.created_at ? new Date(s.created_at) : new Date(),
				priority: 0.7,
				url: `${BASE_URL}/skill/${s.id}`,
			});
		}
	}

	// Items
	if (items.data) {
		for (const i of items.data) {
			entries.push({
				changeFrequency: "monthly",
				lastModified: i.created_at ? new Date(i.created_at) : new Date(),
				priority: 0.6,
				url: `${BASE_URL}/item/${i.id}`,
			});
		}
	}

	// Hyper-techniques — une catégorie à la fois, par le même service que les pages.
	// L'ancienne requête lisait `inagle_keshins` en triant sur `created_at`, colonne
	// qui n'existe pas sur cette table : elle échouait, le garde `if (auras.data)`
	// avalait l'erreur, et AUCUNE fiche d'aura n'était annoncée. Passer par
	// `getAurasList` fait d'une pierre deux coups : les cinq familles sont couvertes
	// (et pas seulement les esprits), et les identifiants sont ceux que les pages
	// résolvent réellement (`keshin_0x…`, `soul_0x…`, …), pas des hash nus.
	for (const categorie of AURA_CATEGORIES) {
		try {
			for (let page = 1; ; page += 1) {
				const { data, total } = await wikiService.getAurasList({
					limit: 200,
					page,
					typeSlug: categorie,
				});
				for (const aura of data) {
					entries.push({
						changeFrequency: "monthly",
						lastModified: new Date(),
						priority: 0.6,
						url: `${BASE_URL}/aura/${categorie}/${encodeURIComponent(String(aura.id))}`,
					});
				}
				if (data.length === 0 || page * 200 >= total) break;
			}
		} catch (erreur) {
			console.error(`[sitemap] auras ${categorie} indisponibles :`, erreur);
		}
	}

	// Tactiques — par le service, donc par `internal_code` (`wht…`). L'ancienne
	// requête publiait les identifiants hexadécimaux d'`inagle_items` filtrés sur
	// `special_tactics` : 70 URLs qui ne résolvaient rien côté tactique, et qui
	// doublonnaient les fiches `/item/<même id>`.
	try {
		for (let page = 1; ; page += 1) {
			const { data, total } = await wikiService.getTacticsList({
				category: "special_tactics",
				limit: 200,
				page,
			});
			for (const tactique of data) {
				const id = (tactique as { itemId?: string }).itemId;
				if (!id) continue;
				entries.push({
					changeFrequency: "monthly",
					lastModified: new Date(),
					priority: 0.6,
					url: `${BASE_URL}/tactic/${encodeURIComponent(id)}`,
				});
			}
			if (data.length === 0 || page * 200 >= total) break;
		}
	} catch (erreur) {
		console.error("[sitemap] tactiques indisponibles :", erreur);
	}

	// Articles / News
	if (articles.data) {
		for (const n of articles.data) {
			entries.push({
				changeFrequency: "monthly",
				lastModified: n.updated_at
					? new Date(n.updated_at)
					: n.published_at
						? new Date(n.published_at)
						: new Date(),
				priority: 0.7,
				url: `${BASE_URL}/news/${n.slug}`,
			});
		}
	}

	// Fiches des sections servies par la bibliothèque `@rosegriffon/azalee` :
	// équipes, entraîneurs, stades, quêtes, succès, lots de capsule, boutiques et
	// passifs. Leurs pages de liste étaient annoncées, mais aucune de leurs
	// ~4 000 fiches ne l'était — elles n'existaient, pour un robot, que par le
	// maillage interne. Chaque source est isolée : une section indisponible ne
	// doit pas vider le plan de site des autres.
	const familles: { chemin: string; priorite: number; ids: () => Promise<string[]> }[] = [
		{ chemin: "/equipe", priorite: 0.6, ids: () => getAllTeamIds() },
		{
			chemin: "/entraineur",
			priorite: 0.5,
			ids: async () => (await getCoachesList()).map((c) => String(c.id)),
		},
		{
			chemin: "/stade",
			priorite: 0.5,
			ids: async () => (await getStadiumsList()).data.map((s) => s.id),
		},
		{ chemin: "/quete", priorite: 0.6, ids: async () => (await getAllQuests()).map((q) => q.id) },
		{
			chemin: "/succes",
			priorite: 0.5,
			ids: async () => (await getAllTrophies()).map((t) => t.id),
		},
		{
			chemin: "/capsule",
			priorite: 0.4,
			// `getCapsuleList` plafonne la page à 200 lignes : on la parcourt.
			ids: async () => {
				const ids: string[] = [];
				for (let page = 1; ; page += 1) {
					const { data, total } = await getCapsuleList({ limit: 200, page });
					ids.push(...data.map((p) => p.id));
					if (ids.length >= total || data.length === 0) break;
				}
				return ids;
			},
		},
		{
			chemin: "/boutique",
			priorite: 0.5,
			ids: async () => (await getShopsList()).map((s) => String(s.shopId)),
		},
		{
			chemin: "/passive",
			priorite: 0.5,
			ids: async () =>
				wikiService
					.getPassivesList({ limit: 5000, page: 1 })
					.data.map((p) => p.passive_id)
					.filter(Boolean),
		},
	];

	for (const famille of familles) {
		try {
			for (const id of await famille.ids()) {
				entries.push({
					changeFrequency: "monthly",
					lastModified: new Date(),
					priority: famille.priorite,
					url: `${BASE_URL}${famille.chemin}/${encodeURIComponent(id)}`,
				});
			}
		} catch (erreur) {
			console.error(`[sitemap] ${famille.chemin} indisponible :`, erreur);
		}
	}

	// Tags (from articles)
	if (tags.data) {
		const uniqueTags = new Set<string>();
		for (const a of tags.data) {
			if (Array.isArray(a.tags)) {
				for (const tag of a.tags) {
					uniqueTags.add(tag);
				}
			}
		}
		for (const tag of uniqueTags) {
			entries.push({
				changeFrequency: "weekly",
				lastModified: new Date(),
				priority: 0.5,
				url: `${BASE_URL}/news/tag/${encodeURIComponent(tag)}`,
			});
		}
	}

	return entries;
}
