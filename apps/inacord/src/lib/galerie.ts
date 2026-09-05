// Galerie d'illustrations — **logique pure** (aucun import Tauri) : d'où viennent les images,
// comment les catégoriser et comment les nommer. `GalleryView` exécute, ce module décide.
//
// ## Pourquoi la galerie du bureau ne ressemble pas à celle du web
//
// Le wiki n'a pas les fichiers du jeu : il compose sa galerie à partir de DEUX fonds qui ne se
// rejoignent jamais — la table `inagle_gallery` (360 lignes, filtrées par `ilike img_path`) et un
// manifeste statique `packages/azalee/src/data/menu-gallery-manifest.json` (3 579 entrées, figé,
// régénéré à la main par `scripts/build-menu-gallery-manifest.ts` depuis un index CPK exporté).
// De là son défaut visible : la pastille « Toutes » annonce 3 939 items alors que la liste sans
// catégorie n'en rend que 360, les deux fonds n'étant pas réunis.
//
// L'explorateur a le VFS monté. Il n'a donc besoin ni de la table ni du manifeste pour SAVOIR CE
// QUI EXISTE : `data/dx11/menu/220_img/` porte **17 085 fichiers `.g4tx`** (mesuré le 2026-09-02
// sur l'installation de référence), soit 4,8 fois le manifeste, et le compte est exact parce
// qu'il est relevé, pas recopié. Les catégories ne sont pas une liste écrite d'avance : ce sont
// les sous-dossiers réels, découverts par `api.ls`. Un dossier ajouté par une mise à jour du jeu
// apparaît tout seul.
//
// La table `gallery_config` (via `api.gameDataGallery`) garde un rôle, mais le bon : elle
// n'énumère pas, elle ENRICHIT — condition de déblocage, épisode d'histoire, vignette dédiée.

/** Racine VFS des illustrations de menu. */
export const RACINE_GALERIE = "data/dx11/menu/220_img";

/** Extension des conteneurs d'illustration — la seule que `thumbs.ts` sait décoder. */
export const EXT_GALERIE = "g4tx";

/**
 * Libellés FR des sous-dossiers connus.
 *
 * Les six premiers viennent des catégories du wiki (`GALLERY_CATEGORIES`), les suivants sont les
 * dossiers que le VFS porte en plus et que le manifeste ignorait. Un dossier absent de cette
 * table reste affiché — sous son nom mis en forme par [`libelleCategorie`] : ne jamais masquer ce
 * qu'on n'a pas su nommer.
 */
export const LIBELLES_CATEGORIE: Record<string, string> = {
  gallery_img2: "Galerie",
  gallery_thumb2: "Galerie (vignettes)",
  ev_pic: "Événements",
  stadium: "Stades",
  vsroute_map: "Cartes de route",
  hlp: "Aide",
  telop_waza: "Bandeaux de technique",
  ev_chronicle_img: "Chroniques",
  activity_photo: "Photos d'activité",
  quest_img: "Quêtes",
  theater_img: "Théâtre",
  ev_telop: "Bandeaux d'événement",
  bookmark_img: "Marque-pages",
  stamp_img: "Tampons",
};

/**
 * Sous-dossiers de langue — reconnus pour être proposés comme filtre secondaire plutôt que
 * mélangés au reste. `telop_waza` porte 1 246 images PAR langue : sans ce découpage, la catégorie
 * afficherait neuf fois la même illustration.
 */
export const LANGUES = new Set(["de", "en", "es", "fr", "it", "pt", "zh_hans", "zh_hant"]);

/** Libellés des langues, pour les pastilles de filtre. */
export const LIBELLES_LANGUE: Record<string, string> = {
  de: "Allemand",
  en: "Anglais",
  es: "Espagnol",
  fr: "Français",
  it: "Italien",
  pt: "Portugais",
  zh_hans: "Chinois simplifié",
  zh_hant: "Chinois traditionnel",
};

/** Met un identifiant de dossier en forme lisible (`ev_chronicle_img` → `Ev Chronicle Img`). */
export function libelleCategorie(nom: string): string {
  const connu = LIBELLES_CATEGORIE[nom];
  if (connu) return connu;
  const propre = nom.replaceAll("_", " ").trim();
  return propre.replace(/\b\w/g, (c) => c.toUpperCase()) || nom;
}

/** Libellé d'un sous-dossier : nom de langue quand c'en est une, mise en forme sinon. */
export function libelleSousDossier(nom: string): string {
  return LIBELLES_LANGUE[nom] ?? libelleCategorie(nom);
}

/**
 * Titre lisible d'une illustration, dérivé de son chemin VFS.
 *
 * Même règle que `build-menu-gallery-manifest.ts` du wiki (préfixes décoratifs retirés,
 * soulignés en espaces, capitales initiales) — c'est bien le meilleur libellé dérivable : aucune
 * table de texte du jeu ne nomme ces images. `thumb_` s'ajoute à la liste des préfixes, absent du
 * wiki parce que son manifeste n'indexait pas `gallery_thumb2`.
 */
