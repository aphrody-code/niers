//! Catalogue du rôle de chaque dossier du VFS — PAS deviné : sourcé de `docs/FORMATS.md`
//! (niveau 2, inventaire exhaustif des 250 800 fichiers réels, daté 2026-06-13) et de
//! `docs/PLAN.md` (état C1 « formats lus »), plus des sous-dossiers vérifiés en DIRECT
//! contre le VFS réel au fil de cette session (`niers vfs find`/`niers vfs chara`, 2026-08-07 —
//! marqués « session »). Une entrée sans provenance vérifiable n'est pas ajoutée : `describe_folder`
//! retourne `None` plutôt que d'inventer un rôle.
//!
//! Correspondance par PLUS LONG préfixe de segment (`data/common/chr/_face` gagne sur
//! `data/common/chr` pour un chemin sous `_face/`).

/// Rôle d'un dossier + statut d'exploitation par niers (✓ exploité / partiel / ✗ non exploité).
pub struct FolderRole {
    pub prefix: &'static str,
    pub role: &'static str,
    pub status: &'static str,
}

/// Table triée implicitement par spécificité croissante — [`describe_folder`] prend le préfixe
/// le plus LONG qui matche, l'ordre de déclaration n'a pas besoin d'être trié.
pub const FOLDER_ROLES: &[FolderRole] = &[
    // ── Niveau 2 (FORMATS.md, 2026-06-13, table « Dossiers de niveau 2 ») ──────────
    FolderRole {
        prefix: "data/common/event",
        role: "Scènes/cutscenes : texte, caméra (g4cm), motion (mevbin), effets (g4pk)",
        status: "partiel (texte ✓, caméra/motion ✗)",
    },
    FolderRole {
        prefix: "data/common/text",
        role: "Textes localisés (ja/fr/en/de/es/it/pt/zh), format T2B",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/dx11/menu",
        role: "Interface : icônes, telops, atlas (g4tx)",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/chr",
        role: "Modèles personnage/technique/keshin (g4md/g4mg/g4sk) + motion",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/sound",
        role: "Lip-sync (.p3lip) par langue — visèmes datés",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/map",
        role: "Maps : objets (objbin), navmesh (g4nv), modèles",
        status: "✗ non exploité (objbin/g4nv non décodés)",
    },
    FolderRole {
        prefix: "data/common/sound_asset",
        role: "Voix/bruitages (acb/awb, codecs HCA/ADX Criware)",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/event_cfg",
        role: "Config d'events (son, séquences)",
        status: "partiel",
    },
    FolderRole {
        prefix: "data/dx11/chr",
        role: "Textures personnage/technique (g4tx)",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/gamedata",
        role: "Tables de jeu (cfg.bin RDBN/T2B — 37+ familles portées dans nie-data)",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/effect",
        role: "Effets : particules (ptlb), fx (fxbin), objbin, shaders pixel (pfxo)",
        status: "✗ non exploité",
    },
    FolderRole {
        prefix: "data/common/menu",
        role: "Données menu (cfg.bin)",
        status: "partiel",
    },
    FolderRole {
        prefix: "data/dx11/shader",
        role: "Shaders compilés DXBC (vfxo/pfxo/gfxo/cfxo) — conteneur lu, bytecode SM5 non désassemblé",
        status: "partiel",
    },
    FolderRole {
        prefix: "data/dx11/map",
        role: "Textures de map + collision (.col, PhysX cooked)",
        status: "partiel (textures ✓, collision ✗)",
    },
    FolderRole {
        prefix: "data/dx11/effect",
        role: "Textures d'effets (g4tx)",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/script",
        role: "Scripts Lua 5.2 (bytecode) — logique de menus/scènes",
        status: "partiel (VM réelle nie-lua exécute le bytecode ; décompilation non faite)",
    },
    FolderRole {
        prefix: "data/common/property",
        role: "Propriétés d'objets",
        status: "✗ non exploité",
    },
    FolderRole {
        prefix: "data/common/movie",
        role: "Vidéos (.usm, conteneur Sofdec2)",
        status: "✓ exploité (flux H.264 démuxé → MP4 ; VP9 non remuxé)",
    },
    FolderRole {
        prefix: "data/dx11/movie",
        role: "Vidéos (.usm, conteneur Sofdec2)",
        status: "✓ exploité (flux H.264 démuxé → MP4 ; VP9 non remuxé)",
    },
    FolderRole {
        prefix: "data/dx11/font",
        role: "Polices",
        status: "✗ non exploité (non requis)",
    },
    FolderRole {
        prefix: "data/common/font",
        role: "Polices",
        status: "✗ non exploité (non requis)",
    },
    FolderRole {
        prefix: "data/common/craft",
        role: "Artisanat (cfg.bin)",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/action",
        role: "Tables d'action",
        status: "partiel",
    },
    // ── Sous-dossiers vérifiés en direct contre le VFS réel (session 2026-08-07) ────────────
    FolderRole {
        prefix: "data/common/chr/_face",
        role: "Modèles de visage par personnage — un dossier par code interne (g4md+g4mg, ex. c01000100)",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/chr/_keshin",
        role: "Modèles de Keshin (avatar spirituel de hissatsu) — g4md+g4mg, composant dédié dans l'assembleur GLB",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/chr/_armd",
        role: "Modèles d'armure (« Armed ») — g4md+g4mg, composant dédié dans l'assembleur GLB",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/chr/_uniform",
        role: "Modèles/textures d'uniforme d'équipe — résolus par CRC dans le manifeste uniforme (3550 entrées)",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/chr/_animal",
        role: "Modèles d'animaux/mascottes (g4sk présent, ex. an000100)",
        status: "✓ exploité (géométrie/squelette)",
    },
    FolderRole {
        prefix: "data/common/gamedata/character",
        role: "Table maîtresse des personnages (chara_base_*.cfg.bin, T2B) — id, code interne, hash de nom, série, équipe",
        status: "✓ exploité (nie-data::chara_base)",
    },
    FolderRole {
        prefix: "data/common/gamedata/skill",
        role: "Techniques : skill_config (RDBN, ~1000 techniques), skill_technic_config, aura_skill_config",
        status: "✓ exploité (nie-data::skill/skill_technic)",
    },
    FolderRole {
        prefix: "data/common/gamedata/menu",
        role: "Objets de menu liés au gameplay (objbin d'aura/kizuna/mixi-max par personnage)",
        status: "partiel (objbin lu, composants non tous exploités)",
    },
    FolderRole {
        prefix: "data/common/gamedata/map",
        role: "Données liées aux maps (placement PNJ « vision », sous-objets par personnage/map)",
        status: "partiel",
    },
    FolderRole {
        prefix: "data/dx11/menu/200_icon/10_icon_chr",
        role: "Icônes de visage/aura de personnage (menu, g4tx)",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/dx11/menu/220_img/telop_waza",
        role: "Cut-ins de technique (telop), un sous-dossier par langue — résolus par CRC32(skillIDStr)",
        status: "✓ exploité",
    },
    FolderRole {
        prefix: "data/common/event/ev_mot",
        role: "Motion d'event par personnage — un dossier par code interne (g4pk d'animation)",
        status: "partiel",
    },
];

/// Rôle du dossier le plus proche de `path` (plus long préfixe de SEGMENT qui matche — `data/
/// common/chr` ne matche pas `data/common/chronicle` grâce à la vérification du séparateur `/`).
/// `None` si aucun dossier connu ne couvre ce chemin — jamais un rôle inventé.
#[must_use]
pub fn describe_folder(path: &str) -> Option<&'static FolderRole> {
    let path = path.trim_end_matches('/');
    FOLDER_ROLES
        .iter()
        .filter(|f| path == f.prefix || path.starts_with(&format!("{}/", f.prefix)))
        .max_by_key(|f| f.prefix.len())
}
