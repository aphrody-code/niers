//! Encodeur CPK/@UTF — contrepartie écriture de [`crate::cpk`] (roadmap
//! `apps/inacord/ROADMAP.md` §1.2, « Encodeur CPK (pack) »).
//!
//! Portée volontairement **restreinte et honnête**, pas une reconstruction générale :
//!
//! - [`encode_utf`] écrit une table `@UTF` générique (magic + schéma + lignes + pool de
//!   chaînes/blobs), byte-exact avec ce que [`crate::cpk::parse_utf`] (déjà validé sur le vrai
//!   jeu) attend en lecture — TOUTES les colonnes en `FLAG_HAS_NAME | FLAG_ROW_STORAGE` (pas de
//!   valeur par défaut constante, cf. `crate::cpk` doc des flags) : plus simple, toujours valide,
//!   juste un peu plus gros qu'un encodage optimal — pas une approximation risquée.
//! - [`encode_cpk`] écrit une archive CPK **NON chiffrée, NON compressée** (magic `CPK ` en
//!   clair, `is_compressed=false` pour toutes les entrées côté lecture — `ExtractSize ==
//!   FileSize`) : ni le chiffrement position-based XOR du vrai jeu (déchiffrement seul est
//!   implémenté côté [`crate::cpk::decrypt_block`], le chiffrer à l'identique n'a pas été
//!   vérifié et n'apporte rien pour un CPK de MOD, jamais destiné à remplacer un fichier chiffré
//!   du jeu en place) ni CRILAYLA (compresseur propriétaire non reversé ici — la référence C++20
//!   externe portée pour §2.2/§1.1 ne l'implémente pas non plus, cf. `ROADMAP.md`). Layout
//!   header→contenu→TOC (`ContentOffset ≤ TocOffset`, cf. le repli de [`crate::cpk::CpkReader::
//!   new`] sur `content_offset = toc_offset` sinon).
//!
//! Vérifié par round-trip réel contre le lecteur DÉJÀ validé sur le vrai jeu
//! ([`crate::cpk::CpkReader`], `apps/inacord` `open_raw_cpk`/`RawCpkView`) : encoder →
//! `CpkReader::new` → `extract` de chaque entrée → mêmes octets que l'entrée (cf. `mod tests`).
//! **Non vérifié en revanche par chargement réel dans `nie.exe`** — contrairement à
//! `encode_rdbn`/`encode_g4tx_single_texture`, il n'existe aucun moyen de charger un CPK
//! reconstruit dans le jeu sans risquer une détection Easy Anti-Cheat (écriture dans le dossier
//! du jeu) : cette limite est documentée, pas cachée. Usage prévu : export d'un mod en `.cpk`
//! autonome (chargé par un futur loader de mods, pas en remplacement in-place d'un CPK du jeu).

extern crate alloc;
use alloc::{format, string::String, string::ToString, vec, vec::Vec};

use crate::cpk::{ColumnType, UtfValue};

const UTF_MAGIC: [u8; 4] = [0x40, 0x55, 0x54, 0x46]; // "@UTF"

/// Descripteur d'une colonne à écrire (nom + type — cf. [`crate::cpk::UtfColumn`] en lecture).
pub struct UtfColumnSpec {
    pub name: String,
    pub col_type: ColumnType,
}

/// Ajoute `s` au pool de chaînes s'il n'y est pas déjà (déduplication), renvoie son offset.
fn intern_string(s: &str, pool: &mut Vec<u8>, offsets: &mut Vec<(String, u32)>) -> u32 {
    if let Some((_, off)) = offsets.iter().find(|(k, _)| k == s) {
        return *off;
    }
    let off = pool.len() as u32;
    pool.extend_from_slice(s.as_bytes());
    pool.push(0);
    offsets.push((s.to_string(), off));
    off
}

