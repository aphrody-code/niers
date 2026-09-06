//! Lecture des objets COFF produits par `rustc --emit=obj` (cible `*-pc-windows-msvc`).
//!
//! C'est le maillon qui rend le projet **falsifiable** : une fonction n'est pas
//! « portée » parce qu'on l'a réécrite en Rust, elle est portée quand le code
//! machine que rustc en produit **coïncide avec les octets originaux**. Ce module
//! extrait le code d'un symbole d'un `.o` et construit le **masque de relocation**
//! (les champs d'adresse ne peuvent pas coïncider avant édition de liens ; ils sont
//! neutralisés, tout le reste doit être identique au byte près).

use crate::{PeError, Result, rd_u16, rd_u32};

/// En-tête d'un fichier objet COFF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoffFileHeader {
    /// Machine cible.
    pub machine: u16,
    /// Nombre de sections.
    pub n_sections: u16,
    /// Offset de la table des symboles.
    pub ptr_symbol_table: u32,
    /// Nombre d'entrées de la table des symboles (auxiliaires comprises).
    pub n_symbols: u32,
}

/// Section d'un objet COFF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoffSection {
    /// Nom (résolu via la table de chaînes si préfixé `/`).
    pub name: String,
    /// Taille des données.
    pub size_raw: u32,
    /// Offset des données dans le fichier.
    pub ptr_raw: u32,
    /// Offset de la table de relocations.
    pub ptr_relocations: u32,
    /// Nombre de relocations.
    pub n_relocations: u16,
    /// Caractéristiques.
    pub characteristics: u32,
}

/// Symbole COFF (entrées auxiliaires écartées).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoffSymbol {
    /// Nom résolu.
    pub name: String,
    /// Valeur (offset dans la section pour un symbole défini).
    pub value: u32,
    /// Numéro de section 1-indexé (0 = externe, <0 = spécial).
    pub section_number: i16,
    /// Classe de stockage.
    pub storage_class: u8,
}

/// Relocation COFF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoffReloc {
    /// Offset du champ à reloger, relatif au début de la section.
    pub offset: u32,
    /// Index du symbole visé.
    pub symbol: u32,
    /// Type de relocation (AMD64).
    pub kind: u16,
}

impl CoffReloc {
    /// Largeur en octets du champ réécrit par la relocation.
    #[must_use]
    pub fn width(self) -> usize {
        match self.kind {
            0x0001 => 8,          // ADDR64
            0x000A => 2,          // SECTION
            0x0000 => 0,          // ABSOLUTE (ignorée)
            0x0002..=0x0009 => 4, // ADDR32 / ADDR32NB / REL32[_1..5]
            0x000B | 0x000E => 4, // SECREL / SREL32
            _ => 4,
        }
    }
}

/// Objet COFF parsé.
#[derive(Debug, Clone)]
pub struct CoffObject {
    /// Tampon complet du fichier.
    pub bytes: Vec<u8>,
    /// En-tête.
    pub header: CoffFileHeader,
    /// Sections.
    pub sections: Vec<CoffSection>,
    /// Symboles (auxiliaires filtrées).
    pub symbols: Vec<CoffSymbol>,
}

/// Code d'un symbole, accompagné de son masque de relocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolCode {
    /// Nom du symbole.
    pub name: String,
    /// Octets de code.
    pub bytes: Vec<u8>,
    /// `true` pour chaque octet réécrit par une relocation (non comparable).
    pub reloc_mask: Vec<bool>,
    /// Nom de la section d'origine.
    pub section: String,
}

impl SymbolCode {
    /// Nombre d'octets comparables (hors champs relogés).
    #[must_use]
    pub fn comparable_len(&self) -> usize {
        self.reloc_mask.iter().filter(|m| !**m).count()
    }
}

