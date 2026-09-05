//! Aperçus exploitables des caméras de cinématique (`.g4cm`) et des navmesh (`.g4nv`).
//!
//! Les décodeurs de `nie-formats` rendent la structure **complète** du fichier : en-tête,
//! compteurs, canaux, sommets, coins, arêtes, et jusqu'aux octets de rembourrage — tout ce
//! qu'il faut pour réencoder à l'octet près. C'est la bonne granularité pour la forge, pas
//! pour une vue : envoyer ça tel quel à l'IPC noierait l'utile sous le fidèle.
//!
//! Ce module aplatit chaque format en ce qui se dessine :
//!
//! - une caméra devient des **pistes** `(objet, canal, temps → valeur)`, prêtes à tracer ;
//! - un navmesh devient des **triangles** et des **arêtes** en coordonnées monde, avec sa
//!   boîte englobante, prêts à projeter en plan.
//!
//! Ce qui n'est pas résolu est signalé, jamais deviné : un canal dont le flux n'est pas `f32`
//! (encodage 2 octets non élucidé) sort avec `resolu = false` et sans valeurs, plutôt qu'avec
//! des nombres inventés qui auraient l'air d'une trajectoire.

use nie_formats::vfs::Vfs;
use serde::Serialize;

/// Plafond de sommets renvoyés pour un navmesh.
///
/// Les navmesh du jeu tiennent largement en dessous (quelques milliers de sommets) ; le
/// plafond protège l'IPC d'un fichier aberrant plutôt qu'il ne tronque un cas normal. Quand il
/// mord, `tronque` passe à `true` — l'affichage doit le dire, pas laisser croire à un maillage
/// complet.
const MAX_SOMMETS: usize = 60_000;

/// Plafond d'échantillons par piste de caméra, même raison.
const MAX_ECHANTILLONS: usize = 20_000;

// ─────────────────────────────────────────────────────────────────────────────
// Caméra (`.g4cm`)
// ─────────────────────────────────────────────────────────────────────────────

/// Une piste d'animation : un canal d'un objet, échantillonné dans le temps.
#[derive(Serialize, specta::Type)]
pub struct PisteCameraDto {
    /// Nom de l'objet animé, tel qu'il figure dans la table de noms du fichier.
    pub objet: String,
    /// Canal : `PosX`, `PosY`, `PosZ`, `RefX`, `RefY`, `RefZ`, `Fov`, ou `Inconnu(0x..)`.
    pub canal: String,
    /// `true` si le flux est un `f32` décodé — donc si `valeurs` a un sens.
    pub resolu: bool,
    /// Numéros de frame des échantillons.
    pub temps: Vec<f32>,
    /// Valeurs, vides quand `resolu` est faux.
    pub valeurs: Vec<f32>,
}

/// Un clip : un intervalle de frames déclaré en tête de fichier.
#[derive(Serialize, specta::Type)]
pub struct ClipCameraDto {
    /// Première frame.
    pub debut: u32,
    /// Dernière frame.
    pub fin: u32,
    /// Index déclaré.
    pub index: u32,
}

/// Tout ce qu'une vue a besoin de savoir d'un `.g4cm`.
#[derive(Serialize, specta::Type)]
pub struct ApercuCameraDto {
    /// Noms des objets animés.
    pub objets: Vec<String>,
    /// Clips déclarés.
    pub clips: Vec<ClipCameraDto>,
    /// Pistes, une par canal.
    pub pistes: Vec<PisteCameraDto>,
    /// Première frame observée, toutes pistes confondues.
    pub frame_min: f32,
    /// Dernière frame observée.
    pub frame_max: f32,
    /// Nombre total de canaux.
    pub canaux: u32,
    /// Nombre de canaux dont le flux est décodé en `f32`.
    pub canaux_resolus: u32,
}

/// Nom lisible d'un canal.
///
/// Les variantes non nommées sortent en hexadécimal plutôt que sous une étiquette inventée :
/// un canal inconnu doit rester visiblement inconnu dans l'interface.
fn nom_canal(kind: nie_formats::g4cm::ChannelKind) -> String {
    use nie_formats::g4cm::ChannelKind as K;
    match kind {
        K::PosX => "PosX".into(),
        K::PosY => "PosY".into(),
        K::PosZ => "PosZ".into(),
        K::RefX => "RefX".into(),
        K::RefY => "RefY".into(),
        K::RefZ => "RefZ".into(),
        K::Fov => "Fov".into(),
        autre => format!("{autre:?}"),
    }
}

