/**
 * Cache SQLite intelligent pour IETV
 *
 * Tables:
 *   - channels (source, title, description, totalEpisodes, lastScrape)
 *   - seasons (channel_id, season, totalEpisodes)
 *   - episodes (channel_id, season, episode, videoId, title, url, description,
 *     thumbnail, publishDate, viewCount, language, duration, quality)
 *   - search_index (videoId, title_fts) pour recherche rapide
 */

import { Database } from "bun:sqlite";
import { existsSync, mkdirSync } from "fs";
import type { ChannelInfo, VideoRef } from "./index";
import {
	LANGUES_SOURCE,
	RE_YOUTUBE_ID,
	reconnaitre,
	type EtatSource,
	type LangueSource,
	type Plateforme,
	type SourceEpisode,
} from "./plateformes";

/**
 * Ramène une valeur de langue à ce que la contrainte `CHECK` accepte.
 *
 * Une langue inconnue devient `unknown` et n'est PAS rejetée : une source qui
 * arrive avec un code exotique doit entrer au catalogue en disant qu'on ne sait
 * pas la nommer, pas faire échouer toute la moisson. C'est la même règle que
 * `langueDeChaine`, qui rend déjà `null` plutôt que de deviner.
 */
function langueValide(valeur: string | null | undefined): LangueSource {
	const code = valeur?.toLowerCase().trim();
	return code && (LANGUES_SOURCE as readonly string[]).includes(code)
		? (code as LangueSource)
		: "unknown";
}

/** `VideoRef` tel que stocké en cache : le nom de chaîne d'origine en plus. */
export type CachedVideoRef = VideoRef & { channel?: string };

/** Filtres acceptés par {@link IETVCache.search}. */
export interface CacheSearchQuery {
	q?: string;
	season?: number;
	episode?: number;
	language?: LangueSource;
	channel?: string;
	limit?: number;
}

/** Compteurs renvoyés par {@link IETVCache.getStats}. */
export interface CacheStats {
	channels: number;
	seasons: number;
	episodes: number;
	byLanguage: Record<string, number>;
	lastUpdate: number;
}

export class IETVCache {
	private db: Database;
	private dbPath: string;

	/**
	 * La table des SOURCES — N façons de regarder un même épisode.
	 *
	 * ── POURQUOI UNE TABLE, ET PAS TROIS COLONNES DE PLUS ──────────────────
	 * `episodes` décrivait à la fois l'ŒUVRE (saison, numéro, titre japonais,
	 * date de diffusion) et sa DIFFUSION (`videoId`, `url`, `thumbnail`,
	 * `quality`). Tant qu'il n'y avait qu'une diffusion par épisode l'amalgame
	 * tenait. Il ne tient plus : la plateforme officielle sert le même épisode
	 * en trois langues, sous trois identifiants, sur deux plateformes
	 * différentes. Ajouter `videoId_en`, `videoId_es` figerait dans le schéma le
	 * nombre de langues connues aujourd'hui.
	 *
	 * `episodes` n'est pas touchée pour autant : l'explorateur
	 * (`apps/inacord/src/lib/animeDb.ts`) lit ses colonnes une par une, et
	 * la vue Cinéma doit continuer de fonctionner sans rien connaître d'ici. La
	 * ligne `episodes` reste la MEILLEURE source de l'épisode ; la table ci-
	 * dessous les porte toutes.
	 *
	 * `verifieeLe IS NULL` est ce qui distingue une source qu'on a lue d'une
	 * source qu'on suppose : sans ce champ, une déduction entrerait en base
	 * avec exactement la même autorité qu'une mesure.
	 */
	static readonly SCHEMA_SOURCES = `
      CREATE TABLE IF NOT EXISTS episode_sources (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        episode_id INTEGER NOT NULL,
        plateforme TEXT NOT NULL CHECK(plateforme IN ('youtube', 'dailymotion', 'page')),
        sourceId TEXT NOT NULL,
        url TEXT NOT NULL,
        langue TEXT NOT NULL CHECK(langue IN ('vo', 'vf', 'vostfr', 'en', 'es', 'de', 'unknown')),
        qualite TEXT,
        officielle INTEGER NOT NULL DEFAULT 1,
        confiance TEXT NOT NULL DEFAULT 'declaree'
          CHECK(confiance IN ('verifiee', 'declaree', 'deduite')),
        verifieeLe INTEGER,
        origine TEXT,
        vignette TEXT,
        titre TEXT,
        createdAt INTEGER DEFAULT (cast(unixepoch() * 1000 as integer)),
        updatedAt INTEGER DEFAULT (cast(unixepoch() * 1000 as integer)),
        FOREIGN KEY(episode_id) REFERENCES episodes(id) ON DELETE CASCADE,
        UNIQUE(episode_id, plateforme, sourceId, langue)
      );

      CREATE INDEX IF NOT EXISTS idx_sources_episode ON episode_sources(episode_id);
      CREATE INDEX IF NOT EXISTS idx_sources_langue ON episode_sources(langue);
      CREATE INDEX IF NOT EXISTS idx_sources_plateforme ON episode_sources(plateforme);
      CREATE INDEX IF NOT EXISTS idx_sources_sourceid ON episode_sources(sourceId);
	`;

	constructor(dbPath = "~/.cache/ietv/episodes.db") {
		this.dbPath = dbPath.replace("~", process.env.HOME || "/root");
		const dir = this.dbPath.substring(0, this.dbPath.lastIndexOf("/"));
		if (!existsSync(dir)) {
			mkdirSync(dir, { recursive: true });
		}

		this.db = new Database(this.dbPath, { create: true });
		this.db.exec("PRAGMA journal_mode = WAL");
		this.db.exec("PRAGMA synchronous = NORMAL");
		this.db.exec("PRAGMA cache_size = -64000");
		this.initSchema();
	}

