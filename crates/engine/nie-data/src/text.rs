//! `text` — nettoyage des chaînes de texte du jeu (port de `sanitizeText`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! Port **1:1** de `packages/inagle/src/core/data-loader.ts` (`sanitizeText`, l.521-553). La
//! fonction TS est une chaîne de `String.replace(regex)` ; on la réplique par une **suite de
//! passes** déterministes (sans dépendance regex, `no_std`). Chaque passe correspond à un
//! `.replace()` dans l'ordre exact :
//!
//! 1. furigana `[Kanji/Reading]` → `Kanji` (`\[([^/]+)\/[^\]]+\]`) ;
//! 2. balises `<X:val>` (sauf `COL:`) → `val` (`<(?!COL:)[^:>]+:([^>]+)>`) ;
//! 3. balises restantes `<…>` → `` (`<[^>]+>`) ;
//! 4. échappements `\n`→LF, `\r`→``, `\t`→TAB, `\"`→`"`, `\'`→`'` ;
//! 5. suppression des caractères de contrôle sauf LF/TAB
//!    (`[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]`) ;
//! 6. espaces multiples → un seul (`[ ]+`) ;
//! 7. ≥3 sauts de ligne → 2 (`\n{3,}`) ;
//! 8. `trim` de chaque ligne, jointure `\n`, `trim` final.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