/// Décode un `.g4cm` du VFS et l'aplatit en pistes traçables.
pub fn apercu_camera(vfs: &Vfs, path: &str) -> Result<ApercuCameraDto, String> {
    let bytes = vfs.read(path).map_err(|e| e.to_string())?;
    let anim = nie_formats::g4cm::parse(&bytes).map_err(|e| format!("parse G4CM {path} : {e}"))?;

    // Un canal ne porte pas le nom de son objet : c'est l'objet qui déclare l'intervalle de
    // canaux qui lui appartient (`first_channel` + `channel_count`). On inverse donc la
    // relation une fois, plutôt que de rechercher l'objet à chaque canal.
    let mut proprietaire: Vec<Option<usize>> = vec![None; anim.channels.len()];
    for (i, objet) in anim.objects.iter().enumerate() {
        let debut = objet.first_channel as usize;
        let fin = debut.saturating_add(objet.channel_count as usize).min(anim.channels.len());
        for slot in proprietaire.iter_mut().take(fin).skip(debut) {
            *slot = Some(i);
        }
    }

    let mut pistes = Vec::with_capacity(anim.channels.len());
    let (mut frame_min, mut frame_max) = (f32::MAX, f32::MIN);
    let mut resolus = 0_u32;

    for (i, canal) in anim.channels.iter().enumerate() {
        let objet = proprietaire
            .get(i)
            .copied()
            .flatten()
            .and_then(|k| anim.names.get(k))
            .cloned()
            .unwrap_or_else(|| format!("objet{i}"));

        let temps: Vec<f32> =
            canal.times(&anim).iter().take(MAX_ECHANTILLONS).map(|t| f32::from(*t)).collect();
        for t in &temps {
            frame_min = frame_min.min(*t);
            frame_max = frame_max.max(*t);
        }

        let valeurs: Vec<f32> = canal
            .track
            .values()
            .map(|v| v.iter().take(MAX_ECHANTILLONS).copied().collect())
            .unwrap_or_default();
        let resolu = canal.track.values().is_some();
        if resolu {
            resolus += 1;
        }

        pistes.push(PisteCameraDto {
            objet,
            canal: nom_canal(canal.kind),
            resolu,
            temps,
            valeurs,
        });
    }

    // Garde sur les bornes : elles partent de `f32::MAX`/`f32::MIN`, et une caméra dont aucune
    // piste ne porte d'échantillon les laisserait à ces valeurs — un axe de temps allant de
    // 3.4e38 à -3.4e38, qui écrase toute courbe sur une ligne plate. Tester `pistes.is_empty()`
    // ne suffit pas : des pistes peuvent exister sans le moindre temps.
    if frame_min > frame_max {
        frame_min = 0.0;
        frame_max = 0.0;
    }

    Ok(ApercuCameraDto {
        objets: anim.names.clone(),
        clips: anim
            .clips
            .iter()
            .map(|c| ClipCameraDto {
                debut: u32::from(c.start),
                fin: u32::from(c.end),
                index: u32::from(c.index),
            })
            .collect(),
        canaux: anim.channels.len() as u32,
        canaux_resolus: resolus,
        pistes,
        frame_min,
        frame_max,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Navmesh (`.g4nv`)
// ─────────────────────────────────────────────────────────────────────────────

/// Une arête du graphe de navigation, en indices de sommets.
#[derive(Serialize, specta::Type)]
pub struct AreteNavmDto {
    /// Sommet de départ.
    pub a: u32,
    /// Sommet d'arrivée.
    pub b: u32,
    /// Coût de franchissement.
    pub cout: f32,
    /// `true` si l'arête est au bord du maillage (elle ne relie qu'un seul polygone).
    pub bord: bool,
}

/// Tout ce qu'une vue a besoin de savoir d'un `.g4nv`.
#[derive(Serialize, specta::Type)]
pub struct ApercuNavmDto {
    /// Sommets, en coordonnées monde `[x, y, z]`.
    pub sommets: Vec<[f32; 3]>,
    /// Triangles, en index de sommets (trois par polygone).
    pub triangles: Vec<[u32; 3]>,
    /// Arêtes du graphe.
    pub aretes: Vec<AreteNavmDto>,
    /// Coin inférieur de la boîte englobante.
    pub bbox_min: [f32; 3],
    /// Coin supérieur de la boîte englobante.
    pub bbox_max: [f32; 3],
    /// Nombre de polygones du fichier (avant tout plafonnement).
    pub polygones: u32,
    /// `true` si l'aperçu a été plafonné — l'affichage doit le signaler.
    pub tronque: bool,
}

/// Décode un `.g4nv` du VFS et l'aplatit en géométrie projetable.
pub fn apercu_navm(vfs: &Vfs, path: &str) -> Result<ApercuNavmDto, String> {
    let bytes = vfs.read(path).map_err(|e| e.to_string())?;
    let navm = nie_formats::navm::parse(&bytes).map_err(|e| format!("parse G4NV {path} : {e}"))?;

    let tronque = navm.vertices.len() > MAX_SOMMETS;
    let sommets: Vec<[f32; 3]> =
        navm.vertices.iter().take(MAX_SOMMETS).map(|v| v.pos).collect();

    let mut bbox_min = [f32::MAX; 3];
    let mut bbox_max = [f32::MIN; 3];
    for p in &sommets {
        for axe in 0..3 {
            bbox_min[axe] = bbox_min[axe].min(p[axe]);
            bbox_max[axe] = bbox_max[axe].max(p[axe]);
        }
    }
    if sommets.is_empty() {
        bbox_min = [0.0; 3];
        bbox_max = [0.0; 3];
    }

    // `corners` porte trois index de sommet par polygone, adressés par `first_corner`. Un
    // polygone dont les coins sortent de la table est ignoré plutôt que rendu à moitié : un
    // triangle incomplet se dessinerait comme une bavure au milieu de la carte.
    let borne = sommets.len() as u32;
    let mut triangles = Vec::with_capacity(navm.polygons.len());
    for poly in &navm.polygons {
        let d = poly.first_corner as usize;
        let Some(coins) = navm.corners.get(d..d.saturating_add(3)) else {
            continue;
        };
        if coins.iter().all(|c| *c < borne) {
            triangles.push([coins[0], coins[1], coins[2]]);
        }
    }

    let aretes = navm
        .edges
        .iter()
        .filter(|e| e.vert_a < borne && e.vert_b < borne)
        .map(|e| AreteNavmDto {
            a: e.vert_a,
            b: e.vert_b,
            cout: e.cost,
            // `u32::MAX` marque l'absence de second polygone : l'arête borde le vide.
            bord: e.poly_a == u32::MAX || e.poly_b == u32::MAX,
        })
        .collect();

    Ok(ApercuNavmDto {
        polygones: navm.polygons.len() as u32,
        sommets,
        triangles,
        aretes,
        bbox_min,
        bbox_max,
        tronque,
    })
}
