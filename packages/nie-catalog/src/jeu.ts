/**
 * Le gisement **jeu** : les fichiers du jeu tels que `nie-model-serve` les décode à la volée.
 *
 * Rien n'est extrait ni copié ici — on ne fabrique que des URL. Le serveur fait le reste :
 * `.g4tx` → PNG, `.acb`/`.awb` → WAV, `.usm` → MP4/WebM, `.g4mg` → GLB.
 *
 * ## Ce module est la source unique des conventions d'URL
 *
 * Elles sont celles du serveur, pas les nôtres. Trois pièges, tous vérifiés dans
 * `crates/tools/nie-model-serve/src/main.rs` :
 *
 * 1. **`/tex/<chemin sans `.g4tx`>.png`** — garder l'extension donne un 400 « chemin invalide » ;
 * 2. **`/vfs/*` prend son chemin en query** (`?path=`), alors que toutes les autres routes le
 *    prennent en **segment** ;
 * 3. **le préfixe `data/` est optionnel** sur les routes à segment : le serveur le repose quand il
 *    manque (`if rest.starts_with("data/") { … } else { format!("data/{rest}") }`). Les deux
 *    formes sont donc valides, et un chemin déjà préfixé ne doit pas être tronqué.
 *
 * Un 404 vient presque toujours de l'URL, jamais du décodage — d'où ces constructeurs, plutôt
 * qu'une chaîne réécrite à chaque appel.
 *
 * ## Deux surfaces que le serveur n'implémente PAS
 *
 * `/dx11/…` et `/g4tx/…` n'existent pas dans `main.rs` : ce sont des `location` nginx
 * (`/etc/nginx/conf.d/cdn.rosegriffon.conf`). `/dx11/` sert le dump sur disque puis retombe sur
 * `@cpk_live`, qui réécrit `/dx11/<x>.png` → `/tex/dx11/<x>.png` ; `/g4tx/` passe par
 * `cdn-variants` (redimensionnement `?w=`, `format=webp`) puis réécrit vers `/tex/`. Elles ne
 * sont donc PAS interchangeables avec `/tex/` : `/dx11/` et `/g4tx/` savent redimensionner, pas
 * `/tex/`. C'est pourquoi elles vivent ici sous leur propre nom, et non comme un alias.
 *
 * ## Client-safe
 *
 * Ce module ne touche ni au disque ni à SQLite : il est importable depuis un composant
 * `"use client"`. Il n'importe volontairement **pas** `./sources.ts`, qui résout les trois autres
 * gisements par `node:fs` — ce seul import suffisait à interdire son usage dans un bundle
 * navigateur, et donc à le disqualifier comme point de convergence.
 */

/** La base HTTP retenue quand `NIE_CDN_URL` n'est pas posée. */
export const BASE_JEU_DEFAUT = "https://cdn.rosegriffon.fr";

/**
 * La base HTTP du serveur de décodage.
 *
 * `NIE_CDN_URL` la force ; une variable posée mais **vide** est ignorée (une chaîne vide n'est
 * pas une base, elle fabriquerait des URL relatives silencieusement fausses). Le `process`
 * est sondé avant d'être lu : dans un bundle navigateur il peut ne pas exister du tout.
 */
export function baseJeu(): string {
	const forcee =
		typeof process === "undefined" ? undefined : process.env["NIE_CDN_URL"]?.trim();
	return (forcee || BASE_JEU_DEFAUT).replace(/\/+$/, "");
}

// ---------------------------------------------------------------------------
// Les chemins, sans base.
//
// C'est la forme que réutilisent les surfaces qui portent DÉJÀ leur propre origine (le wiki
// Azalée sert `https://cdn.rosegriffon.fr` en dur, l'explorateur parle à `127.0.0.1:8790`) :
// elles concatènent leur base à ces chemins, et la convention reste écrite à un seul endroit.
// ---------------------------------------------------------------------------

/** Retire le suffixe `.g4tx`, quelle qu'en soit la casse. */
function sansG4tx(chemin: string): string {
	return chemin.replace(/\.g4tx$/i, "");
}

/** `/health` — la sonde de vie du serveur. */
export function cheminSante(): string {
	return "/health";
}

