/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Tâche SEO : génération / rafraîchissement des fichiers `llms.txt`.
 *
 * Le standard llms.txt (https://llmstxt.org) est un index Markdown placé à la
 * racine d'un site, lu par les crawlers IA pour comprendre le contenu et les
 * pages clés. On génère :
 *   - `llms.txt`      : index concis (présentation + liens principaux)
 *   - `llms-full.txt` : version étendue (sections détaillées)
 *   - `llm.txt`       : alias de `llms.txt` (référencé par les métadonnées du
 *                       website, alternates.types["text/markdown"]).
 *
 * Les fichiers sont écrits dans apps/website/public et apps/azalee/public, donc
 * servis statiquement à la racine de chaque domaine. La régénération
 * via cron garde la date `Généré le …` fraîche (signal de fraîcheur).
 *
 * Ce qui est garanti : des fichiers à jour et conformes au standard, lisibles
 * par tout crawler IA qui le souhaite. Ce qui ne l'est PAS : qu'un LLM précis
 * (Claude, Gemini, Grok, ChatGPT…) ingère ou cite réellement le contenu — cela
 * dépend entièrement de l'éditeur du modèle.
 */

import { writeFile } from "node:fs/promises";
import { join } from "node:path";

import { dansLeDepot, depotRoseGriffon } from "../../lib/racine";

// Le wiki vit dans ce dépôt ; le site vitrine est resté dans `rg` (cf. `docs/FUSION.md`).
// Chacun est donc résolu à l'exécution, jamais par un nombre de « .. » qui change de sens
// selon le dépôt d'où le démon tourne.
const AZALEE_PUBLIC = dansLeDepot("apps", "azalee", "public");
const RG = depotRoseGriffon();
const WEBSITE_PUBLIC = RG ? join(RG, "apps", "website", "public") : null;

const DEV_CREDIT = "yoyo — https://x.com/yoyo__goat";

function nowStamp(): string {
	return new Date().toISOString().slice(0, 10);
}

// ─── WEBSITE (rosegriffon.fr) ────────────────────────────────────────────────

function websiteLlms(): string {
	return `# Rose Griffon

> Association française dédiée à la communauté Inazuma Eleven. Rose Griffon rassemble les fans francophones autour d'événements, de projets créatifs et de contenus sur la licence Inazuma Eleven (LEVEL-5).

Site officiel : https://rosegriffon.fr
Wiki Azalée (Inazuma Eleven: Victory Road) : https://azalee.rosegriffon.fr
Développeur & fondateur : ${DEV_CREDIT}
Généré le : ${nowStamp()}

## Pages principales

- [Accueil](https://rosegriffon.fr/) : présentation de l'association Rose Griffon.
- [Notre équipe](https://rosegriffon.fr/notre-equipe) : association, staff, partenaires.
- [À propos](https://rosegriffon.fr/a-propos) : qui sommes-nous, comment rejoindre.
- [Projets](https://rosegriffon.fr/projets) : projets créatifs de la communauté (Nostalgie, émotes, muraux, outils).
- [Événements](https://rosegriffon.fr/evenements) : tournois, rencontres et événements communautaires.
- [Chroniques](https://rosegriffon.fr/chroniques) : articles et actualités de l'association.
- [Nous soutenir](https://rosegriffon.fr/nous-soutenir) : soutenir Rose Griffon.

## Réseaux & liens

- Wiki Azalée : https://azalee.rosegriffon.fr
- X / Twitter Azalée : https://x.com/Azalee_IE
- Développeur (yoyo) : https://x.com/yoyo__goat

## Ressources

- [Sitemap](https://rosegriffon.fr/sitemap.xml)
- [robots.txt](https://rosegriffon.fr/robots.txt)
`;
}

function websiteLlmsFull(): string {
	return `# Rose Griffon — Référence complète

> Rose Griffon est une association française dédiée à la communauté Inazuma Eleven (licence LEVEL-5). Elle fédère les fans francophones autour d'événements, de projets créatifs collaboratifs et de contenus d'information, et maintient le wiki Azalée consacré à Inazuma Eleven: Victory Road.

Site officiel : https://rosegriffon.fr
Wiki Azalée : https://azalee.rosegriffon.fr
Développeur & fondateur : ${DEV_CREDIT}
Langue : fr-FR
Généré le : ${nowStamp()}

## À propos de l'association

Rose Griffon rassemble la communauté Inazuma Eleven en France. L'association
organise des événements, anime des projets communautaires et produit des
contenus (articles, outils, ressources) autour de la licence Inazuma Eleven.

- [Accueil](https://rosegriffon.fr/)
- [À propos](https://rosegriffon.fr/a-propos)
- [Rejoindre Rose Griffon](https://rosegriffon.fr/a-propos/rejoindre-rg)
- [Contact](https://rosegriffon.fr/a-propos/contact)
- [Charte & engagements](https://rosegriffon.fr/charte-engagements)
- [Cellule de médiation](https://rosegriffon.fr/cellule-de-mediation)

## Équipe

- [Notre équipe](https://rosegriffon.fr/notre-equipe)
- [Association](https://rosegriffon.fr/notre-equipe/association)
- [Staff](https://rosegriffon.fr/notre-equipe/staff)
- [Partenaires](https://rosegriffon.fr/notre-equipe/partenaires)

## Projets

- [Projets](https://rosegriffon.fr/projets)
- [Nostalgie](https://rosegriffon.fr/projets/nostalgie)
- [Émotes](https://rosegriffon.fr/projets/emotes)
- [Muraux](https://rosegriffon.fr/projets/muraux)
- [Outil PFP](https://rosegriffon.fr/projets/outils/pfp)

## Contenus & communauté

- [Événements](https://rosegriffon.fr/evenements)
- [Chroniques](https://rosegriffon.fr/chroniques)
- [Communauté](https://rosegriffon.fr/community)
- [Nous soutenir](https://rosegriffon.fr/nous-soutenir)

## Wiki Azalée

Azalée est le wiki francophone d'Inazuma Eleven: Victory Road maintenu par Rose
Griffon : personnages, techniques, objets, auras (esprits guerriers, totems,
miximax…), tactiques et actualités.

- Wiki : https://azalee.rosegriffon.fr
- Personnages : https://azalee.rosegriffon.fr/chara
- Techniques : https://azalee.rosegriffon.fr/skill
- Objets : https://azalee.rosegriffon.fr/item
- Auras : https://azalee.rosegriffon.fr/aura
- Actualités : https://azalee.rosegriffon.fr/news

## Mentions légales

- [Mentions légales](https://rosegriffon.fr/mentions-legales)
- [CGU](https://rosegriffon.fr/legal/cgu)
- [Confidentialité](https://rosegriffon.fr/legal/confidentialite)

## Crédits

Conception, développement full-stack et architecture : ${DEV_CREDIT}.
Stack : Next.js, React, Bun, TypeScript, Supabase.
`;
}

// ─── AZALÉE (azalee.rosegriffon.fr) ──────────────────────────────────────────

function azaleeLlms(): string {
	return `# Azalée — Wiki Inazuma Eleven: Victory Road

> Wiki francophone consacré à Inazuma Eleven: Victory Road (IEVR), maintenu par l'association Rose Griffon. Base de données complète : personnages, techniques, objets, auras, tactiques et actualités.

Wiki : https://azalee.rosegriffon.fr
Association : Rose Griffon — https://rosegriffon.fr
Développeur & fondateur : ${DEV_CREDIT}
Généré le : ${nowStamp()}

## Sections principales

- [Personnages](https://azalee.rosegriffon.fr/chara) : fiches des joueurs et personnages.
- [Techniques](https://azalee.rosegriffon.fr/skill) : techniques spéciales (tirs, dribbles, blocs, gardiens).
- [Objets](https://azalee.rosegriffon.fr/item) : objets et équipements.
- [Auras](https://azalee.rosegriffon.fr/aura) : esprits guerriers, totems, miximax, éveils, changements de mode.
- [Passifs](https://azalee.rosegriffon.fr/passive) : compétences passives.
- [Tactiques](https://azalee.rosegriffon.fr/tactic) : tactiques d'équipe.
- [Actualités](https://azalee.rosegriffon.fr/news) : news et patch-notes du jeu.
- [Explorateur](https://azalee.rosegriffon.fr/tools/niers) : application de bureau — comparateur, équipe aléatoire, traducteur, galerie.

## Réseaux & liens

- Site Rose Griffon : https://rosegriffon.fr
- X / Twitter Azalée : https://x.com/Azalee_IE
- Développeur (yoyo) : https://x.com/yoyo__goat

## Ressources

- [Sitemap](https://azalee.rosegriffon.fr/sitemap.xml)
- [robots.txt](https://azalee.rosegriffon.fr/robots.txt)
`;
}

function azaleeLlmsFull(): string {
	return `# Azalée — Wiki Inazuma Eleven: Victory Road — Référence complète

> Azalée est le wiki francophone de référence pour Inazuma Eleven: Victory Road (IEVR), le jeu de LEVEL-5. Maintenu par l'association Rose Griffon, il agrège personnages, techniques, objets, auras, tactiques, compétences passives et actualités du jeu, avec des outils communautaires.

Wiki : https://azalee.rosegriffon.fr
Association : Rose Griffon — https://rosegriffon.fr
Développeur & fondateur : ${DEV_CREDIT}
Langue : fr-FR
Généré le : ${nowStamp()}

## Présentation

Azalée recense les données de jeu d'Inazuma Eleven: Victory Road et propose des
fiches détaillées ainsi que des outils (comparaison, génération d'équipe,
traduction). Le wiki est produit et maintenu par Rose Griffon, association
française de la communauté Inazuma Eleven.

## Base de données

- [Personnages](https://azalee.rosegriffon.fr/chara) : fiches complètes des personnages jouables.
- [Techniques](https://azalee.rosegriffon.fr/skill) : tirs, dribbles, blocs et techniques de gardien.
- [Objets](https://azalee.rosegriffon.fr/item) : objets, équipements et consommables.
- [Auras](https://azalee.rosegriffon.fr/aura) : esprits guerriers, totems, miximax, éveils, changements de mode.
  - [Esprits guerriers](https://azalee.rosegriffon.fr/aura/esprits-guerriers)
  - [Totems](https://azalee.rosegriffon.fr/aura/totems)
  - [Miximax](https://azalee.rosegriffon.fr/aura/miximax)
  - [Éveil](https://azalee.rosegriffon.fr/aura/eveil)
  - [Changement de mode](https://azalee.rosegriffon.fr/aura/changement-mode)
- [Passifs](https://azalee.rosegriffon.fr/passive) : compétences passives.
- [Tactiques](https://azalee.rosegriffon.fr/tactic) : tactiques d'équipe.

## Actualités

- [News](https://azalee.rosegriffon.fr/news) : actualités du jeu et de la communauté.
- [Patch-notes](https://azalee.rosegriffon.fr/patch-notes) : notes de mise à jour.

## Outils

- [Explorateur](https://azalee.rosegriffon.fr/tools/niers) : comparateur, équipe aléatoire,
  traducteur, calculateur de stats et galerie d'illustrations vivent désormais dans
  l'application de bureau ; leurs anciennes URL y redirigent.
- [Recherche](https://azalee.rosegriffon.fr/search)

## API (pour les IA et outils)

Azalée expose une API GraphQL publique en lecture seule pour interroger la base de données du jeu.

- Endpoint GraphQL : POST https://azalee.rosegriffon.fr/api/graphql (Content-Type: application/json)
- Queries disponibles : characters, character, skills, skill, items, item, auras, ragSearch, tweets.
- Filtres skills(page, limit, q, category, element) ; characters(page, limit, q, element, position, rarity, team, series).
- Champs Skill : id, name { fr en ja }, description { fr en ja }, category, element, power, tension, image, sheetData. Le champ name est de type LocalizedString (sous-champs fr/en/ja).
- Recherche sémantique (RAG) : POST https://azalee.rosegriffon.fr/api/rag/search ; données formatées LLM : https://azalee.rosegriffon.fr/api/llm/<model>.

Exemple de requête (techniques de Tir) :
POST https://azalee.rosegriffon.fr/api/graphql
{"query":"{ skills(category: \\"Tir\\", limit: 5) { id name { fr en } category element power } }"}

Réponse (extrait) : { "data": { "skills": [ { "name": { "fr": "Feu tout-puissant" }, "category": "Tir", "element": "Feu", "power": "100-640" } ] } }

## Mentions légales

- [CGU](https://azalee.rosegriffon.fr/legal/cgu)
- [Confidentialité](https://azalee.rosegriffon.fr/legal/confidentialite)
- [Mentions légales](https://azalee.rosegriffon.fr/legal/mentions-legales)

## Crédits

Conception, développement et architecture : ${DEV_CREDIT}.
Stack : Next.js, React, Bun, TypeScript. Données : Inazuma Eleven: Victory Road (LEVEL-5).
`;
}

/**
 * Convertit le markdown llms.txt en page HTML lisible (titres, blockquote, listes, liens).
 * But : servir une version `text/html` (et `.md` markdown explicite) que les web-fetchers
 * d'IA (Gemini, etc.) acceptent — certains rejettent/mal-gèrent le `text/plain` de `llm.txt`.
 */
function llmsToHtml(md: string, title: string): string {
	const esc = (s: string) =>
		s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
	const linkify = (s: string) =>
		esc(s)
			.replace(/\[([^\]]+)\]\((https?:\/\/[^)]+)\)/g, '<a href="$2">$1</a>')
			.replace(/(?<!["=])(https?:\/\/[^\s<]+)/g, '<a href="$1">$1</a>');
	const lines = md.split("\n");
	const out: string[] = [];
	let inList = false;
	const closeList = () => {
		if (inList) {
			out.push("</ul>");
			inList = false;
		}
	};
	for (const raw of lines) {
		const l = raw.trimEnd();
		if (/^#{1,6}\s/.test(l)) {
			closeList();
			const lvl = (l.match(/^#+/) as RegExpMatchArray)[0].length;
			out.push(`<h${lvl}>${linkify(l.replace(/^#+\s/, ""))}</h${lvl}>`);
		} else if (/^>\s?/.test(l)) {
			closeList();
			out.push(`<blockquote>${linkify(l.replace(/^>\s?/, ""))}</blockquote>`);
		} else if (/^[-*]\s/.test(l)) {
			if (!inList) {
				out.push("<ul>");
				inList = true;
			}
			out.push(`<li>${linkify(l.replace(/^[-*]\s/, ""))}</li>`);
		} else if (l === "") {
			closeList();
		} else {
			closeList();
			out.push(`<p>${linkify(l)}</p>`);
		}
	}
	closeList();
	return `<!DOCTYPE html><html lang="fr"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>${esc(title)}</title><meta name="robots" content="index, follow"><style>body{max-width:48rem;margin:2rem auto;padding:0 1rem;font-family:system-ui,sans-serif;line-height:1.6}a{color:#0c1730}blockquote{border-left:3px solid #d4af37;margin:0;padding-left:1rem;color:#555}</style></head><body>\n${out.join("\n")}\n</body></html>\n`;
}

export async function runSeoLlmsTxt(): Promise<{ success: boolean; error?: string }> {
	console.log("[SEO llms.txt] Génération des fichiers llms.txt / llm.txt / .md / .html…");
	try {
		const websiteIndex = websiteLlms();
		const azaleeIndex = azaleeLlms();
		const websiteFull = websiteLlmsFull();
		const azaleeFull = azaleeLlmsFull();

		const targets: Array<{ path: string; content: string }> = [];

		// Le site vitrine n'est pas dans ce dépôt : sans lui, on écrit la moitié Azalée et
		// on le DIT. Une tâche qui rend « succès » après n'avoir rien écrit est un faux vert.
		if (WEBSITE_PUBLIC) {
			targets.push(
			// Website : llm.txt est l'alias référencé par les métadonnées Next.
			{ path: join(WEBSITE_PUBLIC, "llms.txt"), content: websiteIndex },
			{ path: join(WEBSITE_PUBLIC, "llm.txt"), content: websiteIndex },
			{ path: join(WEBSITE_PUBLIC, "llms-full.txt"), content: websiteFull },
			// Website — versions markdown (.md) + HTML (.html) fetchables par les IA.
			{ path: join(WEBSITE_PUBLIC, "llm.md"), content: websiteIndex },
			{ path: join(WEBSITE_PUBLIC, "llms.md"), content: websiteIndex },
			{ path: join(WEBSITE_PUBLIC, "llms-full.md"), content: websiteFull },
			{ path: join(WEBSITE_PUBLIC, "llm.html"), content: llmsToHtml(websiteIndex, "Rose Griffon — llm.txt") },
			{ path: join(WEBSITE_PUBLIC, "llms-full.html"), content: llmsToHtml(websiteFull, "Rose Griffon — llms-full") },
			);
		} else {
			console.warn(
				"[SEO llms.txt] Site vitrine introuvable (RG_MONOREPO absent) : seule la moitié Azalée est générée.",
			);
		}

		targets.push(
			// Azalée.
			{ path: join(AZALEE_PUBLIC, "llms.txt"), content: azaleeIndex },
			{ path: join(AZALEE_PUBLIC, "llm.txt"), content: azaleeIndex },
			{ path: join(AZALEE_PUBLIC, "llms-full.txt"), content: azaleeFull },
			// Azalée — versions markdown (.md) + HTML (.html).
			{ path: join(AZALEE_PUBLIC, "llm.md"), content: azaleeIndex },
			{ path: join(AZALEE_PUBLIC, "llms.md"), content: azaleeIndex },
			{ path: join(AZALEE_PUBLIC, "llms-full.md"), content: azaleeFull },
			{ path: join(AZALEE_PUBLIC, "llm.html"), content: llmsToHtml(azaleeIndex, "Azalée — llm.txt") },
			{ path: join(AZALEE_PUBLIC, "llms-full.html"), content: llmsToHtml(azaleeFull, "Azalée — llms-full") },
		);

		for (const { path, content } of targets) {
			await writeFile(path, content, "utf-8");
			console.log(`[SEO llms.txt] Écrit : ${path} (${content.length} octets)`);
		}

		console.log(`[SEO llms.txt] ${targets.length} fichiers générés avec succès.`);
		return { success: true };
	} catch (err: any) {
		console.error("[SEO llms.txt] Erreur lors de la génération :", err);
		return { success: false, error: err.message || String(err) };
	}
}
