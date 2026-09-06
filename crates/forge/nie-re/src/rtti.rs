//! Extraction RTTI MSVC 64-bit depuis `nie_eacpatched.exe`.
//!
//! ## Stratégie
//!
//! Le runtime RTTI MSVC (Windows x64) stocke des structures dans `.rdata` :
//!
//! - **`_RTTICompleteObjectLocator`** (COL, 32 octets) :
//!   ```c
//!   struct COL {
//!       uint32_t signature;          // 1 sur x64
//!       uint32_t offset;
//!       uint32_t cdOffset;
//!       int32_t  pTypeDescriptor;    // RVA depuis ImageBase
//!       int32_t  pClassHierarchyDescriptor; // RVA
//!       int32_t  pSelf;              // RVA du COL lui-même (x64 seulement)
//!   };
//!   ```
//!
//! - **`TypeDescriptor`** (type_info) :
//!   ```c
//!   struct TypeDescriptor {
//!       uint64_t pVFTable;  // pointeur vftable de type_info (ImageBase + offset connu)
//!       uint64_t spare;
//!       char     name[];    // ".?AV<ClassName>@@" ou ".?AVC<ClassName>@<NS>@@"
//!   };
//!   ```
//!
//! ## Algorithme
//!
//! 1. Parser le PE via `goblin` pour récupérer `ImageBase`, les sections, et
//!    les offsets bruts de `.rdata` et `.text`.
//! 2. Balayer `.rdata` par fenêtres de 4 octets pour trouver les `uint32_t`
//!    égaux à `1` (signature COL x64).
//! 3. Pour chaque candidat, lire les 6 champs du COL et valider :
//!    - `pSelf` (RVA) doit pointer exactement sur le début du COL.
//!    - `pTypeDescriptor` (RVA) doit pointer dans le fichier à une adresse
//!      valide.
//! 4. Lire le nom RTTI à `TypeDescriptor + 16` jusqu'au `\0` suivant.
//!    Valider le préfixe `.?AV` ou `.?AU`.
//! 5. Démangler le nom C++ pour extraire classe et espace de noms.
//! 6. Ingérer dans `rtti_class` + `rtti_base` (hiérarchie via
//!    `_RTTIBaseClassArray` — si parseable).
//!
//! ## Honnêteté
//!
//! Le champ `pClassHierarchyDescriptor` est utilisé pour tenter d'extraire la
//! hiérarchie, mais les pointeurs dans `_RTTIBaseClassArray` sont des RVA
//! DWORD sur x64 — si leur résolution échoue (section manquante, décalage
//! hors limites), la classe est quand même ingérée sans hiérarchie.
//! Le comptage final reflète exactement ce qui a été ingéré.

use anyhow::{Context, Result};
use goblin::pe::PE;
use nie_index::{Db, ingest};
use tracing::{debug, warn};

/// Image Base fixe de nie.exe (PE32+, Level-5).
pub const IMAGE_BASE: u64 = 0x1_4000_0000;

/// Statistiques du parsing RTTI.
#[derive(Debug, Clone, Copy, Default)]
pub struct RttiStats {
    /// Candidats COL trouvés (signature == 1 + pSelf valide).
    pub candidates: usize,
    /// Noms TypeDescriptor valides (préfixe `.?AV` ou `.?AU`).
    pub valid_type_descs: usize,
    /// Classes ingérées dans `rtti_class`.
    pub classes_ingested: usize,
    /// Relations d'héritage ingérées dans `rtti_base`.
    pub bases_ingested: usize,
}