/** `/raw/<chemin>` — les octets bruts, décompressés et déchiffrés. */
export function cheminFichier(chemin: string): string {
	return `/raw/${chemin}`;
}

/** `/vfs/stat?path=<chemin>` — taille, rôle, formats d'export, description. */
export function cheminFiche(chemin: string): string {
	return `/vfs/stat?${new URLSearchParams({ path: chemin })}`;
}

/** Les options de pagination communes à `/vfs/find` et `/vfs/ls`. */
export interface OptionsRecherche {
	/** Filtre par extension, sans point (`g4tx`). Une chaîne vide est ignorée par le serveur. */
	ext?: string;
	/** Taille de page. Le serveur plafonne à 20 000 (`main.rs`, `param_usize(query, "limit", …)`). */
	limite?: number;
	/** Décalage. Omis, il n'apparaît pas dans l'URL — le serveur vaut 0 par défaut. */
	decalage?: number;
}

/**
 * `/vfs/find?q=…&limit=…[&offset=…][&ext=…]` — recherche par sous-chaîne dans les chemins.
 *
 * **L'ordre des paramètres est celui que la production émet déjà** (`q`, `limit`, `offset`, puis
 * `ext`) : une URL est une clé de cache, et en changer l'ordre invaliderait tout ce qui est déjà
 * en cache chez nginx comme chez les navigateurs. C'est aussi l'ordre dans lequel
 * `crates/tools/nie-model-serve/src/main.rs:3055` documente la route. Le serveur lit ses
 * paramètres par leur nom (`param(query, "q")`, `param_usize(query, "limit", 200)`,
 * `param_usize(query, "offset", 0)`), donc l'ordre ne change rien pour LUI — seulement pour le
 * cache.
 *
 * `decalage` n'est écrit que s'il est donné : c'est ce qui permet à une URL sans pagination de
 * rester la chaîne courte qu'elle a toujours été.
 */
export function cheminRecherche(texte: string, options: OptionsRecherche = {}): string {
	const q = new URLSearchParams({ q: texte, limit: String(options.limite ?? 100) });
	if (options.decalage !== undefined) {
		q.set("offset", String(options.decalage));
	}
	if (options.ext) {
		q.set("ext", options.ext);
	}
	return `/vfs/find?${q}`;
}

/**
 * `/vfs/ls?path=…&limit=…[&offset=…]` — le listing d'un dossier.
 *
 * Même règle que [`cheminRecherche`] pour `decalage` : absent des options, il est absent de l'URL.
 * `limit` à 0 ne rend que les sous-dossiers et le total — ce que veut un arbre qui n'affiche que
 * la structure.
 */
export function cheminListe(dossier: string, limite = 500, decalage?: number): string {
	const q = new URLSearchParams({ path: dossier, limit: String(limite) });
	if (decalage !== undefined) {
		q.set("offset", String(decalage));
	}
	return `/vfs/ls?${q}`;
}

/** `/vfs/stats` — les compteurs globaux du VFS monté. */
export function cheminStatsVfs(): string {
	return "/vfs/stats";
}

/** `/tex/<chemin sans `.g4tx`>.png` — une texture décodée en PNG. */
export function cheminTexture(chemin: string): string {
	return `/tex/${sansG4tx(chemin)}.png`;
}

/**
 * `/tex/<conteneur>.g4tx/<nom>.png` — UNE texture nommée d'un conteneur G4TX.
 *
 * Un `.g4tx` porte jusqu'à 203 images : sans le nom, la route rend « la plus grande » et tout le
 * reste devient invisible. Le serveur ne se rabat jamais sur un défaut quand le nom est donné —
 * un nom inconnu répond 404, plutôt qu'une image arbitraire qui passerait pour la bonne.
 */
export function cheminTextureNommee(chemin: string, nom: string): string {
	return `/tex/${sansG4tx(chemin)}.g4tx/${nom}.png`;
}

/** `/tex-info/<chemin>.g4tx` — le catalogue des textures d'un conteneur. */
export function cheminCatalogueTextures(chemin: string): string {
	return `/tex-info/${sansG4tx(chemin)}.g4tx`;
}

/** `/video/<chemin>` — une cinématique remuxée dans un conteneur que le navigateur lit. */
export function cheminFilm(chemin: string): string {
	return `/video/${chemin}`;
}