fn push_value_be(
    out: &mut Vec<u8>,
    v: &UtfValue,
    col_type: ColumnType,
    string_offsets: &mut Vec<(String, u32)>,
    string_pool: &mut Vec<u8>,
) -> Result<(), String> {
    match (v, col_type) {
        (UtfValue::U8(x), ColumnType::U8) => out.push(*x),
        (UtfValue::I8(x), ColumnType::I8) => out.push(*x as u8),
        (UtfValue::U16(x), ColumnType::U16) => out.extend_from_slice(&x.to_be_bytes()),
        (UtfValue::I16(x), ColumnType::I16) => out.extend_from_slice(&x.to_be_bytes()),
        (UtfValue::U32(x), ColumnType::U32) => out.extend_from_slice(&x.to_be_bytes()),
        (UtfValue::I32(x), ColumnType::I32) => out.extend_from_slice(&x.to_be_bytes()),
        (UtfValue::U64(x), ColumnType::U64) => out.extend_from_slice(&x.to_be_bytes()),
        (UtfValue::I64(x), ColumnType::I64) => out.extend_from_slice(&x.to_be_bytes()),
        (UtfValue::F32(x), ColumnType::F32) => out.extend_from_slice(&x.to_bits().to_be_bytes()),
        (UtfValue::F64(x), ColumnType::F64) => out.extend_from_slice(&x.to_bits().to_be_bytes()),
        (UtfValue::String(s), ColumnType::String) => {
            out.extend_from_slice(&intern_string(s, string_pool, string_offsets).to_be_bytes())
        }
        (v, t) => {
            return Err(format!(
                "encode_utf : valeur {v:?} incompatible avec le type de colonne {t:?}"
            ));
        }
    }
    Ok(())
}

/// Encode une table `@UTF` générique — magic + schéma de colonnes + lignes + pool de chaînes,
/// byte-exact avec [`crate::cpk::parse_utf`] en lecture (colonnes `HAS_NAME|ROW_STORAGE`
/// uniquement, cf. doc de module). N'écrit PAS de pool de données binaires (`Bytes`/`Guid`) —
/// non nécessaire pour l'usage CPK (`CpkEntry` n'a que des colonnes entières/chaîne).
///
/// # Erreurs
/// `Err` si une ligne n'a pas exactement `columns.len()` valeurs, ou si une valeur ne correspond
/// pas au type déclaré de sa colonne (jamais une conversion silencieuse qui corromprait la table).
pub fn encode_utf(
    table_name: &str,
    columns: &[UtfColumnSpec],
    rows: &[Vec<UtfValue>],
) -> Result<Vec<u8>, String> {
    const FLAG_HAS_NAME: u8 = 0x10;
    const FLAG_ROW_STORAGE: u8 = 0x40;
    const UTF_BASE: usize = 0x08;

    for (i, row) in rows.iter().enumerate() {
        if row.len() != columns.len() {
            return Err(format!(
                "encode_utf : ligne {i} a {} valeurs, attendu {}",
                row.len(),
                columns.len()
            ));
        }
    }

    // Pool de chaînes : nom de table, noms de colonnes, puis valeurs String des lignes (dédupliqué).
    let mut string_pool: Vec<u8> = Vec::new();
    let mut string_offsets: Vec<(String, u32)> = Vec::new();
    let table_name_off = intern_string(table_name, &mut string_pool, &mut string_offsets);
    let col_name_offs: Vec<u32> = columns
        .iter()
        .map(|c| intern_string(&c.name, &mut string_pool, &mut string_offsets))
        .collect();

    // Schéma de colonnes (0x20 + col_count*5 : 1 flag + 4 name_off BE par colonne).
    let mut schema = Vec::new();
    for (col, name_off) in columns.iter().zip(&col_name_offs) {
        schema.push(FLAG_HAS_NAME | FLAG_ROW_STORAGE | (col.col_type as u8));
        schema.extend_from_slice(&name_off.to_be_bytes());
    }

    // Lignes (row_stride = somme des wire_size des colonnes, toutes ROW_STORAGE).
    let row_stride: u16 = columns.iter().map(|c| c.col_type.wire_size() as u16).sum();
    let mut rows_bytes = Vec::new();
    for row in rows {
        for (v, col) in row.iter().zip(columns) {
            push_value_be(
                &mut rows_bytes,
                v,
                col.col_type,
                &mut string_offsets,
                &mut string_pool,
            )?;
        }
    }

    // Assemblage : en-tête(8) + section(24, à UTF_BASE) + schéma + lignes + pool de chaînes.
    let section_end = UTF_BASE + 24 + schema.len();
    let rows_offset_abs = section_end;
    let string_offset_abs = rows_offset_abs + rows_bytes.len();
    let data_offset_abs = string_offset_abs + string_pool.len(); // pool de données vide, placé juste après.

    let mut out = Vec::with_capacity(data_offset_abs);
    out.extend_from_slice(&UTF_MAGIC);
    let table_size = (data_offset_abs - UTF_BASE) as u32;
    out.extend_from_slice(&table_size.to_be_bytes());

    out.extend_from_slice(&((rows_offset_abs - UTF_BASE) as u32).to_be_bytes());
    out.extend_from_slice(&((string_offset_abs - UTF_BASE) as u32).to_be_bytes());
    out.extend_from_slice(&((data_offset_abs - UTF_BASE) as u32).to_be_bytes());
    out.extend_from_slice(&table_name_off.to_be_bytes());
    out.extend_from_slice(&(columns.len() as u16).to_be_bytes());
    out.extend_from_slice(&row_stride.to_be_bytes());
    out.extend_from_slice(&(rows.len() as u32).to_be_bytes());

    out.extend_from_slice(&schema);
    out.extend_from_slice(&rows_bytes);
    out.extend_from_slice(&string_pool);
    // Pool de données binaires : vide (aucune colonne Bytes/Guid émise par ce module).

    Ok(out)
}

