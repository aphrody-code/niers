/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { XClient, XSession, type Tweet } from "@aphrody-code/x";
import { createSupabaseServiceClient } from "@rosegriffon/db/service";
import type { Json } from "@rosegriffon/db";
import { sql } from "../../lib/db.js";
import { getOfficialXAccounts, loadBxcXCookieHeader } from "./x-accounts";

type MediaItem = {
	id: string;
	type: string;
	url?: string;
	original_url?: string;
	video_url?: string;
	preview_url?: string;
	width?: number;
	height?: number;
	[key: string]: string | number | boolean | null | undefined;
};

type RawMediaItem = {
	id?: string;
	type?: string;
	url?: string;
	original_url?: string;
	video_url?: string;
	preview_url?: string;
	width?: number;
	height?: number;
};

// Normalise un media de la lib X (preview_url/video_url/width/height) vers le
// format DB, en propageant l'URL téléchargeable principale + la miniature.
function mapMedia(m: {
	id?: string;
	type?: string;
	url?: string;
	video_url?: string;
	preview_url?: string;
	width?: number;
	height?: number;
}): MediaItem {
	const out: MediaItem = {
		id: m.id ?? "",
		type: m.type ?? "",
		url: m.url,
		original_url: m.url,
	};
	if (m.video_url) out.video_url = m.video_url;
	if (m.preview_url) out.preview_url = m.preview_url;
	if (typeof m.width === "number") out.width = m.width;
	if (typeof m.height === "number") out.height = m.height;
	return out;
}

// Décode la vraie date de publication depuis l'ID Snowflake Twitter
// (epoch Twitter = 1288834974657 ms). Utilisé en repli quand created_at
// est absent de l'API, pour ne JAMAIS retomber sur la date de crawl.
function snowflakeToIso(id: string): string {
	const ms = (BigInt(id) >> 22n) + 1288834974657n;
	return new Date(Number(ms)).toISOString();
}

type DBTweetRow = {
	id: string;
	author_id: string;
	author_name: string;
	author_username: string;
	created_at: string;
	text: string;
	is_thread: boolean;
	tweet_count: number;
	metrics: Json | null;
	media: Json | null;
	quoted_tweets: Json | null;
	raw_tweets: Json | null;
	updated_at: string;
};

