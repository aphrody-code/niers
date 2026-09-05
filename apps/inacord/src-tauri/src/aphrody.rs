//! Façade IPC de `nie-aphrody` : le pet Aphrody, et la chaîne pixel-perfect.
//!
//! Même règle que le reste de ce backend — **ce module n'est qu'une façade**. La mesure, la
//! comparaison, la vectorisation et l'assemblage de planches vivent dans
//! [`nie_aphrody::pixel`] ; l'écriture du CSS, du SVG et du JSON d'une feuille de sprites vit
//! dans `nie_formats::sprite_sheet`. Rien de tout cela n'est réécrit ici, et c'est là-bas que
//! les tests s'exécutent réellement : le harnais de test de ce paquet Tauri ne démarre pas sur
//! toutes les plateformes (`STATUS_ENTRYPOINT_NOT_FOUND` avant le premier test, cf. `CLAUDE.md`).
//!
//! Les DTO ci-dessous sont locaux **volontairement** : les crates moteur du dépôt ne dépendent
//! d'aucun `specta` (vérifié : zéro occurrence dans leurs `Cargo.toml`), et leur en imposer un
//! pour le confort d'une application ferait payer une dépendance d'interface à `nie-wasm`, aux
//! goldens et à la forge.

use base64::Engine as _;
use serde::Serialize;

use nie_aphrody::pixel::{
    Boite, Image, Masque, Reglages, ReglagesVecteur, comparer, mesurer, planche, tokens_css,
    vectoriser,
};
use nie_aphrody::{Pet, assets};

/// Rend l'erreur telle quelle : l'interface affiche le message du domaine, pas un « échec ».
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn charger(chemin: &str) -> Result<Image, String> {
    Image::charger(std::path::Path::new(chemin)).map_err(err)
}

/// Traduit les réglages venus du front en masque du domaine.
///
/// `mode` vaut `alpha`, `sombre` ou `teinte`. Un mode inconnu est une **erreur**, pas un repli
/// silencieux sur `alpha` : un masque qui n'est pas celui demandé produit des mesures
/// plausibles et fausses, le pire des deux mondes.
fn masque_depuis(
    mode: &str,
    seuil: Option<u32>,
    teinte_min: Option<f64>,
    teinte_max: Option<f64>,
    saturation: Option<f64>,
) -> Result<Masque, String> {
    let seuil_u8 = || u8::try_from(seuil.unwrap_or(8).min(255)).unwrap_or(255);
    match mode {
        "alpha" => Ok(Masque::Alpha(seuil_u8())),
        "sombre" => Ok(Masque::Sombre(seuil_u8())),
        "teinte" => Ok(Masque::Teinte {
            min: teinte_min.unwrap_or(0.0),
            max: teinte_max.unwrap_or(360.0),
            sat: saturation.unwrap_or(0.25),
        }),
        autre => Err(format!("masque inconnu « {autre} » (alpha, sombre ou teinte)")),
    }
}

/// Une couleur de la palette mesurée (miroir IPC de `nie_aphrody::pixel::Couleur`).
#[derive(Serialize, specta::Type)]
pub struct CouleurDto {
    /// Part des pixels retenus, en pourcentage.
    pub part_pct: f64,
    /// Forme `#RRGGBB`.
    pub hex: String,
    /// `oklch(L C h)` prêt à coller dans une feuille de style.
    pub oklch: String,
    /// Teinte HSL en degrés, pour trier une palette à l'affichage.
    pub teinte_deg: f64,
}

/// Ce qu'une mesure rend au front (miroir IPC de `nie_aphrody::pixel::Mesure`).
///
/// Les profils de silhouette et les bords ligne à ligne **ne sont pas exposés** : ce sont
/// plusieurs milliers de valeurs par mesure, que l'interface ne sait pas afficher et que l'IPC
/// paierait à chaque appel. Ils restent accessibles côté Rust pour qui en a besoin.
#[derive(Serialize, specta::Type)]
pub struct MesureDto {
    /// Largeur de l'image source.
    pub source_largeur: u32,
    /// Hauteur de l'image source.
    pub source_hauteur: u32,
    /// Boîte englobante du sujet : `[x0, y0, x1, y1]`, bornes incluses.
    pub boite: [u32; 4],
    /// Largeur de la boîte.
    pub largeur: u32,
    /// Hauteur de la boîte.
    pub hauteur: u32,
    /// Largeur / hauteur.
    pub ratio: f64,
    /// Part de la boîte occupée, en pourcentage (78,54 % = un disque plein).
    pub remplissage_pct: f64,
    /// Part de l'image entière occupée.
    pub part_image_pct: f64,
    /// Épaisseur médiane du trait, en pourcentage de la largeur.
    pub trait_pct: f64,
    /// Angle du bord gauche par rapport à la verticale, en degrés.
    pub angle_gauche_deg: f64,
    /// Angle du bord droit.
    pub angle_droit_deg: f64,
    /// Le pire des deux R² d'ajustement des bords.
    pub bord_droiture: f64,
    /// Faux quand `bord_droiture` est sous 0,95 : les angles ci-dessus ne veulent alors **rien
    /// dire** et l'interface doit refuser de les proposer en `skewX`. Le calcul du seuil est
    /// fait ici pour qu'aucun front n'ait à le réinventer — ni à l'oublier.
    pub angles_exploitables: bool,
    /// Palette, triée par part décroissante.
    pub palette: Vec<CouleurDto>,
}