	private initSchema() {
		this.db.exec(`
      CREATE TABLE IF NOT EXISTS channels (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        channel TEXT UNIQUE NOT NULL,
        title TEXT,
        description TEXT,
        avatar TEXT,
        totalEpisodes INTEGER DEFAULT 0,
        lastScrape INTEGER DEFAULT 0,
        createdAt INTEGER DEFAULT (cast(unixepoch() * 1000 as integer)),
        updatedAt INTEGER DEFAULT (cast(unixepoch() * 1000 as integer))
      );

      CREATE TABLE IF NOT EXISTS seasons (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        channel_id INTEGER NOT NULL,
        season INTEGER NOT NULL,
        name TEXT,
        totalEpisodes INTEGER DEFAULT 0,
        createdAt INTEGER DEFAULT (cast(unixepoch() * 1000 as integer)),
        FOREIGN KEY(channel_id) REFERENCES channels(id) ON DELETE CASCADE,
        UNIQUE(channel_id, season)
      );

      CREATE TABLE IF NOT EXISTS episodes (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        channel_id INTEGER NOT NULL,
        season INTEGER NOT NULL,
        episode INTEGER NOT NULL,
        -- PAS d'unicité sur videoId : la même vidéo YouTube est référencée par
        -- le site officiel ET par une chaîne. La clé est le quadruplet plus
        -- bas ; unique ici, « INSERT OR REPLACE » faisait qu'une source
        -- effaçait la ligne de l'autre (cf. libererVideoId).
        videoId TEXT NOT NULL,
        title TEXT NOT NULL,
        url TEXT NOT NULL,
        description TEXT,
        thumbnail TEXT,
        titleJp TEXT,
        romaji TEXT,
        publishDate TEXT,
        viewCount TEXT,
        -- La contrainte connaît SIX langues, pas trois. « vo » manquait alors que
        -- sources.ts déclare la famille depuis toujours : la VO japonaise, seule
        -- source officielle de l'éditeur pour la série d'origine, était donc
        -- REFUSÉE en écriture par le schéma qui prétendait l'accueillir. « en » et
        -- « es » manquaient de même, alors que la plateforme officielle les sert.
        language TEXT CHECK(language IN ('vo', 'vf', 'vostfr', 'en', 'es', 'de', 'unknown')),
        duration INTEGER,
        quality TEXT,
        createdAt INTEGER DEFAULT (cast(unixepoch() * 1000 as integer)),
        FOREIGN KEY(channel_id) REFERENCES channels(id) ON DELETE CASCADE,
        UNIQUE(channel_id, season, episode, language)
      );

      ${IETVCache.SCHEMA_SOURCES}

      CREATE TABLE IF NOT EXISTS metadata (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        expiresAt INTEGER
      );

      CREATE INDEX IF NOT EXISTS idx_episodes_channel ON episodes(channel_id);
      CREATE INDEX IF NOT EXISTS idx_episodes_season ON episodes(season);
      CREATE INDEX IF NOT EXISTS idx_episodes_language ON episodes(language);
      CREATE INDEX IF NOT EXISTS idx_episodes_title ON episodes(title COLLATE NOCASE);
      CREATE INDEX IF NOT EXISTS idx_episodes_videoid ON episodes(videoId);
    `);

		this.migrateSchema();
	}

	/**
	 * Ajoute les colonnes manquantes sur une base créée par une version
	 * antérieure — `CREATE TABLE IF NOT EXISTS` ne met pas à jour un schéma
	 * existant.
	 */
	private migrateSchema() {
		const addColumn = (table: string, column: string, type: string) => {
			const cols = this.db.prepare(`PRAGMA table_info(${table})`).all() as {
				name: string;
			}[];
			if (!cols.some((c) => c.name === column)) {
				this.db.exec(`ALTER TABLE ${table} ADD COLUMN ${column} ${type}`);
			}
		};

		addColumn("channels", "avatar", "TEXT");
		addColumn("seasons", "name", "TEXT");
		addColumn("episodes", "titleJp", "TEXT");
		addColumn("episodes", "romaji", "TEXT");
		addColumn("episodes", "description", "TEXT");
		addColumn("episodes", "publishDate", "TEXT");
		addColumn("episodes", "viewCount", "TEXT");

		this.refondreEpisodes();
		this.db.exec(IETVCache.SCHEMA_SOURCES);

		// ── L'ÉTAT MESURÉ D'UNE SOURCE, À CÔTÉ DE SA CONFIANCE ──────────────
		// `confiance` dit d'où vient la source ; `etat` dit si elle RÉPOND
		// aujourd'hui. Les deux sont indépendants : une source `declaree` peut
		// être parfaitement vivante, et une source `verifiee` il y a un mois
		// peut avoir été retirée depuis. Les confondre revenait à ne jamais
		// pouvoir dire qu'un lien est mort sans effacer d'où il venait.
		//
		// Pas de contrainte `CHECK` : SQLite ne sait pas en ajouter une par
		// `ALTER TABLE`, et une valeur nouvelle ne doit pas exiger de refonte.
		// Le vocabulaire est tenu par `EtatSource` dans `verifier.ts`.
		addColumn("episode_sources", "etat", "TEXT");
		addColumn("episode_sources", "codeHttp", "INTEGER");
		addColumn("episode_sources", "raisonEtat", "TEXT");

		// APRÈS les `addColumn` : la reconstruction recopie les colonnes d'état,
		// qui doivent donc déjà exister. L'ordre inverse effacerait `etat` et
		// `verifieeLe` de 1 858 sources — c'est-à-dire toute la vérification.
		this.refondreSources();

		this.supprimerOrphelins();
		this.reprendreSourcesHeritees();
	}

