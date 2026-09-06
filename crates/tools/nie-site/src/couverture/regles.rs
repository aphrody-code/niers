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

use super::{Etat, Motif, Regle, Source};

/// Raccourci de déclaration : `regle!(id, Source, motif, etat)`.
macro_rules! r {
    ($id:literal, $src:ident, $motif:expr, $etat:expr) => {
        Regle {
            id: $id,
            source: Source::$src,
            motif: $motif,
            etat: $etat,
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
    "ability_learning", "academic_year", "activity", "add_content_equip", "add_model", "ai", 
    "ai_type", "aura", "banner", "basara", "belong_team", "boost_grp", "capsule", 
    "change_aura_skill_config", "chara_bank", "chara_base", "chara_costume", "chara_description", 
    "chara_details", "chara_edit", "chara_menu_resource", "chara_param", "chara_series", 
    "chat_emote", "chronicle_top", "command", "craft", "ctrl_chara", "dictionary", "dungeon", 
    "emblems", "enjoy_mode_team", "event_bustup", "event_map_tag", "event_subtitle", "exp", 
    "extend_story", "fast_travel", "flag_config", "font_color", "formation", "friendmap", 
    "gallery", "game_quest", "growth", "happen_event_npc", "help", "inacode", "input", "item", 
    "item_emission", "light", "menu_setting", "mission", "movie", "music_app", "nfc", 
    "opponent_team", "override_skill", "party", "passive", "phase", "phase_set", "photo_mode", 
    "players_universe", "post", "quest", "real_skill_config", "record", "rpg_battle", 
    "scene_archive", "search_word", "setting_menu", "shop", "skill", "skill_technic", "skill_view", 
    "soccer", "soccer_chara_unique_rarity", "soccer_drop", "soccer_fixed_reward", "soccer_map_env", 
    "soccer_opponent", "soccer_performance", "soccer_placement", "soccer_player_record", 
    "soccer_rank", "soccer_suggest", "special_tactics", "stadium", "super_tactics", 
    "system_unlock", "talk_select", "team_build_config", "telop_waza", "trial_take_over", "trick", 
    "trigger", "trophy", "uniform", "update_notice", "user_name_plate", "video_waza", "vsroute", 
    "weather", "win_treasure",
];

/// Toutes les décisions de classement, source par source.
pub static REGLES: &[Regle] = &[
    // ---------------------------------------------------------------- niers (sous-commandes)
    r!("niers-vfs", Niers, Motif::Exact("vfs"), servi("/b/{*prefixe}")),
    r!("niers-decode", Niers, Motif::Exact("decode"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("niers-format", Niers, Motif::Exact("format"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("niers-lua", Niers, Motif::Exact("lua"), servi("/api/v1/lua/scripts/{*chemin}")),
    r!("niers-textures", Niers, Motif::Exact("textures"), servi("/api/v1/{vue}")),
    r!("niers-video", Niers, Motif::Exact("video"), servi("/api/v1/{vue}")),
    r!("niers-wiki", Niers, Motif::Exact("wiki"), servi("/api/v1/chara")),
    r!("niers-render", Niers, Motif::Exact("render"), servi("/model/{famille}/{fichier}")),
    r!("niers-info", Niers, Motif::Exact("info"), servi("/api/v1/health")),
    // Le moteur de recherche existe (`ignore`, celui de ripgrep) et l'index du VFS est monté :
    // il manque la route. C'est du câblage, et c'est le défaut 1 du lot 8 — `/b` accepte `q`
    // et l'ignore.
    r!("niers-recherche", Niers, Motif::Exact("find"), interne("cherche sur le DISQUE de la machine (moteur `ignore`) ; la recherche dans le VFS du jeu, elle, est servie par /api/v1/recherche")),
    r!("niers-grep", Niers, Motif::Exact("grep"), interne("cherche dans le CONTENU des fichiers du disque : un service web ne lit pas l'arbre de la machine")),
    r!("niers-icons", Niers, Motif::Exact("icons"), manquant("nie_explore::icons — index nom → atlas + rectangle")),
    r!("niers-avatar", Niers, Motif::Exact("avatar"), manquant("nie_data::chara_edit — catalogue, parts, recettes")),
    r!("niers-mode", Niers, Motif::Exact("mode"), manquant("nie_explore::mode_index — écrans, calques, scripts par mode")),
    r!("niers-convert", Niers, Motif::Exact("convert"), manquant("nie_formats::assemble / image_out")),
    r!("niers-img", Niers, Motif::Exact("img"), interne("édition d'image : elle écrit un fichier, un site en lecture seule n'écrit pas")),
    r!("niers-save", Niers, Motif::Exact("save"), interne("sauvegardes de joueur : données personnelles, hors périmètre contractuel")),
    r!("niers-strings", Niers, Motif::Exact("strings"), interne("reverse du binaire : coûteux, privilégié, sans public")),
    r!("niers-coverage", Niers, Motif::Exact("coverage"), interne("couverture du RE : mesure interne du dépôt, pas une capacité du jeu")),
    r!("niers-uniform-map", Niers, Motif::Exact("uniform-map"), interne("construction d'un manifeste d'index : outil de build, consommé par l'amont")),
    r!("niers-refresh", Niers, Motif::Exact("refresh-typed-json"), interne("régénère des fichiers à côté du dump : écriture disque")),
    r!("niers-menu-predecode", Niers, Motif::Exact("menu-predecode"), interne("pré-décode dans le dump disque : écriture disque")),
    r!("niers-seed-ui", Niers, Motif::Exact("seed-ui"), interne("ingère dans la base de connaissance : écriture")),
    r!("niers-vn", Niers, Motif::Exact("vn"), interne("produit un catalogue local jamais versionné")),
    r!("niers-mem", Niers, Motif::Exact("mem"), interne("lit la mémoire d'un process du jeu : privilège, machine locale")),
    r!("niers-steam", Niers, Motif::Exact("steam"), interne("identifiants Steam : secrets, jamais côté site")),
    r!("niers-mod", Niers, Motif::Exact("mod"), interne("écrit dans l'installation du jeu : machine locale")),
    r!("niers-viola", Niers, Motif::Exact("viola"), interne("modding LEVEL-5 : écriture, façade d'administration")),
    r!("niers-cpp", Niers, Motif::Exact("cpp"), interne("façade vers le toolkit C++ : API d'administration, non affichée")),
    r!("niers-cs", Niers, Motif::Exact("cs"), interne("façade vers l'outillage .NET : API d'administration, non affichée")),
    r!("niers-backends", Niers, Motif::Exact("backends"), interne("dit ce qui est construit sur CETTE machine : sans objet en ligne")),
    // Le reverse et la forge, en bloc : neuf commandes qui lisent ou écrivent `var/niers.sqlite`.
    r!("niers-reverse", Niers, Motif::Tout, interne("boucle de reverse-engineering : coûteuse, privilégiée, sans public (§ 5, lot 1)")),

    // ------------------------------------------------------------- Inacord (commandes IPC)
    r!("inacord-pet", Inacord, Motif::Prefixe("aphrody_pet_"), servi("/pet/aphrody.json")),
    r!("inacord-palette", Inacord, Motif::Exact("aphrody_pixel_mesurer"), servi("/api/v1/aphrody/palette")),
    r!("inacord-tokens", Inacord, Motif::Exact("aphrody_pixel_tokens_css"), servi("/api/v1/aphrody/palette")),
    r!("inacord-pixel", Inacord, Motif::Prefixe("aphrody_pixel_"), interne("outillage de direction artistique local (comparer, vectoriser, planche)")),
    // Le VFS : lecture servie par les deux espaces, écriture et export jamais.
    r!("inacord-vfs-ecriture", Inacord, Motif::Prefixe("vfs_write"), interne("écrit dans le VFS : un site en lecture seule n'écrit pas")),
    r!("inacord-vfs-export", Inacord, Motif::Prefixe("vfs_export"), interne("écrit un fichier sur la machine de l'utilisateur")),
    r!("inacord-vfs-extract", Inacord, Motif::Exact("vfs_extract_to"), interne("écrit un fichier sur la machine de l'utilisateur")),
    r!("inacord-vfs-index", Inacord, Motif::Prefixe("vfs_index_scan"), interne("pilote l'indexation locale de l'hôte : sans objet côté serveur")),
    r!("inacord-vfs-cache", Inacord, Motif::Prefixe("vfs_cache"), interne("cache de l'hôte : le site a le sien (moka), non pilotable de l'extérieur")),
    r!("inacord-vfs-texture", Inacord, Motif::Prefixe("vfs_texture"), servi("/assets/{*chemin}")),
    r!("inacord-vfs-glb", Inacord, Motif::Exact("vfs_glb_bytes_b64"), servi("/model/{famille}/{fichier}")),
    r!("inacord-vfs-motion", Inacord, Motif::Exact("vfs_motion_clips"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("inacord-vfs-audio", Inacord, Motif::Prefixe("vfs_audio"), servi("/assets/{*chemin}")),
    r!("inacord-vfs-video", Inacord, Motif::Exact("vfs_video_preview_b64"), servi("/assets/{*chemin}")),
    r!("inacord-vfs-cfgbin", Inacord, Motif::Prefixe("vfs_decode_cfgbin"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("inacord-vfs-camera", Inacord, Motif::Exact("vfs_apercu_camera"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("inacord-vfs-navmesh", Inacord, Motif::Exact("vfs_apercu_navmesh"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("inacord-vfs-recherche", Inacord, Motif::Exact("vfs_find"), servi("/api/v1/recherche")),
    r!("inacord-vfs-recherche-p", Inacord, Motif::Exact("vfs_find_paged"), servi("/api/v1/recherche")),
    r!("inacord-vfs-lecture", Inacord, Motif::Prefixe("vfs_"), servi("/b/{*prefixe}")),
    r!("inacord-preload", Inacord, Motif::Exact("preload_vfs"), interne("montage du VFS de l'hôte : le site le monte en fond au démarrage")),
    // Les catalogues de données : le miroir est là, les routes ne le sont pas.
    r!("inacord-charas", Inacord, Motif::Exact("game_data_charas"), servi("/api/v1/chara")),
    r!("inacord-chara-picker", Inacord, Motif::Exact("game_data_chara_picker"), servi("/api/v1/chara")),
    r!("inacord-movies", Inacord, Motif::Exact("game_data_movies"), servi("/api/v1/{vue}")),
    r!("inacord-musics", Inacord, Motif::Exact("game_data_musics"), servi("/api/v1/{vue}")),
    r!("inacord-gamedata", Inacord, Motif::Prefixe("game_data_"), servi("/api/v1/donnees/famille/{cle}")),
    // Lua : ce qui LIT est servi, ce qui EXÉCUTE ne le sera jamais.
    r!("inacord-lua-session", Inacord, Motif::Prefixe("lua_session"), interne("attache une VM Lua vivante : aucun interpréteur n'est lié dans le service (nie-lua `default-features = false`)")),
    r!("inacord-lua-execute", Inacord, Motif::Exact("lua_execute"), interne("exécute du Lua : refus structurel, cf. `routes::lua`")),
    r!("inacord-lua-eval", Inacord, Motif::Exact("lua_eval"), interne("évalue du Lua : refus structurel, cf. `routes::lua`")),
    r!("inacord-lua-globals", Inacord, Motif::Exact("lua_globals"), interne("énumérer les globales APPELLE le script (cf. `discover_host_calls`) : c'est de l'exécution")),
    r!("inacord-lua-scripts", Inacord, Motif::Exact("lua_list_scripts"), servi("/api/v1/lua/scripts")),
    r!("inacord-lua-chunk", Inacord, Motif::Exact("lua_chunk_info"), servi("/api/v1/lua/scripts/{*chemin}")),
    r!("inacord-lua-disasm", Inacord, Motif::Exact("lua_disassemble"), servi("/api/v1/lua/desassemblage/{*chemin}")),
    r!("inacord-lua", Inacord, Motif::Prefixe("lua_"), interne("touche à l'exécution de scripts : refus structurel")),
    // La 3D et l'amont.
    r!("inacord-model-avatar", Inacord, Motif::Prefixe("model_service_avatar"), manquant("nie_data::chara_edit + l'amont : le catalogue d'avatar n'a pas de route ici")),
    r!("inacord-model", Inacord, Motif::Prefixe("model_service_"), servi("/assets/{*chemin}")),
    r!("inacord-video", Inacord, Motif::Prefixe("video_"), servi("/api/v1/{vue}")),
    r!("inacord-remote", Inacord, Motif::Prefixe("remote_"), servi("/api/v1/chara")),
    // Tout ce qui touche la machine de l'utilisateur, le binaire du jeu ou un process.
    r!("inacord-forge", Inacord, Motif::Prefixe("forge::"), interne("la forge produit `nie.exe` : elle appartient à la machine de développement")),
    r!("inacord-re", Inacord, Motif::Prefixe("re_"), interne("reverse en direct : lit et écrit la mémoire d'un process")),
    r!("inacord-live", Inacord, Motif::Prefixe("live_mod::"), interne("modifie la mémoire du jeu en cours d'exécution")),
    r!("inacord-viola", Inacord, Motif::Prefixe("viola::"), interne("modding : dump, pack, crypto — écriture")),
    r!("inacord-mcp", Inacord, Motif::Prefixe("mcp::"), interne("installe et pilote un serveur MCP local")),
    r!("inacord-blender", Inacord, Motif::Prefixe("blender_"), interne("pilote Blender sur la machine de l'utilisateur")),
    r!("inacord-blender-addon", Inacord, Motif::Exact("install_niers_blender_addon"), interne("installe un greffon sur la machine de l'utilisateur")),
    r!("inacord-cpk", Inacord, Motif::Prefixe("raw_cpk_"), interne("ouvre un CPK arbitraire du disque : le site ne sert que le VFS monté")),
    r!("inacord-save", Inacord, Motif::Prefixe("save_"), interne("sauvegardes de joueur : données personnelles")),
    r!("inacord-disque", Inacord, Motif::Prefixe("disk_"), interne("accès au disque de l'utilisateur")),
    r!("inacord-clipboard", Inacord, Motif::Prefixe("clipboard_"), interne("presse-papiers du système")),
    r!("inacord-defaut", Inacord, Motif::Prefixe("default_"), interne("chemins par défaut de la machine hôte : sans objet en ligne")),
    r!("inacord-open", Inacord, Motif::Prefixe("open_"), interne("ouvre une application locale")),
    r!("inacord-hote", Inacord, Motif::Tout, interne("relève de l'hôte desktop : disque, fenêtre, presse-papiers, corbeille (cf. `capacites()` du contrat asset-source)")),

    // ------------------------------------------------------------------ Azalée (pages)
    r!("azalee-outils-updater", Azalee, Motif::Exact("/tools/niers"), interne("point de mise à jour d'Inacord : il doit rester à l'URL que les 0.5.x interrogent")),
    r!("azalee-outils", Azalee, Motif::Prefixe("/tools"), manquant("les données sont dans le miroir ; ces pages sont redirigées vers Aphrody (les dix 308) sans y avoir d'équivalent")),
    r!("azalee-dashboard", Azalee, Motif::Prefixe("/dashboard"), interne("administration du wiki : authentifiée, produit Rose Griffon")),
    r!("azalee-auth", Azalee, Motif::Prefixe("/auth"), interne("comptes utilisateurs : hors périmètre d'Aphrody")),
    r!("azalee-legal", Azalee, Motif::Prefixe("/legal"), interne("mentions légales de Rose Griffon : propres à Azalée")),
    r!("azalee-news", Azalee, Motif::Prefixe("/news"), interne("éditorial du wiki : Azalée demeure le wiki de référence")),
    r!("azalee-patch", Azalee, Motif::Prefixe("/patch-notes"), interne("éditorial du wiki : Azalée demeure le wiki de référence")),
    r!("azalee-profil", Azalee, Motif::Prefixe("/profil"), interne("profils d'utilisateurs : données personnelles, jamais migrées")),
    r!("azalee-compte", Azalee, Motif::Prefixe("/settings"), interne("réglages de compte : hors périmètre d'Aphrody")),
    r!("azalee-login", Azalee, Motif::Exact("/login"), interne("authentification du wiki")),
    r!("azalee-2fa", Azalee, Motif::Exact("/2fa"), interne("authentification du wiki")),
    r!("azalee-maintenance", Azalee, Motif::Exact("/maintenance"), interne("page de service du wiki")),
    r!("azalee-charte", Azalee, Motif::Exact("/charte"), interne("éditorial Rose Griffon")),
    r!("azalee-contact", Azalee, Motif::Exact("/contact"), interne("éditorial Rose Griffon")),
    r!("azalee-soutenir", Azalee, Motif::Exact("/soutenir"), interne("éditorial Rose Griffon")),
    r!("azalee-accueil", Azalee, Motif::Exact("/"), interne("accueil du wiki : Aphrody a le sien, dans la DA du jeu")),
    r!("azalee-chara", Azalee, Motif::Prefixe("/chara"), servi("/api/v1/chara")),
    // Les fiches encyclopédiques restent sur Azalée — mais leurs DONNÉES doivent être
    // atteignables depuis Aphrody, et elles ne le sont pas. C'est `manquant`, pas `interne` :
    // « reste sur Azalée » justifie la page, jamais l'absence de la donnée.
    // Les fiches encyclopediques restent sur Azalee — c'est le lot 6 du plan, et la separation
    // de marque tient. Ce qui NE devait pas rester ailleurs, c'est la donnee : elle est servie
    // depuis le 2026-09-06 par /api/v1/donnees/famille/{cle}, sur les 121 familles nommees que
    // le VFS porte reellement. « Reste sur Azalee » justifie la page, jamais l'absence de la
    // donnee — et c'est pour cela que cette regle n'a pu devenir `interne` qu'apres le cablage.
    r!("azalee-catalogues", Azalee, Motif::Tout, interne("fiche encyclopedique : Azalee demeure le wiki de reference (lot 6) ; la donnee du jeu, elle, est servie par /api/v1/donnees/famille/{cle}")),

    // -------------------------------------------------------------- Azalée (routes d'API)
    r!("azalee-api-updater", AzaleeApi, Motif::Exact("/tools/niers/latest.json"), interne("point de mise à jour d'Inacord : les 0.5.x déjà installés interrogent CETTE URL, elle ne bouge pas")),
    r!("azalee-api-health", AzaleeApi, Motif::Exact("/api/health"), servi("/api/v1/health")),
    r!("azalee-api-save", AzaleeApi, Motif::Prefixe("/api/save"), manquant("nie-save — la résolution d'effectif depuis une sauvegarde n'a pas de route ici")),
    r!("azalee-api-vroid", AzaleeApi, Motif::Prefixe("/api/vroid"), interne("OAuth VRoid : session utilisateur et secrets tiers")),
    r!("azalee-api-auth", AzaleeApi, Motif::Prefixe("/api/auth"), interne("authentification du wiki")),
    r!("azalee-api-admin", AzaleeApi, Motif::Prefixe("/api/admin"), interne("administration du wiki")),
    r!("azalee-api-cron", AzaleeApi, Motif::Prefixe("/api/cron"), interne("tâches planifiées du wiki")),
    r!("azalee-api-jeton", AzaleeApi, Motif::Exact("/api/supabase-token"), interne("émet un jeton : un secret ne traverse jamais Aphrody")),
    r!("azalee-api-llm", AzaleeApi, Motif::Prefixe("/api/llm"), interne("passerelle vers un service tiers facturé")),
    r!("azalee-api-rag", AzaleeApi, Motif::Prefixe("/api/rag"), interne("index vectoriel du wiki : propre à Azalée")),
    r!("azalee-api-editorial", AzaleeApi, Motif::Tout, interne("éditorial et métadonnées du wiki : Azalée demeure le wiki de référence")),

    // ------------------------------------------------------------------- nie-data (modules)
    r!("data-cfgbin", NieData, Motif::Exact("cfgbin"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("data-typed", NieData, Motif::Exact("typed"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("data-chara-base", NieData, Motif::Exact("chara_base"), servi("/api/v1/chara")),
    r!("data-chara-text", NieData, Motif::Exact("chara_text"), servi("/api/v1/chara")),
    r!("data-hash", NieData, Motif::Exact("hash"), interne("table de hachage interne du décodage : ce n'est pas une donnée du jeu")),
    r!("data-help", NieData, Motif::Exact("help"), interne("textes d'aide du décodeur : outillage")),
    // Les 110 familles restantes : le parseur est écrit, testé par golden, et **rien** ne
    // l'expose. C'est la mesure qui a motivé le plan : le dépôt sait faire dix fois ce qu'il
    // montre.
    r!("data-typees", NieData, Motif::Parmi(MODULES_TYPES), servi("/api/v1/donnees/{*chemin}")),
    // Deux modules que la facade ne peut PAS porter telle quelle, et il faut le dire :
    r!("data-passives", NieData, Motif::Exact("passives"), manquant("`parse_player_passives(root, text_fr, text_en)` prend DEUX tables de texte en plus du conteneur : la facade `decode_by_key(cle, root)` ne les a pas")),
    r!("data-team", NieData, Motif::Exact("team"), manquant("`team::parse_enjoy_mode_team_config` fait doublon avec `enjoy_mode_team`, deja servi — c'est une fusion a faire, pas une route")),
    r!("data-familles", NieData, Motif::Tout, manquant("crates/engine/nie-data/src/<module>.rs — parseur typé, golden testé, sans route")),

    // ---------------------------------------------------------------- nie-formats (modules)
    r!("formats-vfs", NieFormats, Motif::Exact("vfs"), servi("/f/{*chemin}")),
    r!("formats-cpk", NieFormats, Motif::Exact("cpk"), servi("/b/{*prefixe}")),
    r!("formats-crilayla", NieFormats, Motif::Exact("crilayla"), servi("/f/{*chemin}")),
    r!("formats-cfgbin", NieFormats, Motif::Exact("cfgbin"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-level5", NieFormats, Motif::Exact("level5"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-decode", NieFormats, Motif::Exact("decode"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-encodeurs", NieFormats, Motif::Suffixe("_encode"), interne("encodeur : écriture, et `encode_t2b` n'est pas encore fidèle (cf. CLAUDE.md § modding)")),
    r!("formats-patch", NieFormats, Motif::Suffixe("_patch"), interne("patch d'octets en place : écriture, modding")),
    r!("formats-g4tx", NieFormats, Motif::Prefixe("g4tx"), servi("/assets/{*chemin}")),
    r!("formats-dxbc", NieFormats, Motif::Exact("dxbc"), servi("/assets/{*chemin}")),
    r!("formats-audio", NieFormats, Motif::Exact("cri_audio"), servi("/assets/{*chemin}")),
    r!("formats-usm", NieFormats, Motif::Exact("usm"), servi("/api/v1/{vue}")),
    r!("formats-mp4", NieFormats, Motif::Exact("mp4"), servi("/api/v1/{vue}")),
    r!("formats-webm", NieFormats, Motif::Exact("webm"), servi("/api/v1/{vue}")),
    r!("formats-assemble", NieFormats, Motif::Exact("assemble"), servi("/model/{famille}/{fichier}")),
    r!("formats-geometrie", NieFormats, Motif::Prefixe("g4p"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-g4md", NieFormats, Motif::Exact("g4md"), servi("/api/v1/3d")),
    r!("formats-g4mg", NieFormats, Motif::Exact("g4mg"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-g4cm", NieFormats, Motif::Exact("g4cm"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-g4sk", NieFormats, Motif::Exact("g4sk"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-g4mt", NieFormats, Motif::Exact("g4mt"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-mevbin", NieFormats, Motif::Exact("mevbin"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-objbin", NieFormats, Motif::Exact("objbin"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-col", NieFormats, Motif::Exact("col"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-pathname", NieFormats, Motif::Exact("pathname"), interne("portage byte-exact d'une fonction du binaire : brique, pas capacité")),
    r!("formats-libc", NieFormats, Motif::Prefixe("str"), interne("portages byte-exact des fonctions chaîne du binaire : briques du RE")),
    r!("formats-lip", NieFormats, Motif::Exact("lip"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-navm", NieFormats, Motif::Exact("navm"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-g4la", NieFormats, Motif::Exact("g4la"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-g4vs", NieFormats, Motif::Exact("g4vs"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-g4ma", NieFormats, Motif::Exact("g4ma"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("formats-font", NieFormats, Motif::Exact("font"), manquant("crates/engine/nie-formats/src/font.rs — les 9 fontes sont cataloguées, aucune route ne les rend")),
    r!("formats-menu", NieFormats, Motif::Exact("menu"), manquant("crates/engine/nie-formats/src/menu.rs — dispositions d'écran, sans route")),
    r!("formats-restants", NieFormats, Motif::Tout, manquant("crates/engine/nie-formats/src/<module>.rs — parseur écrit, aucune route ne l'appelle")),

    // -------------------------------------------------------------------- nie-lua (pub fn)
    r!("lua-bytecode", NieLua, Motif::Exact("parse"), servi("/api/v1/lua/scripts/{*chemin}")),
    r!("lua-disasm", NieLua, Motif::Exact("disassemble"), servi("/api/v1/lua/desassemblage/{*chemin}")),
    r!("lua-decode", NieLua, Motif::Exact("decode_instruction"), servi("/api/v1/lua/desassemblage/{*chemin}")),
    r!("lua-detection", NieLua, Motif::Exact("is_lua52_bytecode"), servi("/api/v1/lua/scripts/{*chemin}")),
    r!("lua-validation", NieLua, Motif::Exact("validate_bytecode"), servi("/api/v1/lua/scripts/{*chemin}")),
    r!("lua-liste", NieLua, Motif::Exact("collect_lua_files"), servi("/api/v1/lua/scripts")),
    r!("lua-crc32", NieLua, Motif::Exact("crc32"), interne("brique de calcul : pas une capacité du site")),
    // L'analyse statique de `nie-lua` porte sur du Lua SOURCE. Mesure du 2026-09-06 sur les
    // 255 308 entrees du VFS : **0 fichier `.lua`**, 1 197 `.lua.bin`. Le corpus est vide, et
    // la feature `analysis` (tree-sitter) n'est meme pas liee ici. Ce n'est pas un cablage
    // qu'on remet a plus tard : c'est une capacite sans objet sur ce jeu.
    r!("lua-analyse", NieLua, Motif::Prefixe("analyze"), interne("analyse de Lua SOURCE : le jeu n'en contient aucun (0 `.lua` pour 1 197 `.lua.bin` sur 255 308 entrees)")),
    r!("lua-chaines", NieLua, Motif::Exact("is_interesting_string"), interne("predicat de l'analyse statique de Lua source : meme corpus vide")),
    r!("lua-chemins", NieLua, Motif::Prefixe("script_logical_base"), interne("résolution de chemins de l'hôte d'exécution")),
    r!("lua-exec", NieLua, Motif::Tout, interne("VM, hôte de menu, capture de `print`, découverte d'appels : TOUT ceci exécute le script — `nie-site` lie `nie-lua` avec `default-features = false` et un test le vérifie")),

    // ------------------------------------------------------------------ iecode (C++)
    r!("iecode-admin", Iecode, Motif::Tout, interne("toolkit C++ atteint par `niers cpp` : API d'administration, non affichée (§ 5, lot 7)")),

    // ------------------------------------------------------------------ VFS (par extension)
    r!("vfs-cfgbin", Vfs, Motif::Exact(".cfg.bin"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-luabin", Vfs, Motif::Exact(".lua.bin"), servi("/api/v1/lua/scripts/{*chemin}")),
    r!("vfs-g4tx", Vfs, Motif::Exact(".g4tx"), servi("/assets/{*chemin}")),
    r!("vfs-g4md", Vfs, Motif::Exact(".g4md"), servi("/api/v1/3d")),
    r!("vfs-acb", Vfs, Motif::Exact(".acb"), servi("/assets/{*chemin}")),
    r!("vfs-awb", Vfs, Motif::Exact(".awb"), interne("banque de 7,49 Gio catalogée par son `.acb` (0,10 Gio) : l'exposer fichier par fichier dupliquerait le catalogue et servirait des pistes sans nom")),
    r!("vfs-usm", Vfs, Motif::Exact(".usm"), servi("/api/v1/{vue}")),
    r!("vfs-g4pk", Vfs, Motif::Exact(".g4pk"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-g4mg", Vfs, Motif::Exact(".g4mg"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-objbin", Vfs, Motif::Exact(".objbin"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-g4pkm", Vfs, Motif::Exact(".g4pkm"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-g4cm", Vfs, Motif::Exact(".g4cm"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-col", Vfs, Motif::Exact(".col"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-g4sk", Vfs, Motif::Exact(".g4sk"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-mevbin", Vfs, Motif::Exact(".mevbin"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-g4mt", Vfs, Motif::Exact(".g4mt"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-texte", Vfs, Motif::Exact(".log"), servi("/f/{*chemin}")),
    r!("vfs-texte-cfg", Vfs, Motif::Exact(".cfg"), servi("/f/{*chemin}")),
    r!("vfs-acf", Vfs, Motif::Exact(".acf"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-sans-extension", Vfs, Motif::Exact("(sans extension)"), servi("/f/{*chemin}")),
    // Les suffixes de révision (`.r41152`, `.r65902`…) : le nom est unique au fichier près,
    // le contenu ne l'est pas — G4PK au magic, T2B en le lisant. Mesuré le 2026-09-06.
    r!("vfs-revision", Vfs, Motif::Prefixe(".r"), servi("/api/v1/formats/decode/{*chemin}")),
    // Ce que le dépôt sait décoder et que le site ne rend pas — la définition même de
    // `manquant`. Les `.p3lip` en sont l'essentiel : 21 047 fichiers, un parseur écrit.
    // Cablees le 2026-09-06 apres que la matrice les eut fait apparaitre : 21 250 fichiers,
    // 124/124 decodes par `scripts/validation/mesurer-level5.sh`. Cf. `routes::level5`.
    r!("vfs-p3lip", Vfs, Motif::Exact(".p3lip"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-g4nv", Vfs, Motif::Exact(".g4nv"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-g4ma", Vfs, Motif::Exact(".g4ma"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-g4vs", Vfs, Motif::Exact(".g4vs"), servi("/api/v1/formats/decode/{*chemin}")),
    r!("vfs-g4la", Vfs, Motif::Exact(".g4la"), servi("/api/v1/formats/decode/{*chemin}")),
    // Ce qu'aucun parseur du dépôt ne connaît : du reverse d'abord.
    r!("vfs-g4tg", Vfs, Motif::Exact(".g4tg"), bloque("non identifié : sans magic, motif `7f 7f ff ff`, 9 fichiers sous `dx11/` — une hypothèse de format n'est pas une identification")),
    r!("vfs-bin-inconnu", Vfs, Motif::Exact(".bin"), bloque("10 fichiers `.bin` hors `.cfg.bin` et `.lua.bin`, non identifiés")),
    r!("vfs-effets", Vfs, Motif::Tout, bloque("shaders, effets, particules, tissu : aucun parseur dans le dépôt, du reverse est nécessaire avant toute route")),
];