export async function crawlTwitter(options: {
	comptes?: readonly string[];
	parCompte?: number;
} = {}): Promise<{
	success: boolean;
	count?: number;
	error?: string;
}> {
	// Multi-comptes officiels (additif, source x-accounts.ts — mis à jour après discovery bxc x + search).
	// Inclut les comptes validés (principaux + fallback low-activity pour complétude historique).
	// Par défaut, tous les comptes officiels. La veille horaire, elle, ne demande
	// QUE le dernier post d'`@Azalee_IE` : l'archive des autres comptes est déjà
	// en base, et re-parcourir vingt timelines toutes les heures ne ferait que
	// consommer le quota X pour ré-écrire des lignes identiques.
	const screenNames = options.comptes ? [...options.comptes] : getOfficialXAccounts();
	console.log(
		`[Crawl Twitter] Démarrage de la synchronisation des tweets de : ${screenNames.join(", ")}...`
	);

	let totalCount = 0;
	let hasErrors = false;

	try {
		// Utilise pleinement le cookie bxc (~/.bxc/cookies/xcom.json) si présent (prioritaire),
		// fallback sur loadOrEnv ( .aphrody ou X_* env) — supporte le flux CLI bxc x.
		let session: XSession;
		const bxcCookie = loadBxcXCookieHeader();
		if (bxcCookie) {
			session = XSession.fromCookieString(bxcCookie);
		} else {
			session = XSession.loadOrEnv();
		}
		const client = new XClient(session);
		const supabase = createSupabaseServiceClient();

		// Charger le cache d'IDs depuis la DB pour éviter les requêtes API userByScreenName inutiles
		const usernameToUserMap = new Map<string, { id: string; name: string }>();
		try {
			const existingTweets = await sql<
				{ author_id: string | null; author_name: string | null; author_username: string | null }[]
			>`SELECT author_id, author_name, author_username FROM tweets`;
			for (const t of existingTweets) {
				if (t.author_username && t.author_id) {
					usernameToUserMap.set(t.author_username.toLowerCase(), {
						id: t.author_id,
						name: t.author_name || t.author_username,
					});
				}
			}
			console.log(`[Crawl Twitter] Cache d'IDs chargé depuis la DB (${usernameToUserMap.size} profils trouvés).`);
		} catch (dbErr) {
			console.warn("[Crawl Twitter] Impossible de charger le cache d'IDs depuis la DB:", dbErr);
		}

		for (const screenName of screenNames) {
			try {
				let user = usernameToUserMap.get(screenName.toLowerCase());
			if (!user) {
				console.log(`[Crawl Twitter] Résolution du profil @${screenName} via API X...`);
				try {
					const apiUser = await Promise.race([
						client.userByScreenName(screenName),
						new Promise<any>((_, rej) =>
							setTimeout(() => rej(new Error("userByScreenName() timeout")), 20000)
						),
					]);
					if (apiUser && apiUser.id) {
						user = {
							id: apiUser.id,
							name: apiUser.name || screenName,
						};
						usernameToUserMap.set(screenName.toLowerCase(), user);
						console.log(`[Crawl Twitter] ID résolu pour @${screenName} via API X : ${user.id} (${user.name})`);
					}
				} catch (profileErr) {
					console.error(
						`[Crawl Twitter] Erreur lors de la résolution de @${screenName} via API X :`,
						profileErr
					);
				}
			} else {
				console.log(`[Crawl Twitter] Profil @${screenName} résolu via le cache DB : ${user.id} (${user.name})`);
			}

			if (!user) {
				console.warn(`[Crawl Twitter] Impossible de résoudre l'ID de @${screenName}`);
				hasErrors = true;
				continue;
			}

				console.log(`[Crawl Twitter] Crawl historique complet de @${screenName}...`);
				// rg cron + bxc x : MAX plus élevé pour AkihiroHino (CEO Level-5, tweets clés pour Azalée wiki + RAG).
				// "peuple tout les tweets" via pagination userTweets + bxc cookie natif.
				// `parCompte` borne la moisson : la veille horaire ne demande qu'UN
				// post (le dernier), là où un rattrapage d'archive en demande 1200.
				// Sans cette borne, la veille repaginait toute la timeline chaque
				// heure — vingt pages d'API X pour, au mieux, une ligne nouvelle.
				const MAX_TWEETS_PER_ACCOUNT =
					options.parCompte && options.parCompte > 0
						? options.parCompte
						: screenName === "AkihiroHino"
							? 5000
							: 1200;
				const fetchedTweets: Tweet[] = [];
				const seenTimelineIds = new Set<string>();
				let cursor: string | undefined;
				let pageIndex = 0;

				while (fetchedTweets.length < MAX_TWEETS_PER_ACCOUNT) {
					// L'API X throw (rate-limit / erreur transitoire) après N pages.
					// On CONSERVE les tweets déjà accumulés : break, pas de remontée
					// au catch du compte (qui jetterait tout l'historique récupéré).
					let page;
					try {
						// Timeout dur : un backoff interne de userTweets ne doit pas
						// bloquer le crawl indéfiniment (sinon hang sans throw).
						page = await Promise.race([
							client.userTweets(user.id, Math.min(100, MAX_TWEETS_PER_ACCOUNT), cursor),
							new Promise<never>((_, rej) =>
								setTimeout(() => rej(new Error("userTweets() timeout")), 20000)
							),
						]);
					} catch (pageErr) {
						console.warn(
							`[Crawl Twitter] @${screenName} : arrêt pagination après ${pageIndex} page(s) (erreur API, ${fetchedTweets.length} tweets conservés) :`,
							pageErr instanceof Error ? pageErr.message : pageErr
						);
						break;
					}
					const pageTweets = page.tweets || [];

					let addedThisPage = 0;
					for (const t of pageTweets) {
						if (!seenTimelineIds.has(t.id)) {
							seenTimelineIds.add(t.id);
							fetchedTweets.push(t);
							addedThisPage++;
						}
					}

					pageIndex++;
					console.log(
						`[Crawl Twitter] @${screenName} page ${pageIndex} : ${pageTweets.length} reçus, ${addedThisPage} nouveaux (total ${fetchedTweets.length}).`
					);

					// Arrêt : plus de cursor, ou page sans nouveau tweet (anti-boucle).
					if (!page.next_cursor || addedThisPage === 0) {
						break;
					}
					cursor = page.next_cursor;
					// Throttle léger pour limiter le rate-limit X sur l'historique.
					await new Promise((r) => setTimeout(r, 700));
				}

				console.log(
					`[Crawl Twitter] ${fetchedTweets.length} tweets accumulés au total pour @${screenName}.`
				);
				if (screenName === "AkihiroHino") {
					console.log(`[AkihiroHino] Peuplement tweets CEO Level-5 via rg-cron + bxc x (historique étendu, pour sync Azalée + RAG + traductions).`);
				}

				const nonRetweets = fetchedTweets.filter((t) => !t.text.startsWith("RT @"));
				console.log(`[Crawl Twitter] ${nonRetweets.length} tweets après filtrage des RTs.`);

				const conversations = new Map<string, Tweet[]>();
				for (const t of nonRetweets) {
					const convId = t.conversation_id || t.id;
					if (!conversations.has(convId)) {
						conversations.set(convId, []);
					}
					conversations.get(convId)!.push(t);
				}

				const finalTweetsToUpsert: DBTweetRow[] = [];
				const deleteOldTweetIds: string[] = [];
				// Une fois rate-limité sur TweetDetail, on arrête d'appeler thread()
				// pour ce compte (sinon backoffs 429 en cascade qui bloquent le crawl)
				// et on se rabat sur le grouping timeline, déjà complet sur la fenêtre.
				let threadFetchDisabled = false;
				// Idem pour les getTweet() de vérification d'origine de conversation.
				let originCheckDisabled = false;

				for (const [convId, group] of conversations.entries()) {
					group.sort((a, b) => a.id.localeCompare(b.id));

					let isUserThread = group.some((t) => t.id === convId);

					if (!isUserThread && group.length > 0) {
						const dbHeadRows = await sql<
							{ author_username: string | null }[]
						>`SELECT author_username FROM tweets WHERE id = ${convId} LIMIT 1`;
						const dbHead = dbHeadRows[0];

						if (dbHead) {
							isUserThread = dbHead.author_username?.toLowerCase() === screenName.toLowerCase();
						} else if (!originCheckDisabled) {
							try {
								const headTweet = await Promise.race([
									client.getTweet(convId),
									new Promise<never>((_, rej) =>
										setTimeout(() => rej(new Error("getTweet() timeout")), 10000)
									),
								]);
								if (headTweet) {
									isUserThread =
										headTweet.author.username.toLowerCase() === screenName.toLowerCase();
								}
							} catch (apiErr) {
								// Coupe les vérifications d'origine après le 1er échec (429/timeout).
								originCheckDisabled = true;
								console.warn(
									`[Crawl Twitter] Origine conversation ${convId} indisponible (${apiErr instanceof Error ? apiErr.message : String(apiErr)}) — vérifs d'origine désactivées pour @${screenName}.`
								);
							}
						}
					}

					const filteredGroup = group.filter((t) => {
						if (!t.in_reply_to_status_id) {
							return true;
						}
						return isUserThread;
					});

					if (filteredGroup.length === 0) continue;

					const isThread = isUserThread && filteredGroup.length > 1;

					if (isThread) {
						// Récupère TOUS les tweets du thread (au-delà des 100 de timeline)
						// via TweetDetail paginé, puis fusionne avec la timeline.
						let threadGroup = filteredGroup;
						if (!threadFetchDisabled) {
							try {
								const threadTweets: Tweet[] = [];
								let threadCursor: string | undefined;
								const THREAD_MAX_PAGES = 3;
								for (let p = 0; p < THREAD_MAX_PAGES; p++) {
									// Timeout dur : un appel thread() ne doit jamais bloquer le crawl.
									const threadPage = await Promise.race([
										client.thread(convId, threadCursor),
										new Promise<never>((_, rej) =>
											setTimeout(() => rej(new Error("thread() timeout")), 12000)
										),
									]);
									threadTweets.push(...(threadPage.tweets || []));
									if (!threadPage.next_cursor) break;
									threadCursor = threadPage.next_cursor;
								}

								// Fusion timeline + thread, garde uniquement les tweets de l'auteur.
								const merged = new Map<string, Tweet>();
								for (const item of filteredGroup) {
									merged.set(item.id, item);
								}
								for (const item of threadTweets) {
									if (item.author.username.toLowerCase() === screenName.toLowerCase()) {
										if (!merged.has(item.id)) {
											merged.set(item.id, item);
										}
									}
								}
								threadGroup = Array.from(merged.values()).sort((a, b) => a.id.localeCompare(b.id));
								console.log(
									`[Crawl Twitter] Thread ${convId} : ${threadGroup.length} tweets après fusion timeline+thread.`
								);
							} catch (threadErr) {
								// 1er échec (429/timeout) → coupe le fetch thread pour le reste
								// du compte et repli sur le grouping timeline.
								threadFetchDisabled = true;
								console.warn(
									`[Crawl Twitter] Thread complet ${convId} indisponible (${threadErr instanceof Error ? threadErr.message : String(threadErr)}) — fetch thread désactivé pour @${screenName}, repli timeline.`
								);
								threadGroup = filteredGroup;
							}
						}

						// `filteredGroup` est non vide (testé plus haut) et la fusion ne
						// retire rien — mais l'indexation reste `T | undefined` pour le
						// compilateur, et les trente lectures qui suivent partaient donc
						// sur une valeur non vérifiée. Le garde est explicite plutôt
						// qu'assumé : un thread vide se saute, il ne plante pas le crawl.
						const lastTweet = threadGroup.at(-1);
						const firstTweet = threadGroup[0];
						if (!lastTweet || !firstTweet) continue;

						const createdAtIso = lastTweet.created_at
							? new Date(lastTweet.created_at).toISOString()
							: snowflakeToIso(lastTweet.id);

						const allThreadMedia: MediaItem[] = [];
						const seenMediaUrls = new Set<string>();

						for (const item of threadGroup) {
							const itemMedia = (item.media || []).map((m: RawMediaItem) => mapMedia(m));
							for (const m of itemMedia) {
								if (m.url && !seenMediaUrls.has(m.url)) {
									allThreadMedia.push(m);
									seenMediaUrls.add(m.url);
								}
							}
						}

						const tweetRow: DBTweetRow = {
							id: lastTweet.id,
							author_id: lastTweet.author_id || user.id,
							author_name: lastTweet.author.name || user.name,
							author_username: lastTweet.author.username || screenName,
							created_at: createdAtIso,
							text: firstTweet.text, // Introductory text of first tweet
							is_thread: true,
							tweet_count: threadGroup.length,
							metrics: {
								reply_count: lastTweet.reply_count,
								retweet_count: lastTweet.retweet_count,
								like_count: lastTweet.like_count,
								quote_count: lastTweet.quote_count,
								view_count: lastTweet.view_count,
							},
							media: allThreadMedia,
							quoted_tweets: lastTweet.quoted_tweet
								? JSON.parse(JSON.stringify(lastTweet.quoted_tweet))
								: null,
							raw_tweets: threadGroup.map((item) => ({
								id: item.id,
								text: item.text,
								fullText: item.text,
								created_at: item.created_at,
								in_reply_to_status_id: item.in_reply_to_status_id,
								media: (item.media || []).map((m: RawMediaItem) => mapMedia(m)),
							})),
							updated_at: new Date().toISOString(),
						};

						finalTweetsToUpsert.push(tweetRow);

						// Clean up duplicate intermediate tweets of the thread
						for (const item of threadGroup) {
							if (item.id !== lastTweet.id) {
								deleteOldTweetIds.push(item.id);
							}
						}
					} else {
						// Même garde que pour le fil : `filteredGroup` est non vide, mais
						// le compilateur ne le sait pas et les vingt lectures qui suivent
						// s'appuyaient dessus sans l'avoir vérifié.
						const t = filteredGroup[0];
						if (!t) continue;
						const createdAtIso = t.created_at
							? new Date(t.created_at).toISOString()
							: snowflakeToIso(t.id);

						const tMedia = (t.media || []).map((m: RawMediaItem) => mapMedia(m));

						const tweetRow: DBTweetRow = {
							id: t.id,
							author_id: t.author_id || user.id,
							author_name: t.author.name || user.name,
							author_username: t.author.username || screenName,
							created_at: createdAtIso,
							text: t.text,
							is_thread: false,
							tweet_count: 1,
							metrics: {
								reply_count: t.reply_count,
								retweet_count: t.retweet_count,
								like_count: t.like_count,
								quote_count: t.quote_count,
								view_count: t.view_count,
							},
							media: tMedia,
							quoted_tweets: t.quoted_tweet ? JSON.parse(JSON.stringify(t.quoted_tweet)) : null,
							raw_tweets: null,
							updated_at: new Date().toISOString(),
						};

						finalTweetsToUpsert.push(tweetRow);
					}
				}

				if (finalTweetsToUpsert.length > 0) {
					// Deduplicate by ID to prevent ON CONFLICT DO UPDATE constraints error
					let uniqueTweets = Array.from(
						new Map(finalTweetsToUpsert.map((item) => [item.id, item])).values()
					);

					// Anti-régression : la timeline collapse les longs self-threads à ~5
					// tweets, et thread() peut être coupé sur 429 (threadFetchDisabled) →
					// on n'écrit alors qu'un sous-ensemble. NE JAMAIS écraser un thread
					// déjà complet en base par une version plus pauvre : on compare le
					// tweet_count existant et on saute l'upsert s'il régresse.
					const existingCounts = new Map<string, number>();
					try {
						const existingRows = await sql<
							{ id: string; tweet_count: number | null }[]
						>`SELECT id, tweet_count FROM tweets WHERE id IN ${sql(uniqueTweets.map((t) => t.id))}`;
						for (const r of existingRows) {
							existingCounts.set(r.id, r.tweet_count ?? 0);
						}
					} catch (e) {
						console.warn(
							`[Crawl Twitter] Lecture tweet_count existants impossible (garde anti-régression désactivée) :`,
							e instanceof Error ? e.message : e
						);
					}
					uniqueTweets = uniqueTweets.filter((t) => {
						const prev = existingCounts.get(t.id) ?? 0;
						if (t.is_thread && (t.tweet_count ?? 0) < prev) {
							console.warn(
								`[Crawl Twitter] Thread ${t.id} : version tronquée (${t.tweet_count} tweets) < existante (${prev}) — upsert IGNORÉ (anti-régression).`
							);
							return false;
						}
						return true;
					});
					if (uniqueTweets.length === 0) {
						console.log(
							`[Crawl Twitter] Rien à upserter pour @${screenName} après garde anti-régression.`
						);
					}

					console.log(
						`[Crawl Twitter] Envoi de ${uniqueTweets.length} tweets uniques pour @${screenName} (total brut: ${finalTweetsToUpsert.length})...`
					);
					const { error } =
						uniqueTweets.length > 0
							? await supabase.from("tweets").upsert(uniqueTweets)
							: { error: null };
					if (error) {
						console.error(`[Crawl Twitter] Erreur d'upsert pour @${screenName} :`, error);
						hasErrors = true;
					} else {
						totalCount += uniqueTweets.length;
						if (deleteOldTweetIds.length > 0) {
							console.log(
								`[Crawl Twitter] Nettoyage de ${deleteOldTweetIds.length} doublons de threads de la base de données...`
							);
							const { error: deleteErr } = await supabase
								.from("tweets")
								.delete()
								.in("id", deleteOldTweetIds);
							if (deleteErr) {
								console.warn(
									`[Crawl Twitter] Échec de suppression des anciens tweets de thread :`,
									deleteErr
								);
							}
						}
					}
				} else {
					console.log(`[Crawl Twitter] Aucun nouveau tweet à synchroniser pour @${screenName}.`);
				}
			} catch (profileErr) {
				console.error(
					`[Crawl Twitter] Erreur lors de la synchronisation de @${screenName} :`,
					profileErr
				);
				hasErrors = true;
			}
		}

		console.log(
			`[Crawl Twitter] Synchronisation Twitter terminée. ${totalCount} tweets insérés/mis à jour.`
		);
		return { success: !hasErrors, count: totalCount };
	} catch (err: unknown) {
		const msg = err instanceof Error ? err.message : String(err);
		console.error("[Crawl Twitter] Erreur critique lors de la synchronisation Twitter :", err);
		return { success: false, error: msg };
	}
}