	/**
	 * Retire l'unicité de `episodes.videoId` sur une base créée avant ce
	 * correctif.
	 *
	 * ── UNE SOURCE VOLAIT LES ÉPISODES DE L'AUTRE ──────────────────────────
	 * Les épisodes du site officiel SONT des vidéos YouTube : 211 des 355
	 * épisodes du catalogue portent le même `videoId` que la vidéo publiée par
	 * une des chaînes. Avec `videoId TEXT UNIQUE` et des écritures en
	 * `INSERT OR REPLACE`, enregistrer la chaîne SUPPRIMAIT la ligne du site
	 * officiel — le « OR REPLACE » de SQLite efface toute ligne en conflit,
	 * sur n'importe laquelle de ses contraintes d'unicité.
	 *
	 * Mesuré le 2026-09-02 sur une copie de la base de production : le site
	 * officiel annonçait 355 épisodes, il n'en restait que 340 en table, et le
	 * catalogue affichait donc un compte faux.
	 *
	 * La bonne clé est `UNIQUE(channel_id, season, episode, language)`, qui est
	 * déjà là : une même vidéo référencée par deux sources est deux lignes,
	 * c'est la vérité — les deux sources la référencent réellement, et l'UI
	 * affiche déjà « un champ par source ».
	 *
	 * SQLite ne sait pas retirer une contrainte : la table est reconstruite.
	 * L'opération est idempotente — elle ne fait rien si la contrainte est déjà
	 * partie — et se fait en une transaction, pour ne jamais laisser une base à
	 * demi migrée.
	 */
	private refondreEpisodes() {
		const schema = this.db
			.prepare("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'episodes'")
			.get() as { sql?: string } | undefined;
		if (!schema?.sql) return;

		// Les DEUX défauts qu'une reconstruction corrige, testés séparément parce
		// qu'ils n'ont pas la même histoire : une base peut avoir déjà perdu
		// l'unicité sur `videoId` sans connaître « vo ».
		const videoIdUnique = /videoId\s+TEXT\s+UNIQUE/i.test(schema.sql);
		// Le test porte sur la DERNIÈRE langue ajoutée, pas sur `'vo'` : sur une
		// base déjà migrée pour `vo`, chercher `'vo'` répond « à jour » et la
		// contrainte reste bloquée à cinq langues. Une base qui ignore `'de'`
		// refuse en écriture les 67 épisodes allemands que le site sert
		// réellement — un refus silencieux, puisque `saveChannel` avale l'erreur.
		const langueEtroite = !/'de'/.test(schema.sql);
		// ── LE TEST EST CE QUI REND LA MIGRATION REJOUABLE ──────────────────
		// Rejouée sur une base déjà migrée, la fonction sort ici : elle ne
		// reconstruit rien, ne réattribue aucun `id`, et laisse les `createdAt`
		// intacts. C'est ce qui permet à `IETVCache` d'appeler la migration à
		// CHAQUE ouverture sans que la base dérive à chaque fois.
		if (!videoIdUnique && !langueEtroite) return;

		this.db.exec("BEGIN IMMEDIATE");
		try {
			this.db.exec(`
        CREATE TABLE episodes_migration (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          channel_id INTEGER NOT NULL,
          season INTEGER NOT NULL,
          episode INTEGER NOT NULL,
          videoId TEXT NOT NULL,
          title TEXT NOT NULL,
          url TEXT NOT NULL,
          description TEXT,
          thumbnail TEXT,
          titleJp TEXT,
          romaji TEXT,
          publishDate TEXT,
          viewCount TEXT,
          language TEXT CHECK(language IN ('vo', 'vf', 'vostfr', 'en', 'es', 'de', 'unknown')),
          duration INTEGER,
          quality TEXT,
          createdAt INTEGER DEFAULT (cast(unixepoch() * 1000 as integer)),
          FOREIGN KEY(channel_id) REFERENCES channels(id) ON DELETE CASCADE,
          UNIQUE(channel_id, season, episode, language)
        );

        INSERT INTO episodes_migration
          (id, channel_id, season, episode, videoId, title, url, description, thumbnail,
           titleJp, romaji, publishDate, viewCount, language, duration, quality, createdAt)
        SELECT id, channel_id, season, episode, videoId, title, url, description, thumbnail,
               titleJp, romaji, publishDate, viewCount, language, duration, quality, createdAt
        FROM episodes;

        DROP TABLE episodes;
        ALTER TABLE episodes_migration RENAME TO episodes;

        CREATE INDEX IF NOT EXISTS idx_episodes_channel ON episodes(channel_id);
        CREATE INDEX IF NOT EXISTS idx_episodes_season ON episodes(season);
        CREATE INDEX IF NOT EXISTS idx_episodes_language ON episodes(language);
        CREATE INDEX IF NOT EXISTS idx_episodes_title ON episodes(title COLLATE NOCASE);
        CREATE INDEX IF NOT EXISTS idx_episodes_videoid ON episodes(videoId);
      `);
			this.db.exec("COMMIT");
		} catch (err) {
			this.db.exec("ROLLBACK");
			throw err;
		}
	}

	/**
	 * Élargit la contrainte `CHECK` de `episode_sources.langue` sur une base
	 * créée avant l'ajout d'une langue.
	 *
	 * ── POURQUOI CE N'EST PAS OPTIONNEL ────────────────────────────────────
	 * `CREATE TABLE IF NOT EXISTS` ne touche pas une table existante, et SQLite
	 * ne sait pas modifier un `CHECK` par `ALTER TABLE`. Une base créée quand le
	 * vocabulaire comptait six langues REFUSE donc `de` en écriture — et le
	 * refus est silencieux, parce que `saveChannel` attrape l'erreur par
	 * source. On obtiendrait une moisson qui annonce 67 épisodes allemands et
	 * une base qui n'en contient aucun.
	 *
	 * Rejouée sur une base déjà élargie, la fonction sort au premier test : elle
	 * ne reconstruit rien et ne réattribue aucun `id`.
	 */
	private refondreSources() {
		const schema = this.db
			.prepare("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'episode_sources'")
			.get() as { sql?: string } | undefined;
		if (!schema?.sql || /'de'/.test(schema.sql)) return;

		// Les colonnes sont lues sur la table RÉELLE : une base ancienne peut ne
		// pas porter `etat`/`codeHttp`/`raisonEtat`, et les nommer en dur ferait
		// échouer la copie avec « no such column ».
		const colonnes = (
			this.db.prepare("PRAGMA table_info(episode_sources)").all() as { name: string }[]
		).map((c) => c.name);
		const liste = colonnes.join(", ");

		this.db.exec("BEGIN IMMEDIATE");
		try {
			this.db.exec(`
        CREATE TABLE episode_sources_migration (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          episode_id INTEGER NOT NULL,
          plateforme TEXT NOT NULL CHECK(plateforme IN ('youtube', 'dailymotion', 'page')),
          sourceId TEXT NOT NULL,
          url TEXT NOT NULL,
          langue TEXT NOT NULL CHECK(langue IN ('vo', 'vf', 'vostfr', 'en', 'es', 'de', 'unknown')),
          qualite TEXT,
          officielle INTEGER NOT NULL DEFAULT 1,
          confiance TEXT NOT NULL DEFAULT 'declaree'
            CHECK(confiance IN ('verifiee', 'declaree', 'deduite')),
          verifieeLe INTEGER,
          origine TEXT,
          vignette TEXT,
          titre TEXT,
          createdAt INTEGER DEFAULT (cast(unixepoch() * 1000 as integer)),
          updatedAt INTEGER DEFAULT (cast(unixepoch() * 1000 as integer)),
          etat TEXT,
          codeHttp INTEGER,
          raisonEtat TEXT,
          FOREIGN KEY(episode_id) REFERENCES episodes(id) ON DELETE CASCADE,
          UNIQUE(episode_id, plateforme, sourceId, langue)
        );

        INSERT INTO episode_sources_migration (${liste})
        SELECT ${liste} FROM episode_sources;

        DROP TABLE episode_sources;
        ALTER TABLE episode_sources_migration RENAME TO episode_sources;

        CREATE INDEX IF NOT EXISTS idx_sources_episode ON episode_sources(episode_id);
        CREATE INDEX IF NOT EXISTS idx_sources_langue ON episode_sources(langue);
        CREATE INDEX IF NOT EXISTS idx_sources_plateforme ON episode_sources(plateforme);
        CREATE INDEX IF NOT EXISTS idx_sources_sourceid ON episode_sources(sourceId);
      `);
			this.db.exec("COMMIT");
		} catch (err) {
			this.db.exec("ROLLBACK");
			throw err;
		}
	}