/** `/video/<chemin>?track=audio` — la bande-son d'un film : elle vit à côté, pas dedans. */
export function cheminBandeSon(chemin: string): string {
	return `/video/${chemin}?track=audio`;
}

/** `/video/<chemin>?info=1` — la fiche détaillée d'un film, remux mesuré compris. */
export function cheminFicheFilm(chemin: string): string {
	return `/video/${chemin}?info=1`;
}

/** `/video/catalog.json` — l'inventaire complet des cinématiques. */
export function cheminCatalogueFilms(): string {
	return "/video/catalog.json";
}

/** `/audio-info/<chemin>` — le catalogue des cues d'une banque sonore. */
export function cheminBanqueSon(chemin: string): string {
	return `/audio-info/${chemin}`;
}

/**
 * `/audio/<chemin>` — une piste décodée en WAV PCM 16 bits.
 *
 * On adresse par `awbId` (le cue-id AFS2 publié par `/audio-info`) et non par rang : le rang
 * dépend de l'ordre du fichier, l'identifiant est stable. Sans lui, le serveur rend la piste la
 * plus volumineuse de la banque.
 */
export function cheminAudio(chemin: string, awbId?: number | null): string {
	const base = `/audio/${chemin}`;
	return awbId == null ? base : `${base}?id=${awbId}`;
}

/** `/cfg/<chemin>.json` — un `cfg.bin` décodé en RDBN/T2B générique. */
export function cheminCfg(chemin: string): string {
	return `/cfg/${chemin}.json`;
}

/** `/typed/<chemin>.json` — le même `cfg.bin`, décodé en structure de jeu typée `nie-data`. */
export function cheminTypee(chemin: string): string {
	return `/typed/${chemin}.json`;
}

/** `/typed/.../<screen>_setting.cfg.bin.json` — définition typée d'un écran de menu. */
export function menuSettingPath(screen: string): string {
	return cheminTypee(`data/common/gamedata/menu/cfg/${screen}_setting.cfg.bin`);
}

/**
 * `/model-full/<code>.glb` — un personnage assemblé (corps + visage + uniforme).
 *
 * Les trois composants vivent dans trois arbres distincts du VFS ; c'est cette route qui les
 * réunit. Elle répond 404 pour ce qu'elle ne sait pas produire (un uniforme seul, un visage) —
 * on ne l'enferme donc pas derrière une liste écrite à la main.
 */
export function cheminModeleComplet(code: string): string {
	return `/model-full/${code}.glb`;
}

/**
 * `/model-chr/<sous-domaine>/<code>.glb` — un modèle non-personnage assemblé.
 *
 * Le sous-domaine s'écrit **sans** le tiret bas du dossier VFS : `waza`, pas `_waza`. L'écrire
 * autrement rend un 404 « sous-domaine chr non servable ».
 */
export function cheminModeleChr(sousDomaine: string, code: string): string {
	return `/model-chr/${sousDomaine}/${code}.glb`;
}

/** `/model-avatar/<pièces>.glb` — l'avatar composé des pièces choisies. */
export function cheminModeleAvatar(pieces: string): string {
	return `/model-avatar/${pieces}.glb`;
}

/** `/model-edit/<code>.glb` — une pièce de l'éditeur d'avatar. */
export function cheminModeleEdit(code: string): string {
	return `/model-edit/${code}.glb`;
}

/** `/model-map/<code>.glb` — une carte de jeu. */
export function cheminModeleCarte(code: string): string {
	return `/model-map/${code}.glb`;
}

/**
 * `/export/<chemin>?format=…` — un export nommé par le serveur.
 *
 * `id` désigne la **sous-entité** (une cue dans une banque, une texture dans un G4TX) : sans lui,
 * tous les exports d'un même conteneur se recouvriraient sous le nom du fichier source. Le
 * serveur pose le `Content-Disposition` ; c'est lui qui donne son vrai nom au fichier reçu — un
 * `<a download>` vers une origine tierce ne peut pas l'imposer.
 */
export function cheminExport(chemin: string, format: string, id?: string | number): string {
	const q = new URLSearchParams({ format });
	if (id !== undefined) {
		q.set("id", String(id));
	}
	return `/export/${chemin}?${q}`;
}

