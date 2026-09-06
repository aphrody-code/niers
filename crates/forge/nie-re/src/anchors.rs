//! Ancrage de fonctions par leurs références de chaînes, constantes et RTTI.
//!
//! Avant la propagation de labels, on parcourt les tables disponibles et on
//! assigne un sous-système à toute fonction dont une donnée correspond à une
//! règle connue. Les fonctions ainsi ancrées sont verrouillées dans le
//! [`PropagationGraph`] (leur label ne sera jamais écrasé par la propagation).
//!
//! Trois mécanismes :
//! 1. [`anchor_by_strings`] — motifs sur `func_str_ref`.
//! 2. [`anchor_by_rtti`]   — namespace + préfixe de nom des classes RTTI.
//! 3. [`anchor_by_const`]  — constantes magiques de formats connus.

use anyhow::Result;
use nie_index::rusqlite::Connection;
use tracing::debug;

// ---------------------------------------------------------------------------
// Mécanisme 1 : ancrage par chaînes de caractères
// ---------------------------------------------------------------------------

/// Une règle d'ancrage : si la chaîne contient `pattern` (casse-insensible)
/// le sous-système est `subsystem`.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    /// Sous-chaîne à chercher (comparée en minuscules).
    pub pattern: &'static str,
    /// Sous-système assigné si la règle correspond.
    pub subsystem: &'static str,
}