	/**
	 * Supprime les épisodes rattachés à une chaîne qui n'existe plus.
	 *
	 * ── LES DÉGÂTS DÉJÀ ÉCRITS PAR L'ANCIEN `INSERT OR REPLACE` ────────────
	 * Tant que `saveChannel` réinsérait ses chaînes, chaque moisson leur donnait
	 * un nouvel `id` et abandonnait le lot d'épisodes précédent sous l'ancien.
	 * Ces lignes ne sont plus jointes à aucune chaîne — `getAllChannels` ne les
	 * voit pas — mais `COUNT(*) FROM episodes` les compte, et c'est ce compte
	 * que le catalogue affiche. Une base ayant tourné avec l'ancien code porte
	 * donc un lot d'épisodes fantômes par moisson passée.
	 *
	 * Le correctif de `saveChannel` empêche d'en créer de nouveaux ; celui-ci
	 * enlève ceux qui sont déjà là. Rejoué sur une base saine, il ne supprime
	 * rien — la clause `NOT IN` ne désigne alors aucune ligne.
	 */
	private supprimerOrphelins() {
		this.db.exec(
			`DELETE FROM episode_sources
              WHERE episode_id IN (SELECT id FROM episodes
                                    WHERE channel_id NOT IN (SELECT id FROM channels));
       DELETE FROM episodes WHERE channel_id NOT IN (SELECT id FROM channels);
       DELETE FROM seasons  WHERE channel_id NOT IN (SELECT id FROM channels);`
		);
	}

	/**
	 * Convertit en SOURCES les épisodes déjà en base — la reprise de l'existant.
	 *
	 * ── CE QUE LA COLONNE `videoId` CONTENAIT RÉELLEMENT ───────────────────
	 * Trois choses différentes sous un seul nom, mesurées le 2026-09-03 sur les
	 * 355 lignes de production :
	 *
	 *  * **212** identifiants YouTube (onze caractères) — lisibles tels quels ;
	 *  * **143** jetons fabriqués par nous (`off-galaxy-1`, 12 à 19 caractères),
	 *    qu'aucun lecteur ne sait ouvrir — mais dont la VIGNETTE porte, elle, un
	 *    identifiant Dailymotion parfaitement valide ;
	 *  * quelques `url` pointant la page du site sans identifiant du tout.
	 *
	 * La reprise lit donc `videoId`, puis `url`, puis `thumbnail`, et ne
	 * fabrique jamais d'identifiant : à défaut des trois, la source est de type
	 * `page` — « ça s'ouvre là, on ne sait pas l'intégrer », ce qui est vrai,
	 * plutôt qu'un identifiant inventé qui aurait l'air jouable.
	 *
	 * Aucune de ces sources n'est marquée `verifiee` : elles viennent d'une
	 * moisson antérieure, personne n'a rouvert la page. `confiance = 'declaree'`
	 * et `verifieeLe = NULL` disent exactement cela.
	 *
	 * `INSERT OR IGNORE` sur la clé `(episode_id, plateforme, sourceId, langue)`
	 * rend l'opération **rejouable** : au deuxième passage, chaque ligne existe
	 * déjà et rien n'est écrit.
	 */
	private reprendreSourcesHeritees() {
		const lignes = this.db
			.prepare(
				`SELECT e.id, e.videoId, e.url, e.thumbnail, e.title, e.language, e.quality, c.channel
                   FROM episodes e LEFT JOIN channels c ON c.id = e.channel_id
                  WHERE NOT EXISTS (SELECT 1 FROM episode_sources s WHERE s.episode_id = e.id)`
			)
			.all() as {
			id: number;
			videoId: string | null;
			url: string | null;
			thumbnail: string | null;
			title: string | null;
			language: string | null;
			quality: string | null;
			channel: string | null;
		}[];
		if (lignes.length === 0) return;

		const inserer = this.db.prepare(
			`INSERT OR IGNORE INTO episode_sources
         (episode_id, plateforme, sourceId, url, langue, qualite, officielle, confiance,
          verifieeLe, origine, vignette, titre)
       VALUES (?, ?, ?, ?, ?, ?, 1, 'declaree', NULL, ?, ?, ?)`
		);

		const transaction = this.db.transaction((toutes: typeof lignes) => {
			for (const ligne of toutes) {
				const candidats = [ligne.videoId ?? "", ligne.url ?? "", ligne.thumbnail ?? ""];
				let plateforme: Plateforme = "page";
				let sourceId = "";

				if (ligne.videoId && RE_YOUTUBE_ID.test(ligne.videoId)) {
					plateforme = "youtube";
					sourceId = ligne.videoId;
				} else {
					for (const candidat of candidats) {
						const trouve = candidat ? reconnaitre(candidat) : null;
						if (trouve) {
							plateforme = trouve.plateforme;
							sourceId = trouve.sourceId;
							break;
						}
					}
				}

				// Pas d'identifiant de lecture : l'URL de la page EST l'identité de
				// la source. Elle est unique par épisode, donc la clé tient.
				if (sourceId === "") sourceId = ligne.url ?? `episode-${ligne.id}`;

				inserer.run(
					ligne.id,
					plateforme,
					sourceId,
					ligne.url ?? "",
					langueValide(ligne.language),
					ligne.quality,
					ligne.channel,
					ligne.thumbnail,
					ligne.title
				);
			}
		});
		transaction(lignes);
	}

	// =========================================================================
	// Sources — N façons de regarder un épisode
	// =========================================================================

	/**
	 * Écrit les sources d'un épisode déjà en base.
	 *
	 * `ON CONFLICT … DO UPDATE` plutôt que `INSERT OR REPLACE` : remplacer la
	 * ligne effacerait son `createdAt`, c'est-à-dire la date à laquelle cette
	 * source a été DÉCOUVERTE, qui n'a aucune raison de bouger parce qu'on l'a
	 * revue. Seuls la vérification et ce qui peut changer d'un passage à l'autre
	 * sont mis à jour.
	 *
	 * `verifieeLe` ne recule jamais : une source relue sans être rouverte
	 * (`verifieeLe` nul) conserve la date de sa dernière vérification réelle.
	 * Sans ce `COALESCE`, un simple rafraîchissement de liste effacerait la
	 * preuve qu'on avait, un jour, réellement ouvert la page.
	 */
	enregistrerSources(episodeId: number, sources: readonly SourceEpisode[]): number {
		if (sources.length === 0) return 0;
		const stmt = this.db.prepare(
			`INSERT INTO episode_sources
         (episode_id, plateforme, sourceId, url, langue, qualite, officielle, confiance,
          verifieeLe, origine, vignette, titre)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(episode_id, plateforme, sourceId, langue) DO UPDATE SET
         url = excluded.url,
         qualite = COALESCE(excluded.qualite, episode_sources.qualite),
         officielle = excluded.officielle,
         confiance = excluded.confiance,
         verifieeLe = COALESCE(excluded.verifieeLe, episode_sources.verifieeLe),
         origine = COALESCE(excluded.origine, episode_sources.origine),
         vignette = COALESCE(excluded.vignette, episode_sources.vignette),
         titre = COALESCE(excluded.titre, episode_sources.titre),
         updatedAt = cast(unixepoch() * 1000 as integer)`
		);

		const transaction = this.db.transaction((toutes: readonly SourceEpisode[]) => {
			for (const s of toutes) {
				stmt.run(
					episodeId,
					s.plateforme,
					s.sourceId,
					s.url,
					s.langue,
					s.qualite,
					s.officielle ? 1 : 0,
					s.confiance,
					s.verifieeLe,
					s.origine,
					s.vignette,
					s.titre
				);
			}
		});
		transaction(sources);
		return sources.length;
	}

