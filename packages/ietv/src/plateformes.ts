/**
 * Plateformes et langues de diffusion — module PUR, sans réseau ni base.
 *
 * ── POURQUOI CE MODULE EXISTE ──────────────────────────────────────────────
 * Le catalogue tenait dans un seul `videoId` par épisode, et supposait YouTube.
 * Deux mesures cassent cette hypothèse, faites le 2026-09-03 sur la base de
 * production (`data/anime/episodes.db`, 355 lignes) :
 *
 *  * **143 épisodes sur 355 ne sont PAS sur YouTube** — toute la saison 3 sauf
 *    onze épisodes, tout Chrono Stones (51) et tout Galaxy (43). Leur `url`
 *    pointe la plateforme officielle et leur vignette est servie par
 *    Dailymotion : le `videoId` stocké est un jeton local (`off-galaxy-1`).
 *  * **La plateforme officielle sert trois langues** (`?lang=fr|en|es`), et la
 *    MÊME page rend un identifiant DIFFÉRENT selon la langue — parfois sur une
 *    autre plateforme. Mesuré sur `saison1/ep-1` : `fr` → YouTube
 *    `xbpo3u3P9dc`, `en` → Dailymotion `x8c1xw5`, `es` → YouTube `x8F4GnpoCrw`.
 *
 * Une colonne ne peut donc pas décrire un épisode : il faut N sources par
 * épisode, chacune avec sa plateforme, sa langue et son degré de certitude.
 *
 * ── CE QUI EST OFFICIEL, ET RIEN D'AUTRE ───────────────────────────────────
 * Toutes les sources déclarées ici sont des diffusions officielles : la
 * plateforme de l'éditeur européen (`inazuma-eleven.fr`, « Inazuma TV+ ») et
 * des chaînes YouTube dont le caractère officiel est établi par leur titre.
 * Aucun site de streaming tiers, aucun contournement de géoblocage ou de DRM :
 * ce module ne lit que des pages publiques et le balisage qu'elles publient
 * pour être lues.
 */

/** Où vit réellement une vidéo. */
export type Plateforme = "youtube" | "dailymotion" | "page";

/**
 * Langue d'une source, dans le vocabulaire du catalogue.
 *
 * Les clés reprennent celles de `apps/inacord/src/lib/sources.ts` — le
 * sélecteur de langue y déclare déjà les familles `vo`, `vf`, `vostfr`, `en`,
 * `es` : les faire diverger obligerait à traduire dans les deux sens.
 * `unknown` n'est PAS une langue, c'est l'absence de renseignement.
 */
export type LangueSource = "vo" | "vf" | "vostfr" | "en" | "es" | "de" | "unknown";

/** Toutes les valeurs que la contrainte `CHECK` de la base doit accepter. */
export const LANGUES_SOURCE: readonly LangueSource[] = [
	"vo",
	"vf",
	"vostfr",
	"en",
	"es",
	"de",
	"unknown",
];

/**
 * Degré de certitude d'une source — une source n'est pas un fait tant que
 * personne n'a regardé.
 *
 * * `verifiee` — la page a été récupérée et l'identifiant y a été LU ;
 * * `declaree` — une source officielle l'annonce (liste, flux) sans qu'on ait
 *   ouvert la page de lecture ;
 * * `deduite` — reconstruite par nous (jeton local, langue déduite d'un nom de
 *   chaîne). C'est ce qui ne doit jamais être présenté comme mesuré.
 */
export type Confiance = "verifiee" | "declaree" | "deduite";