/// Le verdict d'une comparaison (miroir IPC de `nie_aphrody::pixel::Comparaison`).
#[derive(Serialize, specta::Type)]
pub struct ComparaisonDto {
    /// Similarité structurelle hybride RGB, 0 à 1.
    pub ssim: f64,
    /// Part des pixels dont chaque canal tient dans la tolérance.
    pub pixels_dans_tolerance_pct: f64,
    /// Tolérance appliquée, par canal.
    pub tolerance: u32,
    /// Vrai si les deux images sont rigoureusement égales.
    pub identique: bool,
}

/// Une planche assemblée et les trois formes que le web en attend.
#[derive(Serialize, specta::Type)]
pub struct PlancheDto {
    /// Largeur de la planche.
    pub largeur: u32,
    /// Hauteur de la planche.
    pub hauteur: u32,
    /// Nombre de sprites.
    pub sprites: u32,
    /// PNG de la planche, en base64 — affichable directement en `data:`.
    pub png_b64: String,
    /// Feuille CSS, telle que l'écrit `nie_formats::sprite_sheet`.
    pub css: String,
    /// SVG autonome à `<symbol>`, image embarquée en `data:`.
    pub svg: String,
    /// JSON des régions.
    pub json: String,
}

/// L'état du pet embarqué.
#[derive(Serialize, specta::Type)]
pub struct PetEtatDto {
    /// Nom affichable du pet.
    pub nom: String,
    /// Dimensions de l'atlas.
    pub atlas: [u32; 2],
    /// Grille de l'atlas : colonnes, lignes.
    pub grille: [u32; 2],
    /// Noms des animations disponibles.
    pub animations: Vec<String>,
    /// Nombre de frames vérifiées par le diagnostic.
    pub frames_verifiees: u32,
    /// Vrai si le diagnostic ne relève aucune erreur.
    pub ok: bool,
    /// Erreurs relevées, vides quand `ok`.
    pub erreurs: Vec<String>,
}

fn mesure_dto(m: &nie_aphrody::pixel::Mesure) -> MesureDto {
    MesureDto {
        source_largeur: m.source[0],
        source_hauteur: m.source[1],
        boite: [m.boite.x0, m.boite.y0, m.boite.x1, m.boite.y1],
        largeur: m.boite.largeur(),
        hauteur: m.boite.hauteur(),
        ratio: m.ratio,
        remplissage_pct: m.remplissage_pct,
        part_image_pct: m.part_image_pct,
        trait_pct: m.trait_pct,
        angle_gauche_deg: m.angle_gauche_deg,
        angle_droit_deg: m.angle_droit_deg,
        bord_droiture: m.bord_droiture,
        angles_exploitables: m.bord_droiture >= 0.95,
        palette: m
            .palette
            .iter()
            .map(|c| CouleurDto {
                part_pct: c.part_pct,
                hex: c.hex.clone(),
                oklch: format!("oklch({:.4} {:.4} {:.2})", c.oklch[0], c.oklch[1], c.oklch[2]),
                teinte_deg: c.hsl[0],
            })
            .collect(),
    }
}

/// Construit les réglages d'une mesure depuis les paramètres IPC.
fn reglages_depuis(
    k: Option<u32>,
    boite: Option<Vec<u32>>,
    masque: Masque,
) -> Result<Reglages, String> {
    let boite = match boite {
        Some(b) if b.len() == 4 => Some(Boite { x0: b[0], y0: b[1], x1: b[2], y1: b[3] }),
        Some(_) => return Err("la boîte attend exactement 4 valeurs : x0 y0 x1 y1".into()),
        None => None,
    };
    let mut r = Reglages { masque, boite, ..Reglages::default() };
    if let Some(k) = k {
        r.k = k.max(1) as usize;
    }
    Ok(r)
}

/// Mesure une image du disque.
pub fn mesurer_fichier(
    chemin: &str,
    k: Option<u32>,
    boite: Option<Vec<u32>>,
    mode: &str,
    seuil: Option<u32>,
    teinte_min: Option<f64>,
    teinte_max: Option<f64>,
    saturation: Option<f64>,
) -> Result<MesureDto, String> {
    let masque = masque_depuis(mode, seuil, teinte_min, teinte_max, saturation)?;
    let reglages = reglages_depuis(k, boite, masque)?;
    let m = mesurer(&charger(chemin)?, reglages).map_err(err)?;
    Ok(mesure_dto(&m))
}