	/** Toutes les sources d'un épisode, la plus établie d'abord. */
	sourcesDeEpisode(episodeId: number): SourceEpisode[] {
		return (
			this.db
				.prepare(
					`SELECT plateforme, sourceId, url, langue, qualite, officielle, confiance,
                  verifieeLe, origine, vignette, titre
             FROM episode_sources WHERE episode_id = ?
            ORDER BY CASE confiance WHEN 'verifiee' THEN 0 WHEN 'declaree' THEN 1 ELSE 2 END,
                     CASE plateforme WHEN 'youtube' THEN 0 WHEN 'dailymotion' THEN 1 ELSE 2 END`
				)
				.all(episodeId) as any[]
		).map((r) => ({
			plateforme: r.plateforme as Plateforme,
			sourceId: r.sourceId,
			url: r.url,
			langue: langueValide(r.langue),
			qualite: r.qualite ?? null,
			officielle: r.officielle === 1,
			confiance: r.confiance as SourceEpisode["confiance"],
			verifieeLe: r.verifieeLe ?? null,
			origine: r.origine ?? "",
			vignette: r.vignette ?? null,
			titre: r.titre ?? null,
		}));
	}

	/**
	 * Couverture mesurée : saison × langue × plateforme, et ce qui reste muet.
	 *
	 * C'est le seul chiffre qui vaille pour dire si une moisson a servi à
	 * quelque chose. Il compte des épisodes DISTINCTS (`season, episode`), pas
	 * des lignes : trois sources du même épisode ne font pas trois épisodes
	 * couverts, et c'est exactement l'erreur que `totalEpisodes` faisait déjà
	 * une fois.
	 *
	 * `sansSourceLisible` est la mesure qui compte pour l'utilisateur : un
	 * épisode dont la seule source est de type `page` existe au catalogue et ne
	 * se regarde pas.
	 */
	couverture(): {
		parSaisonLangue: { saison: number; langue: string; episodes: number; sources: number }[];
		parPlateforme: { plateforme: string; sources: number; episodes: number }[];
		episodesDistincts: number;
		sourcesTotal: number;
		sansSourceLisible: { saison: number; episode: number | null }[];
		sourcesParEpisode: { min: number; max: number; moyenne: number };
	} {
		const parSaisonLangue = this.db
			.prepare(
				`SELECT e.season AS saison, s.langue AS langue,
                COUNT(DISTINCT e.season || '/' || COALESCE(e.episode, -1)) AS episodes,
                COUNT(*) AS sources
           FROM episode_sources s JOIN episodes e ON e.id = s.episode_id
          GROUP BY e.season, s.langue ORDER BY e.season, s.langue`
			)
			.all() as { saison: number; langue: string; episodes: number; sources: number }[];

		const parPlateforme = this.db
			.prepare(
				`SELECT s.plateforme AS plateforme, COUNT(*) AS sources,
                COUNT(DISTINCT e.season || '/' || COALESCE(e.episode, -1)) AS episodes
           FROM episode_sources s JOIN episodes e ON e.id = s.episode_id
          GROUP BY s.plateforme ORDER BY sources DESC`
			)
			.all() as { plateforme: string; sources: number; episodes: number }[];

		const distincts = this.db
			.prepare(
				"SELECT COUNT(*) AS n FROM (SELECT DISTINCT season, episode FROM episodes)"
			)
			.get() as { n: number };
		const total = this.db.prepare("SELECT COUNT(*) AS n FROM episode_sources").get() as {
			n: number;
		};

		// Un épisode est muet quand AUCUNE de ses lignes — toutes langues et
		// toutes chaînes confondues — ne porte de source intégrable.
		const sansSourceLisible = this.db
			.prepare(
				`SELECT DISTINCT e.season AS saison, e.episode AS episode
           FROM episodes e
          WHERE NOT EXISTS (
                  SELECT 1 FROM episodes e2
                    JOIN episode_sources s ON s.episode_id = e2.id
                   WHERE e2.season = e.season
                     AND COALESCE(e2.episode, -1) = COALESCE(e.episode, -1)
                     AND s.plateforme <> 'page')
          ORDER BY e.season, e.episode`
			)
			.all() as { saison: number; episode: number | null }[];

		const parEpisode = this.db
			.prepare(
				`SELECT MIN(n) AS mini, MAX(n) AS maxi, AVG(n) AS moyenne FROM (
           SELECT COUNT(s.id) AS n
             FROM episodes e LEFT JOIN episode_sources s ON s.episode_id = e.id
            GROUP BY e.season, COALESCE(e.episode, -1))`
			)
			.get() as { mini: number | null; maxi: number | null; moyenne: number | null };

		return {
			parSaisonLangue,
			parPlateforme,
			episodesDistincts: distincts.n,
			sourcesTotal: total.n,
			sansSourceLisible,
			sourcesParEpisode: {
				min: parEpisode.mini ?? 0,
				max: parEpisode.maxi ?? 0,
				moyenne: Number((parEpisode.moyenne ?? 0).toFixed(2)),
			},
		};
	}

	// =========================================================================
	// Vérification — l'état MESURÉ des sources
	// =========================================================================

	/**
	 * Les sources à sonder, avec de quoi les identifier et les recoller.
	 *
	 * Renvoie des LIGNES, pas des sources dédoublonnées : c'est le vérificateur
	 * qui groupe par cible réseau, parce que lui seul sait que deux lignes
	 * différentes (même vidéo, deux langues déclarées) ne valent qu'un appel.
	 */
	sourcesAVerifier(filtre: { plateforme?: Plateforme; limite?: number } = {}): {
		id: number;
		plateforme: Plateforme;
		sourceId: string;
		url: string;
		langue: LangueSource;
		etat: EtatSource | null;
	}[] {
		const conditions: string[] = [];
		const args: unknown[] = [];
		if (filtre.plateforme) {
			conditions.push("plateforme = ?");
			args.push(filtre.plateforme);
		}
		const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : "";
		const limite = filtre.limite && filtre.limite > 0 ? `LIMIT ${Math.floor(filtre.limite)}` : "";
		return (
			this.db
				.prepare(
					`SELECT id, plateforme, sourceId, url, langue, etat
             FROM episode_sources ${where} ORDER BY id ${limite}`
				)
				.all(...(args as never[])) as any[]
		).map((r) => ({
			id: r.id as number,
			plateforme: r.plateforme as Plateforme,
			sourceId: r.sourceId as string,
			url: r.url as string,
			langue: langueValide(r.langue),
			etat: (r.etat ?? null) as EtatSource | null,
		}));
	}

