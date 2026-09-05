export const dynamic = "force-dynamic";

import type { Metadata } from "next";
import { buildCanonical, LIST_CANONICAL_KEYS } from "@/lib/seo";
import { parseSearchParams } from "@/lib/validations";
import { wikiService } from "@/lib/wiki-service";
import { PlayersClient } from "../players-client";

export async function generateMetadata({
	searchParams,
}: {
	searchParams: Promise<Record<string, string | string[] | undefined>>;
}): Promise<Metadata> {
	const rawParams = await searchParams;
	const { q, element, position, rarity, page } = parseSearchParams(rawParams);

	let title = "Liste des Joueurs";
	const filters: string[] = [];

	if (q) {
		filters.push(`"${q}"`);
	}
	if (element) {
		filters.push(element.toString());
	}
	if (position) {
		filters.push(position.toString());
	}
	if (rarity) {
		filters.push(rarity.toString());
	}

	if (filters.length > 0) {
		title += ` : ${filters.join(", ")}`;
	}

	const pageNum = page ? Number.parseInt(page.toString(), 10) : 1;
	if (pageNum > 1) {
		title += ` (Page ${pageNum})`;
	}

	return {
		alternates: {
			canonical: buildCanonical("/chara", rawParams, LIST_CANONICAL_KEYS),
		},
		description: `Base de données des joueurs Inazuma Eleven: Victory Road. ${filters.length > 0 ? `Filtres : ${filters.join(", ")}. ` : ""}Retrouvez les stats, techniques et méthodes de recrutement de tous les personnages.`,
		openGraph: {
			description: "Encyclopédie complète Inazuma Eleven Victory Road",
			locale: "fr_FR",
			siteName: "Azalée - Inazuma Eleven Victory Road Wiki",
			title: `${title} | Azalée`,
			type: "website",
			url: "/chara",
		},
		title: `${title} | Inazuma Eleven Victory Road - Azalée`,
	};
}

// 60 par defaut, et non 200.
//
// Mesure du 2026-09-05 en production : `/chara` rendait **2 355 397 octets** de HTML pour 620
// liens. L'essentiel de ce poids est du balisage repete, que le navigateur doit analyser avant
// d'afficher quoi que ce soit — sur un telephone, cette page coutait plusieurs secondes pour
// montrer une grille dont l'utilisateur ne voit que la premiere rangee.
//
// 200 reste accessible par `?perPage=200` : le choix n'est pas retire, il cesse d'etre impose.
const DEFAULT_PER_PAGE = 60;
const ALLOWED_PER_PAGE = new Set([50, 60, 100, 200]);

export default async function PlayersPage({
	searchParams,
}: {
	searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
	const rawParams = await searchParams;
	const params = parseSearchParams(rawParams);
	const {
		q,
		element,
		position,
		rarity,
		gender,
		team,
		playstyle,
		series,
		status,
		role,
		ageGroup,
		page,
		perPage,
	} = params;

	const pageNumber = page ? parseInt(page.toString(), 10) : 1;
	const itemsPerPage =
		perPage && ALLOWED_PER_PAGE.has(Number(perPage)) ? Number(perPage) : DEFAULT_PER_PAGE;

	// Data First: Fetch paginated data from DB
	const { data: characters, total } = await wikiService.getCharactersList({
		ageGroup: ageGroup?.toString(),
		element: element?.toString(),
		gender: gender?.toString(),
		limit: itemsPerPage,
		page: pageNumber,
		playstyle: playstyle?.toString(),
		position: position?.toString(),
		q: q?.toString(),
		rarity: rarity?.toString(),
		role: role?.toString(),
		series: series?.toString(),
		status: status?.toString(),
		team: team?.toString(),
	});

	// Fetch filter options
	const teams = await wikiService.getAllTeams();

	const collectionJsonLd = {
		"@context": "https://schema.org",
		"@type": "CollectionPage",
		description: `Base de données de ${total.toLocaleString("fr-FR")} joueurs Inazuma Eleven: Victory Road.`,
		isPartOf: { "@type": "WebSite", name: "Azalée", url: "https://azalee.rosegriffon.fr" },
		mainEntity: {
			"@type": "ItemList",
			itemListElement: characters.slice(0, 10).map((c, i) => ({
				"@type": "ListItem",
				position: (pageNumber - 1) * itemsPerPage + i + 1,
				url: `https://azalee.rosegriffon.fr/chara/${c.slug || c.charaId}`,
				name: c.names?.fr || c.names?.en || "Joueur",
			})),
			numberOfItems: total,
		},
		name: "Personnages - Inazuma Eleven: Victory Road",
		numberOfItems: total,
		url: "https://azalee.rosegriffon.fr/chara",
	};

	return (
		<div className="space-y-6">
			<script
				type="application/ld+json"
				dangerouslySetInnerHTML={{ __html: JSON.stringify(collectionJsonLd) }}
			/>
			<div className="space-y-1">
				<h1 className="text-2xl sm:text-3xl font-extrabold tracking-tight text-on-surface font-display">
					Personnages
				</h1>
				<p className="text-sm text-on-surface-variant">
					Base de données complète des joueurs d'Inazuma Eleven: Victory Road.
				</p>
			</div>
			<PlayersClient
				paginatedCharacters={characters}
				totalItems={total}
				itemsPerPage={itemsPerPage}
				currentPage={pageNumber}
				allVariantsCount={total}
				teams={teams}
				showIncomplete={status === "all"}
				showControllable={status === "jouable"}
			/>
		</div>
	);
}