export function titreIllustration(chemin: string): string {
  const base = chemin.slice(chemin.lastIndexOf("/") + 1).replace(/\.g4tx$/i, "");
  const propre = base.replace(/^(img_|thumb_|back_|hlp_|grid_)/, "").replaceAll("_", " ").trim();
  return propre.replace(/\b\w/g, (c) => c.toUpperCase()) || base;
}

/** Catégorie (1er segment sous la racine) d'un chemin d'illustration, `null` hors galerie. */
export function categorieDe(chemin: string): string | null {
  if (!chemin.startsWith(`${RACINE_GALERIE}/`)) return null;
  const reste = chemin.slice(RACINE_GALERIE.length + 1);
  const barre = reste.indexOf("/");
  return barre === -1 ? null : reste.slice(0, barre);
}

/** Sous-dossier (2e segment) d'un chemin d'illustration, `null` s'il est à plat dans sa catégorie. */
export function sousDossierDe(chemin: string): string | null {
  const cat = categorieDe(chemin);
  if (!cat) return null;
  const reste = chemin.slice(RACINE_GALERIE.length + cat.length + 2);
  const barre = reste.indexOf("/");
  return barre === -1 ? null : reste.slice(0, barre);
}

/** Préfixe VFS d'une catégorie, éventuellement restreinte à un sous-dossier. */
export function prefixeCategorie(categorie: string, sousDossier?: string | null): string {
  return sousDossier
    ? `${RACINE_GALERIE}/${categorie}/${sousDossier}/`
    : `${RACINE_GALERIE}/${categorie}/`;
}

/**
 * Vignette dédiée d'une illustration, quand le jeu en fournit une.
 *
 * `gallery_config` apparie `img_<x>` (dans `gallery_img2/`, ~8 Mo par fichier) et
 * `thumb_<x>` (dans `gallery_thumb2/`, ~55 Ko) — vérifié le 2026-09-02 sur
 * `img_story_ev07_main_0090`. Décoder la vignette du jeu au lieu de réduire l'image pleine, c'est
 * 150 fois moins d'octets à lire dans le CPK. Hors de ce couple, on rend `null` : `thumbs.ts`
 * réduira l'image elle-même côté Rust, ce qui reste borné.
 */
export function vignetteDediee(chemin: string): string | null {
  const cat = categorieDe(chemin);
  if (cat !== "gallery_img2") return null;
  const base = chemin.slice(chemin.lastIndexOf("/") + 1);
  if (!base.startsWith("img_")) return null;
  return `${RACINE_GALERIE}/gallery_thumb2/thumb_${base.slice(4)}`;
}

/** Une illustration prête à afficher. */
export interface Illustration {
  /** Chemin VFS du conteneur pleine résolution. */
  chemin: string;
  /** Chemin VFS de la vignette à décoder (dédiée si elle existe, sinon l'image elle-même). */
  cheminVignette: string;
  titre: string;
  categorie: string;
  sousDossier: string | null;
  octets: number;
  /** Condition de déblocage lue dans `gallery_config`, `null` si l'image n'y figure pas. */
  deblocage: string | null;
  /** Épisode d'histoire associé (`gallery_config`), `null` sinon. */
  episode: number | null;
}

/** Ce que `gallery_config` sait d'une illustration, indexé par nom de fichier sans extension. */
export interface EnrichissementGalerie {
  deblocage: string;
  episode: number | null;
}

/**
 * Assemble la liste affichable : le VFS dit ce qui existe, `gallery_config` complète ce qu'il
 * connaît. L'ordre d'entrée est conservé (chemins triés par `find_paged`).
 */
export function construireIllustrations(
  fichiers: readonly { path: string; size: number }[],
  enrichissements: ReadonlyMap<string, EnrichissementGalerie>,
): Illustration[] {
  return fichiers.map((f) => {
    const base = f.path.slice(f.path.lastIndexOf("/") + 1).replace(/\.g4tx$/i, "");
    const extra = enrichissements.get(base);
    return {
      chemin: f.path,
      cheminVignette: vignetteDediee(f.path) ?? f.path,
      titre: titreIllustration(f.path),
      categorie: categorieDe(f.path) ?? "",
      sousDossier: sousDossierDe(f.path),
      octets: f.size,
      deblocage: extra?.deblocage ?? null,
      episode: extra?.episode ?? null,
    };
  });
}

/** Filtre une liste d'illustrations par sous-chaîne (titre ou nom de fichier), insensible à la casse. */
export function filtrerIllustrations(
  liste: readonly Illustration[],
  recherche: string,
): Illustration[] {
  const besoin = recherche.trim().toLowerCase();
  if (!besoin) return [...liste];
  return liste.filter(
    (i) => i.titre.toLowerCase().includes(besoin) || i.chemin.toLowerCase().includes(besoin),
  );
}
