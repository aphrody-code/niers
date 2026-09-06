export const dynamic = "force-dynamic";

import type { Metadata } from "next";
import { FadeInItem, FadeInStagger } from "@/components/ui/fade-in";
import { SkillFilterBar } from "@/components/wiki/filters/SkillFilterBar";
import { MediaCount } from "@/components/wiki/MediaShell";
import { MoveCard } from "@/components/wiki/MoveCard";
import { WikiPagination } from "@/components/wiki/WikiPagination";
import { WikiSearchToolbar } from "@/components/wiki/WikiSearchToolbar";
import { getSkillCategoryIconUrl } from "@rosegriffon/azalee/images";
import { buildCanonical, SKILL_CANONICAL_KEYS } from "@/lib/seo";
import { parseSearchParams } from "@/lib/validations";
import { wikiService } from "@/lib/wiki-service";

export async function generateMetadata({
	searchParams,
}: {
	searchParams: Promise<Record<string, string | string[] | undefined>>;
}): Promise<Metadata> {
	const rawParams = await searchParams;
	const { q, type: category, element, page } = parseSearchParams(rawParams);

	let title = "Techniques Spéciales";
	const filters: string[] = [];

	if (category) {
		const catLabel = CATEGORY_FILTERS.find((c) => c.value === category)?.label;
		if (catLabel) {
			filters.push(catLabel);
		}
	}
	if (element) {
		const elemLabel = ELEMENT_FILTERS.find(
			(e) => e.value.toLowerCase() === element.toString().toLowerCase()
		)?.label;
		if (elemLabel) {
			filters.push(elemLabel);
		}
	}
	if (q) {
		filters.push(`"${q}"`);
	}

	if (filters.length > 0) {
		title += ` : ${filters.join(" ")}`;
	}

	const pageNum = page ? Number.parseInt(page.toString(), 10) : 1;
	if (pageNum > 1) {
		title += ` (Page ${pageNum})`;
	}

	return {
		alternates: {
			canonical: buildCanonical("/skill", rawParams, SKILL_CANONICAL_KEYS),
		},
		description: `Base de données des techniques spéciales (Hissatsu)${filters.length > 0 ? ` filtrée par ${filters.join(", ")}` : ""}. Puissance, tension et effets.`,
		title: `${title} | Inazuma Eleven Victory Road - Azalée`,
	};
}

const ITEMS_PER_PAGE = 60;

const CATEGORY_FILTERS: Array<{ value: string; label: string; imageIcon?: string }> = [
	{ label: "Tout", value: "" },
	{ imageIcon: getSkillCategoryIconUrl("tir"), label: "Tir", value: "shoot" },
	{ imageIcon: getSkillCategoryIconUrl("defense"), label: "Défense", value: "block" },
	{ imageIcon: getSkillCategoryIconUrl("dribble"), label: "Dribble", value: "dribble" },
	{ imageIcon: getSkillCategoryIconUrl("gardien"), label: "Arrêt", value: "catch" },
];

const ELEMENT_FILTERS = [
	{ icon: "blur_on", label: "Tous", value: "" },
	{ commonSprite: "fire" as const, label: "Feu", value: "Fire" },
	{ commonSprite: "wind" as const, label: "Vent", value: "Wind" },
	{ commonSprite: "forest" as const, label: "Forêt", value: "Forest" },
	{ commonSprite: "mountain" as const, label: "Montagne", value: "Mountain" },
	{ icon: "blur_on", label: "Néant", value: "Void" },
];