/// Table de règles d'ancrage par chaînes. Ordre décroissant de spécificité :
/// les motifs les plus précis sont en tête pour court-circuiter les motifs
/// généraux.
pub static RULES: &[Rule] = &[
    // --- Audio (CriWare / HCA) ---
    Rule {
        pattern: "criatom",
        subsystem: "audio",
    },
    Rule {
        pattern: "criware",
        subsystem: "audio",
    },
    Rule {
        pattern: "criadx",
        subsystem: "audio",
    },
    Rule {
        pattern: "crilayla",
        subsystem: "audio",
    },
    Rule {
        pattern: "crimana",
        subsystem: "audio",
    },
    Rule {
        pattern: "crimv",
        subsystem: "audio",
    },
    Rule {
        pattern: "crincv",
        subsystem: "audio",
    },
    Rule {
        pattern: "hca",
        subsystem: "audio",
    },
    Rule {
        pattern: "adxf",
        subsystem: "audio",
    },
    Rule {
        pattern: "acb",
        subsystem: "audio",
    },
    Rule {
        pattern: "cri_atom",
        subsystem: "audio",
    },
    Rule {
        pattern: "gdssound",
        subsystem: "audio",
    },
    Rule {
        pattern: "soundqueue",
        subsystem: "audio",
    },
    Rule {
        pattern: "soundobj",
        subsystem: "audio",
    },
    Rule {
        pattern: "soundconfig",
        subsystem: "audio",
    },
    // --- Render / DirectX ---
    Rule {
        pattern: "d3d11",
        subsystem: "render",
    },
    Rule {
        pattern: "d3d12",
        subsystem: "render",
    },
    Rule {
        pattern: "dxgi",
        subsystem: "render",
    },
    Rule {
        pattern: "dxbc",
        subsystem: "render",
    },
    Rule {
        pattern: "hlsl",
        subsystem: "render",
    },
    Rule {
        pattern: "g4tx",
        subsystem: "render",
    },
    Rule {
        pattern: "shader",
        subsystem: "render",
    },
    Rule {
        pattern: "rendertexture",
        subsystem: "render",
    },
    Rule {
        pattern: "renderstate",
        subsystem: "render",
    },
    Rule {
        pattern: "renderdata",
        subsystem: "render",
    },
    Rule {
        pattern: "renderprop",
        subsystem: "render",
    },
    Rule {
        pattern: "renderproper",
        subsystem: "render",
    },
    Rule {
        pattern: "bustuprender",
        subsystem: "render",
    },
    Rule {
        pattern: "omnidirectionalshadow",
        subsystem: "render",
    },
    Rule {
        pattern: "decalparam",
        subsystem: "render",
    },
    Rule {
        pattern: "decalsetup",
        subsystem: "render",
    },
    Rule {
        pattern: "decalanim",
        subsystem: "render",
    },
    // --- Scripting (Lua) ---
    Rule {
        pattern: "lua_",
        subsystem: "script",
    },
    Rule {
        pattern: "lua state",
        subsystem: "script",
    },
    Rule {
        pattern: "luastate",
        subsystem: "script",
    },
    Rule {
        pattern: "luaerror",
        subsystem: "script",
    },
    Rule {
        pattern: ".lua",
        subsystem: "script",
    },
    Rule {
        pattern: "scnobjscript",
        subsystem: "script",
    },
    Rule {
        pattern: "clua",
        subsystem: "script",
    },
    // --- Fichiers virtuels / CPK ---
    Rule {
        pattern: "cpk",
        subsystem: "vfs",
    },
    Rule {
        pattern: ".g4pk",
        subsystem: "vfs",
    },
    Rule {
        pattern: ".g4md",
        subsystem: "vfs",
    },
    Rule {
        pattern: "g4pk",
        subsystem: "vfs",
    },
    Rule {
        pattern: "pakfile",
        subsystem: "vfs",
    },
    // --- Gameplay / football ---
    Rule {
        pattern: "soccer",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "ballmove",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "ballcomponent",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "formation",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "shoot",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "dribble",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "goal",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "rpgbattle",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "craftobj",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "cgdd",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "gdsevent",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "gdsquest",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "gdsmission",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "flockn",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "happeneven",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "questaggregate",
        subsystem: "gameplay",
    },
    Rule {
        pattern: "kizunatown",
        subsystem: "gameplay",
    },
    // --- Menu / UI ---
    Rule {
        pattern: "cmenu",
        subsystem: "menu",
    },
    Rule {
        pattern: "menulist",
        subsystem: "menu",
    },
    Rule {
        pattern: "menuview",
        subsystem: "menu",
    },
    Rule {
        pattern: "gdsmenu",
        subsystem: "menu",
    },
    Rule {
        pattern: "abilitymenu",
        subsystem: "menu",
    },
    Rule {
        pattern: "abilitylist",
        subsystem: "menu",
    },
    Rule {
        pattern: "tradmenu",
        subsystem: "menu",
    },
    Rule {
        pattern: "menu",
        subsystem: "menu",
    },
    // --- Personnages / données IEVR ---
    Rule {
        pattern: "chara_param",
        subsystem: "chara",
    },
    Rule {
        pattern: "chara",
        subsystem: "chara",
    },
    Rule {
        pattern: "gdschara",
        subsystem: "chara",
    },
    // --- Physique (PhysX) ---
    Rule {
        pattern: "physx",
        subsystem: "physics",
    },
    Rule {
        pattern: "physics",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxshared",
        subsystem: "physics",
    },
    Rule {
        pattern: "npc",
        subsystem: "physics",
    },
    // Prefixes PhysX (Px*, Scb::, Np*) — distingués des faux-positifs "px" trop courts
    Rule {
        pattern: "pxbase",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxjoint",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxactor",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxrigid",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxparticle",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxmaterial",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxarticulation",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxshape",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxconvexmesh",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxtrianglemesh",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxheightfield",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxconstraint",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxcloth",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxpruning",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxu32",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxu16",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxvec3",
        subsystem: "physics",
    },
    Rule {
        pattern: "pxtype",
        subsystem: "physics",
    },
    Rule {
        pattern: "scb::particlesys",
        subsystem: "physics",
    },
    Rule {
        pattern: "scb::articulat",
        subsystem: "physics",
    },
    Rule {
        pattern: "scb::aggregate",
        subsystem: "physics",
    },
    Rule {
        pattern: "npactor",
        subsystem: "physics",
    },
    Rule {
        pattern: "npshapemanager",
        subsystem: "physics",
    },
    Rule {
        pattern: "npparticle",
        subsystem: "physics",
    },
    Rule {
        pattern: "npmaterial",
        subsystem: "physics",
    },
    Rule {
        pattern: "nparticulation",
        subsystem: "physics",
    },
    Rule {
        pattern: "npaggregate",
        subsystem: "physics",
    },
    Rule {
        pattern: "sq::pruning",
        subsystem: "physics",
    },
    Rule {
        pattern: "pbdswing",
        subsystem: "physics",
    },
    Rule {
        pattern: "pbdobstacle",
        subsystem: "physics",
    },
    // --- Réseau / plateformes ---
    Rule {
        pattern: "eos",
        subsystem: "network",
    },
    Rule {
        pattern: "steam",
        subsystem: "network",
    },
    Rule {
        pattern: "lobby",
        subsystem: "network",
    },
    Rule {
        pattern: "matchmak",
        subsystem: "network",
    },
    Rule {
        pattern: "session",
        subsystem: "network",
    },
    Rule {
        pattern: "netstate",
        subsystem: "network",
    },
    Rule {
        pattern: "netcontroller",
        subsystem: "network",
    },
    Rule {
        pattern: "networkmatch",
        subsystem: "network",
    },
    Rule {
        pattern: "litserver",
        subsystem: "network",
    },
    // --- Animation ---
    Rule {
        pattern: "animation",
        subsystem: "animation",
    },
    Rule {
        pattern: "motion",
        subsystem: "animation",
    },
    Rule {
        pattern: "bone",
        subsystem: "animation",
    },
    Rule {
        pattern: "attach",
        subsystem: "animation",
    },
    Rule {
        pattern: "gdseffect",
        subsystem: "animation",
    },
    Rule {
        pattern: "particleroot",
        subsystem: "animation",
    },
    Rule {
        pattern: "particlesetup",
        subsystem: "animation",
    },
    Rule {
        pattern: "particleindic",
        subsystem: "animation",
    },
    // --- Level / scène (nouveau sous-système) ---
    Rule {
        pattern: "cmap",
        subsystem: "level",
    },
    Rule {
        pattern: "gdsmap",
        subsystem: "level",
    },
    Rule {
        pattern: "mapblock",
        subsystem: "level",
    },
    Rule {
        pattern: "maplightdata",
        subsystem: "level",
    },
    Rule {
        pattern: "mapdoor",
        subsystem: "level",
    },
    Rule {
        pattern: "mapannotate",
        subsystem: "level",
    },
    Rule {
        pattern: "mapsound",
        subsystem: "level",
    },
    Rule {
        pattern: "mapenveff",
        subsystem: "level",
    },
    Rule {
        pattern: "tagobjmesh",
        subsystem: "level",
    },
    Rule {
        pattern: "tagobjeffect",
        subsystem: "level",
    },
    Rule {
        pattern: "tagobjpxcol",
        subsystem: "level",
    },
    Rule {
        pattern: "tagobjpointlight",
        subsystem: "level",
    },
    Rule {
        pattern: "tagobjregis",
        subsystem: "level",
    },
    Rule {
        pattern: "tagbuildobj",
        subsystem: "level",
    },
    Rule {
        pattern: "ctilebase",
        subsystem: "level",
    },
    Rule {
        pattern: "cskypar",
        subsystem: "level",
    },
    Rule {
        pattern: "refmaplightdata",
        subsystem: "level",
    },
    // --- Input ---
    Rule {
        pattern: "win mouse",
        subsystem: "input",
    },
    Rule {
        pattern: "win keyboard",
        subsystem: "input",
    },
    Rule {
        pattern: "virtual pad",
        subsystem: "input",
    },
    Rule {
        pattern: "pad array",
        subsystem: "input",
    },
    Rule {
        pattern: "mouse array",
        subsystem: "input",
    },
    Rule {
        pattern: "mouse virtual",
        subsystem: "input",
    },
    Rule {
        pattern: "onactivevirtualpad",
        subsystem: "input",
    },
    Rule {
        pattern: "onfocusvirtualpad",
        subsystem: "input",
    },
];