/**
 * État MESURÉ d'une source — ce que la plateforme répond aujourd'hui.
 *
 * ── POURQUOI CE N'EST PAS LA MÊME CHOSE QUE `Confiance` ────────────────────
 * `confiance` dit d'OÙ VIENT la source ; `etat` dit si elle RÉPOND. Les deux
 * varient indépendamment : une source `declaree` peut être parfaitement
 * vivante, et une source `verifiee` il y a un mois peut avoir été retirée
 * depuis. Les confondre revenait à ne pas pouvoir marquer un lien mort sans
 * effacer sa provenance.
 *
 * * `vivante` — la plateforme a répondu que la vidéo existe et est diffusable ;
 * * `morte` — elle a répondu qu'elle n'existe pas, est privée, ou n'est pas
 *   intégrable ; ou l'identifiant stocké n'a pas la forme de la plateforme ;
 * * `non_testable` — aucune sonde publique ne tranche. C'est un résultat, pas
 *   un échec : le lecteur officiel de Dailymotion répond 404 hors contexte
 *   d'intégration, y compris sur des vidéos publiques (mesuré le 2026-09-03).
 *   Le déclarer « mort » aurait condamné 143 épisodes qui se lisent très bien.
 */
export type EtatSource = "vivante" | "morte" | "non_testable";

/** Toutes les valeurs d'{@link EtatSource}, pour les validations d'entrée. */
export const ETATS_SOURCE: readonly EtatSource[] = ["vivante", "morte", "non_testable"];

/** Une façon concrète et datée de regarder un épisode. */
export interface SourceEpisode {
	plateforme: Plateforme;
	/** Identifiant de lecture SUR la plateforme (id YouTube, id Dailymotion). */
	sourceId: string;
	/** Page ou flux à ouvrir. */
	url: string;
	langue: LangueSource;
	/** Libellé de définition, quand la source en expose un. */
	qualite: string | null;
	/** Diffusion officielle de l'éditeur ou d'un ayant droit. */
	officielle: boolean;
	confiance: Confiance;
	/** Horodatage ms de la dernière vérification, `null` si jamais vérifiée. */
	verifieeLe: number | null;
	/** Nom de la source d'origine (« Inazuma TV+ (es) », « LEVEL5ch【公式】 »). */
	origine: string;
	vignette: string | null;
	/** Titre tel que CETTE source le nomme — il change avec la langue. */
	titre: string | null;
}

// ── Reconnaissance des plateformes ───────────────────────────────────────────

/** Un identifiant YouTube fait onze caractères de l'alphabet base64url. */
export const RE_YOUTUBE_ID = /^[A-Za-z0-9_-]{11}$/;

/** Identifiant Dailymotion : lettres et chiffres, préfixé `x`. */
export const RE_DAILYMOTION_ID = /^x[A-Za-z0-9]{5,10}$/;