	/**
	 * Inscrit le verdict d'une sonde sur des lignes de sources.
	 *
	 * ── CE QUI EST ÉCRIT, ET CE QUI NE L'EST PAS ───────────────────────────
	 * `confiance` ne passe à `verifiee` que pour un état `vivante` : une sonde
	 * qui répond « cette vidéo n'existe pas » établit un fait sur la source,
	 * mais ne rend pas la source meilleure — la marquer `verifiee` ferait
	 * remonter un lien mort en tête de `sourcesDeEpisode`, dont le tri met les
	 * `verifiee` d'abord. Une source morte garde donc sa confiance d'origine et
	 * porte son état ; c'est l'état qui doit la faire écarter.
	 *
	 * `verifieeLe` n'est posé que sur un test CONCLUANT (`vivante` ou `morte`).
	 * Un `non_testable` n'est pas une vérification : dater ce non-événement
	 * ferait passer pour mesuré ce que personne n'a pu mesurer.
	 */
	marquerVerification(
		verdicts: readonly {
			id: number;
			etat: EtatSource;
			codeHttp: number | null;
			raison: string;
		}[],
		horodatage = Date.now()
	): number {
		if (verdicts.length === 0) return 0;
		const stmt = this.db.prepare(
			`UPDATE episode_sources
          SET etat = ?, codeHttp = ?, raisonEtat = ?,
              confiance = CASE WHEN ? = 'vivante' THEN 'verifiee' ELSE confiance END,
              verifieeLe = CASE WHEN ? IN ('vivante', 'morte') THEN ? ELSE verifieeLe END,
              updatedAt = cast(unixepoch() * 1000 as integer)
        WHERE id = ?`
		);
		const transaction = this.db.transaction((tous: typeof verdicts) => {
			for (const v of tous) {
				stmt.run(v.etat, v.codeHttp, v.raison, v.etat, v.etat, horodatage, v.id);
			}
		});
		transaction(verdicts);
		return verdicts.length;
	}

	/**
	 * Le tableau de vérification : combien de sources dans quel état, ventilé
	 * par plateforme et par langue, en LIGNES et en épisodes distincts.
	 *
	 * `null` (jamais sondée) est rendu tel quel sous la clé `jamais_testee` :
	 * une source qu'on n'a pas regardée n'est pas « non testable », et les
	 * confondre effacerait la seule chose qui distingue un travail restant à
	 * faire d'un travail impossible.
	 */
	etatVerification(): {
		parPlateforme: { plateforme: string; etat: string; sources: number }[];
		parLangue: { langue: string; etat: string; sources: number; episodes: number }[];
		total: { etat: string; sources: number }[];
	} {
		const etat = "COALESCE(s.etat, 'jamais_testee')";
		return {
			parPlateforme: this.db
				.prepare(
					`SELECT s.plateforme AS plateforme, ${etat} AS etat, COUNT(*) AS sources
             FROM episode_sources s GROUP BY 1, 2 ORDER BY 1, 2`
				)
				.all() as { plateforme: string; etat: string; sources: number }[],
			parLangue: this.db
				.prepare(
					`SELECT s.langue AS langue, ${etat} AS etat, COUNT(*) AS sources,
                    COUNT(DISTINCT e.season || '/' || COALESCE(e.episode, -1)) AS episodes
               FROM episode_sources s JOIN episodes e ON e.id = s.episode_id
              GROUP BY 1, 2 ORDER BY 1, 2`
				)
				.all() as { langue: string; etat: string; sources: number; episodes: number }[],
			total: this.db
				.prepare(
					`SELECT ${etat} AS etat, COUNT(*) AS sources
             FROM episode_sources s GROUP BY 1 ORDER BY 2 DESC`
				)
				.all() as { etat: string; sources: number }[],
		};
	}

	/**
	 * Couverture par langue en épisodes DISTINCTS, limitée aux sources qui
	 * tiennent debout : intégrables et non mortes.
	 *
	 * C'est la mesure honnête de « combien d'épisodes puis-je regarder dans
	 * cette langue » — celle de `couverture()` compte tout ce qui est au
	 * catalogue, y compris des pages qu'aucun lecteur n'ouvre.
	 */
	couvertureRegardable(): { langue: string; episodes: number }[] {
		return this.db
			.prepare(
				`SELECT s.langue AS langue,
                COUNT(DISTINCT e.season || '/' || COALESCE(e.episode, -1)) AS episodes
           FROM episode_sources s JOIN episodes e ON e.id = s.episode_id
          WHERE s.plateforme <> 'page' AND COALESCE(s.etat, 'jamais_testee') <> 'morte'
          GROUP BY 1 ORDER BY 2 DESC`
			)
			.all() as { langue: string; episodes: number }[];
	}

	// =========================================================================
	// Channel operations
	// =========================================================================