/// Enveloppe une table `@UTF` déjà encodée dans le conteneur CRI 16 octets attendu par
/// [`crate::cpk::parse_table_container`] : magic ASCII (4, ex. `"CPK "`/`"TOC "` — non validé
/// par le parseur mais écrit pour la compatibilité avec d'autres lecteurs CRIWARE) + pad(4)=0 +
/// taille LE(4, = `utf_bytes.len()`) + pad(4)=0.
fn wrap_table_container(magic: &[u8; 4], utf_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + utf_bytes.len());
    out.extend_from_slice(magic);
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(utf_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(utf_bytes);
    out
}

/// Un fichier à empaqueter (cf. [`crate::cpk::CpkEntry`] en lecture — `directory`/`filename`
/// séparés, comme la table TOC réelle).
pub struct CpkWriteEntry {
    pub filename: String,
    pub directory: String,
    pub data: Vec<u8>,
}

/// Encode une archive CPK **non chiffrée, non compressée** portant `entries` — cf. doc de module
/// pour la portée exacte et ses limites documentées.
///
/// # Erreurs
/// `Err` si `entries` est vide (rien à empaqueter — pas un CPK dégénéré silencieux).
pub fn encode_cpk(entries: &[CpkWriteEntry]) -> Result<Vec<u8>, String> {
    if entries.is_empty() {
        return Err("encode_cpk : aucune entrée à empaqueter".to_string());
    }

    // ── Table TOC (une ligne par fichier) ──────────────────────────────────────────────
    let toc_columns = [
        UtfColumnSpec {
            name: "DirName".to_string(),
            col_type: ColumnType::String,
        },
        UtfColumnSpec {
            name: "FileName".to_string(),
            col_type: ColumnType::String,
        },
        UtfColumnSpec {
            name: "FileSize".to_string(),
            col_type: ColumnType::U64,
        },
        UtfColumnSpec {
            name: "ExtractSize".to_string(),
            col_type: ColumnType::U64,
        },
        UtfColumnSpec {
            name: "FileOffset".to_string(),
            col_type: ColumnType::U64,
        },
    ];
    let mut toc_rows = Vec::with_capacity(entries.len());
    let mut file_off: u64 = 0;
    let mut content = Vec::new();
    for e in entries {
        toc_rows.push(vec![
            UtfValue::String(e.directory.clone()),
            UtfValue::String(e.filename.clone()),
            UtfValue::U64(e.data.len() as u64),
            UtfValue::U64(e.data.len() as u64), // ExtractSize == FileSize : jamais compressé (cf. doc module).
            UtfValue::U64(file_off),
        ]);
        content.extend_from_slice(&e.data);
        file_off += e.data.len() as u64;
    }
    let toc_utf = encode_utf("CpkTocInfo", &toc_columns, &toc_rows)?;
    let toc_container = wrap_table_container(b"TOC ", &toc_utf);

    // ── Table d'en-tête (1 ligne : TocOffset/ContentOffset — les 2 seuls champs lus par
    //    `CpkReader::new`, cf. doc de module, jamais d'autres colonnes réelles devinées) ──────
    let header_columns = [
        UtfColumnSpec {
            name: "ContentOffset".to_string(),
            col_type: ColumnType::U64,
        },
        UtfColumnSpec {
            name: "TocOffset".to_string(),
            col_type: ColumnType::U64,
        },
    ];

    // Layout : conteneur d'en-tête (taille CONNUE d'avance : 1 ligne, colonnes fixes) → contenu
    // → TOC. `ContentOffset` DOIT être ≤ `TocOffset` (sinon `CpkReader::new` retombe sur
    // `content_offset = toc_offset`, cf. sa doc) — header→contenu→TOC le garantit trivialement.
    let header_utf_probe = encode_utf(
        "CpkHeader",
        &header_columns,
        &[vec![UtfValue::U64(0), UtfValue::U64(0)]],
    )?;
    let header_container_len = 16 + header_utf_probe.len();
    let content_offset = header_container_len as u64;
    let toc_offset = content_offset + content.len() as u64;

    let header_utf = encode_utf(
        "CpkHeader",
        &header_columns,
        &[vec![
            UtfValue::U64(content_offset),
            UtfValue::U64(toc_offset),
        ]],
    )?;
    debug_assert_eq!(
        header_utf.len(),
        header_utf_probe.len(),
        "encode_utf non déterministe pour une même entrée — bug"
    );
    let header_container = wrap_table_container(b"CPK ", &header_utf);

    let mut out = Vec::with_capacity(header_container.len() + content.len() + toc_container.len());
    out.extend_from_slice(&header_container);
    out.extend_from_slice(&content);
    out.extend_from_slice(&toc_container);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpk::CpkReader;

    /// Round-trip minimal : encode 3 fichiers, relit avec le VRAI `CpkReader` (déjà validé sur
    /// le jeu), vérifie noms/dossiers/tailles ET contenu octet-exact de chaque entrée.
    #[test]
    fn encode_cpk_round_trip_via_cpk_reader() {
        let entries = vec![
            CpkWriteEntry {
                filename: "a.txt".to_string(),
                directory: "dir1".to_string(),
                data: b"Contenu du fichier A.".to_vec(),
            },
            CpkWriteEntry {
                filename: "b.bin".to_string(),
                directory: String::new(),
                data: (0u8..=255).collect(),
            },
            CpkWriteEntry {
                filename: "c.txt".to_string(),
                directory: "dir1/sub".to_string(),
                data: Vec::new(),
            },
        ];
        let bytes = encode_cpk(&entries).expect("encode_cpk");

        let reader = CpkReader::new(&bytes, "test.cpk").expect("CpkReader::new sur le CPK encodé");
        assert!(!reader.is_encrypted);
        assert_eq!(reader.entries.len(), 3);

        for (entry, original) in reader.entries.iter().zip(&entries) {
            assert_eq!(entry.filename, original.filename);
            assert_eq!(entry.directory, original.directory);
            assert_eq!(entry.size, original.data.len() as u64);
            assert!(!entry.is_compressed);
            let extracted = reader.extract(&bytes, entry).expect("extract");
            assert_eq!(
                extracted, original.data,
                "contenu divergent pour {}/{}",
                original.directory, original.filename
            );
        }
    }

    /// `encode_cpk` refuse un empaquetage vide plutôt que produire un fichier dégénéré.
    #[test]
    fn encode_cpk_rejette_liste_vide() {
        assert!(encode_cpk(&[]).is_err());
    }

    /// Round-trip avec BEAUCOUP d'entrées (noms partagés entre dossiers, cf. `RawCpkState` doc
    /// sur les doublons de nom) — vérifie que le pool de chaînes dédupliqué ne mélange pas les
    /// offsets entre lignes distinctes.
    #[test]
    fn encode_cpk_round_trip_many_entries_noms_repetes() {
        let mut entries = Vec::new();
        for i in 0..50 {
            entries.push(CpkWriteEntry {
                filename: "shared_name.dat".to_string(), // même nom dans des dossiers différents
                directory: format!("dir_{i}"),
                data: alloc::vec![i as u8; 17 + i],
            });
        }
        let bytes = encode_cpk(&entries).expect("encode_cpk");
        let reader = CpkReader::new(&bytes, "test.cpk").expect("CpkReader::new");
        assert_eq!(reader.entries.len(), 50);
        for (entry, original) in reader.entries.iter().zip(&entries) {
            assert_eq!(entry.directory, original.directory);
            let extracted = reader.extract(&bytes, entry).expect("extract");
            assert_eq!(extracted, original.data);
        }
    }
}