/// Passe 1 — furigana `[Kanji/Reading]` → `Kanji`.
///
/// `[^/]+` (kanji, ≥1, sans `/`) puis `/` puis `[^\]]+` (lecture, ≥1, sans `]`) puis `]`.
fn strip_furigana(chars: &[char]) -> Vec<char> {
    let n = chars.len();
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        if chars[i] == '[' {
            // premier `/` après `[` (le kanji ne contient pas de `/`).
            let mut sl = i + 1;
            while sl < n && chars[sl] != '/' {
                sl += 1;
            }
            if sl < n && sl > i + 1 {
                // premier `]` après le `/` (la lecture ne contient pas de `]`).
                let mut close = sl + 1;
                while close < n && chars[close] != ']' {
                    close += 1;
                }
                if close < n && close > sl + 1 {
                    out.extend_from_slice(&chars[i + 1..sl]); // kanji
                    i = close + 1;
                    continue;
                }
            }
            out.push('[');
            i += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Vrai si `chars[start..]` débute par `COL:` (lookahead négatif de la passe 2).
fn is_col_prefix(chars: &[char], start: usize) -> bool {
    matches!(
        (
            chars.get(start),
            chars.get(start + 1),
            chars.get(start + 2),
            chars.get(start + 3)
        ),
        (Some('C'), Some('O'), Some('L'), Some(':'))
    )
}

/// Passe 2 — `<X:val>` (X ≠ `COL`) → `val`.
fn extract_tag_values(chars: &[char]) -> Vec<char> {
    let n = chars.len();
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        if chars[i] == '<' && !is_col_prefix(chars, i + 1) {
            // nom = `[^:>]+` (≥1), puis `:`, puis valeur = `[^>]+` (≥1), puis `>`.
            let start = i + 1;
            let mut j = start;
            while j < n && chars[j] != ':' && chars[j] != '>' {
                j += 1;
            }
            if j < n && chars[j] == ':' && j > start {
                let mut k = j + 1;
                while k < n && chars[k] != '>' {
                    k += 1;
                }
                if k < n && k > j + 1 {
                    out.extend_from_slice(&chars[j + 1..k]); // valeur
                    i = k + 1;
                    continue;
                }
            }
            out.push('<');
            i += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Passe 3 — balises restantes `<…>` (≥1 char) → supprimées.
fn strip_remaining_tags(chars: &[char]) -> Vec<char> {
    let n = chars.len();
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        if chars[i] == '<' {
            let mut j = i + 1;
            while j < n && chars[j] != '>' {
                j += 1;
            }
            if j < n && j > i + 1 {
                i = j + 1; // supprime `<…>`
                continue;
            }
            out.push('<');
            i += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Passe 4 — échappements `\n`/`\r`/`\t`/`\"`/`\'`.
fn apply_escapes(chars: &[char]) -> Vec<char> {
    let n = chars.len();
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        if chars[i] == '\\' && i + 1 < n {
            match chars[i + 1] {
                'n' => {
                    out.push('\n');
                    i += 2;
                }
                'r' => i += 2, // supprimé
                't' => {
                    out.push('\t');
                    i += 2;
                }
                '"' => {
                    out.push('"');
                    i += 2;
                }
                '\'' => {
                    out.push('\'');
                    i += 2;
                }
                _ => {
                    out.push('\\');
                    i += 1;
                }
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Passe 5 — supprime les caractères de contrôle sauf LF (`0x0A`) et TAB (`0x09`).
fn strip_control(chars: &[char]) -> Vec<char> {
    chars
        .iter()
        .copied()
        .filter(|c| {
            let cp = *c as u32;
            !(cp <= 0x08 || cp == 0x0B || cp == 0x0C || (0x0E..=0x1F).contains(&cp) || cp == 0x7F)
        })
        .collect()
}

/// Passe 6 — espaces ASCII multiples → un seul.
fn collapse_spaces(chars: &[char]) -> Vec<char> {
    let mut out = Vec::with_capacity(chars.len());
    let mut prev_space = false;
    for &c in chars {
        if c == ' ' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out
}

/// Passe 7 — ≥3 sauts de ligne consécutifs → 2.
fn collapse_newlines(chars: &[char]) -> Vec<char> {
    let mut out = Vec::with_capacity(chars.len());
    let mut run = 0usize;
    for &c in chars {
        if c == '\n' {
            run += 1;
            if run <= 2 {
                out.push('\n');
            }
        } else {
            run = 0;
            out.push(c);
        }
    }
    out
}

/// Nettoie une chaîne de texte du jeu (port 1:1 de `sanitizeText`).
///
/// Retourne `""` pour une entrée vide.
#[must_use]
pub fn sanitize_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let chars = strip_furigana(&chars);
    let chars = extract_tag_values(&chars);
    let chars = strip_remaining_tags(&chars);
    let chars = apply_escapes(&chars);
    let chars = strip_control(&chars);
    let chars = collapse_spaces(&chars);
    let chars = collapse_newlines(&chars);

    // Passe 8 — trim par ligne, jointure `\n`, trim final.
    let joined: String = chars.into_iter().collect();
    let trimmed_lines: Vec<&str> = joined.split('\n').map(str::trim).collect();
    trimmed_lines.join("\n").trim().into()
}

/// Découpe un libellé du jeu en **texte affichable** et **glyphes spéciaux**.
///
/// Les libellés de menu portent deux familles de marqueurs entre crochets, que `sanitize_text` ne
/// touche pas (elle ne connaît que les balises `<…>` et le furigana `[kanji/lecture]`) :
///
/// - des marqueurs de **style** — `[CR]`, `[C]`, `[CDN]`, `[CFUNCBTN01]`, `[CTOKEN01]` — qui
///   ouvrent et ferment une couleur de police. Le jeu les consomme ; ils ne s'affichent pas ;
/// - des **gaiji** — `[$gaiji_icon_build01]`, `[$gaiji_voice01]` — qui désignent un glyphe image
///   de la police, c'est-à-dire une icône posée dans le fil du texte.
///
/// Le texte rendu est donc débarrassé des deux, et les noms de gaiji sont retournés à part, dans
/// l'ordre de leur apparition : l'appelant peut les rendre comme images là où le jeu le fait.
/// Un `[` isolé, ou un marqueur non fermé, est laissé tel quel plutôt que d'avaler la suite.
#[must_use]
pub fn split_markup(text: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut gaiji = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '[' {
            // Fin du marqueur ; sans `]`, ce n'en est pas un.
            if let Some(fin) = (i + 1..chars.len()).find(|&j| chars[j] == ']') {
                let corps: String = chars[i + 1..fin].iter().collect();
                let est_gaiji = corps.starts_with('$');
                // Marqueur de style : `C` seul, ou `C` suivi de lettres/chiffres sans espace.
                let est_style = corps.starts_with('C')
                    && corps.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if est_gaiji {
                    gaiji.push(corps[1..].to_string());
                    i = fin + 1;
                    continue;
                }
                if est_style {
                    i = fin + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    // Le retrait d'un marqueur peut laisser une espace en tête ou deux à la suite.
    let mut propre = String::with_capacity(out.len());
    let mut espace = false;
    for c in out.chars() {
        if c == ' ' {
            espace = true;
            continue;
        }
        if espace && !propre.is_empty() && !propre.ends_with('\n') {
            propre.push(' ');
        }
        espace = false;
        propre.push(c);
    }
    (propre.trim().to_string(), gaiji)
}

// ── Résolveur de texte universel (port de `text-parser.ts`) ───────────────────

/// Catalogue des fichiers de texte localisés (`common/text/<locale>/<file>.cfg.bin`).
///
/// Port **1:1** de `TEXT_FILE_NAMES` (`packages/inagle/src/parsers/text-parser.ts`) :
/// `(nom convivial, nom de fichier sans extension)`. Sert d'index anti-hallucination des
/// familles de texte résolubles par [`parse_text_file`].
pub const TEXT_FILES: &[(&str, &str)] = &[
    ("ai", "ai_text"),
    ("chara_add", "chara_add_info_text"),
    ("chara_description", "chara_description_text"),
    ("chara_roma", "chara_text_roma"),
    ("chara", "chara_text"),
    ("chat", "chat_text"),
    ("craft", "craft_text"),
    ("data_file", "data_file_text"),
    ("extend_story", "extend_story_text"),
    ("formation", "formation_text"),
    ("help", "help_list_text"),
    ("inacode", "inacode_text"),
    ("item", "item_text"),
    ("item_explain", "item_explain_text"),
    ("map_roma", "map_text_roma"),
    ("map", "map_text"),
    ("medal", "medal_text"),
    ("menu", "menu_text"),
    ("mission", "mission_text"),
    ("music", "music_name_text"),
    ("constellation", "players_universe_text"),
    ("post", "post_text"),
    ("quest_purpose", "quest_purpose_text"),
    ("quest_title", "quest_title_text"),
    ("battle_cmd", "rpg_battle_cmd_text"),
    ("battle_msg", "rpg_battle_message_text"),
    ("battle", "rpg_battle_text"),
    ("archive", "scene_archive_text"),
    ("scout", "scout_phase_text"),
    ("search", "search_word_text"),
    ("setting", "setting_text"),
    ("shop", "shop_text"),
    ("skill", "skill_text"),
    ("soccer_common", "soccer_common_text"),
    ("soccer_title", "soccer_game_title"),
    ("soccer_history", "soccer_history_check_text"),
    ("soccer_quick", "soccer_quick_action_text"),
    ("soccer_suggest", "soccer_suggest_text"),
    ("soccer_passive", "soccer_team_passive_text"),
    ("soccer_technic", "soccer_technic_text"),
    ("staffroll", "staffroll_text"),
    ("system", "system_text"),
    ("team", "team_text"),
    ("theater", "theater_text"),
    ("trophy", "trophy_text"),
];

/// Nom de fichier (sans extension) d'un type de texte convivial (port de `TEXT_FILE_NAMES[type]`).
#[must_use]
pub fn text_file_name(text_type: &str) -> Option<&'static str> {
    TEXT_FILES
        .iter()
        .find(|(k, _)| *k == text_type)
        .map(|(_, f)| *f)
}

/// Parse un noeud `TEXT_INFO`/`NOUN_INFO` en `(hashId, texte nettoyé)`.
///
/// Port de `parseTextInfoNode` : `[0]` = hashId (Int **obligatoire**) ; le texte est à l'index
/// **5** pour un `NOUN_INFO` (nom propre), sinon à l'index **2** (`TEXT_INFO`). Rejet si moins de
/// 3 variables, si `[0]`≠Int, si le texte n'est pas une `String`, ou s'il est vide après nettoyage.
#[must_use]
pub fn parse_text_info_node(node: &Node<'_>) -> Option<(HashId, String)> {
    if node.var_count() < 3 {
        return None;
    }
    let hash_var = node.var(0)?;
    if hash_var.ty != "Int" {
        return None;
    }
    let text_idx = if node.name().starts_with("NOUN_INFO") {
        5
    } else {
        2
    };
    let text_var = node.var(text_idx)?;
    if text_var.ty != "String" {
        return None;
    }
    let text = sanitize_text(text_var.value);
    if text.is_empty() {
        return None;
    }
    Some((hash_var.as_hash(), text))
}

/// Parse un fichier de texte (`*_text.cfg.bin.json` désérialisé) → liste `(hashId, texte)` dans
/// l'ordre du document (port de `loadTextFile`, qui construit une map `last-wins`).
///
/// Couvre les deux familles : `TEXT_INFO*` (texte à l'index 2) et `NOUN_INFO*` (index 5) ; les
/// noeuds de liste `*_BEGIN*` sont écartés par le filtre `BEGIN`.
///
/// Le préfixe est cherché **sans underscore final** : les dumps `*.cfg.bin.json` (iecode / inagle)
/// suffixent les noeuds d'un index (`TEXT_INFO_0`), mais un T2B lu directement depuis le VFS par
/// `nie_formats::cfgbin::cfgbin_parse` les nomme **`TEXT_INFO`** tout court. Exiger l'underscore
/// rendait cette fonction muette (0 texte) sur la donnée live, alors qu'elle marchait sur les dumps.
#[must_use]
pub fn parse_text_file(root: &Value) -> Vec<(HashId, String)> {
    let mut out = Vec::new();
    for prefix in ["TEXT_INFO", "NOUN_INFO"] {
        walk_named(root, prefix, |node| {
            if !node.name().contains("BEGIN")
                && let Some(e) = parse_text_info_node(&node)
            {
                out.push(e);
            }
        });
    }
    out
}

/// Résout un texte par hash dans une liste issue de [`parse_text_file`], sémantique **last-wins**
/// (= `loadTextFile().get`, où une clé répétée garde la dernière valeur).
#[must_use]
pub fn find_text(entries: &[(HashId, String)], hash_id: HashId) -> Option<&str> {
    entries
        .iter()
        .rev()
        .find(|(h, _)| *h == hash_id)
        .map(|(_, t)| t.as_str())
}

#[cfg(test)]
mod tests {
    use super::{
        TEXT_FILES, find_text, parse_text_file, parse_text_info_node, sanitize_text, text_file_name,
    };
    use crate::cfgbin::Node;
    use crate::hash::HashId;
    use alloc::string::ToString;
    use serde_json::json;

    #[test]
    fn catalog_lookup() {
        assert_eq!(text_file_name("skill"), Some("skill_text"));
        assert_eq!(text_file_name("chara"), Some("chara_text"));
        assert_eq!(
            text_file_name("constellation"),
            Some("players_universe_text")
        );
        assert_eq!(text_file_name("quest_title"), Some("quest_title_text"));
        assert_eq!(text_file_name("unknown_xyz"), None);
        assert_eq!(TEXT_FILES.len(), 45);
    }

    #[test]
    fn text_info_index_2() {
        // TEXT_INFO : texte à l'index 2.
        let n = json!({ "name": "TEXT_INFO_0",
            "variables": [json!({"type":"Int","value":"305419896"}), // 0x12345678
                          json!({"type":"Int","value":"0"}),
                          json!({"type":"String","value":"<COL:F>Bonjour<CLO>"})],
            "children": [] });
        let (h, t) = parse_text_info_node(&Node::new(&n)).expect("valide");
        assert_eq!(h, HashId(0x1234_5678));
        assert_eq!(t, "Bonjour");
    }

    #[test]
    fn noun_info_index_5() {
        // NOUN_INFO : texte à l'index 5.
        let v = json!({"type":"String","value":"x"});
        let n = json!({ "name": "NOUN_INFO_0",
            "variables": [json!({"type":"Int","value":"1"}), v, v, v, v,
                          json!({"type":"String","value":"Mark"})],
            "children": [] });
        let (h, t) = parse_text_info_node(&Node::new(&n)).expect("valide");
        assert_eq!(h, HashId(1));
        assert_eq!(t, "Mark");
    }

    #[test]
    fn parse_file_and_last_wins() {
        let ti = |name: &str, hash: i64, txt: &str| {
            json!({ "name": name,
                "variables": [json!({"type":"Int","value":hash.to_string()}),
                              json!({"type":"Int","value":"0"}),
                              json!({"type":"String","value":txt})],
                "children": [] })
        };
        let root = json!({ "entries": [{
            "name": "TEXT_INFO_BEGIN_0", "variables": [json!({"type":"Int","value":"3"})],
            "children": [ ti("TEXT_INFO_0", 0xAB, "premier"),
                          ti("TEXT_INFO_1", 0xCD, "autre"),
                          ti("TEXT_INFO_2", 0xAB, "dernier") ] // hash dupliqué
        }]});
        let entries = parse_text_file(&root);
        assert_eq!(entries.len(), 3);
        assert_eq!(find_text(&entries, HashId(0xAB)), Some("dernier")); // last-wins
        assert_eq!(find_text(&entries, HashId(0xCD)), Some("autre"));
        assert_eq!(find_text(&entries, HashId(0xFF)), None);
    }

    #[test]
    fn empty() {
        assert_eq!(sanitize_text(""), "");
    }

    #[test]
    fn furigana_kept_kanji() {
        // `[Kanji/Reading]` → `Kanji` (exemple documenté inagle).
        assert_eq!(sanitize_text("[漢字/かんじ]"), "漢字");
        assert_eq!(sanitize_text("a[円堂/えんどう]b"), "a円堂b");
        // Pas de `/` → pas une furigana, laissé tel quel.
        assert_eq!(sanitize_text("[plain]"), "[plain]");
    }

    #[test]
    fn value_tags_extracted_except_col() {
        // `<X:val>` (X≠COL) → val (exemples documentés : FLA/FUL/VAL).
        assert_eq!(sanitize_text("<FLA:SAKAMAKI>"), "SAKAMAKI");
        assert_eq!(sanitize_text("<FUL:DAISUKE>"), "DAISUKE");
        assert_eq!(sanitize_text("<VAL:10>"), "10");
        // COL n'est PAS extrait (passe 2 saute), puis la passe 3 le supprime entièrement.
        assert_eq!(sanitize_text("<COL:FFAA00>rouge<CLO>"), "rouge");
    }

    #[test]
    fn markup_split_strips_style_and_extracts_gaiji() {
        let (t, g) = super::split_markup("[CFUNCBTN01]Cacher cheveux[C]");
        assert_eq!(t, "Cacher cheveux");
        assert!(g.is_empty());

        let (t, g) = super::split_markup("[$gaiji_icon_build04] Tension");
        assert_eq!(t, "Tension");
        assert_eq!(g, alloc::vec!["gaiji_icon_build04"]);

        let (t, g) = super::split_markup("[CR]Certains maillots ne\ncorrespondent pas.[C]");
        assert_eq!(t, "Certains maillots ne\ncorrespondent pas.");
        assert!(g.is_empty());

        // Un crochet qui n'est pas un marqueur reste dans le texte.
        let (t, _) = super::split_markup("Niveau [5] requis");
        assert_eq!(t, "Niveau [5] requis");
    }

    #[test]
    fn bare_tags_stripped() {
        assert_eq!(sanitize_text("a<CLO>b<TX0>c"), "abc");
        // `<>` vide (0 char) n'est pas une balise → laissé.
        assert_eq!(sanitize_text("x<>y"), "x<>y");
    }

    #[test]
    fn escapes() {
        assert_eq!(sanitize_text("a\\nb"), "a\nb");
        assert_eq!(sanitize_text("a\\tb"), "a\tb");
        assert_eq!(sanitize_text("a\\rb"), "ab"); // \r supprimé
        assert_eq!(sanitize_text("say \\\"hi\\\""), "say \"hi\"");
    }

    #[test]
    fn collapse_spaces_and_trim() {
        assert_eq!(sanitize_text("a    b"), "a b");
        assert_eq!(sanitize_text("   trim me   "), "trim me");
        // trim par ligne.
        assert_eq!(sanitize_text("  l1  \\n  l2  "), "l1\nl2");
    }

    #[test]
    fn collapse_3plus_newlines() {
        // 3+ sauts → 2 (les `\n` littéraux via l'échappement).
        assert_eq!(sanitize_text("a\\n\\n\\n\\nb"), "a\n\nb");
    }

    #[test]
    fn control_chars_removed_except_lf_tab() {
        let raw = "a\u{0001}b\u{0007}c\u{007F}d".to_string();
        assert_eq!(sanitize_text(&raw), "abcd");
        // TAB conservé.
        assert_eq!(sanitize_text("a\tb"), "a\tb");
    }

    #[test]
    fn combined_real_shape() {
        // Mélange réaliste : balise valeur + couleur + furigana + espaces.
        let s = "<COL:FF0000>[必殺/ひっさつ] <VAL:5> coups<CLO>";
        assert_eq!(sanitize_text(s), "必殺 5 coups");
    }
}
