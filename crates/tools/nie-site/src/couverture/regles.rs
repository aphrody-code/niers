//! Le **classement** — les décisions humaines de la matrice, écrites une fois, versionnées.
//!
//! Chaque règle dit ce qu'elle reconnaît, l'état qu'elle pose et **pourquoi**. L'ordre compte :
//! la première règle qui reconnaît une capacité la classe, et un [`Motif::Tout`] ferme chaque
//! source. Une capacité qu'aucune règle ne reconnaît n'est pas silencieusement ignorée : elle
//! sort en `manquant` « non classée » (cf. [`super::construire`]).
//!
//! Ce fichier est le seul endroit du dépôt où l'on ait le droit d'écrire « c'est voulu ». Une
//! raison qui ne tient pas à la lecture est une raison qui ne tient pas.

use std::borrow::Cow;

use super::{Etat, Motif, Portee, Regle, Source};

/// Une **décision nommée** : `r!(id, Source, motif, etat)`.
///
/// Elle vise des capacités précises. Si elle n'en classe plus aucune, c'est qu'elle est périmée
/// — une commande renommée, un module supprimé — et elle sort dans `regles_mortes`, que le
/// service attend **vide**.
macro_rules! r {
    ($id:literal, $src:ident, $motif:expr, $etat:expr) => {
        Regle {
            id: $id,
            source: Source::$src,
            motif: $motif,
            etat: $etat,
            portee: Portee::Decision,
        }
    };
}

/// Un **filet** : `filet!(id, Source, motif, etat)` — il ferme une source.
///
/// Son vide est l'objectif, pas une anomalie : il veut dire que chaque capacité de la source
/// porte une décision nommée. Ce qu'il attrape est publié dans `filets`, avec son compte : une
/// seule raison couvrant N capacités est une dette, et elle se chiffre.
///
/// Tout [`Motif::Tout`] doit passer par ici — le déclarer avec `r!` ne compile pas.
macro_rules! filet {
    ($id:literal, $src:ident, $motif:expr, $etat:expr) => {
        Regle {
            id: $id,
            source: Source::$src,
            motif: $motif,
            etat: $etat,
            portee: Portee::Filet,
        }
    };
}

const fn servi(route: &'static str) -> Etat {
    Etat::Servi {
        route: Cow::Borrowed(route),
    }
}
const fn manquant(decodeur: &'static str) -> Etat {
    Etat::Manquant {
        decodeur: Cow::Borrowed(decodeur),
    }
}
const fn bloque(raison: &'static str) -> Etat {
    Etat::Bloque {
        raison: Cow::Borrowed(raison),
    }
}
const fn interne(raison: &'static str) -> Etat {
    Etat::Interne {
        raison: Cow::Borrowed(raison),
    }
}

/// Les modules de `nie-data` que `nie_data::typed::decode_by_key` atteint, et donc que
/// `/api/v1/donnees/{chemin}` sert.
///
/// Cette liste est **écrite**, mais elle n'est pas crue : un test la confronte au fichier
/// source (`crates/engine/nie-data/src/typed.rs`) et rougit dès qu'un module y entre ou en
/// sort. C'est ce qui la distingue d'un inventaire tenu à la main — celui-là se périme en
/// silence, et ce dépôt l'a déjà payé avec `app::ROUTES` figé à 19 sur un routeur qui en
/// montait 37.
pub static MODULES_TYPES: &[&str] = &[
    "ability_learning",
    "academic_year",
    "activity",
    "add_content_equip",
    "add_model",
    "ai",
    "ai_type",
    "aura",
    "banner",
    "basara",
    "belong_team",
    "boost_grp",
    "capsule",
    "change_aura_skill_config",
    "chara_bank",
    "chara_base",
    "chara_costume",
    "chara_description",
    "chara_details",
    "chara_edit",
    "chara_menu_resource",
    "chara_param",
    "chara_series",
    "chat_emote",
    "chronicle_top",
    "command",
    "craft",
    "ctrl_chara",
    "dictionary",
    "dungeon",
    "emblems",
    "enjoy_mode_team",
    "event_bustup",
    "event_map_tag",
    "event_subtitle",
    "exp",
    "extend_story",
    "fast_travel",
    "flag_config",
    "font_color",
    "formation",
    "friendmap",
    "gallery",
    "game_quest",
    "growth",
    "happen_event_npc",
    "help",
    "inacode",
    "input",
    "item",
    "item_emission",
    "light",
    "menu_setting",
    "mission",
    "movie",
    "music_app",
    "nfc",
    "opponent_team",
    "override_skill",
    "party",
    "passive",
    "phase",
    "phase_set",
    "photo_mode",
    "players_universe",
    "post",
    "quest",
    "real_skill_config",
    "record",
    "rpg_battle",
    "scene_archive",
    "search_word",
    "setting_menu",
    "shop",
    "skill",
    "skill_technic",
    "skill_view",
    "soccer",
    "soccer_chara_unique_rarity",
    "soccer_drop",
    "soccer_fixed_reward",
    "soccer_map_env",
    "soccer_opponent",
    "soccer_performance",
    "soccer_placement",
    "soccer_player_record",
    "soccer_rank",
    "soccer_suggest",
    "special_tactics",
    "stadium",
    "super_tactics",
    "system_unlock",
    "talk_select",
    "team_build_config",
    "telop_waza",
    "text",
    "trial_take_over",
    "trick",
    "trigger",
    "trophy",
    "uniform",
    "update_notice",
    "user_name_plate",
    "video_waza",
    "vsroute",
    "weather",
    "win_treasure",
];

