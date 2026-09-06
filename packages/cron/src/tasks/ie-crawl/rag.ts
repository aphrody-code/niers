/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import * as cheerio from "cheerio";
import { vectorStore, getEmbedding, crawlerTracker } from "@rosegriffon/db/redis";
import { indexZukanCharacters } from "./rag-zukan";
import { chunkText, computeStringHash } from "./rag-utils";
import { ingestSources, type RagSource } from "./rag-store-local";
import { chunkBySource, type SourceKind } from "./rag-chunkers";
import { sql } from "../../lib/db.js";
import { dansLeDepot } from "../../lib/racine";

interface DBTweetRow {
	id: string;
	author_username: string | null;
	author_name: string | null;
	text: string | null;
	created_at: string | null;
	// champs complets pour chunker RAG pgvector (métriques, médias, raw si besoin)
	metrics?: unknown;
	media?: unknown;
	translation?: string | null;
	category?: string | null;
}

const DATA_ROOT = dansLeDepot("data", "inazuma.jp", "victory-road");

function parseFrontmatter(fileContent: string): {
	data: Record<string, string[] | string>;
	content: string;
} {
	const frontmatterRegex = /^---\r?\n([\s\S]*?)\r?\n---\r?\n/;
	const match = fileContent.match(frontmatterRegex);

	if (!match) {
		return { data: {}, content: fileContent };
	}

	const yamlBlock = match[1] || "";
	const content = fileContent.replace(frontmatterRegex, "");
	const data: Record<string, string[] | string> = {};

	const lines = yamlBlock.split("\n");
	for (const line of lines) {
		const colonIdx = line.indexOf(":");
		if (colonIdx === -1) continue;

		const key = line.slice(0, colonIdx).trim();
		let val = line.slice(colonIdx + 1).trim();

		if (val.startsWith('"') && val.endsWith('"')) {
			val = val.slice(1, -1);
		} else if (val.startsWith("'") && val.endsWith("'")) {
			val = val.slice(1, -1);
		}

		if (val.startsWith("[") && val.endsWith("]")) {
			data[key] = val
				.slice(1, -1)
				.split(",")
				.map((s) => s.trim().replace(/^['"]|['"]$/g, ""))
				.filter(Boolean);
		} else {
			data[key] = val;
		}
	}

	return { data, content };
}

interface ScrapedItem {
	id: string;
	type: "topic" | "patchnote" | "re" | "cross" | "level5" | "doc";
	title: string;
	url: string;
	date: string;
	htmlPath: string;
	metadata: Record<string, string[] | string | number | boolean>;
}

/**
 * Normalise les espaces et retours à la ligne en évitant de tout aplatir.
 * Conserve un saut de ligne vide maximum (\n\n) pour le découpage sémantique du RAG.
 */
function normalizeText(text: string): string {
	return text
		.replace(/\r\n/g, "\n")
		.split("\n")
		.map((line) => line.trim().replace(/[ \t]+/g, " "))
		.filter((line, index, arr) => {
			if (line === "") {
				// Garder au maximum une seule ligne vide consécutive
				return index > 0 && arr[index - 1] !== "";
			}
			return true;
		})
		.join("\n")
		.trim();
}

/**
 * Nettoie le contenu HTML pour en extraire du texte structuré sous forme de Markdown simplifié.
 */
function cleanHtmlToMarkdown(html: string): string {
	const $ = cheerio.load(html);

	// Supprimer les éléments inutiles (scripts, styles, navigation, pied de page)
	$("script, style, nav, footer, header, noscript, iframe").remove();

	// Cibles fréquentes pour les articles/contenus
	let contentArea = $(".detailArea, .newsArea, #newsDetail, .mainContents, article").first();
	if (contentArea.length === 0) {
		contentArea = $("body");
	}

	// Remplacer les <br> par des sauts de ligne
	contentArea.find("br").replaceWith("\n");

	// Convertir les titres en Markdown
	for (let i = 1; i <= 6; i++) {
		contentArea.find(`h${i}`).each((_, elem) => {
			const text = $(elem).text().trim();
			if (text) {
				$(elem).replaceWith(`\n\n${"#".repeat(i)} ${text}\n\n`);
			}
		});
	}

	// Convertir les listes
	contentArea.find("li").each((_, elem) => {
		const text = $(elem).text().trim();
		if (text) {
			$(elem).replaceWith(`\n- ${text}\n`);
		}
	});

	// Convertir les gras / emphasis
	contentArea.find("strong, b").each((_, elem) => {
		const text = $(elem).text().trim();
		if (text) {
			$(elem).replaceWith(` **${text}** `);
		}
	});
	contentArea.find("em, i").each((_, elem) => {
		const text = $(elem).text().trim();
		if (text) {
			$(elem).replaceWith(` *${text}* `);
		}
	});

	// Ajouter des sauts de ligne pour les autres blocs afin de préserver la structure
	contentArea.find("p, div, tr, section, article").each((_, elem) => {
		$(elem).before("\n").after("\n");
	});

	return normalizeText(contentArea.text());
}



/**
 * Scanne le dossier de données local pour collecter tous les topics et patch notes scrapés.
 */
function collectScrapedItems(): ScrapedItem[] {
	const items: ScrapedItem[] = [];

	// 1. Collecter les Topics (fr/topics, en/topics, ja/topics, es/topics, etc.)
	const topicLocales = ["fr", "en", "ja", "es", "de", "it", "zh-tw", "zh-cn", "pt-br"] as const;
	for (const locale of topicLocales) {
		const topicsDir = join(DATA_ROOT, locale, "topics");
		if (existsSync(topicsDir)) {
			const subDirs = readdirSync(topicsDir, { withFileTypes: true })
				.filter((dirent) => dirent.isDirectory())
				.map((dirent) => dirent.name);

			for (const dir of subDirs) {
				const dirPath = join(topicsDir, dir);
				const metaPath = join(dirPath, "meta.json");
				const htmlPath = join(dirPath, "index.html");

				if (existsSync(metaPath) && existsSync(htmlPath)) {
					try {
						const meta = JSON.parse(readFileSync(metaPath, "utf-8"));
						items.push({
							id: `topic-${locale}-${dir}`,
							type: "topic",
							title: meta.title || "Topic",
							url: meta.url || "",
							date: meta.date || "",
							htmlPath,
							metadata: {
								id: dir,
								category: meta.category || "General",
								language: locale,
							},
						});
					} catch (err) {
						console.error(
							`[RAG Sync] Erreur de lecture de meta pour topic ${dir} [${locale}] :`,
							err
						);
					}
				}
			}
		}
	}

	// 2. Collecter les Patch Notes (en/patchnote, ja/patchnote)
	const patchLocales = ["en", "ja"] as const;
	const platforms = ["ps-steam", "switch", "xbox"];
	for (const locale of patchLocales) {
		const patchNotesDir = join(DATA_ROOT, locale, "patchnote");
		if (existsSync(patchNotesDir)) {
			for (const platform of platforms) {
				const platformDir = join(patchNotesDir, platform);
				if (!existsSync(platformDir)) continue;

				const subDirs = readdirSync(platformDir, { withFileTypes: true })
					.filter((dirent) => dirent.isDirectory())
					.map((dirent) => dirent.name);

				for (const dir of subDirs) {
					const dirPath = join(platformDir, dir);
					const dataPath = join(dirPath, "data.json");
					const htmlPath = join(dirPath, "index.html");

					if (existsSync(dataPath) && existsSync(htmlPath)) {
						try {
							const data = JSON.parse(readFileSync(dataPath, "utf-8"));
							items.push({
								id: `patchnote-${locale}-${platform}-${dir}`,
								type: "patchnote",
								title: data.title || "Patch Note",
								url: data.url || "",
								date: data.date || "",
								htmlPath,
								metadata: {
									id: dir,
									platform: data.platform || [platform],
									language: locale,
								},
							});
						} catch (err) {
							console.error(
								`[RAG Sync] Erreur de lecture de data pour patch ${dir} [${locale}] :`,
								err
							);
						}
					}
				}
			}
		}
	}

	// 3. Collecter les pages Inazuma Eleven RE (re)
	const reDir = "/home/ubuntu/niers/data/inazuma.jp/re";
	if (existsSync(reDir)) {
		const subDirs = readdirSync(reDir, { withFileTypes: true })
			.filter((dirent) => dirent.isDirectory())
			.map((dirent) => dirent.name);

		for (const dir of subDirs) {
			const dirPath = join(reDir, dir);
			const metaPath = join(dirPath, "meta.json");
			const htmlPath = join(dirPath, "index.html");

			if (existsSync(metaPath) && existsSync(htmlPath)) {
				try {
					const meta = JSON.parse(readFileSync(metaPath, "utf-8"));
					items.push({
						id: `re-${dir}`,
						type: "re" as any,
						title: meta.title || "Inazuma Eleven RE",
						url: meta.url || "",
						date: meta.date || "",
						htmlPath,
						metadata: {
							id: dir,
							category: meta.category || "RE Remake",
							language: meta.language || dir,
						},
					});
				} catch (err) {
					console.error(`[RAG Sync] Erreur de lecture de meta pour RE ${dir} :`, err);
				}
			}
		}
	}

	// 4. Collecter les pages Inazuma Eleven: Cross (cross)
	const crossDir = "/home/ubuntu/niers/data/inazuma-cross.jp";
	if (existsSync(crossDir)) {
		const subDirs = readdirSync(crossDir, { withFileTypes: true })
			.filter((dirent) => dirent.isDirectory())
			.map((dirent) => dirent.name);

		for (const dir of subDirs) {
			const dirPath = join(crossDir, dir);
			const metaPath = join(dirPath, "meta.json");
			const htmlPath = join(dirPath, "index.html");

			if (existsSync(metaPath) && existsSync(htmlPath)) {
				try {
					const meta = JSON.parse(readFileSync(metaPath, "utf-8"));
					items.push({
						id: `cross-${dir}`,
						type: "cross" as any,
						title: meta.title || "Inazuma Eleven: Cross",
						url: meta.url || "",
						date: meta.date || "",
						htmlPath,
						metadata: {
							id: dir,
							category: meta.category || "Cross Mobile",
							language: meta.language || dir,
						},
					});
				} catch (err) {
					console.error(`[RAG Sync] Erreur de lecture de meta pour Cross ${dir} :`, err);
				}
			}
		}
	}

	// 5. Collecter les articles LEVEL-5 (level5)
	const level5Dir = "/home/ubuntu/niers/data/level5.co.jp/inazuma";
	if (existsSync(level5Dir)) {
		const subDirs = readdirSync(level5Dir, { withFileTypes: true })
			.filter((dirent) => dirent.isDirectory())
			.map((dirent) => dirent.name);

		for (const dir of subDirs) {
			const dirPath = join(level5Dir, dir);
			const metaPath = join(dirPath, "meta.json");
			const htmlPath = join(dirPath, "index.html");

			if (existsSync(metaPath) && existsSync(htmlPath)) {
				try {
					const meta = JSON.parse(readFileSync(metaPath, "utf-8"));
					items.push({
						id: `level5-${dir}`,
						type: "level5" as any,
						title: meta.title || "LEVEL-5 Announcement",
						url: meta.url || "",
						date: meta.date || "",
						htmlPath,
						metadata: {
							id: dir,
							category: meta.category || "LEVEL-5 Press Release",
							language: meta.language || "ja",
						},
					});
				} catch (err) {
					console.error(`[RAG Sync] Erreur de lecture de meta pour Level5 ${dir} :`, err);
				}
			}
		}
	}

	// 6. Collecter les articles du blog LEVEL-5 (ja/en/zh-tw/zh-cn)
	const blogDir = "/home/ubuntu/niers/data/level5.co.jp/blog";
	const blogLocales = ["ja", "en", "zh-tw", "zh-cn"] as const;
	for (const locale of blogLocales) {
		const localeBlogDir = join(blogDir, locale);
		if (existsSync(localeBlogDir)) {
			const subDirs = readdirSync(localeBlogDir, { withFileTypes: true })
				.filter((dirent) => dirent.isDirectory())
				.map((dirent) => dirent.name);

			for (const dir of subDirs) {
				const dirPath = join(localeBlogDir, dir);
				const metaPath = join(dirPath, "meta.json");
				const htmlPath = join(dirPath, "index.html");

				if (existsSync(metaPath) && existsSync(htmlPath)) {
					try {
						const meta = JSON.parse(readFileSync(metaPath, "utf-8"));
						items.push({
							id: `level5-blog-${locale}-${dir}`,
							type: "level5" as any,
							title: meta.title || "LEVEL-5 Blog Post",
							url: meta.url || "",
							date: meta.date || "",
							htmlPath,
							metadata: {
								id: dir,
								category: meta.category || "LEVEL-5 Developer Blog",
								language: locale,
							},
						});
					} catch (err) {
						console.error(
							`[RAG Sync] Erreur de lecture de meta pour LEVEL-5 Blog ${dir} [${locale}] :`,
							err
						);
					}
				}
			}
		}
	}

	// 7. Collecter les documentations du monorepo (docs/*.md et docs/reference/*.md)
	const docsDir = dansLeDepot("docs");
	const docPaths: { path: string; isReference: boolean }[] = [];

	if (existsSync(docsDir)) {
		const rootFiles = readdirSync(docsDir, { withFileTypes: true })
			.filter(
				(dirent) =>
					!dirent.isDirectory() && (dirent.name.endsWith(".md") || dirent.name.endsWith(".mdx"))
			)
			.map((dirent) => ({ path: join(docsDir, dirent.name), isReference: false }));
		docPaths.push(...rootFiles);

		const refDir = join(docsDir, "reference");
		if (existsSync(refDir)) {
			const refFiles = readdirSync(refDir, { withFileTypes: true })
				.filter(
					(dirent) =>
						!dirent.isDirectory() && (dirent.name.endsWith(".md") || dirent.name.endsWith(".mdx"))
				)
				.map((dirent) => ({ path: join(refDir, dirent.name), isReference: true }));
			docPaths.push(...refFiles);
		}
	}

	for (const docInfo of docPaths) {
		const filePath = docInfo.path;
		const file = filePath.split("/").pop() || "";
		if (file === "llms.txt" || file === "llms-full.txt") continue;

		try {
			const rawContent = readFileSync(filePath, "utf-8");
			const { data } = parseFrontmatter(rawContent);

			items.push({
				id: `doc-${file}`,
				type: "doc",
				title: (data.title as string) || file,
				url: docInfo.isReference ? `file:///docs/reference/${file}` : `file:///docs/${file}`,
				date: (data.last_updated as string) || "",
				htmlPath: filePath,
				metadata: {
					id: file,
					category: data.scope ? (data.scope as string[]).join(", ") : (docInfo.isReference ? "Reference Doc" : "Monorepo Doc"),
					language: "fr",
				},
			});
		} catch (err) {
			console.error(`[RAG Sync] Erreur de lecture de doc ${file} :`, err);
		}
	}

	// 8. Collecter les posts Reddit (reddit)
	const redditRoot = "/home/ubuntu/niers/data/reddit";
	if (existsSync(redditRoot)) {
		const subDirs = readdirSync(redditRoot, { withFileTypes: true })
			.filter((dirent) => dirent.isDirectory())
			.map((dirent) => dirent.name);

		for (const dir of subDirs) {
			const dirPath = join(redditRoot, dir);
			const metaPath = join(dirPath, "meta.json");
			const htmlPath = join(dirPath, "index.html");

			if (existsSync(metaPath) && existsSync(htmlPath)) {
				try {
					const meta = JSON.parse(readFileSync(metaPath, "utf-8"));
					items.push({
						id: `reddit-${dir}`,
						type: "reddit" as any,
						title: meta.title || "Post Reddit",
						url: meta.url || "",
						date: meta.date || "",
						htmlPath,
						metadata: {
							id: dir,
							category: "Reddit",
							language: meta.language || "en",
							author: meta.author || "",
							score: meta.score || 0,
							num_comments: meta.num_comments || 0,
						},
					});
				} catch (err) {
					console.error(`[RAG Sync] Erreur de lecture de meta pour Reddit ${dir} :`, err);
				}
			}
		}
	}

	// 9. Collecter les discussions Discord (discord)
	const discordRoot = "/home/ubuntu/niers/data/discord";
	if (existsSync(discordRoot)) {
		const subDirs = readdirSync(discordRoot, { withFileTypes: true })
			.filter((dirent) => dirent.isDirectory())
			.map((dirent) => dirent.name);

		for (const dir of subDirs) {
			const dirPath = join(discordRoot, dir);
			const metaPath = join(dirPath, "meta.json");
			const htmlPath = join(dirPath, "index.html");

			if (existsSync(metaPath) && existsSync(htmlPath)) {
				try {
					const meta = JSON.parse(readFileSync(metaPath, "utf-8"));
					items.push({
						id: `discord-${dir}`,
						type: "discord" as any,
						title: meta.title || "Discussion Discord",
						url: meta.url || "",
						date: meta.date || "",
						htmlPath,
						metadata: {
							id: dir,
							category: "Discord",
							language: meta.language || "fr",
							channelName: meta.channelName || "",
						},
					});
				} catch (err) {
					console.error(`[RAG Sync] Erreur de lecture de meta pour Discord ${dir} :`, err);
				}
			}
		}
	}

	return items;
}



/**
 * Synchronise et vectorise tous les nouveaux documents ou documents modifiés dans Redis.
 */
export async function runRagSync(): Promise<{
	success: boolean;
	processed: number;
	errors: number;
}> {
	console.log("[RAG Sync] Lancement de la synchronisation vectorielle...");

	let processed = 0;
	let errors = 0;

	try {
		// 0. Cœur du RAG : les personnages du Zukan officiel (mirror corrigé).
		const zukan = await indexZukanCharacters();
		processed += zukan.processed;
		errors += zukan.errors;

		const items = collectScrapedItems();
		console.log(`[RAG Sync] ${items.length} documents scrapés trouvés localement.`);

		for (const item of items) {
			try {
				const fileContent = readFileSync(item.htmlPath, "utf-8");
				let cleanText = "";
				if (item.type === "doc") {
					const { content } = parseFrontmatter(fileContent);
					cleanText = content;
				} else {
					cleanText = cleanHtmlToMarkdown(fileContent);
				}

				if (cleanText.length < 50) {
					// Document vide ou invalide
					continue;
				}

				// Ajout d'une entête descriptive pour enrichir le contexte RAG
				const formattedDocumentText = `Titre: ${item.title}\nDate: ${item.date}\nLangue: ${item.metadata.language || "unknown"}\nType: ${item.type}\nContenu:\n${cleanText}`;
				const contentHash = computeStringHash(formattedDocumentText);

				const shouldSync = await crawlerTracker.shouldProcess(item.id, contentHash);

				if (!shouldSync) {
					// Déjà indexé avec ce contenu exact
					continue;
				}

				console.log(`[RAG Sync] Vectorisation de : ${item.title} (${item.type})`);

				// Supprimer préventivement l'ancien document et tous les chunks possibles en une seule opération
				const idsToDelete = [item.id];
				for (let i = 0; i < 50; i++) {
					idsToDelete.push(`${item.id}#chunk-${i}`);
				}
				await vectorStore.deleteDocuments("inazuma-news", idsToDelete);

				// Découper le texte nettoyé en chunks
				const kind: SourceKind = item.type === "doc" ? "doc" : "web";
				const textChunks = chunkBySource(kind, cleanText);
				console.log(`[RAG Sync] -> Découpé en ${textChunks.length} chunks.`);

				for (let i = 0; i < textChunks.length; i++) {
					const chunk = textChunks[i]!;
					const formattedChunkText = `Titre: ${item.title}\nDate: ${item.date}\nLangue: ${item.metadata.language || "unknown"}\nType: ${item.type} [Partie ${i + 1}/${textChunks.length}]\nContenu:\n${chunk.content}`;

					// Génération de l'embedding pour le chunk
					const embedding = await getEmbedding(formattedChunkText);

					// Sauvegarde dans le vectorStore Redis
					await vectorStore.saveDocument("inazuma-news", {
						id: `${item.id}#chunk-${i}`,
						text: formattedChunkText,
						embedding,
						metadata: {
							title: item.title,
							url: item.url,
							date: item.date,
							type: item.type,
							chunkIndex: i,
							totalChunks: textChunks.length,
							symbol: chunk.symbol ?? null,
							...item.metadata,
						},
					});
				}

				// Enregistrer le hash du document global pour marquer le document comme traité
				await crawlerTracker.markProcessed(item.id, contentHash);
				processed++;
			} catch (err) {
				console.error(`[RAG Sync] Erreur lors de la vectorisation de ${item.title} :`, err);
				errors++;
			}
		}

		// 7. Collecter et synchroniser les tweets depuis Supabase
		console.log("[RAG Sync] Récupération des tweets depuis la base de données...");
		let dbTweets: DBTweetRow[] = [];
		try {
			dbTweets = await sql<DBTweetRow[]>`
				SELECT id, author_username, author_name, text, created_at, metrics, media, translation, category
				FROM tweets
			`;
		} catch (err) {
			console.error("[RAG Sync] Exception lors de la récupération des tweets :", err);
			errors++;
		}

		console.log(`[RAG Sync] ${dbTweets.length} tweets trouvés dans la base de données.`);

		for (const tweet of dbTweets) {
			try {
				const username = tweet.author_username || "unknown";
				const displayName = tweet.author_name || username;
				const text = tweet.text || "";

				if (text.length < 20) {
					// Tweet trop court, pas besoin de l'indexer
					continue;
				}

				// Ajout d'une entête descriptive pour enrichir le contexte RAG
				const title = `Tweet de ${displayName} (@${username})`;
				const url = `https://x.com/${username}/status/${tweet.id}`;
				const date = tweet.created_at || "";

				const formattedTweetText = `Auteur: @${username} (${displayName})\nDate: ${date}\nType: tweet\nContenu:\n${text}`;
				const contentHash = computeStringHash(formattedTweetText);

				const itemId = `tweet-${tweet.id}`;
				const shouldSync = await crawlerTracker.shouldProcess(itemId, contentHash);

				if (!shouldSync) {
					// Déjà indexé avec ce contenu exact
					continue;
				}

				console.log(`[RAG Sync] Vectorisation du tweet : @${username} - ${tweet.id}`);

				// Supprimer préventivement l'ancien document et tous les chunks possibles
				const idsToDelete = [itemId];
				for (let i = 0; i < 50; i++) {
					idsToDelete.push(`${itemId}#chunk-${i}`);
				}
				await vectorStore.deleteDocuments("inazuma-news", idsToDelete);

				// Découper le texte du tweet en chunks (normalement 1 seul chunk pour la plupart des tweets)
				const textChunks = chunkText(text, 800, 150);
				console.log(`[RAG Sync] -> Tweet découpé en ${textChunks.length} chunks.`);

				for (let i = 0; i < textChunks.length; i++) {
					const chunkTextContent = textChunks[i] || "";
					const formattedChunkText = `Auteur: @${username} (${displayName})\nDate: ${date}\nType: tweet [Partie ${i + 1}/${textChunks.length}]\nContenu:\n${chunkTextContent}`;

					// Génération de l'embedding pour le chunk
					const embedding = await getEmbedding(formattedChunkText);

					// Sauvegarde dans le vectorStore Redis
					await vectorStore.saveDocument("inazuma-news", {
						id: `${itemId}#chunk-${i}`,
						text: formattedChunkText,
						embedding,
						metadata: {
							title,
							url,
							date,
							type: "tweet",
							chunkIndex: i,
							totalChunks: textChunks.length,
							author_username: username,
							author_name: displayName,
						},
					});
				}

				// Enregistrer le hash du document global pour marquer le document comme traité
				await crawlerTracker.markProcessed(itemId, contentHash);
				processed++;
			} catch (err) {
				console.error(`[RAG Sync] Erreur lors de la vectorisation du tweet ${tweet.id} :`, err);
				errors++;
			}
		}

		// 7b. Branche X/tweets → RAG pgvector (additif, multi-comptes, idempotent par tweet id, chunker enrichi).
		// Utilise chunkTweet (auteur/date/métriques/médias) → embed (BGE-M3 / OpenAI / hash 1024) → upsert inagle_rag_chunks.
		// La table tweets est déjà multi-comptes (via crawlTwitter + x-accounts). Le Redis ancien reste en // (compat).
		try {
			const tweetSources: RagSource[] = dbTweets
				.filter((t) => (t.text || "").length >= 20)
				.map((tweet) => {
					const username = tweet.author_username || "unknown";
					const displayName = tweet.author_name || username;
					const date = tweet.created_at || "";
					const url = `https://x.com/${username}/status/${tweet.id}`;
					// Passe structuré (JSON) → chunkTweet gère et enrichit (métriques, media urls, etc).
					const rawForChunk = JSON.stringify({
						id: tweet.id,
						text: tweet.text || "",
						author_username: username,
						author_name: displayName,
						created_at: date,
						metrics: (tweet as any).metrics || null,
						media: (tweet as any).media || null,
						url,
					});
					return {
						sourceId: `tweet-${tweet.id}`,
						kind: "tweet" as const,
						raw: rawForChunk,
						title: `Tweet de ${displayName} (@${username})`,
						url,
						lang: undefined,
						meta: {
							author_username: username,
							author_name: displayName,
							date,
							category: (tweet as any).category || null,
							has_translation: !!(tweet as any).translation,
						},
					};
				});
			if (tweetSources.length > 0) {
				console.log(`[RAG PG] Ingestion ${tweetSources.length} tweets (chunker tweet enrichi + pgvector)...`);
				const pgStats = await ingestSources(tweetSources);
				console.log(
					`[RAG PG] Tweets pgvector: ${pgStats.chunks} chunks (${pgStats.sources} sources, backend=${pgStats.backend}, skipped=${pgStats.skipped}).`
				);
			}
		} catch (pgErr) {
			console.warn(
				"[RAG PG] Ingestion tweets pgvector échouée (Redis inchangé):",
				pgErr instanceof Error ? pgErr.message : pgErr
			);
		}

		console.log(
			`[RAG Sync] Fin de la synchronisation. ${processed} documents vectorisés, ${errors} erreurs.`
		);
		return { success: true, processed, errors };
	} catch (err) {
		console.error("[RAG Sync] Erreur critique lors de la synchronisation RAG :", err);
		return { success: false, processed, errors };
	}
}

/**
 * Interroge le RAG en cherchant les documents les plus proches sémantiquement.
 */
export async function queryRag(question: string, limit = 4) {
	console.log(`[RAG Query] Recherche sémantique pour : "${question}"`);

	// Générer l'embedding de la question
	const questionEmbedding = await getEmbedding(question);

	// Rechercher dans la collection
	const results = await vectorStore.search("inazuma-news", questionEmbedding, limit, question);

	return results.map((r) => ({
		id: r.document.id,
		title: r.document.metadata.title,
		date: r.document.metadata.date,
		url: r.document.metadata.url,
		type: r.document.metadata.type,
		text: r.document.text,
		score: r.score,
	}));
}