export default async function SkillsPage({
	searchParams,
}: {
	searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
	const rawParams = await searchParams;
	const params = parseSearchParams(rawParams);
	const {
		q,
		page,
		type: category,
		element,
		has_video,
		show_aura,
		overdrive,
		sort,
		power_min,
		power_max,
	} = params;

	const pageNumber = page ? Number.parseInt(page.toString(), 10) : 1;

	// Data First: Fetch paginated from DB
	const { data: skills, total } = await wikiService.getSkillsList({
		category: category?.toString(),
		element: element?.toString(),
		has_video: has_video?.toString(),
		limit: ITEMS_PER_PAGE,
		overdrive: overdrive?.toString(),
		page: pageNumber,
		power_max: power_max?.toString(),
		power_min: power_min?.toString(),
		q: q?.toString(),
		show_aura: show_aura?.toString(),
		sort: sort?.toString(),
	});

	// Re-map for UI
	const categoryCounts = CATEGORY_FILTERS.map((f) => ({ ...f, count: 0 })); // Counts disabled for perf
	const elementCounts = ELEMENT_FILTERS.map((f) => ({ ...f, count: 0 }));

	const collectionJsonLd = {
		"@context": "https://schema.org",
		"@type": "CollectionPage",
		description: `${total.toLocaleString("fr-FR")} techniques spéciales (Hissatsu) avec puissance, tension et vidéos.`,
		isPartOf: { "@type": "WebSite", name: "Azalée", url: "https://azalee.rosegriffon.fr" },
		mainEntity: {
			"@type": "ItemList",
			// Toutes les techniques affichées, pas les dix premières : le balisage
			// décrivait un sixième de la page, et une liste qui s'arrête à 10 quand
			// l'écran en montre 60 se lit comme une page presque vide. Les positions
			// sont absolues (numérotation continue de page en page), ce qui rattache
			// chaque page de pagination au même ensemble.
			itemListElement: skills.map((s: any, i: number) => ({
				"@type": "ListItem",
				position: (pageNumber - 1) * ITEMS_PER_PAGE + i + 1,
				url: `https://azalee.rosegriffon.fr/skill/${s.skillId}`,
				// `displayName` porte déjà le repli des techniques sans nom (quatre
				// lignes dont les trois noms valent le code interne) : le balisage ne
				// doit pas plus publier un code de fichier que la page ne l'affiche.
				name: s.name_FR || s.name_EN || s.displayName || "Technique",
			})),
			numberOfItems: total,
		},
		name: "Techniques Spéciales - Inazuma Eleven: Victory Road",
		numberOfItems: total,
		url: "https://azalee.rosegriffon.fr/skill",
	};

	return (
		<div className="space-y-6">
			<script
				type="application/ld+json"
				dangerouslySetInnerHTML={{ __html: JSON.stringify(collectionJsonLd) }}
			/>
			<div className="space-y-1">
				<h1 className="text-2xl sm:text-3xl font-extrabold tracking-tight text-on-surface font-display">
					Techniques Spéciales
				</h1>
				<p className="text-sm text-on-surface-variant">
					Base de données complète des techniques (Hissatsu) d'Inazuma Eleven: Victory Road.
				</p>
			</div>
			<div className="space-y-4">
				<WikiSearchToolbar placeholder="Rechercher une technique..." showFilters={false} />
				<SkillFilterBar
					currentCategory={category || ""}
					currentElement={element || ""}
					currentHasVideo={has_video || ""}
					currentShowAura={show_aura || ""}
					currentOverdrive={overdrive || ""}
					currentSort={sort || ""}
					currentPowerMin={power_min || ""}
					currentPowerMax={power_max || ""}
					categories={categoryCounts}
					elements={elementCounts}
				/>
			</div>

			{/* Ligne de compte héritée des pages média (parties dans l'explorateur) :
			    /skill la réimplémentait à l'identique, elle vit dans MediaShell. */}
			<MediaCount
				left={`${total.toLocaleString("fr-FR")} techniques`}
				right={`Page ${pageNumber} sur ${Math.max(1, Math.ceil(total / ITEMS_PER_PAGE))}`}
			/>

			{skills.length > 0 ? (
				<FadeInStagger className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-2">
					{skills.map((skill: any) => (
						<FadeInItem key={skill.skillId} className="h-full">
							<MoveCard
								id={skill.skillId}
								name={skill.name_FR || skill.name_EN || "Inconnu"}
								category={skill.categoryName?.fr || "Spécial"}
								powerMin={skill.power_min}
								powerMax={skill.power_max}
								tensionCost={skill.consumeTp}
								element={skill.elementName?.fr}
								videoUrl={skill.videoUrl || undefined}
								posterUrl={skill.posterUrl || undefined}
								thumbnailUrl={skill.thumbnailUrl || undefined}
								shop={skill.shop || undefined}
								className="h-full"
							/>
						</FadeInItem>
					))}
				</FadeInStagger>
			) : (
				<div className="py-20 text-center rounded-[32px] border border-outline-variant bg-surface-container-low border-dashed">
					<p className="text-on-surface-variant italic">Aucune technique trouvée.</p>
				</div>
			)}

			<WikiPagination totalItems={total} itemsPerPage={ITEMS_PER_PAGE} currentPage={pageNumber} />
		</div>
	);
}
