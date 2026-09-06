//! Références de chaînes reconstruites par désassemblage, et nommage
//! **sémantique** de ce qu'elles désignent sans ambiguïté.
//!
//! C'est la seule source de sens qui reste dans un binaire sans PDB : les
//! chaînes que le code manipule. Une fonction qui charge l'adresse de
//! `"soccer_menu_root"` et qu'aucune autre ne charge parle d'elle-même —
//! contrairement à un nom structurel (`vtbl_…::slot_7`), qui identifie sans
//! rien dire.
//!
//! ## Méthode
//!
//! 1. Relever toutes les chaînes ASCII terminées par `NUL` de `.rdata`/`.data`,
//!    avec leur adresse virtuelle.
//! 2. Décoder le corps de chaque fonction dont les bornes sont connues et
//!    relever les opérandes mémoire **relatives à `rip`** (`lea`, `mov`) qui
//!    tombent exactement sur le début d'une chaîne.
//! 3. Enregistrer chaque couple (fonction, chaîne) dans `func_str_ref`.
//! 4. Nommer `fn_<slug>` toute fonction sans nom qui référence une chaîne
//!    *identifiante* dont elle est le **seul** référent.
//!
//! ## Ce qui est exigé d'une chaîne pour donner un nom
//!
//! - référencée par **une seule** fonction — sinon le nom ne discrimine pas ;
//! - être la **seule** chaîne discriminante de cette fonction — sinon le choix
//!   du nom serait arbitraire. Une fonction qui manipule un identifiant unique
//!   et trois messages d'erreur partagés reste, elle, parfaitement désignée ;
//! - de [`MIN_IDENT`] à [`MAX_IDENT`] octets, uniquement `[A-Za-z0-9_./-]`,
//!   commençant par une lettre : un identifiant, pas une phrase. Un message
//!   d'erreur (« The work buffer size is too small. ») dit à quoi sert le code,
//!   pas ce qu'il est, et ferait un nom trompeur.
//!
//! ## Honnêteté
//!
//! Ce nom reste une **désignation**, pas le symbole d'origine : il dit « la
//! fonction qui, seule, manipule cette chaîne ». Il porte la source
//! `name_source='strref'` pour rester distinguable, et n'écrase jamais un nom
//! existant.

use anyhow::{Context, Result};
use goblin::pe::PE;
use hashbrown::{HashMap, HashSet};
use iced_x86::{Decoder, DecoderOptions, Instruction, Register};
use nie_index::{Db, rusqlite};
use tracing::info;

/// Longueur minimale d'une chaîne pour être considérée comme identifiante.
const MIN_IDENT: usize = 4;
/// Longueur maximale d'une chaîne pour être considérée comme identifiante.
const MAX_IDENT: usize = 48;
/// Longueur minimale d'une chaîne relevée dans les sections de données.
const MIN_STR: usize = 4;
/// Longueur maximale décodée pour un corps de fonction.
const MAX_BODY: u64 = 64 * 1024;

/// Statistiques de la passe de références de chaînes.
#[derive(Debug, Clone, Copy, Default)]
pub struct StrRefStats {
    /// Chaînes relevées dans les sections de données.
    pub strings: usize,
    /// Fonctions dont le corps a été décodé.
    pub scanned: usize,
    /// Couples (fonction, chaîne) trouvés.
    pub refs: usize,
    /// Couples réellement insérés dans `func_str_ref`.
    pub refs_new: usize,
    /// Chaînes identifiantes référencées par exactement une fonction.
    pub unique_idents: usize,
    /// Fonctions nommées `fn_<slug>`.
    pub named: usize,
}

/// Vrai si `s` a la forme d'un identifiant utilisable comme nom.
fn is_ident(s: &str) -> bool {
    if s.len() < MIN_IDENT || s.len() > MAX_IDENT {
        return false;
    }
    let mut chars = s.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'))
}

