/**
 * Client des routes REELLES de `crates/tools/nie-site` : chaque champ ci-dessous
 * est copie des structures Rust (`routes/api_v1.rs`, `routes/mod.rs`,
 * `index_vfs.rs`, `etat.rs`), aucune n'est devinee. Un chemin VFS cite de
 * memoire est presque toujours faux — c'est le serveur qui les enumere.
 *
 * Deux espaces, un seul principe (amendement A3) : le chemin VFS voyage EN
 * SEGMENT, verbatim, extension du jeu conservee. Jamais en query. Les vues
 * nommees ne designent pas des fichiers : ce sont des filtres enregistres.
 */

/** Une ressource du jeu, par son chemin VFS exact. */
export const urlFichier = (cheminVfs: string) => `/f/${cheminVfs}`;

/**
 * Le contenu d'un prefixe du VFS, filtres compris.
 *
 * Les filtres voyagent en QUERY parce que ce n'en sont pas des chemins ; le prefixe, lui, reste
 * en segment (amendement A3). Une valeur vide n'est pas envoyee : `/b?q=` est un `400` cote
 * serveur, et a raison de l'etre — ni « pas de filtre » ni « egal a la chaine vide » ne sont
 * devinables.
 */
export const urlDossier = (
	prefixe = "",
	filtres: {
		q?: string;
		ext?: string;
		tri?: string;
		ordre?: string;
		tailleMin?: number;
		tailleMax?: number;
		parPage?: number;
		page?: number;
	} = {},
) => {
	const base = prefixe ? `/b/${prefixe}` : "/b";
	const params = new URLSearchParams();
	if (filtres.q?.trim()) params.set("q", filtres.q.trim());
	if (filtres.ext?.trim()) params.set("ext", filtres.ext.trim().replace(/^\./, ""));
	if (filtres.tri?.trim()) params.set("tri", filtres.tri.trim());
	if (filtres.ordre?.trim()) params.set("ordre", filtres.ordre.trim());
	// `0` est une borne LEGITIME (il existe des fichiers de zero octet, mesure du 2026-09-06) :
	// tester la verite ferait disparaitre `taille_max=0` en silence.
	if (Number.isFinite(filtres.tailleMin)) params.set("taille_min", String(filtres.tailleMin));
	if (Number.isFinite(filtres.tailleMax)) params.set("taille_max", String(filtres.tailleMax));
	// Sans `per_page`, le serveur en rend 50 — et un dossier de 373 entrees se presentait comme
	// un dossier de 50, avec le bon total a cote. Le defaut ne se voyait pas a l'ecran.
	if (filtres.parPage) params.set("per_page", String(filtres.parPage));
	if (filtres.page && filtres.page > 1) params.set("page", String(filtres.page));
	const query = params.toString();
	return query ? `${base}?${query}` : base;
};

/** Les quatre filtres enregistres servis par `/api/v1/<vue>`. */
export type VueCatalogue = "textures" | "modeles" | "sons" | "videos";

/** Une page, telle que `Page<T>` la serialise. */
export interface Page<T> {
	elements: T[];
	page: number;
	per_page: number;
	total: number;
	pages: number;
}

/** Une entree du VFS (`index_vfs::Fichier`). */
export interface Fichier {
	/** Chemin VFS verbatim — c'est aussi l'URL, sous `/f/`. */
	chemin: string;
	/** Nom de la feuille, extension du jeu conservee. */
	nom: string;
	taille: number;
	/**
	 * Le CPK d'origine, quand la route le rend.
	 *
	 * Optionnel parce que les routes ne le rendent pas toutes : `/api/v1/recherche` le publie
	 * depuis le 2026-09-06, `/b` non. Le declarer obligatoire ferait mentir le type sur la
	 * seconde ; l'omettre le ferait mentir sur la premiere.
	 */
	cpk?: string | null;
}

/** Ce que le serveur sait faire a l'instant de la mesure (`etat::Capacites`). */
export interface Capacites {
	/** `en_cours`, `pret` ou `absent` — l'index du VFS se monte en tache de fond. */
	vfs: "en_cours" | "pret" | "absent";
	vfs_entrees: number;
	vfs_dump: boolean;
	vfs_contenu: boolean;
	gisement: boolean;
	bundle: boolean;
}

/** Corps de `/api/v1/health`. */
export interface SanteApi {
	service: string;
	api: string;
	version: string;
	capacites: Capacites;
	/** Un resume par filtre ; `total` reste `null` tant que le VFS n'est pas pret. */
	vues: { nom: string; extensions: string[]; total: number | null }[];
}

async function lire<T>(url: string, signal?: AbortSignal): Promise<T> {
	const r = await fetch(url, { signal, headers: { accept: "application/json" } });
	if (!r.ok) throw new Error(`${url} a repondu ${r.status}`);
	return (await r.json()) as T;
}

/** Une page d'un catalogue. `per_page` est borne a 200 par le serveur. */
export function catalogue(
	vue: VueCatalogue,
	{
		page = 1,
		parPage = 60,
		q,
		ext,
		tri,
		ordre,
		signal,
	}: {
		page?: number;
		parPage?: number;
		q?: string;
		ext?: string;
		tri?: string;
		ordre?: string;
		signal?: AbortSignal;
	} = {},
): Promise<Page<Fichier>> {
	// `q` est comparé sans casse au chemin ENTIER côté serveur : chercher `chr/` fonctionne
	// autant qu'un nom de fichier. `URLSearchParams` encode tout, ce qui compte ici : un chemin
	// du jeu contient des `/`, et un motif tapé par un humain peut contenir un `&`.
	const params = new URLSearchParams({ page: String(page), per_page: String(parPage) });
	// Une valeur vide n'est PAS envoyée : `?ext=` est un 400 côté serveur, et il a raison — ni
	// « pas de filtre » ni « extension vide » ne sont devinables.
	if (q?.trim()) params.set("q", q.trim());
	if (ext?.trim()) params.set("ext", ext.trim().replace(/^\./, ""));
	if (tri?.trim()) params.set("tri", tri.trim());
	if (ordre?.trim()) params.set("ordre", ordre.trim());
	return lire(`/api/v1/${vue}?${params}`, signal);
}

/** L'etat du serveur. */
export const sante = (signal?: AbortSignal) => lire<SanteApi>("/api/v1/health", signal);