/// Applique toutes les règles à une valeur de chaîne et renvoie le
/// sous-système correspondant au premier motif qui correspond, ou `None`.
#[must_use]
pub fn classify_str(value: &str) -> Option<&'static str> {
    let lower = value.to_lowercase();
    for rule in RULES {
        if lower.contains(rule.pattern) {
            return Some(rule.subsystem);
        }
    }
    None
}

/// Résultat agrégé de tous les ancrages.
#[derive(Debug, Default, Clone, Copy)]
pub struct AnchorStats {
    /// Fonctions ancrées par chaînes de caractères.
    pub anchored_str: usize,
    /// Fonctions ancrées par correspondance RTTI (namespace/préfixe).
    pub anchored_rtti: usize,
    /// Fonctions ancrées par constante magique de format.
    pub anchored_const: usize,
}

impl AnchorStats {
    /// Total de fonctions nouvellement ancrées (sans double-compte possible
    /// car chaque mécanisme n'écrit que sur les fonctions encore standalone).
    #[must_use]
    pub fn total(&self) -> usize {
        self.anchored_str + self.anchored_rtti + self.anchored_const
    }
}

/// Parcourt `func_str_ref` et pose des ancres (`subsystem` + `subsys_src='str'`)
/// sur les fonctions dont le sous-système est encore `'standalone'` ou `NULL`,
/// en appliquant [`RULES`]. Opère sous une seule transaction.
///
/// Les fonctions déjà classifiées (subsystem hors standalone/NULL) ne sont
/// pas modifiées : leur label est supposé fiable.
pub fn anchor_by_strings(conn: &Connection, binary_id: i64) -> Result<usize> {
    // Charge toutes les paires (function_id, vaddr, str_value) pour les
    // fonctions encore non classifiées.
    let mut stmt = conn.prepare(
        "SELECT DISTINCT f.id, f.vaddr, sr.value
         FROM function f
         JOIN func_str_ref sr ON sr.function_id = f.id
         WHERE f.binary_id = ?1
           AND (f.subsystem IS NULL
                OR f.subsystem = 'standalone')",
    )?;

    // Collecte avant modification pour ne pas tenir le statement ouvert lors
    // des UPDATE.
    let rows: Vec<(i64, i64, String)> = stmt
        .query_map([binary_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<_, _>>()?;

    // Pour chaque fonction, prend le premier sous-système trouvé parmi toutes
    // ses chaînes (ordre de la requête).
    let mut best: hashbrown::HashMap<i64, (&'static str, i64)> = hashbrown::HashMap::new();
    for (fid, vaddr, value) in &rows {
        if let Some(sub) = classify_str(value) {
            best.entry(*fid).or_insert((sub, *vaddr));
        }
    }

    let mut anchored = 0usize;
    for (fid, (sub, _vaddr)) in &best {
        conn.execute(
            "UPDATE function SET subsystem=?1, subsys_src='str', confidence=0.8
             WHERE id=?2",
            nie_index::rusqlite::params![sub, fid],
        )?;
        debug!(fid, sub, "ancre str posée");
        anchored += 1;
    }

    Ok(anchored)
}

// ---------------------------------------------------------------------------
// Mécanisme 2 : ancrage par RTTI (namespace + préfixe de nom de classe)
// ---------------------------------------------------------------------------

/// Mappe un (namespace, nom_classe) RTTI vers un sous-système.
///
/// La logique est : pour un namespace de premier niveau (`game`, `lives`,
/// `physx`, `CryptoPP`, …) on affine selon le préfixe du nom de classe.
/// Retourne `None` si aucune règle ne s'applique.
#[must_use]
pub fn classify_rtti(namespace: &str, class_name: &str) -> Option<&'static str> {
    let ns = namespace.trim();
    // Namespaces canoniques connus (on ignore les namespaces mal démanglés comme `game::::$0BA::::`).
    let clean_ns = if ns.contains("::") || ns.contains('$') || ns.contains('V') || ns.contains('U')
    {
        // namespace mal démanglé, on tente un prefix match sur les namespaces simples
        if ns.starts_with("game") {
            "game"
        } else if ns.starts_with("lives") {
            "lives"
        } else if ns.starts_with("physx") {
            "physx"
        } else if ns.starts_with("CryptoPP") {
            "CryptoPP"
        } else {
            return None;
        }
    } else {
        ns
    };

    let n = class_name.to_lowercase();

    match clean_ns {
        "game" => classify_game_class(&n),
        "lives" => classify_lives_class(&n),
        "physx" => Some("physics"),
        "CryptoPP" | "cryptopp" => Some("network"), // chiffrement → réseau / sécurité
        "std" => None,                              // stdlib → trop générique
        _ => None,
    }
}

/// Classifie une classe du namespace `game::` d'après le préfixe de son nom.
fn classify_game_class(lower_name: &str) -> Option<&'static str> {
    // Menu — le plus spécifique en premier
    if lower_name.starts_with("cmenu")
        || lower_name.starts_with("gdsmenu")
        || lower_name.ends_with("menu")
        || lower_name.starts_with("abilitylearing")
        || lower_name.starts_with("abilitylist")
        || lower_name.starts_with("bbstadiumtop")
        || lower_name.starts_with("bbstadiumcampaign")
        || lower_name.starts_with("informationtop")
        || lower_name.starts_with("beans")
    {
        return Some("menu");
    }
    // Chara / GDD
    if lower_name.starts_with("cgddchara")
        || lower_name.starts_with("cgddedit")
        || lower_name.starts_with("cgddcraftchara")
        || lower_name.starts_with("cgddhuman")
        || lower_name.starts_with("cgddbasara")
        || lower_name.starts_with("cgddhero")
        || lower_name.starts_with("cgddelement")
        || lower_name.starts_with("cgddnormal")
        || lower_name.starts_with("cgddtoken")
        || lower_name.starts_with("cgddinventory")
        || lower_name.starts_with("gdschara")
        || lower_name.starts_with("ccharaedit")
        || lower_name.starts_with("chara")
    {
        return Some("chara");
    }
    // Gameplay / GDD game data
    if lower_name.starts_with("cgdd")
        || lower_name.starts_with("ball")
        || lower_name.starts_with("csoccer")
        || lower_name.starts_with("crpg")
        || lower_name.starts_with("rpgbattle")
        || lower_name.starts_with("craftobj")
        || lower_name.starts_with("ccraft")
        || lower_name.starts_with("cflockn")
        || lower_name.starts_with("chappenevent")
        || lower_name.starts_with("questaggregate")
        || lower_name.starts_with("ckizuna")
        || lower_name.starts_with("cscene")
        || lower_name.starts_with("cachievement")
        || lower_name.starts_with("caccount")
        || lower_name.starts_with("gdsevent")
        || lower_name.starts_with("gdsquest")
        || lower_name.starts_with("gdsmission")
        || lower_name.starts_with("activity")
        || lower_name.starts_with("ai")
        // Census live (dump nie.exe) — système de commandes / état de match (frontière C3) :
        || lower_name.starts_with("play_dynamic")
        || lower_name.starts_with("play_static")
        || lower_name.starts_with("ccallback")
        || lower_name.starts_with("execpassive")
        || lower_name.starts_with("execsoccer")
        || lower_name.starts_with("funcpoint")
    {
        return Some("gameplay");
    }
    // Render
    if lower_name.starts_with("crender")
        || lower_name.starts_with("ccamera")
        || lower_name.starts_with("cgamecamera")
        || lower_name.starts_with("bustuprender")
        || lower_name.starts_with("texturecache")
        || lower_name.starts_with("overriderender")
        || lower_name.starts_with("chararender")
        || lower_name.starts_with("menurenderdata")
        || lower_name.starts_with("gdstexture")
        || lower_name.starts_with("gdschara")
    {
        return Some("render");
    }
    // Level / scène
    if lower_name.starts_with("cmap")
        || lower_name.starts_with("ctag")
        || lower_name.starts_with("gdsmap")
        || lower_name.starts_with("ctilebase")
        || lower_name.starts_with("csky")
        || lower_name.starts_with("cref")
        || lower_name.starts_with("areacollect")
        || lower_name.starts_with("game_map") // census live : GAME_MAP_AREA_INFO
        || lower_name.starts_with("tbox")
    // census live : TBoxInfo (coffre)
    {
        return Some("level");
    }
    // Script
    if lower_name.starts_with("cscript") || lower_name.starts_with("ccommonmodulelua") {
        return Some("script");
    }
    // Audio
    if lower_name.starts_with("gdssound") || lower_name.starts_with("soundobj") {
        return Some("audio");
    }
    // Animation / effets
    if lower_name.starts_with("gdseffect")
        || lower_name.starts_with("lineeffect")
        || lower_name.starts_with("motmng")
    // census live : MotMngBase (gestionnaire de motion)
    {
        return Some("animation");
    }
    None
}

/// Classifie une classe du namespace `lives::` d'après le préfixe de son nom.
fn classify_lives_class(lower_name: &str) -> Option<&'static str> {
    // Préfixes ground-truth issus du census d'objets vivants (dump live de nie.exe,
    // cf. docs/game-data/dump-exploitation.md) : seuls les noms à sémantique claire
    // sont ancrés ; les bases génériques (CObject, CListData*, CInternalFile,
    // CRefelencLinkObj) restent non classifiées (anti-faux).
    if lower_name.starts_with("cmenu") || lower_name.starts_with("crefmodelmenu") {
        return Some("menu");
    }
    if lower_name.starts_with("ccri") || lower_name.starts_with("ccrimana") {
        return Some("audio");
    }
    if lower_name.starts_with("clua") {
        return Some("script");
    }
    if lower_name.starts_with("ccamera")
        || lower_name.starts_with("cfont")
        || lower_name.starts_with("csystemfont")
        || lower_name.starts_with("cdraw")
        || lower_name.starts_with("clight")
        || lower_name.starts_with("ccompute")
        || lower_name.starts_with("cindex")
        || lower_name.starts_with("cuniform")
        || lower_name.starts_with("cvertex")
        || lower_name.starts_with("crender")
        || lower_name.starts_with("crestexture")
        || lower_name.starts_with("cresmesh")
        || lower_name.starts_with("gmdcobjmodel")
        || lower_name.starts_with("ccustomrenderstate")
        || lower_name.starts_with("ccustomshader")
    {
        return Some("render");
    }
    if lower_name.starts_with("ceffect")
        || lower_name.starts_with("cdynamic")
        || lower_name.starts_with("cresanime")
        || lower_name.starts_with("cresrefanime")
        || lower_name.starts_with("cresmotevent")
        || lower_name.starts_with("cresblendshape")
        || lower_name.starts_with("cresparticle")
        || lower_name.starts_with("creseffect")
        || lower_name.starts_with("cresskeleton")
        || lower_name.starts_with("gmdcanim")
        || lower_name.starts_with("gmdcshareobjanim")
    {
        return Some("animation");
    }
    if lower_name.starts_with("clives") && lower_name.contains("px") {
        return Some("physics");
    }
    if lower_name.starts_with("cjob") {
        return Some("physics");
    } // job manager = threading physx
    None
}