/// Rend `s` utilisable comme identifiant de fonction.
fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Reconstruit les références de chaînes par désassemblage et nomme les
/// fonctions que ces chaînes désignent sans ambiguïté.
///
/// # Errors
///
/// Échoue si le PE est illisible, si `.text` manque, ou sur toute erreur
/// SQLite.
#[allow(clippy::too_many_lines)]
pub fn ingest_string_refs(
    db: &mut Db,
    bin: i64,
    exe_path: &std::path::Path,
) -> Result<StrRefStats> {
    let bytes =
        std::fs::read(exe_path).with_context(|| format!("lecture {}", exe_path.display()))?;
    let pe = PE::parse(&bytes).context("goblin: parse PE")?;
    let image_base = pe.image_base;

    let text = pe
        .sections
        .iter()
        .find(|s| s.name().is_ok_and(|n| n.starts_with(".text")))
        .context(".text introuvable")?;
    let text_va = image_base + u64::from(text.virtual_address);
    let text_off = text.pointer_to_raw_data as usize;
    let text_len = text.virtual_size.min(text.size_of_raw_data) as usize;
    let text_end = text_va + text_len as u64;

    let mut stats = StrRefStats::default();

    // 1. Chaînes des sections de données, indexées par adresse de début.
    let mut strings: HashMap<u64, String> = HashMap::new();
    for name in [".rdata", ".data", ".rodata"] {
        let Some(sec) = pe
            .sections
            .iter()
            .find(|s| s.name().is_ok_and(|n| n.starts_with(name)))
        else {
            continue;
        };
        let base = image_base + u64::from(sec.virtual_address);
        let off = sec.pointer_to_raw_data as usize;
        let len = sec.virtual_size.min(sec.size_of_raw_data) as usize;
        let Some(raw) = bytes.get(off..off + len) else {
            continue;
        };
        let mut start: Option<usize> = None;
        for (i, &b) in raw.iter().enumerate() {
            if (0x20..0x7f).contains(&b) {
                start.get_or_insert(i);
            } else {
                // Seul un `NUL` clôt une chaîne : un octet non imprimable
                // quelconque interrompt la suite sans la valider.
                if let Some((s, v)) = start
                    .filter(|&s| b == 0 && i - s >= MIN_STR)
                    .and_then(|s| std::str::from_utf8(&raw[s..i]).ok().map(|v| (s, v)))
                {
                    strings.insert(base + s as u64, v.to_string());
                }
                start = None;
            }
        }
    }
    stats.strings = strings.len();

    // 2. Corps de fonction connus, bornés par le début suivant.
    let mut funcs: Vec<(u64, u64)> = {
        let mut q = db
            .conn()
            .prepare("SELECT vaddr, size FROM function WHERE binary_id=?1 ORDER BY vaddr")?;
        q.query_map([bin], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64))
        })?
        .collect::<std::result::Result<_, _>>()?
    };
    funcs.retain(|&(a, _)| (text_va..text_end).contains(&a));

    // (fonction → chaînes) et (chaîne → fonctions), pour trancher l'unicité.
    let mut by_func: HashMap<u64, HashSet<u64>> = HashMap::new();
    let mut by_str: HashMap<u64, HashSet<u64>> = HashMap::new();

    let mut insn = Instruction::default();
    for i in 0..funcs.len() {
        let (a, size) = funcs[i];
        let next = funcs.get(i + 1).map_or(text_end, |&(b, _)| b);
        let end = if size > 0 {
            a + size
        } else {
            next.min(a + MAX_BODY)
        };
        let end = end.min(text_end).min(a + MAX_BODY);
        if end <= a {
            continue;
        }
        let off = text_off + (a - text_va) as usize;
        let Some(buf) = bytes.get(off..off + (end - a) as usize) else {
            continue;
        };
        stats.scanned += 1;
        let mut dec = Decoder::with_ip(64, buf, a, DecoderOptions::NONE);
        while dec.can_decode() {
            dec.decode_out(&mut insn);
            if insn.is_invalid() {
                break;
            }
            if insn.memory_base() != Register::RIP {
                continue;
            }
            let target = insn.ip_rel_memory_address();
            if strings.contains_key(&target) {
                by_func.entry(a).or_default().insert(target);
                by_str.entry(target).or_default().insert(a);
                stats.refs += 1;
            }
        }
    }

    // 3+4. Écriture des références, puis nommage des désignations sans ambiguïté.
    let tx = db.conn_mut().transaction()?;
    {
        let mut ins = tx.prepare_cached(
            "INSERT INTO func_str_ref(binary_id, function_id, value, source)
             SELECT ?1, id, ?3, 'strref' FROM function WHERE binary_id=?1 AND vaddr=?2
               AND NOT EXISTS (SELECT 1 FROM func_str_ref r
                               WHERE r.function_id = function.id AND r.value = ?3)",
        )?;
        for (&f, ss) in &by_func {
            for &s in ss {
                let Some(v) = strings.get(&s) else { continue };
                stats.refs_new += ins.execute(rusqlite::params![bin, f as i64, v])?;
            }
        }
    }
    {
        let mut upd = tx.prepare_cached(
            "UPDATE function SET name=?1, name_source='strref'
             WHERE binary_id=?2 AND vaddr=?3 AND name IS NULL",
        )?;
        for (&s, fs) in &by_str {
            if fs.len() != 1 {
                continue;
            }
            let Some(v) = strings.get(&s) else { continue };
            if !is_ident(v) {
                continue;
            }
            stats.unique_idents += 1;
            let f = *fs.iter().next().expect("un seul référent");
            // Le nom ne doit pas être un choix arbitraire : parmi les chaînes
            // que cette fonction référence, une seule doit être identifiante
            // *et* n'avoir qu'elle pour référent. Exiger qu'elle n'en
            // référence qu'une seule au total serait plus strict que
            // nécessaire — une fonction qui manipule un identifiant unique et
            // trois messages d'erreur partagés est parfaitement désignée par
            // son identifiant.
            let discriminants = by_func.get(&f).map_or(0, |ss| {
                ss.iter()
                    .filter(|c| {
                        strings.get(*c).is_some_and(|t| is_ident(t))
                            && by_str.get(*c).is_some_and(|r| r.len() == 1)
                    })
                    .count()
            });
            if discriminants != 1 {
                continue;
            }
            stats.named +=
                upd.execute(rusqlite::params![format!("fn_{}", slug(v)), bin, f as i64])?;
        }
    }
    tx.commit()?;

    info!(
        strings = stats.strings,
        refs = stats.refs,
        named = stats.named,
        "strref: références de chaînes reconstruites"
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_identifiant_est_accepte() {
        assert!(is_ident("soccer_menu_root"));
        assert!(is_ident("data/dx11/menu.g4tx"));
        assert!(is_ident("AbilityLearningBoardMenu"));
    }

    #[test]
    fn une_phrase_n_est_pas_un_identifiant() {
        // Un message d'erreur dit à quoi sert le code, pas ce qu'il est.
        assert!(!is_ident("The work buffer size is too small."));
        assert!(!is_ident("CPK Analyzer::CRC Error in %s"));
    }

    #[test]
    fn les_bornes_de_longueur_sont_appliquees() {
        assert!(!is_ident("abc"), "trop court");
        assert!(is_ident("abcd"));
        assert!(!is_ident(&"a".repeat(MAX_IDENT + 1)), "trop long");
        assert!(is_ident(&"a".repeat(MAX_IDENT)));
    }

    #[test]
    fn un_identifiant_commence_par_une_lettre() {
        assert!(!is_ident("_private"));
        assert!(!is_ident("0start"));
        assert!(!is_ident("/chemin/absolu"));
    }

    #[test]
    fn le_slug_neutralise_les_separateurs() {
        assert_eq!(slug("data/dx11/menu.g4tx"), "data_dx11_menu_g4tx");
        assert_eq!(slug("deja_propre"), "deja_propre");
    }
}