	saveChannel(info: ChannelInfo) {
		// ── `INSERT OR REPLACE` CHANGEAIT L'IDENTIFIANT DE LA CHAÎNE ─────────
		// Il SUPPRIME la ligne en conflit puis en insère une neuve, qui reçoit un
		// nouvel `id` AUTOINCREMENT. Les épisodes, eux, gardaient l'ancien
		// `channel_id` : ils devenaient orphelins d'une chaîne disparue, pendant
		// que la moisson en réinsérait une copie complète sous le nouvel id.
		//
		// Mesuré : après une seule moisson, `channels.id` était passé de 64 à 67
		// et la base portait 355 épisodes orphelins en `channel_id = 64` EN PLUS
		// des 355 remoissonnés — chaque épisode compté deux fois, chaque source
		// présente en double sous deux confiances différentes.
		//
		// L'upsert met à jour la ligne EN PLACE : `id` ne bouge plus jamais.
		const stmt = this.db.prepare(`
      INSERT INTO channels (channel, title, description, avatar, totalEpisodes, lastScrape, updatedAt)
      VALUES (?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(channel) DO UPDATE SET
        title = excluded.title,
        description = COALESCE(excluded.description, channels.description),
        avatar = COALESCE(excluded.avatar, channels.avatar),
        totalEpisodes = excluded.totalEpisodes,
        lastScrape = excluded.lastScrape,
        updatedAt = excluded.updatedAt
    `);

		const now = Date.now();
		stmt.run(
			info.channel,
			info.title,
			info.description || null,
			info.avatar || null,
			info.totalEpisodes,
			now,
			now
		);

		const channelId = this.db
			.prepare("SELECT id FROM channels WHERE channel = ?")
			.get(info.channel) as { id: number };

		// Save seasons
		for (const season of info.seasons) {
			this.db
				.prepare(
					// Même raison que pour `channels` : `seasons.id` n'est référencé
					// par rien aujourd'hui, mais un `OR REPLACE` qui réattribue des
					// identifiants à chaque moisson est une bombe à retardement — c'est
					// exactement comme ça que les épisodes se sont dédoublés.
					`INSERT INTO seasons (channel_id, season, name, totalEpisodes) VALUES (?, ?, ?, ?)
           ON CONFLICT(channel_id, season) DO UPDATE SET
             name = COALESCE(excluded.name, seasons.name),
             totalEpisodes = excluded.totalEpisodes`
				)
				.run(channelId.id, season.season, season.name ?? null, season.totalEpisodes);

			// ── L'IDENTIFIANT DE LA LIGNE DOIT SURVIVRE À UNE REMOISSON ──────
			// `INSERT OR REPLACE` SUPPRIME puis réinsère : la ligne repart avec un
			// nouveau `rowid`. Tant que rien ne référençait `episodes.id`, c'était
			// sans effet. `episode_sources.episode_id` le référence désormais, et
			// comme `PRAGMA foreign_keys` n'est pas posé, SQLite ne cascaderait
			// même pas — chaque remoisson aurait silencieusement détaché TOUTES
			// les sources de leur épisode, et la couverture serait retombée à zéro
			// au premier passage du cron.
			//
			// L'upsert sur la clé métier met à jour la ligne EN PLACE : `id` ne
			// bouge pas, les sources restent attachées.
			for (const ep of season.episodes) {
				this.db
					.prepare(
						`INSERT INTO episodes
           (channel_id, season, episode, videoId, title, url, description, thumbnail,
            titleJp, romaji, publishDate, viewCount, language, duration, quality)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(channel_id, season, episode, language) DO UPDATE SET
           videoId = excluded.videoId,
           title = excluded.title,
           url = excluded.url,
           description = COALESCE(excluded.description, episodes.description),
           thumbnail = COALESCE(excluded.thumbnail, episodes.thumbnail),
           titleJp = COALESCE(excluded.titleJp, episodes.titleJp),
           romaji = COALESCE(excluded.romaji, episodes.romaji),
           publishDate = COALESCE(excluded.publishDate, episodes.publishDate),
           viewCount = COALESCE(excluded.viewCount, episodes.viewCount),
           duration = COALESCE(excluded.duration, episodes.duration),
           quality = COALESCE(excluded.quality, episodes.quality)`
					)
					.run(
						channelId.id,
						season.season,
						ep.episode,
						ep.videoId,
						ep.title,
						ep.url,
						ep.description || null,
						ep.thumbnail || null,
						ep.titleJp || null,
						ep.romaji || null,
						ep.publishDate || null,
						ep.viewCount || null,
						ep.language,
						ep.duration ?? null,
						ep.quality || null
					);

				// Les sources que la moisson a réellement observées pour cet épisode.
				// Elles sont écrites APRÈS l'upsert, donc contre un `id` stable.
				if (ep.sources && ep.sources.length > 0) {
					const ligne = this.db
						.prepare(
							`SELECT id FROM episodes
                WHERE channel_id = ? AND season = ? AND episode IS ? AND language IS ?`
						)
						.get(channelId.id, season.season, ep.episode, ep.language) as
						| { id: number }
						| undefined;
					if (ligne) this.enregistrerSources(ligne.id, ep.sources);
				}
			}
		}

		// ── LE COMPTEUR SE RECALCULE, IL NE SE DÉCLARE PAS ──────────────────
		// `totalEpisodes` était écrit d'après ce que le scraping annonçait, pas
		// d'après ce qui était réellement entré en table. Toute ligne perdue en
		// chemin — conflit d'unicité, langue refusée par la contrainte CHECK —
		// laissait la colonne à un compte plus élevé que la réalité, et c'est
		// elle que `/episodes catalogue` affiche. Mesuré : 355 annoncés pour
		// 340 lignes. On compte donc les lignes.
		this.db
			.prepare(
				`UPDATE seasons SET totalEpisodes =
           (SELECT COUNT(*) FROM episodes WHERE channel_id = seasons.channel_id
              AND season = seasons.season)
         WHERE channel_id = ?`
			)
			.run(channelId.id);
		this.db
			.prepare(
				"UPDATE channels SET totalEpisodes = (SELECT COUNT(*) FROM episodes WHERE channel_id = ?) WHERE id = ?"
			)
			.run(channelId.id, channelId.id);
	}

	getChannel(channel: string): ChannelInfo | null {
		const ch = this.db
			.prepare(
				"SELECT id, channel, title, description, avatar, totalEpisodes FROM channels WHERE channel = ?"
			)
			.get(channel) as any;

		if (!ch) return null;

		const seasons = this.db
			.prepare(
				"SELECT season, name, totalEpisodes FROM seasons WHERE channel_id = ? ORDER BY season"
			)
			.all(ch.id) as any[];

		const result: ChannelInfo = {
			channel: ch.channel,
			title: ch.title,
			description: ch.description,
			avatar: ch.avatar,
			totalEpisodes: ch.totalEpisodes,
			seasons: seasons.map((s) => ({
				season: s.season,
				name: s.name ?? null,
				totalEpisodes: s.totalEpisodes,
				episodes: this.getEpisodesBySeason(ch.id, s.season),
			})),
		};

		return result;
	}

	getAllChannels(): ChannelInfo[] {
		const channels = this.db
			.prepare(
				"SELECT id, channel, title, description, avatar, totalEpisodes FROM channels ORDER BY channel"
			)
			.all() as any[];

		return channels.map((ch) => {
			const seasons = this.db
				.prepare(
					"SELECT season, name, totalEpisodes FROM seasons WHERE channel_id = ? ORDER BY season"
				)
				.all(ch.id) as any[];

			return {
				channel: ch.channel,
				title: ch.title,
				description: ch.description,
				avatar: ch.avatar,
				totalEpisodes: ch.totalEpisodes,
				seasons: seasons.map((s) => ({
					season: s.season,
					name: s.name ?? null,
					totalEpisodes: s.totalEpisodes,
					episodes: this.getEpisodesBySeason(ch.id, s.season),
				})),
			};
		});
	}

	// =========================================================================
	// Episode operations
	// =========================================================================

	private getEpisodesBySeason(channelId: number, season: number): VideoRef[] {
		return (
			this.db
				.prepare(
					`SELECT videoId, season, episode, title, url, description, thumbnail,
                titleJp, romaji, publishDate, viewCount, language, duration, quality
         FROM episodes WHERE channel_id = ? AND season = ? ORDER BY episode`
				)
				.all(channelId, season) as any[]
		).map((ep) => ({
			videoId: ep.videoId,
			title: ep.title,
			url: ep.url,
			description: ep.description ?? null,
			season: ep.season,
			episode: ep.episode,
			thumbnail: ep.thumbnail ?? null,
			titleJp: ep.titleJp ?? null,
			romaji: ep.romaji ?? null,
			publishDate: ep.publishDate ?? null,
			viewCount: ep.viewCount ?? null,
			language: ep.language,
			duration: ep.duration ?? null,
			quality: ep.quality ?? null,
		}));
	}