/// Ancre les fonctions standalone dont une référence de chaîne correspond au
/// **nom d'une classe RTTI** connue, en dérivant le sous-système depuis le
/// namespace RTTI (via [`classify_rtti`]).
///
/// Ce mécanisme est complémentaire à [`anchor_by_strings`] : il capture des
/// fonctions qui référencent des noms de classes C++ (souvent dans des
/// introspections ou des log d'assertion) sans que ces noms correspondent aux
/// motifs généraux de [`RULES`].
pub fn anchor_by_rtti(conn: &Connection, binary_id: i64) -> Result<usize> {
    // Charge les noms + namespaces de toutes les classes RTTI du binaire.
    // `namespace` peut être NULL dans la DB → on le lit comme Option<String>.
    let rtti: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT name, COALESCE(namespace, '') FROM rtti_class WHERE binary_id=?1",
        )?;
        stmt.query_map([binary_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<_, _>>()?
    };

    // Pré-calcule le mapping nom_classe → sous-système (ignore les None).
    let class_to_sub: hashbrown::HashMap<String, &'static str> = rtti
        .iter()
        .filter_map(|(name, ns)| classify_rtti(ns, name).map(|sub| (name.clone(), sub)))
        .collect();

    if class_to_sub.is_empty() {
        return Ok(0);
    }

    // Charge les (function_id, str_value) des fonctions encore standalone.
    let mut stmt = conn.prepare(
        "SELECT DISTINCT f.id, sr.value
         FROM function f
         JOIN func_str_ref sr ON sr.function_id = f.id
         WHERE f.binary_id = ?1
           AND (f.subsystem IS NULL OR f.subsystem = 'standalone')",
    )?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([binary_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<_, _>>()?;

    // Pour chaque fonction, cherche la première correspondance dans le mapping.
    let mut best: hashbrown::HashMap<i64, &'static str> = hashbrown::HashMap::new();
    for (fid, value) in &rows {
        if let Some(&sub) = class_to_sub.get(value.as_str()) {
            best.entry(*fid).or_insert(sub);
        }
    }

    let mut anchored = 0usize;
    for (fid, sub) in &best {
        conn.execute(
            "UPDATE function SET subsystem=?1, subsys_src='rtti', confidence=0.75
             WHERE id=?2",
            nie_index::rusqlite::params![sub, fid],
        )?;
        debug!(fid, sub, "ancre rtti posée");
        anchored += 1;
    }

    Ok(anchored)
}

// ---------------------------------------------------------------------------
// Mécanisme 3 : ancrage par constante magique de format
// ---------------------------------------------------------------------------

/// Constantes magiques de formats de fichiers connus (valeur `u64` telle que
/// stockée en little-endian dans `func_const`).
static MAGIC_MAP: &[(i64, &str)] = &[
    // G4TX = 'G','4','T','X' → 0x47345458 en LE
    (0x4734_5458, "render"),
    // G4MD = 'G','4','M','D' → 0x47344D44 en LE
    (0x4734_4D44, "vfs"),
    // G4PK = 'G','4','P','K' → 0x47345058 en LE
    (0x4734_5058, "vfs"),
    // CPK space = 0x43504B20 en LE
    (0x4350_4B20, "vfs"),
    // @UTF = 0x40555446 en LE
    (0x4055_5446, "vfs"),
    // RDBN = 'R','D','B','N' → 0x5244424E en LE
    (0x5244_424E, "vfs"),
];

/// Ancre les fonctions standalone dont un `func_const` correspond à un magic
/// de format connu, en leur assignant le sous-système du format.
pub fn anchor_by_const(conn: &Connection, binary_id: i64) -> Result<usize> {
    // Charge (function_id, value) pour les fonctions encore standalone.
    let mut stmt = conn.prepare(
        "SELECT DISTINCT f.id, fc.value
         FROM function f
         JOIN func_const fc ON fc.function_id = f.id
         WHERE f.binary_id = ?1
           AND (f.subsystem IS NULL OR f.subsystem = 'standalone')",
    )?;
    let rows: Vec<(i64, i64)> = stmt
        .query_map([binary_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<std::result::Result<_, _>>()?;

    let mut best: hashbrown::HashMap<i64, &'static str> = hashbrown::HashMap::new();
    for (fid, value) in &rows {
        for &(magic, sub) in MAGIC_MAP {
            if *value == magic {
                best.entry(*fid).or_insert(sub);
                break;
            }
        }
    }

    let mut anchored = 0usize;
    for (fid, sub) in &best {
        conn.execute(
            "UPDATE function SET subsystem=?1, subsys_src='magic', confidence=0.85
             WHERE id=?2",
            nie_index::rusqlite::params![sub, fid],
        )?;
        debug!(fid, sub, "ancre magic posée");
        anchored += 1;
    }

    Ok(anchored)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_str_audio() {
        assert_eq!(classify_str("criAtom_OpenWithId"), Some("audio"));
        assert_eq!(classify_str("hca_decode"), Some("audio"));
        assert_eq!(classify_str("CriLayla decompression"), Some("audio"));
    }

    #[test]
    fn classify_str_render() {
        assert_eq!(classify_str("d3d11CreateDevice"), Some("render"));
        assert_eq!(classify_str("G4TX_MAGIC"), Some("render"));
        assert_eq!(classify_str("RenderTextureProperty"), Some("render"));
    }

    #[test]
    fn classify_str_vfs() {
        assert_eq!(classify_str("cpkFilename"), Some("vfs"));
    }

    #[test]
    fn classify_str_gameplay() {
        assert_eq!(classify_str("BallMoveNormal"), Some("gameplay"));
        assert_eq!(classify_str("SoccerManager"), Some("gameplay"));
        assert_eq!(
            classify_str("RpgBattleWeaponVisibleComponent"),
            Some("gameplay")
        );
    }

    #[test]
    fn classify_str_chara() {
        assert_eq!(classify_str("CHARA_PARAM_INFO"), Some("chara"));
    }

    #[test]
    fn classify_str_physics_px() {
        assert_eq!(classify_str("PxBase"), Some("physics"));
        assert_eq!(classify_str("PxJoint"), Some("physics"));
        assert_eq!(classify_str("PxRigidDynamic"), Some("physics"));
        assert_eq!(classify_str("Scb::ParticleSystem"), Some("physics"));
        assert_eq!(classify_str("NpActor"), Some("physics"));
    }

    #[test]
    fn classify_str_level() {
        assert_eq!(classify_str("CMapBlockParam"), Some("level"));
        assert_eq!(classify_str("CTagObjMesh"), Some("level"));
        // gdsMapSoundConfig contient "gdssound" (audio) en premier → audio est correct,
        // la configuration du son de la map est traitée côté audio dans IEVR.
        assert_eq!(classify_str("gdsMapSoundConfig"), Some("audio"));
        assert_eq!(classify_str("gdsMapDoorConfig"), Some("level"));
    }

    #[test]
    fn classify_str_input() {
        assert_eq!(classify_str("Win Mouse"), Some("input"));
        assert_eq!(classify_str("Virtual Pad"), Some("input"));
    }

    #[test]
    fn classify_str_no_match() {
        assert_eq!(classify_str("E2009011412"), None);
        assert_eq!(classify_str("xyz_unknown_data"), None);
    }

    #[test]
    fn classify_str_case_insensitive() {
        assert_eq!(classify_str("CriAtom_OpenEx"), Some("audio"));
        assert_eq!(classify_str("CRIATOM"), Some("audio"));
    }

    #[test]
    fn classify_rtti_game_menu() {
        assert_eq!(classify_rtti("game", "CMenuAdventCalendar"), Some("menu"));
        assert_eq!(classify_rtti("game", "AbilityListMenu"), Some("menu"));
    }

    #[test]
    fn classify_rtti_game_gameplay() {
        assert_eq!(classify_rtti("game", "BallMoveBezier"), Some("gameplay"));
        assert_eq!(
            classify_rtti("game", "RpgBattleCameraInfo"),
            Some("gameplay")
        );
        assert_eq!(classify_rtti("game", "CGDDCharaStatus"), Some("chara"));
    }

    #[test]
    fn classify_rtti_game_render() {
        assert_eq!(
            classify_rtti("game", "CRenderTextureProperty"),
            Some("render")
        );
        assert_eq!(classify_rtti("game", "CGameCameraCtrl"), Some("render"));
    }

    #[test]
    fn classify_rtti_game_level() {
        assert_eq!(classify_rtti("game", "CMapBlockParam"), Some("level"));
        assert_eq!(classify_rtti("game", "CTagObjMesh"), Some("level"));
    }

    #[test]
    fn classify_rtti_physx() {
        assert_eq!(classify_rtti("physx", "PxScene"), Some("physics"));
    }

    #[test]
    fn classify_rtti_lives_audio() {
        assert_eq!(classify_rtti("lives", "CCriSofdecDecoder"), Some("audio"));
        assert_eq!(classify_rtti("lives", "CCriSoundController"), Some("audio"));
    }

    #[test]
    fn classify_rtti_lives_render() {
        assert_eq!(classify_rtti("lives", "CFontManager"), Some("render"));
        assert_eq!(classify_rtti("lives", "CLightManager"), Some("render"));
    }

    #[test]
    fn classify_rtti_std_is_none() {
        assert_eq!(classify_rtti("std", "vector"), None);
    }

    #[test]
    fn classify_rtti_lives_census() {
        // Noms ground-truth du census d'objets vivants (dump live de nie.exe).
        assert_eq!(classify_rtti("lives", "CUniformBlock"), Some("render"));
        assert_eq!(classify_rtti("lives", "CVertexBuffer"), Some("render"));
        assert_eq!(classify_rtti("lives", "CRenderBuffer"), Some("render"));
        assert_eq!(classify_rtti("lives", "CSystemFont"), Some("render"));
        assert_eq!(classify_rtti("lives", "CResTexture"), Some("render"));
        assert_eq!(classify_rtti("lives", "CResAnime"), Some("animation"));
        assert_eq!(classify_rtti("lives", "CResSkeleton"), Some("animation"));
        assert_eq!(classify_rtti("lives", "gmdCAnimation"), Some("animation"));
        assert_eq!(classify_rtti("lives", "CRefModelMenuText"), Some("menu"));
        // Bases génériques laissées non classifiées (anti-faux).
        assert_eq!(classify_rtti("lives", "CObject"), None);
        assert_eq!(classify_rtti("lives", "CObjectLink"), None);
        assert_eq!(classify_rtti("lives", "CListDataBase"), None);
    }

    #[test]
    fn classify_rtti_game_census() {
        // Système de commandes / état de match (frontière C3) + carte/coffre.
        assert_eq!(classify_rtti("game", "PLAY_DYNAMIC_INFO"), Some("gameplay"));
        assert_eq!(classify_rtti("game", "PLAY_STATIC_INFO"), Some("gameplay"));
        assert_eq!(
            classify_rtti("game", "CCallbackPlayCommand"),
            Some("gameplay")
        );
        assert_eq!(
            classify_rtti("game", "CCallbackJudgeCommand"),
            Some("gameplay")
        );
        assert_eq!(
            classify_rtti("game", "ExecPassiveSkillEffectInfo"),
            Some("gameplay")
        );
        assert_eq!(classify_rtti("game", "GAME_MAP_AREA_INFO"), Some("level"));
        assert_eq!(classify_rtti("game", "TBoxInfo"), Some("level"));
    }
}