/// Palette d'une image en propriétés personnalisées CSS.
pub fn tokens_css_fichier(chemin: &str, prefixe: &str, k: Option<u32>) -> Result<String, String> {
    let reglages = reglages_depuis(k, None, Masque::Alpha(8))?;
    let m = mesurer(&charger(chemin)?, reglages).map_err(err)?;
    Ok(tokens_css(&m, prefixe))
}

/// Compare deux images du disque.
pub fn comparer_fichiers(a: &str, b: &str, tolerance: Option<u32>) -> Result<ComparaisonDto, String> {
    let tol = u8::try_from(tolerance.unwrap_or(0).min(255)).unwrap_or(255);
    let c = comparer(&charger(a)?, &charger(b)?, tol).map_err(err)?;
    Ok(ComparaisonDto {
        ssim: c.ssim,
        pixels_dans_tolerance_pct: c.pixels_dans_tolerance_pct,
        tolerance: u32::from(c.tolerance),
        identique: c.identique,
    })
}

/// Vectorise une image du disque en SVG.
pub fn vectoriser_fichier(
    chemin: &str,
    k: Option<u32>,
    tolerance: Option<f64>,
    mode: &str,
    seuil: Option<u32>,
) -> Result<String, String> {
    let mut r = ReglagesVecteur { masque: masque_depuis(mode, seuil, None, None, None)?, ..ReglagesVecteur::default() };
    if let Some(k) = k {
        r.k = k.max(1) as usize;
    }
    if let Some(t) = tolerance {
        r.tolerance = t;
    }
    vectoriser(&charger(chemin)?, r).map_err(err)
}

/// Assemble des images en planche et rend PNG + CSS + SVG + JSON.
///
/// Le nom de chaque sprite vient du **fichier**, jamais de son rang : un rang se décale au
/// premier ajout et tous les sélecteurs CSS déjà écrits pointent alors ailleurs.
pub fn planche_fichiers(
    chemins: &[String],
    colonnes: Option<u32>,
    nom: &str,
) -> Result<PlancheDto, String> {
    let mut images = Vec::with_capacity(chemins.len());
    for chemin in chemins {
        let p = std::path::Path::new(chemin);
        let nom_sprite = p
            .file_stem()
            .map_or_else(|| chemin.clone(), |s| s.to_string_lossy().into_owned());
        images.push((nom_sprite, charger(chemin)?));
    }
    let (img, feuille) = planche(&images, colonnes.map(|c| c.max(1) as usize), nom).map_err(err)?;
    let png = assets::encoder_png(&img.rgba, img.largeur, img.hauteur).map_err(err)?;
    let png_b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Ok(PlancheDto {
        largeur: img.largeur,
        hauteur: img.hauteur,
        sprites: u32::try_from(feuille.sprites.len()).unwrap_or(u32::MAX),
        css: feuille.vers_css(&format!("{nom}.png")),
        svg: feuille.vers_svg(&nie_formats::sprite_sheet::data_uri(&png, "image/png")),
        json: feuille.vers_json(),
        png_b64,
    })
}

/// État du pet embarqué, diagnostic compris.
pub fn pet_etat() -> Result<PetEtatDto, String> {
    let pet = Pet::bundled().map_err(err)?;
    let rapport = pet.diagnose();
    let atlas = &pet.manifest.atlas;
    Ok(PetEtatDto {
        nom: pet.manifest.pet.display_name.clone(),
        atlas: [atlas.width, atlas.height],
        grille: [atlas.columns, atlas.rows],
        animations: pet.manifest.animations.keys().cloned().collect(),
        frames_verifiees: u32::try_from(rapport.checked_frames).unwrap_or(u32::MAX),
        ok: rapport.ok(),
        erreurs: rapport.errors.clone(),
    })
}

/// Une frame du pet, extraite sans rééchantillonnage, en PNG base64.
///
/// `index` est l'indice **dans l'animation**, pas dans l'atlas.
pub fn pet_frame_png_b64(animation: &str, index: u32) -> Result<String, String> {
    let pet = Pet::bundled().map_err(err)?;
    let anim = pet
        .animation(animation)
        .ok_or_else(|| format!("animation « {animation} » absente du pet"))?;
    let frame = anim
        .frames
        .get(index as usize)
        .ok_or_else(|| format!("frame {index} absente : l'animation en compte {}", anim.frames.len()))?;
    let rgba = pet.extract(frame).map_err(err)?;
    let (w, h) = (frame.atlas_rect.width, frame.atlas_rect.height);
    let png = assets::encoder_png(&rgba, w, h).map_err(err)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}