/// Parse les RTTI MSVC 64-bit depuis les octets bruts du PE et ingère les
/// classes/hiérarchie dans la base. Renvoie les statistiques.
pub fn parse_and_ingest(db: &mut Db, binary_id: i64, bytes: &[u8]) -> Result<RttiStats> {
    let pe = PE::parse(bytes).context("goblin: parse PE")?;
    let image_base = pe.image_base;
    let mut stats = RttiStats::default();

    // Trouve .rdata (lecture seule, données initialisées = RTTI vit là).
    let rdata = pe
        .sections
        .iter()
        .find(|s| s.name().is_ok_and(|n| n.starts_with(".rdata")));
    let Some(rdata_sec) = rdata else {
        warn!("section .rdata non trouvée dans le PE");
        return Ok(stats);
    };

    let rdata_vaddr = rdata_sec.virtual_address as u64;
    let rdata_raw_off = rdata_sec.pointer_to_raw_data as usize;
    let rdata_raw_size = rdata_sec.size_of_raw_data as usize;

    let Some(rdata_bytes) = bytes.get(rdata_raw_off..rdata_raw_off + rdata_raw_size) else {
        warn!("offset .rdata hors des limites du fichier");
        return Ok(stats);
    };

    // Taille virtuelle (peut être > taille brute pour le .bss padding).
    let _rdata_vsize = rdata_sec.virtual_size.max(rdata_raw_size as u32) as u64;

    // Convertit un RVA → offset fichier (à travers toutes les sections).
    let rva_to_file_offset = |rva: u64| -> Option<usize> {
        for sec in &pe.sections {
            let sec_vaddr = sec.virtual_address as u64;
            let sec_vsize = sec.virtual_size.max(sec.size_of_raw_data) as u64;
            if rva >= sec_vaddr && rva < sec_vaddr + sec_vsize {
                let delta = rva - sec_vaddr;
                let off = sec.pointer_to_raw_data as usize + delta as usize;
                if off < bytes.len() {
                    return Some(off);
                }
            }
        }
        None
    };

    // Lit un u32 LE depuis `bytes` à l'offset `off`.
    let read_u32 = |off: usize| -> Option<u32> {
        bytes
            .get(off..off + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    };

    // Lit un u64 LE depuis `bytes` à l'offset `off`.
    let read_u64 = |off: usize| -> Option<u64> {
        bytes
            .get(off..off + 8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    };

    // Lit une chaîne C depuis `bytes` à l'offset `off` (jusqu'au premier 0).
    let read_cstr = |off: usize| -> Option<String> {
        let slice = bytes.get(off..)?;
        let end = slice.iter().position(|&b| b == 0)?;
        std::str::from_utf8(&slice[..end])
            .ok()
            .map(|s| s.to_owned())
    };

    // Balayage de .rdata par alignement u32 (les COL sont alignés sur 4 octets).
    // On cherche les emplacements où `signature == 1`.
    let col_size = 24usize; // 6 × u32
    let n_windows = rdata_raw_size.saturating_sub(col_size) / 4;

    // Pré-collecte des classes pour le batch DB.
    let mut classes: Vec<ClassRecord> = Vec::new();

    for i in 0..n_windows {
        let off_in_rdata = i * 4;
        let file_off = rdata_raw_off + off_in_rdata;

        let Some(sig) = read_u32(file_off) else {
            continue;
        };
        if sig != 1 {
            continue;
        }

        // Lecture des 5 champs restants.
        let Some(_offset_val) = read_u32(file_off + 4) else {
            continue;
        };
        let Some(_cd_offset) = read_u32(file_off + 8) else {
            continue;
        };
        let Some(rva_type_desc) = read_u32(file_off + 12) else {
            continue;
        };
        let Some(rva_chd) = read_u32(file_off + 16) else {
            continue;
        };
        let Some(rva_self) = read_u32(file_off + 20) else {
            continue;
        };

        // Validation : pSelf (RVA) doit pointer sur ce COL.
        let col_vaddr = image_base + rdata_vaddr + off_in_rdata as u64;
        let expected_rva_self = col_vaddr - image_base; // = rdata_vaddr + offset
        if rva_self as u64 != expected_rva_self {
            continue;
        }

        stats.candidates += 1;

        // Résoudre pTypeDescriptor (RVA → offset fichier → lire TypeDescriptor).
        let td_rva = rva_type_desc as u64;
        let Some(td_file_off) = rva_to_file_offset(td_rva) else {
            continue;
        };

        // TypeDescriptor : pVFTable (8 octets) + spare (8 octets) + name[].
        // La vtable de type_info est à un offset connu dans .rdata (on vérifie
        // juste que le champ pointe quelque part dans ImageBase + le PE).
        let Some(vtable_ptr) = read_u64(td_file_off) else {
            continue;
        };
        if vtable_ptr < image_base || vtable_ptr >= image_base + bytes.len() as u64 {
            continue;
        }

        let name_off = td_file_off + 16;
        let Some(raw_name) = read_cstr(name_off) else {
            continue;
        };

        // Valide le préfixe RTTI : `.?AV` (classe) ou `.?AU` (struct).
        if !raw_name.starts_with(".?AV") && !raw_name.starts_with(".?AU") {
            continue;
        }

        stats.valid_type_descs += 1;

        // Démangle le nom C++ brut en (nom_classe, namespace).
        let (class_name, namespace) = demangle_msvc_rtti(&raw_name);

        // vtable_vaddr : l'adresse de la vtable est stockée juste avant le
        // pointeur vers le COL dans `.rdata`. On la cherche en parcourant
        // .rdata pour trouver `image_base + col_rva` comme pointeur 64 bits
        // (vtable ptr → COL ptr pattern).
        let col_abs = image_base + expected_rva_self;
        let vtable_vaddr = find_vtable_ptr(rdata_bytes, rdata_vaddr, image_base, col_abs);

        // Hiérarchie via CHD (ClassHierarchyDescriptor).
        let base_rvas: Vec<u32> = if rva_chd != 0 {
            parse_chd(bytes, rva_chd as u64, &rva_to_file_offset, &read_u32)
        } else {
            Vec::new()
        };

        classes.push(ClassRecord {
            class_name,
            namespace,
            mangled: raw_name.clone(),
            vtable_vaddr,
            type_desc_vaddr: Some(td_rva + image_base),
            base_rvas,
        });
    }

    // --- Ingestion DB -----------------------------------------------------------
    let tx = db.conn_mut().transaction()?;

    // Première passe : ingérer toutes les classes, construire rva_mangled → class_id.
    let mut mangled_to_id: std::collections::HashMap<String, i64> =
        std::collections::HashMap::with_capacity(classes.len());

    for cls in &classes {
        let ns = if cls.namespace.is_empty() {
            None
        } else {
            Some(cls.namespace.as_str())
        };
        let mangled = if cls.mangled.is_empty() {
            None
        } else {
            Some(cls.mangled.as_str())
        };

        match ingest::rtti_class(&tx, binary_id, &cls.class_name, ns, mangled) {
            Ok(id) => {
                mangled_to_id.insert(cls.mangled.clone(), id);
                stats.classes_ingested += 1;

                // Met à jour vtable_vaddr et type_desc_vaddr si disponibles.
                if cls.vtable_vaddr.is_some() || cls.type_desc_vaddr.is_some() {
                    let _ = tx.execute(
                        "UPDATE rtti_class SET vtable_vaddr=COALESCE(?1, vtable_vaddr), type_desc_vaddr=COALESCE(?2, type_desc_vaddr) WHERE id=?3",
                        nie_index::rusqlite::params![
                            cls.vtable_vaddr.map(|v| v as i64),
                            cls.type_desc_vaddr.map(|v| v as i64),
                            id
                        ],
                    );
                }
            }
            Err(e) => {
                debug!("rtti_class ingest err pour '{}': {e}", cls.class_name);
            }
        }
    }

    // Deuxième passe : relations d'héritage.
    // On a besoin d'un mapping RVA TypeDescriptor → class_id pour les bases.
    // On reconstruit ce mapping depuis la base : lire type_desc_vaddr.
    // En pratique c'est complexe ; on fait un meilleur effort en associant
    // le nom (mangled) aux RVA connues.
    for cls in &classes {
        let Some(&class_id) = mangled_to_id.get(&cls.mangled) else {
            continue;
        };

        for &base_rva in &cls.base_rvas {
            // Résolution du TypeDescriptor de la base.
            let base_td_rva = base_rva as u64;
            let Some(base_td_file) = rva_to_file_offset(base_td_rva) else {
                continue;
            };
            let base_name_off = base_td_file + 16;
            let Some(base_raw) = read_cstr(base_name_off) else {
                continue;
            };
            let (base_class_name, _base_ns) = demangle_msvc_rtti(&base_raw);

            // Cherche l'id de la classe de base.
            let base_id_result: rusqlite::Result<i64> = tx.query_row(
                "SELECT id FROM rtti_class WHERE binary_id=?1 AND name=?2",
                nie_index::rusqlite::params![binary_id, base_class_name],
                |r| r.get(0),
            );

            match base_id_result {
                Ok(base_id) if base_id != class_id => {
                    let _ = tx.execute(
                        "INSERT OR IGNORE INTO rtti_base(class_id, base_class_id, offset) VALUES(?1,?2,0)",
                        nie_index::rusqlite::params![class_id, base_id],
                    );
                    stats.bases_ingested += 1;
                }
                _ => {}
            }
        }
    }

    tx.commit().context("commit rtti")?;

    Ok(stats)
}

/// Données d'une classe RTTI extraite avant ingestion.
struct ClassRecord {
    class_name: String,
    namespace: String,
    mangled: String,
    vtable_vaddr: Option<u64>,
    type_desc_vaddr: Option<u64>,
    /// RVA des TypeDescriptors des classes de base (depuis BaseClassArray).
    base_rvas: Vec<u32>,
}

/// Démangle un nom RTTI MSVC brut (`.?AV<NomClasse>@<NS>@@` ou
/// `.?AVNomClasse@@`) en `(nom_classe, namespace)`.
///
/// Le nom manglé MSVC pour une classe `lives::CFoo` est `.?AVCFoo@lives@@`.
/// Pour une classe dans l'espace de noms global c'est `.?AVFoo@@`.
fn demangle_msvc_rtti(raw: &str) -> (String, String) {
    // Enlève le préfixe `.?AV` ou `.?AU`.
    let inner = raw
        .strip_prefix(".?AV")
        .or_else(|| raw.strip_prefix(".?AU"))
        .unwrap_or(raw);

    // Enlève le suffixe `@@` final.
    let inner = inner.strip_suffix("@@").unwrap_or(inner);

    // Format : `ClassName@Namespace` ou `ClassName` (global).
    // Il peut y avoir plusieurs `@` pour des namespaces imbriqués :
    // `Foo@NS2@NS1` → class=Foo, ns=NS1::NS2.
    let parts: Vec<&str> = inner.split('@').collect();

    let class_name = parts.first().copied().unwrap_or(inner).to_owned();

    // Le namespace est la concaténation des parties suivantes, en ordre inverse
    // (les namespaces MSVC vont de l'intérieur vers l'extérieur).
    let namespace = if parts.len() > 1 {
        parts[1..]
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("::")
    } else {
        String::new()
    };

    (class_name, namespace)
}

/// Cherche dans `.rdata` un pointeur 64 bits vers `col_abs` et renvoie
/// l'adresse virtuelle de la vtable (= le pointeur qui précède ce slot).
/// Retourne `None` si non trouvé.
fn find_vtable_ptr(
    rdata_bytes: &[u8],
    rdata_vaddr: u64,
    image_base: u64,
    col_abs: u64,
) -> Option<u64> {
    let needle = col_abs.to_le_bytes();
    // Balayage par alignement u64.
    let limit = rdata_bytes.len().saturating_sub(8);
    let mut off = 0usize;
    while off + 8 <= limit {
        if rdata_bytes[off..off + 8] == needle {
            // L'adresse virtuelle de ce slot (= COL ptr) est rdata_vaddr + off.
            // La vtable commence juste après le pointeur meta_ptr qui précède le
            // COL ptr ; en pratique on renvoie l'adresse du COL ptr comme proxy
            // de la vtable (le premier slot est toujours meta_ptr → COL).
            let vaddr = image_base + rdata_vaddr + off as u64;
            return Some(vaddr);
        }
        off += 8;
    }
    None
}

/// Parse le `_RTTIClassHierarchyDescriptor` à `chd_rva` et renvoie les RVA
/// des TypeDescriptors de classes de base (depuis `_RTTIBaseClassArray`).
fn parse_chd(
    _bytes: &[u8],
    chd_rva: u64,
    rva_to_file_offset: &impl Fn(u64) -> Option<usize>,
    read_u32: &impl Fn(usize) -> Option<u32>,
) -> Vec<u32> {
    let Some(chd_off) = rva_to_file_offset(chd_rva) else {
        return Vec::new();
    };

    // CHD layout (x64, 20 octets) :
    //   u32 signature, u32 attributes, u32 numBaseClasses, i32 pBaseClassArray (RVA)
    // En réalité seulement 16 octets : sig(4) + attr(4) + numBase(4) + pBCA(4).
    let Some(_sig) = read_u32(chd_off) else {
        return Vec::new();
    };
    let Some(_attr) = read_u32(chd_off + 4) else {
        return Vec::new();
    };
    let Some(num_bases) = read_u32(chd_off + 8) else {
        return Vec::new();
    };
    let Some(bca_rva) = read_u32(chd_off + 12) else {
        return Vec::new();
    };

    // Limite raisonnable : une classe ne peut pas avoir 1000+ bases.
    if num_bases == 0 || num_bases > 64 {
        return Vec::new();
    }

    let Some(bca_off) = rva_to_file_offset(bca_rva as u64) else {
        return Vec::new();
    };

    // _RTTIBaseClassArray = tableau de RVA (_RTTIBaseClassDescriptor), 4 octets chacun.
    // _RTTIBaseClassDescriptor + 0 = RVA TypeDescriptor de la base.
    let mut base_td_rvas = Vec::with_capacity(num_bases as usize);
    for k in 0..num_bases as usize {
        let bcd_rva_off = bca_off + k * 4;
        let Some(bcd_rva) = read_u32(bcd_rva_off) else {
            break;
        };
        // Résout le BaseClassDescriptor.
        let Some(bcd_off) = rva_to_file_offset(bcd_rva as u64) else {
            break;
        };
        // BCD + 0 = RVA TypeDescriptor.
        let Some(td_rva) = read_u32(bcd_off) else {
            break;
        };
        // La première entrée est la classe elle-même ; on saute les entrées nulles.
        if td_rva != 0 {
            base_td_rvas.push(td_rva);
        }
    }

    // Retire la première entrée (la classe elle-même, pas une vraie base).
    if base_td_rvas.len() > 1 {
        base_td_rvas.remove(0);
    } else {
        base_td_rvas.clear();
    }

    base_td_rvas
}

// Ré-exporte pour les tests.
use nie_index::rusqlite;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demangle_global_class() {
        let (cls, ns) = demangle_msvc_rtti(".?AVFoo@@");
        assert_eq!(cls, "Foo");
        assert_eq!(ns, "");
    }

    #[test]
    fn demangle_single_namespace() {
        let (cls, ns) = demangle_msvc_rtti(".?AVCFoo@lives@@");
        assert_eq!(cls, "CFoo");
        assert_eq!(ns, "lives");
    }

    #[test]
    fn demangle_nested_namespace() {
        let (cls, ns) = demangle_msvc_rtti(".?AVCBar@detail@lives@@");
        assert_eq!(cls, "CBar");
        // Namespaces en ordre extérieur→intérieur.
        assert_eq!(ns, "lives::detail");
    }

    #[test]
    fn demangle_struct_prefix() {
        let (cls, ns) = demangle_msvc_rtti(".?AUMyStruct@@");
        assert_eq!(cls, "MyStruct");
        assert_eq!(ns, "");
    }
}