/// Toutes les décisions de classement, source par source.
pub static REGLES: &[Regle] = &[
    // ---------------------------------------------------------------- niers (sous-commandes)
    r!(
        "niers-vfs",
        Niers,
        Motif::Exact("vfs"),
        servi("/b/{*prefixe}")
    ),
    r!(
        "niers-decode",
        Niers,
        Motif::Exact("decode"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "niers-format",
        Niers,
        Motif::Exact("format"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "niers-lua",
        Niers,
        Motif::Exact("lua"),
        servi("/api/v1/lua/scripts/{*chemin}")
    ),
    r!(
        "niers-textures",
        Niers,
        Motif::Exact("textures"),
        servi("/api/v1/{vue}")
    ),
    r!(
        "niers-video",
        Niers,
        Motif::Exact("video"),
        servi("/api/v1/{vue}")
    ),
    r!(
        "niers-wiki",
        Niers,
        Motif::Exact("wiki"),
        servi("/api/v1/chara")
    ),
    r!(
        "niers-render",
        Niers,
        Motif::Exact("render"),
        servi("/model/{famille}/{fichier}")
    ),
    r!(
        "niers-info",
        Niers,
        Motif::Exact("info"),
        servi("/api/v1/health")
    ),
    // Le moteur de recherche existe (`ignore`, celui de ripgrep) et l'index du VFS est monté :
    // il manque la route. C'est du câblage, et c'est le défaut 1 du lot 8 — `/b` accepte `q`
    // et l'ignore.
    r!(
        "niers-recherche",
        Niers,
        Motif::Exact("find"),
        interne(
            "cherche sur le DISQUE de la machine (moteur `ignore`) ; la recherche dans le VFS du jeu, elle, est servie par /api/v1/recherche"
        )
    ),
    r!(
        "niers-grep",
        Niers,
        Motif::Exact("grep"),
        interne(
            "cherche dans le CONTENU des fichiers du disque : un service web ne lit pas l'arbre de la machine"
        )
    ),
    // Ces quatre-là ont été câblées le 2026-09-06, et trois des quatre raisons écrites
    // ci-dessus étaient FAUSSES : `nie_explore::icons` et `nie_explore::mode_index` n'existent
    // pas — les deux modules vivent dans `nie-cli`, qui n'a pas de cible `[lib]` et n'est donc
    // importable par personne. La logique a été réécrite dans `routes::screens` contre
    // `nie-formats`/`nie-lua`, sans une feature de plus. Une raison qui cite un chemin
    // inexistant envoie le lot suivant chercher au mauvais endroit.
    r!(
        "niers-icons",
        Niers,
        Motif::Exact("icons"),
        servi("/api/v1/icons")
    ),
    r!(
        "niers-mode",
        Niers,
        Motif::Exact("mode"),
        servi("/api/v1/modes/{slug}")
    ),
    // L'avatar, lui, n'a jamais rien demandé de neuf : `chara_edit` et
    // `chara_edit_parts_type_config` sont dans `typed::decode_by_key` depuis le 2026-09-06, et
    // `/api/v1/donnees/famille/{cle}` les sert sans qu'on ait à connaître le chemin VFS.
    r!(
        "niers-avatar",
        Niers,
        Motif::Exact("avatar"),
        servi("/api/v1/donnees/famille/{cle}")
    ),
    // `convert` a deux moitiés. Les feuilles de sprites (CSS/SVG/JSON) sont pur `std` et sont
    // servies en process ; l'encodage d'image (8 formats) reste chez `nie-model-serve`, qui
    // porte les features `images`/`textures` que ce service refuse délibérément d'allumer.
    r!(
        "niers-convert",
        Niers,
        Motif::Exact("convert"),
        servi("/assets/{*chemin}")
    ),
    r!(
        "niers-img",
        Niers,
        Motif::Exact("img"),
        interne("édition d'image : elle écrit un fichier, un site en lecture seule n'écrit pas")
    ),
    r!(
        "niers-save",
        Niers,
        Motif::Exact("save"),
        interne("sauvegardes de joueur : données personnelles, hors périmètre contractuel")
    ),
    r!(
        "niers-strings",
        Niers,
        Motif::Exact("strings"),
        interne("reverse du binaire : coûteux, privilégié, sans public")
    ),
    r!(
        "niers-coverage",
        Niers,
        Motif::Exact("coverage"),
        interne("couverture du RE : mesure interne du dépôt, pas une capacité du jeu")
    ),
    r!(
        "niers-uniform-map",
        Niers,
        Motif::Exact("uniform-map"),
        interne("construction d'un manifeste d'index : outil de build, consommé par l'amont")
    ),
    r!(
        "niers-refresh",
        Niers,
        Motif::Exact("refresh-typed-json"),
        interne("régénère des fichiers à côté du dump : écriture disque")
    ),
    r!(
        "niers-menu-predecode",
        Niers,
        Motif::Exact("menu-predecode"),
        interne("pré-décode dans le dump disque : écriture disque")
    ),
    r!(
        "niers-seed-ui",
        Niers,
        Motif::Exact("seed-ui"),
        interne("ingère dans la base de connaissance : écriture")
    ),
    r!(
        "niers-vn",
        Niers,
        Motif::Exact("vn"),
        interne("produit un catalogue local jamais versionné")
    ),
    r!(
        "niers-mem",
        Niers,
        Motif::Exact("mem"),
        interne("lit la mémoire d'un process du jeu : privilège, machine locale")
    ),
    r!(
        "niers-steam",
        Niers,
        Motif::Exact("steam"),
        interne("identifiants Steam : secrets, jamais côté site")
    ),
    r!(
        "niers-mod",
        Niers,
        Motif::Exact("mod"),
        interne("écrit dans l'installation du jeu : machine locale")
    ),
    r!(
        "niers-viola",
        Niers,
        Motif::Exact("viola"),
        interne("modding LEVEL-5 : écriture, façade d'administration")
    ),
    r!(
        "niers-cpp",
        Niers,
        Motif::Exact("cpp"),
        interne("façade vers le toolkit C++ : API d'administration, non affichée")
    ),
    r!(
        "niers-cs",
        Niers,
        Motif::Exact("cs"),
        interne("façade vers l'outillage .NET : API d'administration, non affichée")
    ),
    r!(
        "niers-backends",
        Niers,
        Motif::Exact("backends"),
        interne("dit ce qui est construit sur CETTE machine : sans objet en ligne")
    ),
    // Le reverse et la forge, en bloc : neuf commandes qui lisent ou écrivent `var/niers.sqlite`.
    filet!(
        "niers-reverse",
        Niers,
        Motif::Tout,
        interne("boucle de reverse-engineering : coûteuse, privilégiée, sans public (§ 5, lot 1)")
    ),
    // ------------------------------------------------------------- Inacord (commandes IPC)
    r!(
        "inacord-pet",
        Inacord,
        Motif::Prefixe("aphrody_pet_"),
        servi("/pet/aphrody.json")
    ),
    r!(
        "inacord-palette",
        Inacord,
        Motif::Exact("aphrody_pixel_mesurer"),
        servi("/api/v1/aphrody/palette")
    ),
    r!(
        "inacord-tokens",
        Inacord,
        Motif::Exact("aphrody_pixel_tokens_css"),
        servi("/api/v1/aphrody/palette")
    ),
    r!(
        "inacord-pixel",
        Inacord,
        Motif::Prefixe("aphrody_pixel_"),
        interne("outillage de direction artistique local (comparer, vectoriser, planche)")
    ),
    // Le VFS : lecture servie par les deux espaces, écriture et export jamais.
    r!(
        "inacord-vfs-ecriture",
        Inacord,
        Motif::Prefixe("vfs_write"),
        interne("écrit dans le VFS : un site en lecture seule n'écrit pas")
    ),
    r!(
        "inacord-vfs-export",
        Inacord,
        Motif::Prefixe("vfs_export"),
        interne("écrit un fichier sur la machine de l'utilisateur")
    ),
    r!(
        "inacord-vfs-extract",
        Inacord,
        Motif::Exact("vfs_extract_to"),
        interne("écrit un fichier sur la machine de l'utilisateur")
    ),
    r!(
        "inacord-vfs-index",
        Inacord,
        Motif::Prefixe("vfs_index_scan"),
        interne("pilote l'indexation locale de l'hôte : sans objet côté serveur")
    ),
    r!(
        "inacord-vfs-cache",
        Inacord,
        Motif::Prefixe("vfs_cache"),
        interne("cache de l'hôte : le site a le sien (moka), non pilotable de l'extérieur")
    ),
    r!(
        "inacord-vfs-texture",
        Inacord,
        Motif::Prefixe("vfs_texture"),
        servi("/assets/{*chemin}")
    ),
    r!(
        "inacord-vfs-glb",
        Inacord,
        Motif::Exact("vfs_glb_bytes_b64"),
        servi("/model/{famille}/{fichier}")
    ),
    r!(
        "inacord-vfs-motion",
        Inacord,
        Motif::Exact("vfs_motion_clips"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "inacord-vfs-audio",
        Inacord,
        Motif::Prefixe("vfs_audio"),
        servi("/assets/{*chemin}")
    ),
    r!(
        "inacord-vfs-video",
        Inacord,
        Motif::Exact("vfs_video_preview_b64"),
        servi("/assets/{*chemin}")
    ),
    r!(
        "inacord-vfs-cfgbin",
        Inacord,
        Motif::Prefixe("vfs_decode_cfgbin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "inacord-vfs-camera",
        Inacord,
        Motif::Exact("vfs_apercu_camera"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "inacord-vfs-navmesh",
        Inacord,
        Motif::Exact("vfs_apercu_navmesh"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "inacord-vfs-recherche",
        Inacord,
        Motif::Exact("vfs_find"),
        servi("/api/v1/recherche")
    ),
    r!(
        "inacord-vfs-recherche-p",
        Inacord,
        Motif::Exact("vfs_find_paged"),
        servi("/api/v1/recherche")
    ),
    r!(
        "inacord-vfs-lecture",
        Inacord,
        Motif::Prefixe("vfs_"),
        servi("/b/{*prefixe}")
    ),
    r!(
        "inacord-preload",
        Inacord,
        Motif::Exact("preload_vfs"),
        interne("montage du VFS de l'hôte : le site le monte en fond au démarrage")
    ),
    // Les catalogues de données : le miroir est là, les routes ne le sont pas.
    r!(
        "inacord-charas",
        Inacord,
        Motif::Exact("game_data_charas"),
        servi("/api/v1/chara")
    ),
    r!(
        "inacord-chara-picker",
        Inacord,
        Motif::Exact("game_data_chara_picker"),
        servi("/api/v1/chara")
    ),
    r!(
        "inacord-movies",
        Inacord,
        Motif::Exact("game_data_movies"),
        servi("/api/v1/{vue}")
    ),
    r!(
        "inacord-musics",
        Inacord,
        Motif::Exact("game_data_musics"),
        servi("/api/v1/{vue}")
    ),
    r!(
        "inacord-gamedata",
        Inacord,
        Motif::Prefixe("game_data_"),
        servi("/api/v1/donnees/famille/{cle}")
    ),
    // Lua : ce qui LIT est servi, ce qui EXÉCUTE ne le sera jamais.
    r!(
        "inacord-lua-session",
        Inacord,
        Motif::Prefixe("lua_session"),
        interne(
            "attache une VM Lua vivante : aucun interpréteur n'est lié dans le service (nie-lua `default-features = false`)"
        )
    ),
    r!(
        "inacord-lua-execute",
        Inacord,
        Motif::Exact("lua_execute"),
        interne("exécute du Lua : refus structurel, cf. `routes::lua`")
    ),
    r!(
        "inacord-lua-eval",
        Inacord,
        Motif::Exact("lua_eval"),
        interne("évalue du Lua : refus structurel, cf. `routes::lua`")
    ),
    r!(
        "inacord-lua-globals",
        Inacord,
        Motif::Exact("lua_globals"),
        interne(
            "énumérer les globales APPELLE le script (cf. `discover_host_calls`) : c'est de l'exécution"
        )
    ),
    r!(
        "inacord-lua-scripts",
        Inacord,
        Motif::Exact("lua_list_scripts"),
        servi("/api/v1/lua/scripts")
    ),
    r!(
        "inacord-lua-chunk",
        Inacord,
        Motif::Exact("lua_chunk_info"),
        servi("/api/v1/lua/scripts/{*chemin}")
    ),
    r!(
        "inacord-lua-disasm",
        Inacord,
        Motif::Exact("lua_disassemble"),
        servi("/api/v1/lua/desassemblage/{*chemin}")
    ),
    // Filet de famille, pas décision : les quatre commandes `lua_*` d'Inacord sont toutes prises
    // par une règle nommée au-dessus. Son vide dit que la famille est classée une par une —
    // c'est l'objectif, et c'est ce qui le faisait remonter à tort dans `regles_mortes`.
    filet!(
        "inacord-lua",
        Inacord,
        Motif::Prefixe("lua_"),
        interne("touche à l'exécution de scripts : refus structurel")
    ),
    // La 3D et l'amont.
    r!(
        "inacord-model-avatar",
        Inacord,
        Motif::Prefixe("model_service_avatar"),
        servi("/api/v1/donnees/famille/{cle}")
    ),
    r!(
        "inacord-model",
        Inacord,
        Motif::Prefixe("model_service_"),
        servi("/assets/{*chemin}")
    ),
    r!(
        "inacord-video",
        Inacord,
        Motif::Prefixe("video_"),
        servi("/api/v1/{vue}")
    ),
    r!(
        "inacord-remote",
        Inacord,
        Motif::Prefixe("remote_"),
        servi("/api/v1/chara")
    ),
    // Tout ce qui touche la machine de l'utilisateur, le binaire du jeu ou un process.
    r!(
        "inacord-forge",
        Inacord,
        Motif::Prefixe("forge::"),
        interne("la forge produit `nie.exe` : elle appartient à la machine de développement")
    ),
    r!(
        "inacord-re",
        Inacord,
        Motif::Prefixe("re_"),
        interne("reverse en direct : lit et écrit la mémoire d'un process")
    ),
    r!(
        "inacord-live",
        Inacord,
        Motif::Prefixe("live_mod::"),
        interne("modifie la mémoire du jeu en cours d'exécution")
    ),
    r!(
        "inacord-viola",
        Inacord,
        Motif::Prefixe("viola::"),
        interne("modding : dump, pack, crypto — écriture")
    ),
    r!(
        "inacord-mcp",
        Inacord,
        Motif::Prefixe("mcp::"),
        interne("installe et pilote un serveur MCP local")
    ),
    r!(
        "inacord-blender",
        Inacord,
        Motif::Prefixe("blender_"),
        interne("pilote Blender sur la machine de l'utilisateur")
    ),
    r!(
        "inacord-blender-addon",
        Inacord,
        Motif::Exact("install_niers_blender_addon"),
        interne("installe un greffon sur la machine de l'utilisateur")
    ),
    r!(
        "inacord-cpk",
        Inacord,
        Motif::Prefixe("raw_cpk_"),
        interne("ouvre un CPK arbitraire du disque : le site ne sert que le VFS monté")
    ),
    r!(
        "inacord-save",
        Inacord,
        Motif::Prefixe("save_"),
        interne("sauvegardes de joueur : données personnelles")
    ),
    r!(
        "inacord-disque",
        Inacord,
        Motif::Prefixe("disk_"),
        interne("accès au disque de l'utilisateur")
    ),
    r!(
        "inacord-clipboard",
        Inacord,
        Motif::Prefixe("clipboard_"),
        interne("presse-papiers du système")
    ),
    r!(
        "inacord-defaut",
        Inacord,
        Motif::Prefixe("default_"),
        interne("chemins par défaut de la machine hôte : sans objet en ligne")
    ),
    r!(
        "inacord-open",
        Inacord,
        Motif::Prefixe("open_"),
        interne("ouvre une application locale")
    ),
    filet!(
        "inacord-hote",
        Inacord,
        Motif::Tout,
        interne(
            "relève de l'hôte desktop : disque, fenêtre, presse-papiers, corbeille (cf. `capacites()` du contrat asset-source)"
        )
    ),
    // ------------------------------------------------------------------ Azalée (pages)
    r!(
        "azalee-outils-updater",
        Azalee,
        Motif::Exact("/tools/niers"),
        interne(
            "point de mise à jour d'Inacord : il doit rester à l'URL que les 0.5.x interrogent"
        )
    ),
    // Les cinq outils du wiki, un par un. La règle de préfixe qui les couvrait toutes les
    // classait `manquant` en bloc ; la mesure du 2026-09-06 dit qu'elles n'étaient pas dans le
    // même état — deux étaient déjà servies sans que personne l'ait vu, et deux ne demandaient
    // qu'une route sur du moteur déjà écrit. Un état unique pour cinq capacités différentes,
    // c'est exactement ce que la matrice existe pour empêcher.
    r!(
        "azalee-outils-stats",
        Azalee,
        Motif::Exact("/tools/stats"),
        servi("/api/v1/regles/stats")
    ),
    r!(
        "azalee-outils-comparaison",
        Azalee,
        Motif::Exact("/tools/compare"),
        servi("/api/v1/regles/comparaison")
    ),
    // Le tirage est du hasard d'interface ; ce qui manquait était l'accès aux viviers, et
    // `/api/v1/entites/{table}` les sert avec ses filtres par colonne depuis le 2026-09-06.
    r!(
        "azalee-outils-tirage",
        Azalee,
        Motif::Exact("/tools/random-team"),
        servi("/api/v1/entites/{table}")
    ),
    // La notation d'équipe : `nie_core::optimisation::calculer_synergie_equipe`, écrite,
    // testée, et routée nulle part jusqu'ici — pendant que le wiki la recalculait en
    // TypeScript dans le navigateur. Deux implémentations d'une règle divergent.
    r!(
        "azalee-outils-equipe",
        Azalee,
        Motif::Exact("/tools/my-team"),
        servi("/api/v1/team/synergy")
    ),
    // La traduction. Celle du wiki interroge sept tables avec un score flou ; celle-ci aligne
    // les langues par le HASH du texte du jeu, qui est ce qui les aligne réellement.
    r!(
        "azalee-outils-traduction",
        Azalee,
        Motif::Exact("/tools/translator"),
        servi("/api/v1/text/translate")
    ),
    r!(
        "azalee-outils",
        Azalee,
        Motif::Prefixe("/tools"),
        interne(
            "page d'index sans donnée : cinq cartes de navigation écrites en dur. L'équivalent d'Aphrody est son menu, engendré par le serveur — une page de liens ne se porte pas, elle se remplace"
        )
    ),
    r!(
        "azalee-dashboard",
        Azalee,
        Motif::Prefixe("/dashboard"),
        interne("administration du wiki : authentifiée, produit Rose Griffon")
    ),
    r!(
        "azalee-auth",
        Azalee,
        Motif::Prefixe("/auth"),
        interne("comptes utilisateurs : hors périmètre d'Aphrody")
    ),
    r!(
        "azalee-legal",
        Azalee,
        Motif::Prefixe("/legal"),
        interne("mentions légales de Rose Griffon : propres à Azalée")
    ),
    r!(
        "azalee-news",
        Azalee,
        Motif::Prefixe("/news"),
        interne("éditorial du wiki : Azalée demeure le wiki de référence")
    ),
    r!(
        "azalee-patch",
        Azalee,
        Motif::Prefixe("/patch-notes"),
        interne("éditorial du wiki : Azalée demeure le wiki de référence")
    ),
    r!(
        "azalee-profil",
        Azalee,
        Motif::Prefixe("/profil"),
        interne("profils d'utilisateurs : données personnelles, jamais migrées")
    ),
    r!(
        "azalee-compte",
        Azalee,
        Motif::Prefixe("/settings"),
        interne("réglages de compte : hors périmètre d'Aphrody")
    ),
    r!(
        "azalee-login",
        Azalee,
        Motif::Exact("/login"),
        interne("authentification du wiki")
    ),
    r!(
        "azalee-2fa",
        Azalee,
        Motif::Exact("/2fa"),
        interne("authentification du wiki")
    ),
    r!(
        "azalee-maintenance",
        Azalee,
        Motif::Exact("/maintenance"),
        interne("page de service du wiki")
    ),
    r!(
        "azalee-charte",
        Azalee,
        Motif::Exact("/charte"),
        interne("éditorial Rose Griffon")
    ),
    r!(
        "azalee-contact",
        Azalee,
        Motif::Exact("/contact"),
        interne("éditorial Rose Griffon")
    ),
    r!(
        "azalee-soutenir",
        Azalee,
        Motif::Exact("/soutenir"),
        interne("éditorial Rose Griffon")
    ),
    r!(
        "azalee-accueil",
        Azalee,
        Motif::Exact("/"),
        interne("accueil du wiki : Aphrody a le sien, dans la DA du jeu")
    ),
    r!(
        "azalee-chara",
        Azalee,
        Motif::Prefixe("/chara"),
        servi("/api/v1/chara")
    ),
    // Les fiches encyclopédiques restent sur Azalée — mais leurs DONNÉES doivent être
    // atteignables depuis Aphrody, et elles ne le sont pas. C'est `manquant`, pas `interne` :
    // « reste sur Azalée » justifie la page, jamais l'absence de la donnée.
    // Les fiches encyclopediques restent sur Azalee — c'est le lot 6 du plan, et la separation
    // de marque tient. Ce qui NE devait pas rester ailleurs, c'est la donnee : elle est servie
    // depuis le 2026-09-06 par /api/v1/donnees/famille/{cle}, sur les 121 familles nommees que
    // le VFS porte reellement. « Reste sur Azalee » justifie la page, jamais l'absence de la
    // donnee — et c'est pour cela que cette regle n'a pu devenir `interne` qu'apres le cablage.
    filet!(
        "azalee-catalogues",
        Azalee,
        Motif::Tout,
        interne(
            "fiche encyclopedique : Azalee demeure le wiki de reference (lot 6) ; la donnee du jeu, elle, est servie par /api/v1/donnees/famille/{cle}"
        )
    ),
    // -------------------------------------------------------------- Azalée (routes d'API)
    r!(
        "azalee-api-updater",
        AzaleeApi,
        Motif::Exact("/tools/niers/latest.json"),
        interne(
            "point de mise à jour d'Inacord : les 0.5.x déjà installés interrogent CETTE URL, elle ne bouge pas"
        )
    ),
    r!(
        "azalee-api-health",
        AzaleeApi,
        Motif::Exact("/api/health"),
        servi("/api/v1/health")
    ),
    r!(
        "azalee-api-save",
        AzaleeApi,
        Motif::Prefixe("/api/save"),
        servi("/api/v1/save/roster")
    ),
    r!(
        "azalee-api-vroid",
        AzaleeApi,
        Motif::Prefixe("/api/vroid"),
        interne("OAuth VRoid : session utilisateur et secrets tiers")
    ),
    r!(
        "azalee-api-auth",
        AzaleeApi,
        Motif::Prefixe("/api/auth"),
        interne("authentification du wiki")
    ),
    r!(
        "azalee-api-admin",
        AzaleeApi,
        Motif::Prefixe("/api/admin"),
        interne("administration du wiki")
    ),
    r!(
        "azalee-api-cron",
        AzaleeApi,
        Motif::Prefixe("/api/cron"),
        interne("tâches planifiées du wiki")
    ),
    r!(
        "azalee-api-jeton",
        AzaleeApi,
        Motif::Exact("/api/supabase-token"),
        interne("émet un jeton : un secret ne traverse jamais Aphrody")
    ),
    r!(
        "azalee-api-llm",
        AzaleeApi,
        Motif::Prefixe("/api/llm"),
        interne("passerelle vers un service tiers facturé")
    ),
    r!(
        "azalee-api-rag",
        AzaleeApi,
        Motif::Prefixe("/api/rag"),
        interne("index vectoriel du wiki : propre à Azalée")
    ),
    filet!(
        "azalee-api-editorial",
        AzaleeApi,
        Motif::Tout,
        interne("éditorial et métadonnées du wiki : Azalée demeure le wiki de référence")
    ),
    // ------------------------------------------------------------------- nie-data (modules)
    r!(
        "data-cfgbin",
        NieData,
        Motif::Exact("cfgbin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "data-typed",
        NieData,
        Motif::Exact("typed"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "data-chara-base",
        NieData,
        Motif::Exact("chara_base"),
        servi("/api/v1/chara")
    ),
    r!(
        "data-chara-text",
        NieData,
        Motif::Exact("chara_text"),
        servi("/api/v1/chara")
    ),
    r!(
        "data-hash",
        NieData,
        Motif::Exact("hash"),
        interne("table de hachage interne du décodage : ce n'est pas une donnée du jeu")
    ),
    r!(
        "data-help",
        NieData,
        Motif::Exact("help"),
        interne("textes d'aide du décodeur : outillage")
    ),
    // Les 110 familles restantes : le parseur est écrit, testé par golden, et **rien** ne
    // l'expose. C'est la mesure qui a motivé le plan : le dépôt sait faire dix fois ce qu'il
    // montre.
    r!(
        "data-typees",
        NieData,
        Motif::Parmi(MODULES_TYPES),
        servi("/api/v1/donnees/{*chemin}")
    ),
    // Deux modules que la facade ne peut PAS porter telle quelle, et il faut le dire :
    // Le diagnostic tenait : `parse_player_passives` prend TROIS tables de texte (pas deux) en
    // plus du conteneur, et aucune façade à un argument ne peut l'exprimer. La réponse n'était
    // donc pas une entrée de plus dans `decode_by_key` mais une route qui joint cinq fichiers.
    r!(
        "data-passives",
        NieData,
        Motif::Exact("passives"),
        servi("/api/v1/passives")
    ),
    // Pas de règle `team` : le module a été FUSIONNÉ dans `enjoy_mode_team` le 2026-09-06 (deux
    // ports du même fichier, arrivés dans le même commit, sans antériorité pour les départager).
    // Lui laisser une pierre tombale ici serait un piège : un futur `team.rs`, sans rapport,
    // hériterait de son classement. La règle fourre-tout le rattraperait en `manquant`, ce qui
    // est le bon défaut. L'histoire de la fusion vit dans `enjoy_mode_team::parse_enjoy_mode_teams`.
    r!(
        "data-playstyle",
        NieData,
        Motif::Exact("playstyle"),
        servi("/api/v1/playstyles")
    ),
    // `cond` (cadrage) et `unlock_condition` (sémantique) lisent le MÊME blob base64, pris dans
    // un champ d'un autre fichier. Ils ne prennent pas de conteneur : `decode_by_key` ne peut
    // pas les appeler. C'était un manque d'adresse, pas de code.
    r!(
        "data-cond",
        NieData,
        Motif::Exact("cond"),
        servi("/api/v1/conditions/{blob}")
    ),
    r!(
        "data-unlock",
        NieData,
        Motif::Exact("unlock_condition"),
        servi("/api/v1/conditions/{blob}")
    ),
    filet!(
        "data-familles",
        NieData,
        Motif::Tout,
        manquant("crates/engine/nie-data/src/<module>.rs — parseur typé, golden testé, sans route")
    ),
    // ---------------------------------------------------------------- nie-formats (modules)
    r!(
        "formats-vfs",
        NieFormats,
        Motif::Exact("vfs"),
        servi("/f/{*chemin}")
    ),
    r!(
        "formats-cpk",
        NieFormats,
        Motif::Exact("cpk"),
        servi("/b/{*prefixe}")
    ),
    r!(
        "formats-crilayla",
        NieFormats,
        Motif::Exact("crilayla"),
        servi("/f/{*chemin}")
    ),
    r!(
        "formats-cfgbin",
        NieFormats,
        Motif::Exact("cfgbin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-level5",
        NieFormats,
        Motif::Exact("level5"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-decode",
        NieFormats,
        Motif::Exact("decode"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-encodeurs",
        NieFormats,
        Motif::Suffixe("_encode"),
        interne(
            "encodeur : écriture, et `encode_t2b` n'est pas encore fidèle (cf. CLAUDE.md § modding)"
        )
    ),
    r!(
        "formats-patch",
        NieFormats,
        Motif::Suffixe("_patch"),
        interne("patch d'octets en place : écriture, modding")
    ),
    r!(
        "formats-g4tx",
        NieFormats,
        Motif::Prefixe("g4tx"),
        servi("/assets/{*chemin}")
    ),
    r!(
        "formats-dxbc",
        NieFormats,
        Motif::Exact("dxbc"),
        servi("/assets/{*chemin}")
    ),
    r!(
        "formats-audio",
        NieFormats,
        Motif::Exact("cri_audio"),
        servi("/assets/{*chemin}")
    ),
    r!(
        "formats-usm",
        NieFormats,
        Motif::Exact("usm"),
        servi("/api/v1/{vue}")
    ),
    r!(
        "formats-mp4",
        NieFormats,
        Motif::Exact("mp4"),
        servi("/api/v1/{vue}")
    ),
    r!(
        "formats-webm",
        NieFormats,
        Motif::Exact("webm"),
        servi("/api/v1/{vue}")
    ),
    r!(
        "formats-assemble",
        NieFormats,
        Motif::Exact("assemble"),
        servi("/model/{famille}/{fichier}")
    ),
    r!(
        "formats-geometrie",
        NieFormats,
        Motif::Prefixe("g4p"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-g4md",
        NieFormats,
        Motif::Exact("g4md"),
        servi("/api/v1/3d")
    ),
    r!(
        "formats-g4mg",
        NieFormats,
        Motif::Exact("g4mg"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-g4cm",
        NieFormats,
        Motif::Exact("g4cm"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-g4sk",
        NieFormats,
        Motif::Exact("g4sk"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-g4mt",
        NieFormats,
        Motif::Exact("g4mt"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-mevbin",
        NieFormats,
        Motif::Exact("mevbin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-objbin",
        NieFormats,
        Motif::Exact("objbin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-col",
        NieFormats,
        Motif::Exact("col"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-pathname",
        NieFormats,
        Motif::Exact("pathname"),
        interne("portage byte-exact d'une fonction du binaire : brique, pas capacité")
    ),
    r!(
        "formats-libc",
        NieFormats,
        Motif::Prefixe("str"),
        interne("portages byte-exact des fonctions chaîne du binaire : briques du RE")
    ),
    r!(
        "formats-lip",
        NieFormats,
        Motif::Exact("lip"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-navm",
        NieFormats,
        Motif::Exact("navm"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-g4la",
        NieFormats,
        Motif::Exact("g4la"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-g4vs",
        NieFormats,
        Motif::Exact("g4vs"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "formats-g4ma",
        NieFormats,
        Motif::Exact("g4ma"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    // Les sept inspecteurs, câblés le 2026-09-06. Ils partagent une propriété que la règle
    // fourre-tout écrasait : ils ne partent PAS des octets d'un fichier mais d'une structure
    // déjà lue — un atlas, un canvas RGBA, deux images à comparer. `decode/{chemin}` ne
    // pouvait donc pas les servir, et ce n'était pas un manque de décodeur.
    //
    // Aucun n'a demandé d'allumer `images` ni `textures` : les six sont sous `std` seul, la
    // feature par défaut. Encore la leçon du § 9 — le code était déjà lié dans le binaire.
    r!(
        "formats-font",
        NieFormats,
        Motif::Exact("font"),
        servi("/api/v1/inspect/font/{*path}")
    ),
    r!(
        "formats-menu",
        NieFormats,
        Motif::Exact("menu"),
        servi("/api/v1/inspect/menu/{*path}")
    ),
    r!(
        "formats-spritesheet",
        NieFormats,
        Motif::Exact("sprite_sheet"),
        servi("/api/v1/inspect/spritesheet/{*path}")
    ),
    r!(
        "formats-imgmetric",
        NieFormats,
        Motif::Exact("imgmetric"),
        servi("/api/v1/inspect/compare")
    ),
    r!(
        "formats-planche",
        NieFormats,
        Motif::Exact("planche"),
        servi("/api/v1/inspect/plate")
    ),
    // La route existe, décode et se teste — mais **le corpus de ce jeu est vide** : `niers vfs
    // find 'nxtch'` rend 0, et le contenu d'un `.g4tx` est du DDS (vérifié sur `an000100.g4tx`,
    // 6 magics `DDS `, 0 `NXTCH`). La réponse publie `corpus: 0` plutôt que de laisser croire à
    // une panne. Servi veut dire « la route rend le contenu interprété », pas « ce jeu en a ».
    r!(
        "formats-nxtch",
        NieFormats,
        Motif::Exact("nxtch"),
        servi("/api/v1/inspect/texture-chunk/{*path}")
    ),
    // `raster2d` n'expose que `crop_rgba` et `scale_nearest` sur un tampon fourni : c'est une
    // brique de calcul, comme `pathname` et les portages libc classés `interne` juste au-dessus.
    r!(
        "formats-raster2d",
        NieFormats,
        Motif::Exact("raster2d"),
        interne(
            "primitives 2D pures (rognage, mise à l'échelle au plus proche) sur un tampon RGBA fourni : brique de calcul, pas une capacité — même classement que `pathname` et les portages libc"
        )
    ),
    // `image_out` est le SEUL des huit hors d'atteinte en process : son module entier est
    // `#[cfg(feature = "images")]`, et l'allumer tirerait `image` 0.25 et ses sept back-ends
    // dans ce service. La capacité, elle, est servie — par l'amont qui porte la feature, comme
    // `g4tx` juste au-dessus. Mesuré le 2026-09-06 à travers le proxy, pas supposé : les huit
    // formats d'`ImageOut::TOUS` répondent 200 sur
    // `/assets/export/dx11/menu/200_icon/02_icon_item/icon_item01.g4tx?format=<f>` —
    // webp 56 534 o, png 58 844, gif 21 384, bmp 262 266, tga 127 420, tiff 262 358,
    // qoi 62 879, jpg 27 315, chacun avec son `Content-Type`.
    r!(
        "formats-image-out",
        NieFormats,
        Motif::Exact("image_out"),
        servi("/assets/{*chemin}")
    ),
    filet!(
        "formats-restants",
        NieFormats,
        Motif::Tout,
        manquant(
            "crates/engine/nie-formats/src/<module>.rs — parseur écrit, aucune route ne l'appelle"
        )
    ),
    // -------------------------------------------------------------------- nie-lua (pub fn)
    r!(
        "lua-bytecode",
        NieLua,
        Motif::Exact("parse"),
        servi("/api/v1/lua/scripts/{*chemin}")
    ),
    r!(
        "lua-disasm",
        NieLua,
        Motif::Exact("disassemble"),
        servi("/api/v1/lua/desassemblage/{*chemin}")
    ),
    r!(
        "lua-decode",
        NieLua,
        Motif::Exact("decode_instruction"),
        servi("/api/v1/lua/desassemblage/{*chemin}")
    ),
    r!(
        "lua-detection",
        NieLua,
        Motif::Exact("is_lua52_bytecode"),
        servi("/api/v1/lua/scripts/{*chemin}")
    ),
    r!(
        "lua-validation",
        NieLua,
        Motif::Exact("validate_bytecode"),
        servi("/api/v1/lua/scripts/{*chemin}")
    ),
    r!(
        "lua-liste",
        NieLua,
        Motif::Exact("collect_lua_files"),
        servi("/api/v1/lua/scripts")
    ),
    r!(
        "lua-crc32",
        NieLua,
        Motif::Exact("crc32"),
        interne("brique de calcul : pas une capacité du site")
    ),
    // L'analyse statique de `nie-lua` porte sur du Lua SOURCE. Mesure du 2026-09-06 sur les
    // 255 308 entrees du VFS : **0 fichier `.lua`**, 1 197 `.lua.bin`. Le corpus est vide, et
    // la feature `analysis` (tree-sitter) n'est meme pas liee ici. Ce n'est pas un cablage
    // qu'on remet a plus tard : c'est une capacite sans objet sur ce jeu.
    r!(
        "lua-analyse",
        NieLua,
        Motif::Prefixe("analyze"),
        interne(
            "analyse de Lua SOURCE : le jeu n'en contient aucun (0 `.lua` pour 1 197 `.lua.bin` sur 255 308 entrees)"
        )
    ),
    r!(
        "lua-chaines",
        NieLua,
        Motif::Exact("is_interesting_string"),
        interne("predicat de l'analyse statique de Lua source : meme corpus vide")
    ),
    r!(
        "lua-chemins",
        NieLua,
        Motif::Prefixe("script_logical_base"),
        interne("résolution de chemins de l'hôte d'exécution")
    ),
    filet!(
        "lua-exec",
        NieLua,
        Motif::Tout,
        interne(
            "VM, hôte de menu, capture de `print`, découverte d'appels : TOUT ceci exécute le script — `nie-site` lie `nie-lua` avec `default-features = false` et un test le vérifie"
        )
    ),
    // ------------------------------------------------------------------ iecode (C++)
    filet!(
        "iecode-admin",
        Iecode,
        Motif::Tout,
        interne(
            "toolkit C++ atteint par `niers cpp` : API d'administration, non affichée (§ 5, lot 7)"
        )
    ),
    // ------------------------------------------------------------------ VFS (par extension)
    r!(
        "vfs-cfgbin",
        Vfs,
        Motif::Exact(".cfg.bin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-luabin",
        Vfs,
        Motif::Exact(".lua.bin"),
        servi("/api/v1/lua/scripts/{*chemin}")
    ),
    r!(
        "vfs-g4tx",
        Vfs,
        Motif::Exact(".g4tx"),
        servi("/assets/{*chemin}")
    ),
    r!("vfs-g4md", Vfs, Motif::Exact(".g4md"), servi("/api/v1/3d")),
    r!(
        "vfs-acb",
        Vfs,
        Motif::Exact(".acb"),
        servi("/assets/{*chemin}")
    ),
    r!(
        "vfs-awb",
        Vfs,
        Motif::Exact(".awb"),
        interne(
            "banque de 7,49 Gio catalogée par son `.acb` (0,10 Gio) : l'exposer fichier par fichier dupliquerait le catalogue et servirait des pistes sans nom"
        )
    ),
    r!("vfs-usm", Vfs, Motif::Exact(".usm"), servi("/api/v1/{vue}")),
    r!(
        "vfs-g4pk",
        Vfs,
        Motif::Exact(".g4pk"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-g4mg",
        Vfs,
        Motif::Exact(".g4mg"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-objbin",
        Vfs,
        Motif::Exact(".objbin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-g4pkm",
        Vfs,
        Motif::Exact(".g4pkm"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-g4cm",
        Vfs,
        Motif::Exact(".g4cm"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-col",
        Vfs,
        Motif::Exact(".col"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-g4sk",
        Vfs,
        Motif::Exact(".g4sk"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-mevbin",
        Vfs,
        Motif::Exact(".mevbin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-g4mt",
        Vfs,
        Motif::Exact(".g4mt"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-texte",
        Vfs,
        Motif::Exact(".log"),
        servi("/f/{*chemin}")
    ),
    r!(
        "vfs-texte-cfg",
        Vfs,
        Motif::Exact(".cfg"),
        servi("/f/{*chemin}")
    ),
    r!(
        "vfs-acf",
        Vfs,
        Motif::Exact(".acf"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-sans-extension",
        Vfs,
        Motif::Exact("(sans extension)"),
        servi("/f/{*chemin}")
    ),
    // Les suffixes de révision (`.r41152`, `.r65902`…) : le nom est unique au fichier près,
    // le contenu ne l'est pas — G4PK au magic, T2B en le lisant. Mesuré le 2026-09-06.
    r!(
        "vfs-revision",
        Vfs,
        Motif::Prefixe(".r"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    // Ce que le dépôt sait décoder et que le site ne rend pas — la définition même de
    // `manquant`. Les `.p3lip` en sont l'essentiel : 21 047 fichiers, un parseur écrit.
    // Cablees le 2026-09-06 apres que la matrice les eut fait apparaitre : 21 250 fichiers,
    // 124/124 decodes par `scripts/validation/mesurer-level5.sh`. Cf. `routes::level5`.
    r!(
        "vfs-p3lip",
        Vfs,
        Motif::Exact(".p3lip"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-g4nv",
        Vfs,
        Motif::Exact(".g4nv"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-g4ma",
        Vfs,
        Motif::Exact(".g4ma"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-g4vs",
        Vfs,
        Motif::Exact(".g4vs"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-g4la",
        Vfs,
        Motif::Exact(".g4la"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    // ─────────────────────────────────────────────────────────────────────────────────────
    // Les huit familles que ce plan appelait « shaders, effets, particules, tissu : aucun
    // parseur dans le dépôt, du reverse est nécessaire avant toute route ».
    //
    // **3 591 fichiers, et le dépôt les décodait déjà.** C'est la QUATRIÈME occurrence du même
    // défaut (§ 9 bis pour `.g4ma`/`.g4vs`/`.g4la`, puis les `.bin` ci-dessous), et il se
    // répète pour une raison structurelle : le classement se fait sur l'**extension**, la
    // lecture sur le **magic**. Un `.pfxo` ressortait « ni magic connu » en publiant
    // `44 58 42 43` — `DXBC` en ASCII. Le message d'erreur portait la réfutation de ce qu'il
    // affirmait, et personne ne l'a lu pendant des semaines.
    //
    // Mesuré le 2026-09-06 par `scripts/validation/mesurer-formats-bloques.sh`, échantillon à
    // pas régulier, jeton de format exigé dans le corps : **219/219 décodages conformes**.
    //
    // | Famille | Fichiers | Jeton | Ce qui les lit |
    // |---|---:|---|---|
    // | `.vfxo` | 1 335 | `dxbc` | `nie_formats::dxbc` — shaders de sommets |
    // | `.pfxo` | 1 113 | `dxbc` | idem — shaders de pixels |
    // | `.cfxo` | 29 | `dxbc` | idem — shaders de calcul |
    // | `.gfxo` | 20 | `dxbc` | idem — shaders de géométrie |
    // | `.ptlb` | 657 | `t2b` | conteneur T2B — particules |
    // | `.fxbin` | 372 | `t2b` | conteneur T2B — effets |
    // | `.clobin` | 39 | `t2b` | conteneur T2B — tissu |
    // | `.linb` | 16 | `t2b` | conteneur T2B |
    //
    // Le branchement DXBC a demandé **onze lignes** dans `routes::formats::identifier` : le
    // parseur, lui, existait « depuis toujours », et `nie_formats::decode` le disait dans un
    // commentaire que la matrice n'a jamais lu. Un plan ne lit pas les commentaires : il faut
    // interroger la route.
    r!(
        "vfs-shaders",
        Vfs,
        Motif::Suffixe("fxo"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-fxbin",
        Vfs,
        Motif::Exact(".fxbin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-ptlb",
        Vfs,
        Motif::Exact(".ptlb"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-clobin",
        Vfs,
        Motif::Exact(".clobin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    r!(
        "vfs-linb",
        Vfs,
        Motif::Exact(".linb"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    // Ce qu'aucun parseur du dépôt ne connaît : du reverse d'abord.
    // `.g4tg` — IDENTIFIÉ le 2026-09-06, toujours `bloqué`, et la nuance est celle du § 4 :
    // « bloqué » veut dire qu'il faut du reverse, pas qu'on ignore ce que c'est.
    //
    // Ce qui a été mesuré sur les 9 fichiers extraits : (1) chacun est **voisin d'un `.g4tx` de
    // même stem**, 9/9 ; (2) chaque taille est un **multiple exact de 1 024 octets**, 9/9 —
    // l'alignement de page d'un téléversement GPU ; (3) l'écart entre lignes voisines, à des
    // largeurs plausibles, tombe **3 à 4 fois sous** celui des mêmes lignes mélangées
    // (`eb01800` en 512×352 : 6,28 contre 23,72), donc la donnée a une structure spatiale ;
    // (4) plusieurs en-têtes et queues sont des constantes RGBA franches (`000000ff`,
    // `808080ff`, `ffffff00`, `7f7fffff`).
    //
    // Ce qui a été **réfuté** : ce n'est ni du RGBA8 linéaire ni du BC3 aux dimensions
    // évidentes — les deux rendus ont été produits et regardés. La disposition est donc tuilée
    // ou permutée, et elle n'est pas reversée. Dire « texture » sans dire « disposition
    // inconnue » serait exactement l'hypothèse prise pour une identification que la version
    // précédente de cette raison refusait.
    r!(
        "vfs-g4tg",
        Vfs,
        Motif::Exact(".g4tg"),
        bloque(
            "charge utile de TEXTURE, disposition non reversée : 9/9 voisins d'un `.g4tx` de même stem, 9/9 alignés sur 1 024 o, structure spatiale mesurée (écart inter-lignes 3 à 4× sous le mélange) ; ni RGBA8 linéaire ni BC3 aux dimensions évidentes — les deux rendus ont été produits et écartés"
        )
    ),
    // `.bin` — ce n'était PAS du reverse, c'était une erreur de classement.
    //
    // Les 10 fichiers portent le numéro de version **avant** le `.bin` au lieu d'après
    // (`formation_config.cfg_0.00.32.bin` et non `formation_config_0.00.32.cfg.bin`), si bien
    // que l'extension dérivée est `.bin` et qu'aucune règle de `cfg.bin` ne les attrapait. Le
    // contenu, lui, n'a jamais changé : mesuré le 2026-09-06 en interrogeant la route montée,
    // **10/10 décodent** — 6 RDBN (`formation_config`, `soccer_game_config`, `team_config`,
    // deux versions chacun) et 4 T2B (`font_style` ×3 locales, `ev99_90100.cfg_test`).
    //
    // C'est la troisième fois que ce plan classe `bloqué` quelque chose que le dépôt décodait
    // déjà (cf. § 9 bis, les `.g4ma`/`.g4vs`/`.g4la`). La leçon ne change pas : on interroge la
    // route avant d'écrire « aucun parseur ».
    r!(
        "vfs-bin-cfg",
        Vfs,
        Motif::Exact(".bin"),
        servi("/api/v1/formats/decode/{*chemin}")
    ),
    filet!(
        "vfs-effets",
        Vfs,
        Motif::Tout,
        bloque(
            "shaders, effets, particules, tissu : aucun parseur dans le dépôt, du reverse est nécessaire avant toute route"
        )
    ),
];