impl CoffObject {
    /// Parse un objet COFF.
    ///
    /// # Erreurs
    /// Retourne une erreur si la structure est tronquée ou incohérente.
    pub fn parse(bytes: Vec<u8>) -> Result<Self> {
        let machine = rd_u16(&bytes, 0)?;
        let n_sections = rd_u16(&bytes, 2)?;
        let ptr_symbol_table = rd_u32(&bytes, 8)?;
        let n_symbols = rd_u32(&bytes, 12)?;
        let size_optional = rd_u16(&bytes, 16)? as usize;

        let strtab_off = ptr_symbol_table as usize + n_symbols as usize * 18;
        let resolve = |raw: &[u8], bytes: &[u8]| -> String {
            if raw[..4] == [0, 0, 0, 0] {
                let off = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]) as usize;
                let at = strtab_off + off;
                let end = bytes[at..]
                    .iter()
                    .position(|&c| c == 0)
                    .map_or(bytes.len(), |p| at + p);
                String::from_utf8_lossy(&bytes[at..end]).into_owned()
            } else {
                let end = raw.iter().position(|&c| c == 0).unwrap_or(8);
                String::from_utf8_lossy(&raw[..end]).into_owned()
            }
        };

        let sec_off = 20 + size_optional;
        let mut sections = Vec::with_capacity(n_sections as usize);
        for i in 0..n_sections as usize {
            let at = sec_off + i * 40;
            let raw = bytes.get(at..at + 40).ok_or(PeError::Truncated {
                at,
                need: 40,
                len: bytes.len(),
            })?;
            let mut name = {
                let end = raw[..8].iter().position(|&c| c == 0).unwrap_or(8);
                String::from_utf8_lossy(&raw[..end]).into_owned()
            };
            if let Some(rest) = name.strip_prefix('/') {
                // Nom long : `/<offset décimal>` dans la table de chaînes.
                if let Ok(off) = rest.trim().parse::<usize>() {
                    let at = strtab_off + off;
                    let end = bytes[at..]
                        .iter()
                        .position(|&c| c == 0)
                        .map_or(bytes.len(), |p| at + p);
                    name = String::from_utf8_lossy(&bytes[at..end]).into_owned();
                }
            }
            sections.push(CoffSection {
                name,
                size_raw: rd_u32(raw, 16)?,
                ptr_raw: rd_u32(raw, 20)?,
                ptr_relocations: rd_u32(raw, 24)?,
                n_relocations: rd_u16(raw, 32)?,
                characteristics: rd_u32(raw, 36)?,
            });
        }

        let mut symbols = Vec::new();
        let mut i = 0usize;
        while i < n_symbols as usize {
            let at = ptr_symbol_table as usize + i * 18;
            let raw = bytes.get(at..at + 18).ok_or(PeError::Truncated {
                at,
                need: 18,
                len: bytes.len(),
            })?;
            let n_aux = raw[17] as usize;
            symbols.push(CoffSymbol {
                name: resolve(&raw[..8], &bytes),
                value: rd_u32(raw, 8)?,
                section_number: rd_u16(raw, 12)? as i16,
                storage_class: raw[16],
            });
            i += 1 + n_aux;
        }

        Ok(Self {
            bytes,
            header: CoffFileHeader {
                machine,
                n_sections,
                ptr_symbol_table,
                n_symbols,
            },
            sections,
            symbols,
        })
    }

    /// Relocations d'une section (index 0-indexé).
    ///
    /// # Erreurs
    /// Retourne une erreur si la table est tronquée.
    pub fn relocations(&self, section_index: usize) -> Result<Vec<CoffReloc>> {
        let s = self
            .sections
            .get(section_index)
            .ok_or_else(|| PeError::Coff(format!("section {section_index} absente")))?;
        let mut out = Vec::with_capacity(s.n_relocations as usize);
        for i in 0..s.n_relocations as usize {
            let at = s.ptr_relocations as usize + i * 10;
            let raw = self.bytes.get(at..at + 10).ok_or(PeError::Truncated {
                at,
                need: 10,
                len: self.bytes.len(),
            })?;
            out.push(CoffReloc {
                offset: rd_u32(raw, 0)?,
                symbol: rd_u32(raw, 4)?,
                kind: rd_u16(raw, 8)?,
            });
        }
        Ok(out)
    }

    /// Extrait le code d'un symbole défini, avec son masque de relocation.
    ///
    /// L'étendue du symbole est déduite du symbole suivant dans la même section,
    /// ou de la fin de section. Compiler avec `-Z function-sections=yes` (défaut
    /// sur `x86_64-pc-windows-msvc`) donne une section par fonction : l'étendue
    /// est alors exacte.
    ///
    /// # Erreurs
    /// Retourne une erreur si le symbole est absent, externe, ou hors bornes.
    pub fn symbol_code(&self, name: &str) -> Result<SymbolCode> {
        let sym = self
            .symbols
            .iter()
            .find(|s| s.name == name || s.name.trim_start_matches('_') == name)
            .ok_or_else(|| PeError::Coff(format!("symbole `{name}` absent de l'objet")))?;
        if sym.section_number <= 0 {
            return Err(PeError::Coff(format!(
                "symbole `{name}` non défini dans une section (section_number={})",
                sym.section_number
            )));
        }
        let idx = sym.section_number as usize - 1;
        let sec = self
            .sections
            .get(idx)
            .ok_or_else(|| PeError::Coff(format!("section {idx} absente pour `{name}`")))?;

        let start = sym.value as usize;
        let end = self
            .symbols
            .iter()
            .filter(|s| s.section_number == sym.section_number && s.value as usize > start)
            .map(|s| s.value as usize)
            .min()
            .unwrap_or(sec.size_raw as usize)
            .min(sec.size_raw as usize);
        if end <= start {
            return Err(PeError::Coff(format!(
                "étendue vide pour `{name}` ({start:#x}..{end:#x})"
            )));
        }

        let base = sec.ptr_raw as usize;
        let code = self
            .bytes
            .get(base + start..base + end)
            .ok_or(PeError::Truncated {
                at: base + start,
                need: end - start,
                len: self.bytes.len(),
            })?
            .to_vec();

        let mut mask = vec![false; code.len()];
        for r in self.relocations(idx)? {
            let w = r.width();
            if w == 0 {
                continue;
            }
            let off = r.offset as usize;
            if off + w <= start || off >= end {
                continue;
            }
            for m in mask
                .iter_mut()
                .take(end - start)
                .skip(off.saturating_sub(start))
                .take(w)
            {
                *m = true;
            }
        }

        Ok(SymbolCode {
            name: sym.name.clone(),
            bytes: code,
            reloc_mask: mask,
            section: sec.name.clone(),
        })
    }

    /// Liste les symboles de fonction définis (classe externe/statique, section de code).
    #[must_use]
    pub fn defined_functions(&self) -> Vec<&CoffSymbol> {
        self.symbols
            .iter()
            .filter(|s| {
                s.section_number > 0
                    && matches!(s.storage_class, 2 | 3)
                    && self
                        .sections
                        .get(s.section_number as usize - 1)
                        .is_some_and(|sec| sec.characteristics & 0x2000_0020 != 0)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Objet COFF minimal : 1 section `.text` de 8 octets, 1 symbole, 1 relocation REL32.
    fn synth_obj() -> Vec<u8> {
        let sec_data: [u8; 8] = [0x48, 0x89, 0xE8, 0xE8, 0x00, 0x00, 0x00, 0x00];
        let hdr = 20usize;
        let sec_tab = hdr;
        let data_off = sec_tab + 40;
        let reloc_off = data_off + sec_data.len();
        let symtab_off = reloc_off + 10;
        let total = symtab_off + 18 + 4;
        let mut b = vec![0u8; total];
        b[0..2].copy_from_slice(&0x8664u16.to_le_bytes());
        b[2..4].copy_from_slice(&1u16.to_le_bytes());
        b[8..12].copy_from_slice(&u32::try_from(symtab_off).unwrap().to_le_bytes());
        b[12..16].copy_from_slice(&1u32.to_le_bytes());
        // section .text
        b[sec_tab..sec_tab + 5].copy_from_slice(b".text");
        b[sec_tab + 16..sec_tab + 20]
            .copy_from_slice(&u32::try_from(sec_data.len()).unwrap().to_le_bytes());
        b[sec_tab + 20..sec_tab + 24]
            .copy_from_slice(&u32::try_from(data_off).unwrap().to_le_bytes());
        b[sec_tab + 24..sec_tab + 28]
            .copy_from_slice(&u32::try_from(reloc_off).unwrap().to_le_bytes());
        b[sec_tab + 32..sec_tab + 34].copy_from_slice(&1u16.to_le_bytes());
        b[sec_tab + 36..sec_tab + 40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        b[data_off..data_off + sec_data.len()].copy_from_slice(&sec_data);
        // relocation REL32 sur l'opérande de `call` (offset 4)
        b[reloc_off..reloc_off + 4].copy_from_slice(&4u32.to_le_bytes());
        b[reloc_off + 8..reloc_off + 10].copy_from_slice(&4u16.to_le_bytes());
        // symbole `essai` @ 0
        b[symtab_off..symtab_off + 5].copy_from_slice(b"essai");
        b[symtab_off + 12..symtab_off + 14].copy_from_slice(&1u16.to_le_bytes());
        b[symtab_off + 16] = 2; // IMAGE_SYM_CLASS_EXTERNAL
        b[symtab_off + 18..symtab_off + 22].copy_from_slice(&4u32.to_le_bytes());
        b
    }

    #[test]
    fn extrait_le_code_et_le_masque_de_relocation() {
        let o = CoffObject::parse(synth_obj()).expect("parse");
        assert_eq!(o.sections.len(), 1);
        assert_eq!(o.symbols.len(), 1);
        let c = o.symbol_code("essai").expect("symbole");
        assert_eq!(c.bytes, vec![0x48, 0x89, 0xE8, 0xE8, 0, 0, 0, 0]);
        assert_eq!(
            c.reloc_mask,
            vec![false, false, false, false, true, true, true, true]
        );
        assert_eq!(c.comparable_len(), 4);
        assert_eq!(o.defined_functions().len(), 1);
    }

    #[test]
    fn symbole_absent_est_une_erreur_explicite() {
        let o = CoffObject::parse(synth_obj()).expect("parse");
        let e = o.symbol_code("inconnu").unwrap_err();
        assert!(matches!(e, PeError::Coff(m) if m.contains("absent")));
    }
}