/** `/avatar/catalog.json` — le catalogue des parts d'avatar. */
export function cheminCatalogueAvatar(): string {
	return "/avatar/catalog.json";
}

/** `/avatar/layout/<écran>.json` — la mise en page d'un écran de l'éditeur d'avatar. */
export function cheminLayoutAvatar(ecran: string): string {
	return `/avatar/layout/${ecran}.json`;
}

/** `/avatar/icon/<nom>.png` — une vignette de part, décodée depuis son atlas. */
export function cheminIconeAvatar(nom: string): string {
	return `/avatar/icon/${nom}.png`;
}

/** `/ui/theme.json` — la palette `FONT_COLOR` du jeu et ses polices. */
export function cheminTheme(): string {
	return "/ui/theme.json";
}

/** `/icons/index.json` — l'index des icônes de menu. */
export function cheminIndexIcones(): string {
	return "/icons/index.json";
}

// ---------------------------------------------------------------------------
// Les deux surfaces nginx — `/dx11/` et `/g4tx/`.
//
// Elles ne sont PAS des routes de `nie-model-serve` (le test `jeu.test.ts` l'exige) : ce sont
// des `location` de `/etc/nginx/conf.d/cdn.rosegriffon.conf`, qui savent redimensionner avant de
// réécrire vers `/tex/`. Les remplacer par `cheminTexture` ferait perdre `?w=` et `format=webp`.
// ---------------------------------------------------------------------------

/** Options de rendu d'une image servie par les `location` nginx. */
export interface OptionsImage {
	/** Largeur cible en pixels ; le redimensionnement est fait par `cdn-variants`. */
	largeur?: number;
	/** Sert la variante WebP plutôt que le PNG. N'a de sens qu'avec `largeur`. */
	webp?: boolean;
}

/** Ajoute `?w=…&format=webp` à un chemin d'image, dans cet ordre — c'est la forme en cache. */
function avecVariante(chemin: string, options: OptionsImage): string {
	if (options.largeur === undefined) {
		return chemin;
	}
	return options.webp === false
		? `${chemin}?w=${options.largeur}`
		: `${chemin}?w=${options.largeur}&format=webp`;
}

/**
 * `/dx11/<chemin sous `data/dx11/`, sans `.g4tx`>.png` — une texture du dump PC.
 *
 * Les `.g4tx` vivent à 100 % sous `data/dx11/`. nginx sert d'abord le dump sur disque, puis
 * retombe sur `@cpk_live`, qui réécrit vers `/tex/dx11/<x>.png`. Le chemin donné peut porter ou
 * non son préfixe `data/dx11/` : les deux formes rendent la même URL.
 */