	search(query: CacheSearchQuery): CachedVideoRef[] {
		let sql = `
      SELECT DISTINCT e.videoId, e.season, e.episode, e.title, e.url, e.description, e.thumbnail,
             e.titleJp, e.romaji, e.publishDate, e.viewCount, e.language, e.duration, e.quality,
             c.channel, c.title as channel_title
      FROM episodes e
      JOIN channels c ON e.channel_id = c.id
      WHERE 1=1
    `;

		const params: any[] = [];

		if (query.q) {
			// La recherche porte sur les QUATRE champs qui nomment un épisode, pas seulement sur
			// son titre localisé : la base garde 330 titres japonais, 327 transcriptions romaji et
			// 355 résumés, et c'est souvent par le romaji (« Sakkā Yarō Ze! ») ou par un détail du
			// résumé qu'on retrouve un épisode dont on ne sait plus le titre français. Sans eux,
			// une recherche pourtant exacte ne rendait rien.
			sql += ` AND (e.title LIKE ? OR e.titleJp LIKE ? OR e.romaji LIKE ? OR e.description LIKE ? OR c.title LIKE ?)`;
			const q = `%${query.q}%`;
			params.push(q, q, q, q, q);
		}
		if (query.season) {
			sql += ` AND e.season = ?`;
			params.push(query.season);
		}
		if (query.episode) {
			sql += ` AND e.episode = ?`;
			params.push(query.episode);
		}
		if (query.language) {
			sql += ` AND e.language = ?`;
			params.push(query.language);
		}
		if (query.channel) {
			sql += ` AND c.channel = ?`;
			params.push(query.channel);
		}

		sql += ` ORDER BY e.season DESC, e.episode DESC`;
		if (query.limit) {
			sql += ` LIMIT ?`;
			params.push(query.limit);
		}

		return (this.db.prepare(sql).all(...params) as any[]).map((ep) => ({
			videoId: ep.videoId,
			title: ep.title,
			url: ep.url,
			description: ep.description ?? null,
			season: ep.season,
			episode: ep.episode,
			thumbnail: ep.thumbnail ?? null,
			titleJp: ep.titleJp ?? null,
			romaji: ep.romaji ?? null,
			publishDate: ep.publishDate ?? null,
			viewCount: ep.viewCount ?? null,
			language: ep.language,
			duration: ep.duration ?? null,
			quality: ep.quality ?? null,
			channel: ep.channel,
		}));
	}

	// =========================================================================
	// Statistics
	// =========================================================================

	getStats(): CacheStats {
		const channels = this.db.prepare("SELECT COUNT(*) as count FROM channels").get() as any;
		const seasons = this.db.prepare("SELECT COUNT(*) as count FROM seasons").get() as any;
		const episodes = this.db.prepare("SELECT COUNT(*) as count FROM episodes").get() as any;
		const byLanguage = this.db
			.prepare(
				"SELECT language, COUNT(*) as count FROM episodes GROUP BY language ORDER BY language"
			)
			.all() as any[];

		const lastUpdate = this.db
			.prepare("SELECT MAX(lastScrape) as lastScrape FROM channels")
			.get() as any;

		return {
			channels: channels.count || 0,
			seasons: seasons.count || 0,
			episodes: episodes.count || 0,
			byLanguage: byLanguage.reduce<Record<string, number>>((acc, row) => {
				acc[row.language] = row.count;
				return acc;
			}, {}),
			lastUpdate: lastUpdate.lastScrape || 0,
		};
	}

	// =========================================================================
	// Metadata (cache expiration)
	// =========================================================================

	setMetadata(key: string, value: string, ttlMs?: number) {
		const expiresAt = ttlMs ? Date.now() + ttlMs : null;
		this.db
			.prepare("INSERT OR REPLACE INTO metadata (key, value, expiresAt) VALUES (?, ?, ?)")
			.run(key, value, expiresAt);
	}

	getMetadata(key: string): string | null {
		const row = this.db
			.prepare("SELECT value, expiresAt FROM metadata WHERE key = ?")
			.get(key) as any;

		if (!row) return null;
		if (row.expiresAt && row.expiresAt < Date.now()) {
			this.db.prepare("DELETE FROM metadata WHERE key = ?").run(key);
			return null;
		}

		return row.value;
	}

	// =========================================================================
	// Cleanup
	// =========================================================================

	clearExpired() {
		this.db.prepare("DELETE FROM metadata WHERE expiresAt IS NOT NULL AND expiresAt < ?").run(
			Date.now()
		);
	}

	/**
	 * Vide tout le catalogue.
	 *
	 * Les sources s'effacent EXPLICITEMENT et en premier : `ON DELETE CASCADE`
	 * est bien déclaré sur `episode_sources`, mais SQLite ne l'applique que sous
	 * `PRAGMA foreign_keys = ON`, qui n'est pas posé ici (même piège que
	 * `clearChannel`). S'y fier laisserait toute la table des sources en place,
	 * rattachée à des épisodes disparus — et la première remoisson aurait
	 * réattribué ces `episode_id` à d'autres épisodes.
	 */
	clear() {
		this.db.exec(
			"DELETE FROM episode_sources; DELETE FROM episodes; DELETE FROM seasons; DELETE FROM channels;"
		);
	}

	/**
	 * Efface UNE source et tout ce qui lui appartient.
	 *
	 * C'est ce qui permet à un rafraîchissement de remplacer les sources qu'il a
	 * réellement lues sans toucher aux autres : `clear()` vide tout, et une
	 * source momentanément injoignable (YouTube qui refuse son flux Atom, un
	 * site en maintenance) y perdait alors TOUS ses épisodes jusqu'au passage
	 * suivant. Le catalogue rétrécissait au rythme des pannes d'en face.
	 *
	 * Les suppressions sont explicites et dans l'ordre : `ON DELETE CASCADE` est
	 * bien déclaré sur le schéma mais SQLite ne l'applique QUE si
	 * `PRAGMA foreign_keys = ON`, qui n'est pas posé ici. S'y fier laisserait
	 * des épisodes orphelins rattachés à un `channel_id` disparu.
	 */
	clearChannel(channel: string) {
		const ligne = this.db.prepare("SELECT id FROM channels WHERE channel = ?").get(channel) as
			| { id: number }
			| undefined;
		if (!ligne) return;
		this.db
			.prepare(
				"DELETE FROM episode_sources WHERE episode_id IN (SELECT id FROM episodes WHERE channel_id = ?)"
			)
			.run(ligne.id);
		this.db.prepare("DELETE FROM episodes WHERE channel_id = ?").run(ligne.id);
		this.db.prepare("DELETE FROM seasons WHERE channel_id = ?").run(ligne.id);
		this.db.prepare("DELETE FROM channels WHERE id = ?").run(ligne.id);
	}

	close() {
		this.db.close();
	}
}
