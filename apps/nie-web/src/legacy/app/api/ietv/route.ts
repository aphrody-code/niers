// `/api/ietv` — le catalogue des épisodes de la série, servi depuis le VPS.
//
// ## Pourquoi cette route existe
//
// L'explorateur embarque `data/anime/episodes.db` dans son installeur : 355 épisodes figés au
// jour du build. La série, elle, continue d'être publiée, et `packages/cron/src/tasks/ietv-cache`
// rafraîchit la base du VPS toutes les nuits. Sans porte de sortie, la seule façon de mettre à
// jour une installation était de réinstaller l'application.
//
// Cette route est cette porte. Elle sert du JSON — pas le fichier SQLite : remplacer sous les
// pieds d'une application une base que `sqlx` tient ouverte est le genre de manœuvre qui ne
// casse qu'une fois sur dix, et jamais sur la machine où on l'a testée. Le client fusionne
// ligne à ligne (`INSERT … ON CONFLICT DO UPDATE`) et garde la main.
//
// ## Delta
//
// `?since=<epoch ms>` ne rend que ce qui a été moissonné après cette date (`createdAt`). Un
// client à jour reçoit alors un tableau vide et quelques centaines d'octets. Sans `since`, la
// route rend tout le catalogue (355 lignes, ~180 Ko).
//
// ## Lecture seule, et sous Bun
//
// `apps/azalee` démarre par `bun --bun next start` : `bun:sqlite` est donc disponible sans
// ajouter une seule dépendance — ce qui compte dans un dépôt où une version en dur casse la
// résolution du catalogue (cf. CLAUDE.md). L'import est dynamique et opaque à l'analyse statique
// pour que le bundler ne cherche pas à résoudre un module qui n'existe que dans Bun.
import { NextResponse } from "next/server";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

/** Où vit la base moissonnée par `ietv-cache`. Le service la pose sous `~/.cache/ietv`. */
function cheminBase(): string {
	if (process.env.IETV_DB_PATH) return process.env.IETV_DB_PATH;
	const home = process.env.HOME || process.env.USERPROFILE || "";
	return `${home}/.cache/ietv/episodes.db`;
}

interface LigneEpisode {
	id: number;
	saison: number;
	episode: number | null;
	videoId: string;
	titre: string;
	url: string;
	description: string | null;
	titreJp: string | null;
	romaji: string | null;
	vignette: string | null;
	publie: string | null;
	langue: string | null;
	duree: number | null;
	chaine: string;
	moissonne: number;
}

export async function GET(requete: Request) {
	const params = new URL(requete.url).searchParams;
	const depuis = Number(params.get("since") ?? 0);
	const limite = Math.min(Number(params.get("limit") ?? 5000), 20_000);

	let db: { query: (sql: string) => { all: (...args: unknown[]) => unknown[] }; close: () => void };
	try {
		// Expression opaque : `await import("bun:sqlite")` littéral ferait échouer la compilation
		// Next, qui tente de résoudre le spécificateur au build.
		const nom = "bun:sqlite";
		const { Database } = (await import(/* webpackIgnore: true */ nom)) as {
			Database: new (chemin: string, options: { readonly: boolean }) => typeof db;
		};
		db = new Database(cheminBase(), { readonly: true });
	} catch (e) {
		// Une base absente n'est pas une erreur du client : elle dit que ce serveur-ci ne moissonne
		// pas la série. Le 503 le distingue d'un 500, et le client sait alors ne pas réessayer en
		// boucle.
		return NextResponse.json(
			{ erreur: "catalogue indisponible sur ce serveur", detail: String(e) },
			{ status: 503 }
		);
	}

	try {
		const chaines = db
			.query("SELECT id, channel, title FROM channels ORDER BY id")
			.all() as { id: number; channel: string; title: string | null }[];

		const saisons = db
			.query(
				`SELECT s.channel_id AS chaineId, s.season AS saison,
				        COALESCE(s.name, 'Saison ' || s.season) AS nom,
				        (SELECT count(*) FROM episodes e WHERE e.season = s.season) AS total
				   FROM seasons s ORDER BY s.season`
			)
			.all() as { chaineId: number; saison: number; nom: string; total: number }[];

		const episodes = db
			.query(
				`SELECT e.id, e.season AS saison, e.episode, e.videoId, e.title AS titre, e.url,
				        e.description, e.titleJp AS titreJp, e.romaji, e.thumbnail AS vignette,
				        e.publishDate AS publie, e.language AS langue, e.duration AS duree,
				        c.channel AS chaine, e.createdAt AS moissonne
				   FROM episodes e JOIN channels c ON c.id = e.channel_id
				  WHERE e.createdAt > ?
				  ORDER BY e.season, COALESCE(e.episode, 9999)
				  LIMIT ?`
			)
			.all(depuis, limite) as LigneEpisode[];

		return NextResponse.json(
			{ genere: Date.now(), depuis, chaines, saisons, episodes, total: episodes.length },
			{ headers: { "cache-control": "public, max-age=900" } }
		);
	} finally {
		db.close();
	}
}