export function cheminDx11(chemin: string, options: OptionsImage = {}): string {
	const relatif = chemin.startsWith("data/dx11/")
		? chemin.slice("data/dx11/".length)
		: chemin.replace(/^data\//, "");
	return avecVariante(`/dx11/${sansG4tx(relatif)}.png`, options);
}

/**
 * Une image que le serveur a DÉJÀ nommée, éventuellement redimensionnée.
 *
 * `/tex-info` publie pour chaque texture d'un conteneur son propre `path` (`/tex/<…>.g4tx/<nom>
 * .png`) : on le sert tel quel plutôt que de le reconstruire, et on n'y ajoute que la variante.
 */
export function cheminImage(chemin: string, options: OptionsImage = {}): string {
	return avecVariante(chemin, options);
}

// ---------------------------------------------------------------------------
// Les URL absolues — un chemin, préfixé de [`baseJeu`].
// ---------------------------------------------------------------------------

/** Les octets bruts d'un fichier du VFS, décompressés et déchiffrés. */
export function urlFichier(chemin: string): string {
	return baseJeu() + cheminFichier(chemin);
}

/** Les métadonnées d'un fichier : taille, rôle, formats d'export disponibles. */
export function urlFiche(chemin: string): string {
	return baseJeu() + cheminFiche(chemin);
}

/** Recherche par sous-chaîne dans les 255 308 entrées du VFS. */
export function urlRecherche(texte: string, options: OptionsRecherche = {}): string {
	return baseJeu() + cheminRecherche(texte, options);
}

/** Le listing d'un dossier du VFS. */
export function urlListe(dossier: string, limite = 500, decalage?: number): string {
	return baseJeu() + cheminListe(dossier, limite, decalage);
}

/** Une texture, décodée en PNG. Le `.g4tx` se retire — le garder donne un 400. */
export function urlTexture(chemin: string): string {
	return baseJeu() + cheminTexture(chemin);
}

/** Une texture nommée à l'intérieur d'un conteneur G4TX. */
export function urlTextureNommee(chemin: string, nom: string): string {
	return baseJeu() + cheminTextureNommee(chemin, nom);
}

/** Le catalogue des textures d'un conteneur G4TX. */
export function urlCatalogueTextures(chemin: string): string {
	return baseJeu() + cheminCatalogueTextures(chemin);
}

/** Une cinématique, remuxée dans un conteneur que le navigateur lit. */
export function urlFilm(chemin: string): string {
	return baseJeu() + cheminFilm(chemin);
}

/** La bande-son d'une cinématique — elle vit à côté du film, pas dedans. */
export function urlBandeSon(chemin: string): string {
	return baseJeu() + cheminBandeSon(chemin);
}

/** La fiche détaillée d'un film. */
export function urlFicheFilm(chemin: string): string {
	return baseJeu() + cheminFicheFilm(chemin);
}

/** Le catalogue complet des cinématiques, publié hors ligne par `niers video catalogue`. */
export function urlCatalogueFilms(): string {
	return baseJeu() + cheminCatalogueFilms();
}

/** Le catalogue des cues d'une banque sonore. */
export function urlBanqueSon(chemin: string): string {
	return baseJeu() + cheminBanqueSon(chemin);
}

/** Une piste audio décodée en WAV. */
export function urlAudio(chemin: string, awbId?: number | null): string {
	return baseJeu() + cheminAudio(chemin, awbId);
}

/** Un `cfg.bin` décodé en RDBN/T2B générique. */
export function urlCfg(chemin: string): string {
	return baseJeu() + cheminCfg(chemin);
}

/** Un `cfg.bin` décodé en structure de jeu typée. */
export function urlTypee(chemin: string): string {
	return baseJeu() + cheminTypee(chemin);
}

/** URL absolue de la définition typée d'un écran de menu. */
export function menuSettingUrl(screen: string): string {
	return baseJeu() + menuSettingPath(screen);
}

/** Un personnage assemblé, en GLB. */
export function urlModeleComplet(code: string): string {
	return baseJeu() + cheminModeleComplet(code);
}

/** Un modèle non-personnage assemblé, en GLB. */
export function urlModeleChr(sousDomaine: string, code: string): string {
	return baseJeu() + cheminModeleChr(sousDomaine, code);
}

/** Un export nommé. */
export function urlExport(chemin: string, format: string, id?: string | number): string {
	return baseJeu() + cheminExport(chemin, format, id);
}

/** Une texture du dump PC, servie par la `location` nginx qui sait la redimensionner. */
export function urlDx11(chemin: string, options: OptionsImage = {}): string {
	return baseJeu() + cheminDx11(chemin, options);
}

/** Une image déjà nommée par le serveur, préfixée de la base et éventuellement redimensionnée. */
export function urlImage(chemin: string, options: OptionsImage = {}): string {
	return baseJeu() + cheminImage(chemin, options);
}

/** Vrai si le serveur de décodage répond. Une seconde d'attente au plus : c'est une sonde. */
export async function jeuJoignable(delaiMs = 1000): Promise<boolean> {
	try {
		const reponse = await fetch(baseJeu() + cheminSante(), {
			signal: AbortSignal.timeout(delaiMs),
		});
		return reponse.ok;
	} catch {
		return false;
	}
}

// ---------------------------------------------------------------------------
// Les fiches que le gisement publie — et leurs formateurs.
//
// Une convention d'URL ne suffit pas : deux surfaces qui appellent la même route doivent aussi
// lire la même forme de réponse, et l'écrire de la même façon. Ces types décrivent ce que
// `nie_explore::cinema` et `nie_explore::audio` sérialisent sur `/video/catalog.json`,
// `/video/<x>?info=1` et `/audio-info/<x>` ; ils vivent ici pour la même raison que les URL.
// ---------------------------------------------------------------------------

/** Une piste sonore portée par le conteneur du film lui-même — 2 films sur 97. */
export interface FilmPisteInterne {
	/** Numéro de canal déclaré par le conteneur. */
	canal: number;
	/** Codec de la piste (`hca`, `adx`…). */
	codec: string;
	/** Fréquence d'échantillonnage, en hertz. */
	frequence: number;
	/** Nombre de canaux. */
	canaux: number;
	/** Taille de la piste, en octets. */
	octets: number;
}

/** La bande-son d'un film qui n'en porte pas dans son conteneur. */
export interface FilmBandeSon {
	/** Nom de la cue dans `anime_stream`, ex. `ev01_00050_bgm`. */
	cue: string;
	/** Identifiant AFS2 de la forme d'onde. */
	awbId: number;
	/** Codec déclaré par la banque. */
	codec: string;
	/** Fréquence d'échantillonnage, en hertz. */
	frequence: number;
	/** Nombre de canaux. */
	canaux: number;
	/** Durée de la cue, en millisecondes — ce que le jeu joue. */
	dureeMs: number;
	/** Durée de la forme d'onde, en millisecondes — ce que le fichier contient. */
	dureeOndeMs: number;
	/** Vrai quand le `bgmName` du gamedata confirme la cue trouvée par son nom. */
	confirmeParHash: boolean;
}

/** Ce que les tables du jeu disent d'un film (`movie_playing_config`, `event_movie_config`). */
export interface FilmGamedata {
	/** Fichier de jeu d'où vient la ligne. */
	source: string;
	/** Identifiant du film, tel que le jeu le hache. */
	movieId?: string;
	/** Événement d'histoire qui déclenche le film. */
	eventId?: string;
	/** Menu depuis lequel le film est joué. */
	menuId?: string;
	/** Identifiant de la légende associée. */
	captionId?: string;
	/** « Nom de musique » — en réalité le CRC32 du nom du film. */
	bgmName?: string;
	/** Durée du fondu d'entrée, en secondes. */
	fedeInTime?: number;
	/** Durée du fondu de sortie, en secondes. */
	fedeOutTime?: number;
	/** Générique joué par-dessus le film, quand il y en a un. */
	staffrollDataName?: string;
	/** Chemin des textes de sous-titres, `<LG>` restant à substituer par la langue. */
	subtitleTextPath?: string;
	/** Chemin des réglages de sous-titres. */
	subtitleSettingPath?: string;
}

/**
 * La fiche d'un film telle que le SERVEUR la publie.
 *
 * À ne pas confondre avec le `FilmDto` de l'explorateur Tauri : celui-là est **généré** depuis le
 * même crate par `tauri-specta` (`apps/inacord/src/lib/bindings.ts`) et porte les noms de
 * champs Rust (`nom_origine`, `sous_titres`, `lisible`). Les deux décrivent la même fonction, pas
 * la même sérialisation — les unifier exige de régénérer les bindings, pas de recopier un type.
 */
export interface FilmDto {
	/** Chemin VFS complet du film. */
	chemin: string;
	/** Radical du fichier (`ev01_00050`) — la clé de tout : jointure, cue, libellé. */
	nom: string;
	/** Rubrique déduite du nom, convention du jeu (`Chapitre 01`, `Écrans-titres`…). */
	rubrique: string;
	/** Code de langue quand le nom en porte un, `null` sinon. */
	langue: string | null;
	/** Taille du conteneur `.usm`, en octets. */
	octets: number;
	/** Message d'erreur si le film n'a pas pu être lu. */
	erreur?: string;
	/** Codec vidéo constaté : `h264`, `mpeg2`, `vp9`. */
	codec: string;
	/** Vrai si un navigateur sait décoder ce codec — faux pour les 20 MPEG-2. */
	lisibleNavigateur: boolean;
	/** Largeur en pixels. */
	largeur: number;
	/** Hauteur en pixels. */
	hauteur: number;
	/** Nombre d'images réellement présentes dans le conteneur. */
	images: number;
	/** Nombre d'images que l'en-tête annonce. */
	totalImagesDeclare: number;
	/** Cadence en images par seconde, `null` si l'en-tête ne la déclare pas. */
	cadence: number | null;
	/** Durée en secondes. */
	duree: number | null;
	/** Total des octets vidéo, hors en-têtes de bloc et bourrage. */
	octetsVideo: number;
	/** Vrai si le conteneur était chiffré par l'enveloppe CRI. */
	dechiffre?: boolean;
	/** Nom du fichier tel que l'encodeur l'a inscrit. */
	nomOrigine?: string;
	/** Pistes sonores du conteneur — vide pour 95 films sur 97. */
	audio: FilmPisteInterne[];
	/** Bande-son externe résolue dans `anime_stream`, quand le conteneur est muet. */
	bandeSon?: FilmBandeSon;
	/** Nombre de blocs de sous-titres du conteneur. */
	sousTitres?: number;
	/** Type MIME du conteneur web produit par le remux (fiche détaillée seulement). */
	conteneur?: string;
	/** Taille du conteneur web produit, en octets. */
	conteneurOctets?: number;
	/** Nombre d'images-clés — ce sur quoi un lecteur peut se repositionner. */
	cles?: number;
	/** Part du fichier économisée par le remux, en pourcentage. */
	gainRemux?: number;
	/** Raison pour laquelle aucun conteneur web n'est possible. */
	remuxImpossible?: string;
	/** Ce que les tables du jeu disent du film. */
	gamedata?: FilmGamedata;
}

/** Une langue du jeu, code et nom. */
export interface LangueDto {
	/** Code tel qu'il apparaît dans les noms de fichiers (`JP`, `fr`…). */
	code: string;
	/** Nom en français. */
	nom: string;
}

/** Le catalogue complet des cinématiques, tel que `/video/catalog.json` le rend. */
export interface CatalogueVideo {
	/** Les films, triés par chemin. */
	films: FilmDto[];
	/** Les rubriques présentes — de quoi bâtir un filtre sans le deviner. */
	rubriques: string[];
	/** Les neuf langues du jeu. */
	langues: LangueDto[];
	/** Empreinte du corpus servie par le serveur (nombre de films : volume total). */
	empreinte?: string;
}

/** Un cue d'une banque audio, résolu jusqu'à sa forme d'onde (`/audio-info/<x>`). */
export interface AudioCue {
	/** Rang du cue dans la `CueTable` de la banque. */
	index: number;
	/** Identifiant du cue (`CueTable.CueId`). */
	cueId: number;
	/** Nom du cue, ex. `ev60_00010_me`. `null` si la banque n'en donne pas. */
	name: string | null;
	/** Codec de la forme d'onde : `hca`, `adx`, `autre` ou `inconnu`. */
	codec: string;
	/** Nombre de canaux, `null` si non résolu. */
	channels: number | null;
	/** Fréquence d'échantillonnage en Hz, `null` si non résolue. */
	sampleRate: number | null;
	/** Nombre d'échantillons, `null` si non résolu. */
	numSamples: number | null;
	/** Durée en secondes. */
	durationSec: number;
	/** La forme d'onde boucle (typique des BGM). */
	looped: boolean;
	/** La forme d'onde vit dans l'AWB externe plutôt qu'en mémoire. */
	streaming: boolean;
	/** Identifiant AWB — c'est LUI qu'on passe à `?id=`, pas `index`. */
	awbId: number | null;
	/** Rang de l'entrée dans l'AWB, quand l'en-tête AFS2 embarqué permet de le résoudre. */
	awbIndex: number | null;
}

/** Catalogue complet d'une banque audio. */
export interface AudioBank {
	/** Chemin VFS de l'ACB. */
	path: string;
	/** Type de conteneur (`acb`, `hca`, `adx`). */
	container: string;
	/** Nom de la cue sheet déclaré par la banque. */
	name: string | null;
	/** Version de l'outil CRI ayant produit la banque. */
	version: number | null;
	/** Nombre de cues. */
	cueCount: number;
	/** Nombre d'entrées dans l'AWB, `null` si l'en-tête n'est pas embarqué. */
	awbEntryCount: number | null;
	/** La banque porte son AWB en interne. */
	embeddedAwb: boolean;
	/** Chemin VFS de l'AWB frère, `null` si le conteneur n'est pas un ACB. */
	externalAwb: string | null;
	/** Les cues, dans l'ordre de la `CueTable`. */
	cues: AudioCue[];
}

/** Deux chiffres, zéro devant. */
function deuxChiffres(n: number): string {
	return String(n).padStart(2, "0");
}

/**
 * Durée d'un film en `m:ss`, ou `h:mm:ss` au-delà de l'heure. `null` si la durée est inconnue.
 *
 * Rendre `null` plutôt qu'un `--:--` laisse l'appelant décider de ce qu'il affiche à la place :
 * une liste veut sauter la cellule, un lecteur veut un gabarit. C'est pour cela que l'explorateur
 * Tauri garde le sien, qui rend `--:--` — deux besoins, pas deux copies de la même règle.
 */
export function formatDuree(secondes: number | null | undefined): string | null {
	if (secondes == null || !Number.isFinite(secondes) || secondes <= 0) {
		return null;
	}
	const total = Math.round(secondes);
	const h = Math.floor(total / 3600);
	const m = Math.floor((total % 3600) / 60);
	const s = total % 60;
	return h > 0
		? `${h}:${deuxChiffres(m)}:${deuxChiffres(s)}`
		: `${m}:${deuxChiffres(s)}`;
}

/**
 * Durée d'un cue audio : `m:ss` à partir de la minute, `12,3 s` en deçà, `0,50 s` sous la seconde.
 *
 * Distincte de [`formatDuree`] et volontairement : une cue de voix dure une seconde et demie, et
 * `0:02` en dirait moins que `1,50 s`. La virgule décimale est celle du français.
 */
export function formatDureeCue(secondes: number): string {
	if (!Number.isFinite(secondes) || secondes <= 0) {
		return "—";
	}
	if (secondes < 1) {
		return `${secondes.toFixed(2).replace(".", ",")} s`;
	}
	const m = Math.floor(secondes / 60);
	const s = Math.round(secondes % 60);
	return m > 0
		? `${m}:${deuxChiffres(s)}`
		: `${secondes.toFixed(1).replace(".", ",")} s`;
}

/** Taille en unités binaires, telle qu'on l'annonce à côté d'un lien de téléchargement. */
export function formatOctets(octets: number): string {
	const mio = octets / 1024 ** 2;
	return mio >= 1024
		? `${(mio / 1024).toFixed(1).replace(".", ",")} Gio`
		: `${Math.round(mio)} Mio`;
}

/**
 * Définition `1920×1080`, ou `null` quand le conteneur ne la déclare pas.
 *
 * Prend les deux nombres plutôt qu'une fiche : c'est la même règle pour le `FilmDto` du serveur
 * (`largeur: number`) et pour celui de l'explorateur (`largeur: number | null`), qui ne sont pas
 * le même type.
 */
export function formatDefinition(
	largeur: number | null | undefined,
	hauteur: number | null | undefined,
): string | null {
	return largeur && hauteur ? `${largeur}×${hauteur}` : null;
}

/**
 * Ordre d'affichage des rubriques : les chapitres dans l'ordre du jeu, le reste ensuite.
 *
 * Un tri alphabétique mettrait « Chronicle » entre deux chapitres et « Écrans-titres » en tête à
 * cause de son accent. L'ordre du récit est celui qui se lit.
 */
export function ordreRubrique(rubrique: string): number {
	const chapitre = /^Chapitre (\d+)$/.exec(rubrique);
	if (chapitre) {
		return Number(chapitre[1]);
	}
	if (rubrique === "Chronicle") {
		return 900;
	}
	if (rubrique === "Écrans-titres") {
		return 901;
	}
	return 902;
}

/**
 * Le format de téléchargement qui correspond au codec d'un film.
 *
 * H.264 → MP4, VP9 → WebM, MPEG-2 → flux élémentaire `.m2v` (VLC et mpv le lisent ; aucun
 * navigateur ne le décode, et l'emballer en MP4 serait un mensonge).
 */
export function formatSortie(codec: string | null | undefined): {
	id: string;
	ext: string;
	libelle: string;
} {
	switch (codec) {
		case "vp9":
			return { id: "webm", ext: "webm", libelle: "WebM" };
		case "mpeg2":
			return { id: "m2v", ext: "m2v", libelle: "MPEG-2" };
		default:
			return { id: "mp4", ext: "mp4", libelle: "MP4" };
	}
}
