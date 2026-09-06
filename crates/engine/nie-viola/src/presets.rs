//! Presets de sélection du dump — une **donnée**, pas seulement du code.
//!
//! Ces listes ne se déduisent d'aucune règle : elles disent quelles catégories du jeu sont
//! réellement lues en aval, et lesquelles ne le sont jamais. Les retrouver demanderait de
//! ré-auditer les consommateurs ; elles sont donc portées telles quelles, commentaires compris.
//!
//! ⚠️ Les chemins du `cpk_list` sont **préfixés `data/`** et le matching est ancré : tout glob
//! doit inclure ce préfixe, sinon il matche zéro fichier. C'est le piège n°1 de ce fichier.
//!
//! La syntaxe est celle de [`crate::filtre::Filtre`] (listes séparées par des virgules, `**`,
//! préfixe `!` pour exclure).

/// Catégories `common/gamedata/*` réellement chargées en aval (Game Data API).
///
/// Exclut volontairement `event`, `movie`, `rpg_battle`, `friendmap`, `dungeon`, `mission`, `ai`,
/// `command`, `motion`, `weather`, `staffroll` — jamais lues, et ce sont les plus volumineuses.
const CATEGORIES_GAMEDATA: [&str; 21] = [
    "boost_grp",
    "capsule",
    "character",
    "chat_emote",
    "dictionary",
    "extend_story",
    "formation",
    "gallery",
    "inacode",
    "item",
    "nfc",
    "party",
    "phase",
    "players_universe",
    "quest",
    "scene_archive",
    "skill",
    "soccer",
    "team",
    "trophy",
    "user_name_plate",
];

/// Textes localisés (noms de personnages, descriptions…).
const TEXTE: &str = "data/common/text/**";

/// Assets graphiques.
///
/// `dx11/**` = textures rendues, dont `dx11/menu/**` : icônes de personnage
/// (`icon_chr/{face,uniform}`, auras `aura_{fs,soul,armed,mixi}`), emblèmes et icônes d'objets
/// (`200_icon/{01_icon_emblem,02_icon_item}`), et les bandeaux telop des techniques **et** des
/// auras (`220_img/telop_waza/{fr,en}`). `chr/**` = ressources de personnage brutes (coachs,
/// spirits) que `dx11` n'adresse pas ; conservé pour ne rien perdre.
const ASSETS: &str = "data/dx11/**,data/chr/**";

/// Noms de presets disponibles, dans l'ordre d'affichage de l'aide.
pub const NOMS: [&str; 3] = ["inagle", "azalee", "inagle-azalee"];

/// Globs des catégories gamedata, une entrée précise par catégorie.
fn gamedata() -> String {
    CATEGORIES_GAMEDATA
        .iter()
        .map(|c| format!("data/common/gamedata/{c}/**"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Résout un nom de preset en spécification de filtre. Insensible à la casse.
///
/// Rend `None` si le nom est inconnu — l'appelant doit alors le signaler plutôt que de dumper
/// le jeu entier par défaut.
#[must_use]
pub fn resoudre(nom: &str) -> Option<String> {
    let n = nom.trim().to_ascii_lowercase();
    let g = gamedata();
    match n.as_str() {
        "inagle" => Some(format!("{g},{TEXTE}")),
        "azalee" => Some(format!("{g},{ASSETS}")),
        "inagle-azalee" => Some(format!("{g},{TEXTE},{ASSETS}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filtre::Filtre;

    #[test]
    fn les_trois_presets_se_resolvent_sans_la_casse() {
        for n in NOMS {
            assert!(resoudre(n).is_some(), "{n}");
            assert!(resoudre(&n.to_uppercase()).is_some(), "{n} en majuscules");
        }
        assert!(resoudre("inconnu").is_none());
    }

    #[test]
    fn les_21_categories_sont_toutes_presentes() {
        let spec = resoudre("inagle").unwrap();
        for c in CATEGORIES_GAMEDATA {
            assert!(
                spec.contains(&format!("gamedata/{c}/**")),
                "catégorie absente : {c}"
            );
        }
        assert_eq!(CATEGORIES_GAMEDATA.len(), 21);
    }

    /// Le piège du fichier : un glob sans `data/` matcherait zéro fichier.
    #[test]
    fn tous_les_globs_sont_prefixes_data() {
        for n in NOMS {
            for glob in resoudre(n).unwrap().split(',') {
                let corps = glob.strip_prefix('!').unwrap_or(glob);
                assert!(
                    corps.starts_with("data/"),
                    "{n} : glob non préfixé — {glob}"
                );
            }
        }
    }

    /// Ce que les presets retiennent et écartent, sur de vrais chemins du VFS.
    #[test]
    fn les_presets_selectionnent_ce_qu_ils_annoncent() {
        let inagle = Filtre::parse(&resoudre("inagle").unwrap());
        assert!(inagle.accepte("data/common/gamedata/skill/skill_config_4.00.17.00.cfg.bin"));
        assert!(inagle.accepte("data/common/text/fr/chara_text.cfg.bin"));
        // Les catégories volumineuses jamais lues restent dehors.
        assert!(!inagle.accepte("data/common/gamedata/event/ev01/x.cfg.bin"));
        assert!(!inagle.accepte("data/common/gamedata/map/w10/y.cfg.bin"));
        // inagle ne prend pas les textures…
        assert!(!inagle.accepte("data/dx11/chr/_face/01_ie1/c01001900/c01001900.g4tx"));
        // …azalee si.
        let azalee = Filtre::parse(&resoudre("azalee").unwrap());
        assert!(azalee.accepte("data/dx11/chr/_face/01_ie1/c01001900/c01001900.g4tx"));
        assert!(azalee.accepte("data/chr/quelque/chose"));
        // Le cumul des deux.
        let tout = Filtre::parse(&resoudre("inagle-azalee").unwrap());
        assert!(tout.accepte("data/common/text/fr/chara_text.cfg.bin"));
        assert!(tout.accepte("data/dx11/menu/200_icon/02_icon_item/x.g4tx"));
    }
}