/** Identifiant YouTube porté par n'importe quelle forme d'URL YouTube. */
export function idYoutubeDeUrl(url: string): string | null {
	const trouve =
		/youtube(?:-nocookie)?\.com\/embed\/([A-Za-z0-9_-]{11})/.exec(url) ??
		/youtube\.com\/watch\?(?:[^#]*&)?v=([A-Za-z0-9_-]{11})/.exec(url) ??
		/youtu\.be\/([A-Za-z0-9_-]{11})/.exec(url) ??
		/img\.youtube\.com\/vi\/([A-Za-z0-9_-]{11})/.exec(url) ??
		/i\.ytimg\.com\/vi\/([A-Za-z0-9_-]{11})/.exec(url);
	return trouve ? trouve[1]! : null;
}

/**
 * Identifiant Dailymotion porté par une URL — lecteur, vignette ou page.
 *
 * Les trois formes coexistent réellement dans le corpus : la base ne garde
 * aujourd'hui que la VIGNETTE (`dailymotion.com/thumbnail/video/x7v8ls0`),
 * alors que la page d'épisode publie aussi le lecteur
 * (`dailymotion.com/player/xm8tv.html?video=x7v8ls0`). C'est de là que les 143
 * épisodes hors YouTube tirent leur identifiant de lecture.
 */
export function idDailymotionDeUrl(url: string): string | null {
	const trouve =
		/dailymotion\.com\/player\/[A-Za-z0-9]+\.html\?(?:[^#]*&)?video=([A-Za-z0-9]+)/.exec(url) ??
		/dailymotion\.com\/(?:embed\/)?video\/([A-Za-z0-9]+)/.exec(url) ??
		/dailymotion\.com\/thumbnail\/video\/([A-Za-z0-9]+)/.exec(url) ??
		/dai\.ly\/([A-Za-z0-9]+)/.exec(url);
	return trouve ? trouve[1]! : null;
}

/**
 * Plateforme et identifiant d'une URL, `null` si l'URL n'en désigne aucune.
 *
 * L'ordre compte : une page d'épisode du site officiel PORTE une URL
 * Dailymotion dans son HTML, mais l'URL de la page elle-même n'est ni l'une ni
 * l'autre — elle retombe sur `page`, ce qui est la vérité (« ça s'ouvre là,
 * on ne sait pas l'intégrer »).
 */
export function reconnaitre(url: string): { plateforme: Plateforme; sourceId: string } | null {
	const yt = idYoutubeDeUrl(url);
	if (yt) return { plateforme: "youtube", sourceId: yt };
	const dm = idDailymotionDeUrl(url);
	if (dm) return { plateforme: "dailymotion", sourceId: dm };
	return null;
}

/**
 * URL d'intégration d'une source, `null` quand elle n'est pas intégrable.
 *
 * `youtube-nocookie` et le Dailymotion sans partage ni file d'attente : une
 * grille de catalogue affiche des dizaines de vignettes, elle n'a pas à ouvrir
 * autant de traceurs. Même choix que `urlIntegrationEpisode` côté explorateur —
 * les deux doivent rester d'accord.
 */
export function urlIntegration(source: SourceEpisode, depart?: number): string | null {
	const p = new URLSearchParams({ autoplay: "1" });
	if (depart && depart > 0) p.set("start", String(Math.floor(depart)));
	if (source.plateforme === "youtube") {
		p.set("rel", "0");
		p.set("modestbranding", "1");
		return `https://www.youtube-nocookie.com/embed/${source.sourceId}?${p}`;
	}
	if (source.plateforme === "dailymotion") {
		p.set("queue-enable", "false");
		p.set("sharing-enable", "false");
		return `https://www.dailymotion.com/embed/video/${source.sourceId}?${p}`;
	}
	return null;
}

/** Une source est lisible dans une webview si on sait l'intégrer. */
export function estLisible(source: SourceEpisode): boolean {
	return source.plateforme !== "page";
}

// ── Le catalogue des sources officielles ─────────────────────────────────────

/** Une langue servie par la plateforme officielle européenne. */
export interface LangueOfficielle {
	/** Valeur du paramètre `?lang=` de `inazuma-eleven.fr`. */
	code: string;
	langue: LangueSource;
	/** Nom affichable de la source, tel qu'il entre en base comme « chaîne ». */
	origine: string;
}

/**
 * Les trois langues réellement servies par `inazuma-eleven.fr` — mesurées, pas
 * supposées.
 *
 * Sondées le 2026-09-03 : `fr`, `en` et `es` rendent chacune un index et des
 * pages distinctes. `ja`, `jp`, `vostfr`, `it`, `pt` et `nl` répondent 200 mais
 * servent **octet pour octet la page française** (sha256 identique à `fr` sur
 * `saison1/ep-1`, 24 999 o) : ce sont des retombées silencieuses, pas des
 * langues. Les inscrire ici remplirait le catalogue de copies de la VF sous des
 * étiquettes fausses. Il n'y a donc **ni VO ni VOSTFR** sur cette plateforme.
 *
 * ── `de` EST UNE VRAIE QUATRIÈME LANGUE, CONTRAIREMENT À CE QUI ÉTAIT ÉCRIT ─
 * Cette note affirmait que `de` retombait lui aussi sur la page française.
 * C'est faux au 2026-09-03 : `?lang=de` rend un sha256 DIFFÉRENT (24 481 o) et
 * un identifiant de vidéo distinct — `saison1/ep-1` donne `G_kly6CVpX8` en `de`
 * contre `xbpo3u3P9dc` en `fr`. Vérifié page par page sur les deux premiers
 * arcs : **67 / 67** portent un identifiant allemand. À partir de `saison3` et
 * pour tous les arcs GO, la page `de` n'en porte aucun.
 *
 * `de` est donc inscrit ici, et le travail qu'il implique a été fait : une
 * valeur de plus dans {@link LangueSource}, dans les contraintes `CHECK` des
 * deux tables (par reconstruction — SQLite ne sait pas modifier un `CHECK`), et
 * dans les tableaux de couverture.
 */
export const LANGUES_OFFICIELLES: readonly LangueOfficielle[] = [
	{ code: "fr", langue: "vf", origine: "inazuma-eleven.fr (official)" },
	{ code: "en", langue: "en", origine: "inazuma-eleven.fr (en)" },
	{ code: "es", langue: "es", origine: "inazuma-eleven.fr (es)" },
	{ code: "de", langue: "de", origine: "inazuma-eleven.fr (de)" },
];

/**
 * Numéro de saison par SLUG, et non par rang dans la liste du site.
 *
 * ── UN PIÈGE QUI AURAIT MÉLANGÉ DEUX SAISONS ───────────────────────────────
 * L'ancien code prenait `position` — le rang de la catégorie dans l'index —
 * comme numéro de saison. Or l'index anglais ne publie que quatre catégories :
 * les films y sont en **position 4**, là où ils sont en position 10 en français.
 * Ajouter l'anglais sans changer cette règle aurait rangé les quatre films
 * anglais dans « GO » et fait cohabiter, sous le même numéro d'épisode, un film
 * et un épisode de série.
 *
 * Le slug (`films`, `go`, `chronoStones`) est stable d'une langue à l'autre :
 * c'est lui la clé.
 */
export const SAISON_PAR_SLUG: Readonly<Record<string, number>> = {
	saison1: 1,
	saison2: 2,
	saison3: 3,
	go: 4,
	chronoStones: 5,
	galaxy: 6,
	outerCode: 7,
	ares: 8,
	orion: 9,
	films: 10,
};

/**
 * Numéro de saison d'un slug. Rend `null` sur un slug inconnu — un arc nouveau
 * ne doit pas atterrir sous un numéro pris au hasard, il doit être remarqué.
 */
export function saisonDeSlug(slug: string): number | null {
	return SAISON_PAR_SLUG[slug] ?? null;
}

/**
 * Chaînes YouTube officielles, avec la langue qu'elles diffusent.
 *
 * ── COMMENT CETTE LISTE A ÉTÉ ÉTABLIE ──────────────────────────────────────
 * Quatorze identifiants candidats ont été résolus le 2026-09-03 en lisant
 * l'`externalId` de chaque page de chaîne. N'ont été retenus que ceux dont le
 * TITRE atteste l'officialité (« … officiel », « 【公式】 »). Cinq candidats
 * plausibles par leur handle rendaient en réalité des chaînes de fans
 * (`@InazumaElevenItalia` → « Goku 0402 », `@level5inc` → « j ») : un handle
 * n'est pas une preuve, et ces cinq-là sont écartés.
 *
 * `LEVEL5ch【公式】` est la chaîne de l'éditeur : c'est la seule source **VO**
 * du catalogue, et elle republie la série d'origine épisode par épisode.
 *
 * ── LE TITRE NE SUFFISAIT PAS : LA DESCRIPTION CONTREDIT DEUX ENTRÉES ──────
 * Le critère retenu ci-dessus était le TITRE de la chaîne. Les descriptions ont
 * été lues le 2026-09-03, et deux d'entre elles disent l'inverse de ce que leur
 * titre laissait croire :
 *
 *  * `inazumaelevengofrance` — « Je suis une chaîne **non officielle**, je ne
 *    représente en aucun cas la licence », sur une chaîne qui annonce
 *    « l'intégralité des épisodes des 3 saisons ». C'est une redistribution non
 *    autorisée, énoncée par son auteur même.
 *  * `inazumatvfr` et `InazumaTVFR__` — « Pour voir ou **télécharger** tous les
 *    épisodes d'Inazuma Eleven en VF ou VOSTFR, c'est sur le site Inazuma TV
 *    FR ». Un site qui propose le téléchargement de l'intégralité d'une série
 *    sous licence n'est pas un diffuseur de l'ayant droit. Le compte Dailymotion
 *    `inaztvfr`, même marque, est `verified: false` — son `partner: true` est le
 *    programme de monétisation, pas un statut d'ayant droit.
 *
 * Ces trois entrées sont donc listées dans {@link OFFICIALITE_NON_ETABLIE} et
 * ne doivent pas servir à ÉTENDRE le catalogue. Elles ne sont pas retirées de
 * la liste ci-dessous par ce constat seul : des sources déjà en base en
 * dépendent, et les effacer est un arbitrage éditorial, pas une mesure.
 */
export interface ChaineOfficielle {
	handle: string;
	channelId: string;
	titre: string;
	langue: LangueSource;
}

export const CHAINES_OFFICIELLES: readonly ChaineOfficielle[] = [
	{
		handle: "inazumaelevenfrance1",
		channelId: "UCGMvTdioudzJSa5uTAY6FDw",
		titre: "Inazuma Eleven France officiel",
		langue: "vf",
	},
	{
		handle: "inazumaelevengofrance",
		channelId: "UCWY2mgz63totT3D_E8SqiFA",
		titre: "Inazuma Eleven Go France",
		langue: "vf",
	},
	{
		handle: "InazumaTVFR__",
		channelId: "UCXFes6UCUtCUZXFVYT8AD7A",
		titre: "Inazuma TV FR",
		langue: "vf",
	},
	{
		handle: "inazumatvfr",
		channelId: "UC1cdmvDug3oRgl_d-w1fdTg",
		titre: "Inazuma TV FR (VOSTFR)",
		langue: "vostfr",
	},
	{
		handle: "LEVEL5ch",
		channelId: "UClfhcLqicImW9Se7NKaFADQ",
		titre: "LEVEL5ch【公式】",
		langue: "vo",
	},
];

/**
 * Chaînes dont le DROIT DE DIFFUSER n'est pas établi — à ne pas moissonner.
 *
 * Chaque entrée porte la phrase mesurée qui la disqualifie, pas une impression.
 * Voir l'en-tête de {@link CHAINES_OFFICIELLES} pour la méthode.
 *
 * ── POURQUOI UNE LISTE, ET PAS UNE SUPPRESSION ─────────────────────────────
 * Retirer ces chaînes de la liste précédente les ferait simplement redécouvrir
 * au prochain élargissement, avec les mêmes titres rassurants et sans la
 * mesure qui les écarte. Une liste nommée porte le constat ; c'est ce qui
 * empêche de refaire l'erreur.
 */
export const OFFICIALITE_NON_ETABLIE: readonly { handle: string; motif: string }[] = [
	{
		handle: "inazumaelevengofrance",
		motif: "sa description se declare « chaine non officielle », et elle heberge les 3 saisons de GO",
	},
	{
		handle: "inazumatvfr",
		motif: "renvoie vers un site proposant le TELECHARGEMENT de tous les episodes VF et VOSTFR",
	},
	{
		handle: "InazumaTVFR__",
		motif: "meme marque et meme renvoi que inazumatvfr",
	},
	{
		// Le compte Dailymotion de la même marque. Il est nommé ici et pas
		// ailleurs pour que `moissonnable` réponde sur les deux plateformes.
		handle: "inaztvfr",
		motif: "compte Dailymotion de la marque Inazuma TV FR, non verifie, meme renvoi",
	},
];

/** Une chaîne est moissonnable si son droit de diffuser n'est pas contesté. */
export function moissonnable(handle: string): boolean {
	return !OFFICIALITE_NON_ETABLIE.some((c) => c.handle === handle);
}

/**
 * Numéro d'épisode porté par un titre de vidéo, toutes conventions confondues.
 *
 * Les cinq chaînes ne nomment pas pareil, et chacune est réellement observée
 * dans les flux du 2026-09-03 :
 * `Épisode 127 "…"`, `INAZUMA ELEVEN VF - EP59 - …`,
 * `Inazuma Eleven Go Galaxy - 25 - "…"`, `「イナズマイレブン」第67話 …`.
 *
 * Rend `null` quand rien ne tranche : une bande-annonce n'est pas l'épisode 8
 * parce que son titre contient un huit.
 */
export function numeroEpisodeDeTitre(titre: string): number | null {
	const motifs = [
		/第\s*(\d{1,3})\s*話/, // japonais : 第67話
		/\bEP\s*\.?\s*(\d{1,3})\b/i, // EP59, EP. 59
		// PAS de `\b` devant la classe : `É` n'est pas un caractère de mot en
		// regex JavaScript, donc `\bÉpisode` ne matche JAMAIS après une espace.
		// Ce détail coûtait toute la chaîne « Inazuma Eleven France officiel »,
		// dont les 15 entrées s'intitulent « … - Épisode 127 "…" » : la moisson
		// n'en tirait zéro épisode, sans la moindre erreur.
		/[ÉE]pisode\s+(\d{1,3})\b/i, // Épisode 127
		/\bEpisodio\s+(\d{1,3})\b/i,
		/\bCap[íi]tulo\s+(\d{1,3})\b/i,
		/\s-\s(\d{1,3})\s*-\s/, // « Galaxy - 25 - "…" »
	];
	for (const motif of motifs) {
		const trouve = motif.exec(titre);
		if (trouve) {
			const n = Number.parseInt(trouve[1]!, 10);
			if (n > 0 && n < 1000) return n;
		}
	}
	return null;
}

/**
 * Arc désigné par un titre, et son mode de numérotation.
 *
 * ── SANS ÇA, TROIS FLUX ONT FABRIQUÉ 23 ÉPISODES QUI N'EXISTENT PAS ────────
 * Mesuré lors de la première moisson complète : les flux Atom retombaient sur
 * « saison 1 » faute de marqueur explicite dans le titre. Résultat en base,
 * `INAZUMA ELEVEN VF - EP45` … `EP59` rangés en **saison 1, épisodes 45 à 59**,
 * alors que la saison 1 en compte 26 ; `第55話` … `第67話` de même ; et
 * `Inazuma Eleven Go Galaxy - 25` rangé en saison 1 au lieu de la saison 6. La
 * saison 1 annonçait 41 épisodes en VF pour 26 réels, et le total distinct
 * passait de 355 à 378 — 23 épisodes purement inventés, tous d'apparence
 * parfaitement normale.
 *
 * Le nom de l'arc est dans le titre ; il suffit de le lire. Quand il n'y en a
 * pas, le numéro est ABSOLU sur la série d'origine (1 à 127, à répartir en arcs
 * de 26, 41 et 60) — c'est le rôle de `situerAbsolu`.
 *
 * L'ordre des motifs est critique : « GO GALAXY » contient « GO », et « GO
 * CHRONO STONE » aussi. Tester « GO » d'abord range tout Galaxy en saison 4.
 * Les motifs les plus spécifiques passent en premier.
 */
export interface ArcSerie {
	/** Numéro de saison du catalogue, ou `null` quand le numéro est absolu. */
	saison: number | null;
	absolu: boolean;
}

const ARCS: readonly { motif: RegExp; arc: ArcSerie }[] = [
	{ motif: /GO\s+CHRONO\s*STONES?|クロノ・?ストーン/i, arc: { saison: 5, absolu: false } },
	{ motif: /GO\s+GALAXY|ギャラクシー/i, arc: { saison: 6, absolu: false } },
	{ motif: /OUTER\s*CODE/i, arc: { saison: 7, absolu: false } },
	{ motif: /\bARES\b|アレスの天秤/i, arc: { saison: 8, absolu: false } },
	{ motif: /\bORION\b|オリオンの刻印/i, arc: { saison: 9, absolu: false } },
	{ motif: /\bGO\b|イナズマイレブンGO/i, arc: { saison: 4, absolu: false } },
	// En dernier, le gabarit sans nom d'arc : la série d'origine, numérotée
	// d'une traite sur ses trois premières saisons. Le titre japonais
	// 「イナズマイレブン」 en fait partie — c'est ce que republie LEVEL5ch.
	{ motif: /INAZUMA\s+ELEVEN|イナズマイレブン/i, arc: { saison: null, absolu: true } },
];

/** Arc d'un titre, `null` quand rien ne correspond. */
export function arcDeTitre(titre: string): ArcSerie | null {
	for (const { motif, arc } of ARCS) {
		if (motif.test(titre)) return arc;
	}
	return null;
}

/**
 * Tailles des arcs de la série d'origine — le barème de `situerAbsolu`.
 *
 * Mesurées sur le catalogue officiel lui-même, pas citées : 26, 41 et 60, soit
 * 127 épisodes.
 */
export const ARCS_SERIE_ORIGINE: readonly { season: number; totalEpisodes: number }[] = [
	{ season: 1, totalEpisodes: 26 },
	{ season: 2, totalEpisodes: 41 },
	{ season: 3, totalEpisodes: 60 },
];

/**
 * Clé de dédoublonnage d'une source.
 *
 * Deux sources sont la MÊME quand elles désignent la même vidéo dans la même
 * langue : c'est le cas du site officiel et d'une chaîne YouTube qui pointent
 * tous deux `xbpo3u3P9dc` — 211 des 355 épisodes actuels sont dans ce cas. La
 * plateforme entre dans la clé parce qu'un identifiant Dailymotion et un
 * identifiant YouTube peuvent, en principe, se ressembler.
 *
 * ── LE SÉPARATEUR EST CONSTRUIT, PAS ÉCRIT ─────────────────────────────────
 * Ce gabarit portait deux octets NUL *réels*, posés là par un outil d'édition.
 * Le code marchait — mais `file` classait la source en `data` et `rg` refusait
 * de la parcourir (« binary file matches »), donc toute recherche dans le
 * module rendait silencieusement zéro ligne. Le remplacer par la séquence
 * échappée ne suffit pas : plusieurs outils d'édition la reconvertissent en
 * octet en écrivant le fichier, et le piège revient à l'identique (vécu deux
 * fois le 2026-09-03). {@link SEPARATEUR_CLE} le fabrique donc à l'exécution :
 * la chaîne produite est la même, et la source reste du texte quoi qu'il
 * arrive.
 */
export const SEPARATEUR_CLE = String.fromCharCode(31);

/** Clé de dédoublonnage — cf. {@link SEPARATEUR_CLE}. */
export function cleSource(source: SourceEpisode): string {
	return [source.plateforme, source.sourceId, source.langue].join(SEPARATEUR_CLE);
}

/**
 * Fusionne des sources en gardant, pour chaque clé, la MIEUX ÉTABLIE.
 *
 * L'ordre de préférence est celui de la confiance, jamais celui d'arrivée :
 * un flux qui « déclare » une vidéo ne doit pas écraser la même vidéo dont la
 * page a été ouverte et lue. À confiance égale, la première gagne — le site
 * officiel passe avant les flux dans l'ordre d'appel.
 */
export function fusionnerSources(sources: readonly SourceEpisode[]): SourceEpisode[] {
	const rang: Record<Confiance, number> = { verifiee: 0, declaree: 1, deduite: 2 };
	const par = new Map<string, SourceEpisode>();
	for (const source of sources) {
		const cle = cleSource(source);
		const present = par.get(cle);
		if (!present || rang[source.confiance] < rang[present.confiance]) {
			par.set(cle, source);
		}
	}
	return [...par.values()];
}
