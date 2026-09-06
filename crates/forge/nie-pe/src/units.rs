//! Découpage du fichier en **unités de forge** et réassemblage byte-exact.
//!
//! Invariant central : les unités forment un **recouvrement total** du fichier.
//! Chaque octet appartient à exactement une unité, offsets contigus depuis `0`
//! jusqu'à la taille du fichier — en-têtes, bourrage de section, trous entre
//! sections et overlay compris. C'est ce qui rend la génération vérifiable :
//! `assemble()` ne consulte jamais le fichier d'origine, il ne fait que
//! concaténer des charges utiles dans l'ordre.
//!
//! Une unité porte sa **provenance** : soit elle est produite par du code Rust
//! (en-têtes ré-émis, fonction portée dont le codegen coïncide), soit elle vient
//! du binaire de référence. Le rapport de couverture de `nie-forge` mesure
//! exactement ce ratio.

use crate::image::PeImage;
use crate::pdata::{self, CodeRange};
use crate::{PeError, Result, sha256_hex};

/// Nature d'une unité de forge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum UnitKind {
    /// En-tête DOS + stub 16 bits (opaque, conservé verbatim).
    DosStub,
    /// Signature PE + en-tête COFF + en-tête optionnel + table des sections.
    /// **Ré-émissible** depuis les structures (`PeImage::emit_headers`).
    PeHeaders,
    /// Bourrage du linker entre la table des sections et `SizeOfHeaders`.
    HeaderPad,
    /// Corps de fonction délimité par `.pdata` (entrée racine).
    Function,
    /// Fragment de code chaîné (`UNW_FLAG_CHAININFO`).
    CodeFragment,
    /// Octets d'une section de code non couverts par `.pdata` : fonctions
    /// feuilles sans information d'unwind, tables de saut, données inline.
    ///
    /// Le bourrage `int3` qui les sépare est extrait dans [`UnitKind::Padding`],
    /// si bien qu'un résidu correspond en pratique à **un** corps candidat.
    CodeResidue,
    /// Bourrage `int3` (`0xCC`) inséré par le linker entre deux corps de code.
    Padding,
    /// Données déposées **au milieu** d'une section de code : tables de sauts,
    /// constantes vectorielles alignées, littéraux flottants.
    ///
    /// MSVC les place entre les instructions qui les utilisent. Tant qu'elles
    /// restaient soudées au corps qui les entoure, l'unité entière échouait au
    /// relevé — mesuré sur `nie.exe` : 998 unités et 1 034 147 octets refusés
    /// par le désassembleur, dont **990 445 octets de code parfaitement
    /// décodable** que 39 968 octets de données rendaient inexploitables.
    InlineData,
    /// Contenu brut d'une section de données.
    SectionData,
    /// Trou entre deux régions de données de section.
    Gap,
    /// Octets au-delà de la dernière section (overlay, signature…).
    Overlay,
}

impl UnitKind {
    /// Libellé court stable (utilisé dans les identifiants et les rapports).
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::DosStub => "dos",
            Self::PeHeaders => "hdr",
            Self::HeaderPad => "hdrpad",
            Self::Function => "fn",
            Self::CodeFragment => "frag",
            Self::CodeResidue => "res",
            Self::Padding => "pad",
            Self::InlineData => "idata",
            Self::SectionData => "data",
            Self::Gap => "gap",
            Self::Overlay => "overlay",
        }
    }

    /// Vrai si l'unité contient du code exécutable.
    #[must_use]
    pub fn is_code(self) -> bool {
        matches!(
            self,
            Self::Function | Self::CodeFragment | Self::CodeResidue
        )
    }
}

/// Une unité : une tranche du fichier, identifiée et empreintée.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Unit {
    /// Identifiant stable, unique dans le recouvrement (ex. `fn.140001000`).
    pub id: String,
    /// Nature de l'unité.
    pub kind: UnitKind,
    /// Section propriétaire, si applicable.
    pub section: Option<String>,
    /// Offset dans le fichier.
    pub file_off: usize,
    /// Longueur en octets.
    pub len: usize,
    /// Adresse virtuelle absolue du premier octet, si l'unité est mappée.
    pub va: Option<u64>,
    /// Empreinte SHA-256 de la charge utile de référence.
    pub sha256: String,
}

impl Unit {
    /// Étendue fichier `[file_off, file_off+len)`.
    #[must_use]
    pub fn range(&self) -> core::ops::Range<usize> {
        self.file_off..self.file_off + self.len
    }

    /// Régénère la charge utile d'une unité dont la **règle de construction du
    /// linker est connue**, sans jamais consulter le binaire de référence.
    ///
    /// Aujourd'hui une seule règle : le bourrage `int3`. Elle n'est pas une
    /// recopie déguisée, et c'est le découpage lui-même qui le garantit —
    /// `push_residue` ne ferme une unité [`UnitKind::Padding`] que sur le
    /// critère « l'octet vaut `0xCC` », si bien qu'une telle unité ne peut,
    /// par construction, contenir autre chose. Mesuré sur `nie.exe` : les
    /// 106 565 unités de bourrage, 1 146 297 octets, sont du `0xCC` pur à
    /// 100 %. `nie-forge build` le revérifie de toute façon à l'octet près,
    /// l'identité globale du fichier restant le contrat.
    ///
    /// Elle vit ici, et non chez l'appelant, pour la même raison que
    /// `image::tables::emit_for` : la construction et la mesure doivent lire
    /// la même règle, sinon elles divergent.
    ///
    /// Le résultat est **confronté à l'empreinte de l'unité** avant d'être
    /// rendu : une règle qui se tromperait ne produit rien plutôt que de
    /// gonfler la mesure. `sha256` est une empreinte, pas la donnée — la
    /// charge utile reste calculée, jamais lue dans la référence.
    #[must_use]
    pub fn emit_rule(&self) -> Option<Vec<u8>> {
        let bytes = match self.kind {
            UnitKind::Padding => vec![INT3; self.len],
            // Bourrage du linker entre la table des sections et `SizeOfHeaders`.
            UnitKind::HeaderPad => vec![0u8; self.len],
            _ => return None,
        };
        (bytes.len() == self.len && self.checks(&bytes)).then_some(bytes)
    }

    /// Vrai si `bytes` correspond à l'empreinte de référence de l'unité.
    ///
    /// Une unité sans empreinte renseignée (recouvrement synthétique d'un test)
    /// accepte toute charge de la bonne taille.
    #[must_use]
    pub fn checks(&self, bytes: &[u8]) -> bool {
        self.sha256.is_empty() || sha256_hex(bytes) == self.sha256
    }
}

/// Octet de bourrage inséré par le linker MSVC entre deux corps de code.
const INT3: u8 = 0xCC;

/// Contexte d'une section pendant le découpage.
struct SectionCtx<'a> {
    /// Nom de la section.
    name: &'a str,
    /// Offset fichier du début de la section.
    file_start: usize,
    /// Adresse virtuelle du début de la section.
    va: u64,
}

/// Subdivise une zone de code non couverte par `.pdata` en corps candidats et
/// bourrage `int3`.
///
/// MSVC sépare les fonctions par des `int3` : isoler ces runs donne, dans les
/// faits, une unité par **fonction feuille** — précisément la population que
/// `.pdata` ne décrit pas et où se trouvent les petits gestionnaires portables.
fn push_residue<F>(
    units: &mut Vec<Unit>,
    mk: &F,
    file: &[u8],
    sec: &SectionCtx<'_>,
    from: usize,
    to: usize,
) where
    F: Fn(UnitKind, Option<&str>, usize, usize, Option<u64>) -> Unit,
{
    let (section, sec_start, sec_va) = (sec.name, sec.file_start, sec.va);
    let mut pos = from;
    while pos < to {
        let is_pad = file[pos] == INT3;
        let mut end = pos;
        while end < to && (file[end] == INT3) == is_pad {
            end += 1;
        }
        let kind = if is_pad {
            UnitKind::Padding
        } else {
            UnitKind::CodeResidue
        };
        let va = sec_va + (pos - sec_start) as u64;
        units.push(mk(kind, Some(section), pos, end - pos, Some(va)));
        pos = end;
    }
}

/// Recouvrement total d'un fichier par des unités contiguës.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cover {
    /// Taille du fichier couvert.
    pub total_len: usize,
    /// Empreinte SHA-256 du fichier couvert.
    pub sha256: String,
    /// Unités, dans l'ordre des offsets.
    pub units: Vec<Unit>,
}

impl Cover {
    /// Découpe une image en unités.
    ///
    /// Les sections exécutables sont subdivisées par les bornes `.pdata` ; le
    /// reste de leur contenu devient du résidu explicitement compté. Les sections
    /// de données restent d'un seul tenant (subdivision fine = travail ultérieur,
    /// jamais un trou silencieux).
    ///
    /// # Erreurs
    /// Retourne une erreur si les régions de section se chevauchent dans le
    /// fichier, ou si le recouvrement produit n'est pas total.
    pub fn split(img: &PeImage) -> Result<Self> {
        Self::split_with(img, &[])
    }

    /// Découpe une image en unités, en tenant compte de **plages de fonctions
    /// supplémentaires** (RVA de début, longueur).
    ///
    /// `.pdata` ne décrit que les fonctions qui ont des données de déroulement :
    /// sur `nie.exe` il laisse 1,8 Mo de `.text` en résidu, haché par les seules
    /// bornes de remplissage. Ce résidu n'est pas relevable — une unité peut
    /// commencer au milieu d'une instruction ou couvrir plusieurs fonctions.
    /// Les feuilles récupérées par l'échafaudage RE (`nie_re::recover`) le
    /// découpent correctement.
    ///
    /// Les plages qui chevauchent une plage `.pdata` déjà posée sont ignorées :
    /// `.pdata` reste la vérité terrain.
    ///
    /// # Erreurs
    /// Mêmes conditions que [`Cover::split`].
    pub fn split_with(img: &PeImage, extra: &[(u32, u32)]) -> Result<Self> {
        Self::split_with_data(img, extra, &[])
    }

    /// Découpe une image en isolant en plus des **données inline** `(RVA, len)`
    /// déposées au milieu du code.
    ///
    /// Une table de sauts ou une constante vectorielle soudée au corps qui
    /// l'entoure fait échouer le relevé de la fonction entière. Les isoler rend
    /// relevable le code qui les encadre ; elles-mêmes restent comptées comme
    /// des données, jamais comme du code produit.
    ///
    /// Les plages sont exprimées en RVA parce que c'est ce que manipulent le
    /// désassembleur et `.pdata` ; celles qui ne tombent pas dans une unité de
    /// code sont ignorées sans erreur.
    ///
    /// # Erreurs
    /// Mêmes conditions que [`Cover::split`].
    pub fn split_with_data(
        img: &PeImage,
        extra: &[(u32, u32)],
        inline: &[(u32, u32)],
    ) -> Result<Self> {
        let mut cover = Self::split_raw(img, extra)?;
        if !inline.is_empty() {
            let plages: Vec<(usize, usize)> = inline
                .iter()
                .filter_map(|&(rva, len)| {
                    let off = img.rva_to_offset(rva)?;
                    (len > 0).then_some((off, len as usize))
                })
                .collect();
            cover.subdivide_data(&img.bytes, &plages);
            cover.validate()?;
        }
        Ok(cover)
    }

    /// Isole les plages `(offset fichier, longueur)` données comme des unités
    /// [`UnitKind::InlineData`], en scindant les unités de code qui les portent.
    ///
    /// Le recouvrement reste total : une unité scindée rend la somme exacte de
    /// ses morceaux. Une plage qui déborde de l'unité est tronquée à celle-ci.
    fn subdivide_data(&mut self, file: &[u8], plages: &[(usize, usize)]) {
        use std::collections::BTreeMap;
        // Regroupées par unité porteuse pour ne parcourir le recouvrement
        // qu'une fois : 200 000 unités contre quelques milliers de plages.
        let mut par_unite: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
        for (i, u) in self.units.iter().enumerate() {
            if !u.kind.is_code() {
                continue;
            }
            for &(off, len) in plages {
                let d0 = off.max(u.file_off);
                let d1 = (off + len).min(u.file_off + u.len);
                if d0 < d1 {
                    par_unite.entry(i).or_default().push((d0, d1));
                }
            }
        }
        if par_unite.is_empty() {
            return;
        }

        let mut out: Vec<Unit> = Vec::with_capacity(self.units.len() + par_unite.len() * 2);
        for (i, u) in self.units.iter().enumerate() {
            let Some(mut coupes) = par_unite.remove(&i) else {
                out.push(u.clone());
                continue;
            };
            coupes.sort_unstable();
            let base_va = u.va;
            let fin = u.file_off + u.len;
            let mut pos = u.file_off;
            let mut premier = true;
            let mut pousser = |kind: UnitKind, a: usize, b: usize, premier: bool| {
                if a >= b {
                    return;
                }
                let va = base_va.map(|v| v + (a - u.file_off) as u64);
                // Le premier morceau garde l'identité de l'unité d'origine :
                // le registre et les preuves déjà écrites y restent adossés.
                let id = if premier {
                    u.id.clone()
                } else {
                    match kind {
                        UnitKind::InlineData => format!("idata.{:x}", va.unwrap_or(a as u64)),
                        _ => format!("{}.{:x}", kind.tag(), va.unwrap_or(a as u64)),
                    }
                };
                out.push(Unit {
                    id,
                    kind,
                    section: u.section.clone(),
                    file_off: a,
                    len: b - a,
                    va,
                    sha256: sha256_hex(&file[a..b]),
                });
            };
            for (d0, d1) in coupes {
                if d0 > pos {
                    pousser(u.kind, pos, d0, premier);
                    premier = false;
                }
                pousser(UnitKind::InlineData, d0, d1, premier);
                premier = false;
                pos = d1;
            }
            // La reprise de code après les données est un fragment du corps.
            let reprise = if u.kind == UnitKind::Function {
                UnitKind::CodeFragment
            } else {
                u.kind
            };
            pousser(reprise, pos, fin, premier);
        }
        self.units = out;
    }

    /// Découpe brute, sans isolation des données inline.
    fn split_raw(img: &PeImage, extra: &[(u32, u32)]) -> Result<Self> {
        let file = &img.bytes;
        let total_len = file.len();
        let mut units: Vec<Unit> = Vec::new();

        let mk = |kind: UnitKind,
                  section: Option<&str>,
                  off: usize,
                  len: usize,
                  va: Option<u64>|
         -> Unit {
            let id = match kind {
                UnitKind::DosStub => "dos".to_string(),
                UnitKind::PeHeaders => "hdr".to_string(),
                UnitKind::HeaderPad => "hdrpad".to_string(),
                UnitKind::Overlay => "overlay".to_string(),
                UnitKind::Function | UnitKind::CodeFragment => {
                    format!("{}.{:x}", kind.tag(), va.unwrap_or_default())
                }
                UnitKind::SectionData => {
                    format!("data.{}", section.unwrap_or("?").trim_start_matches('.'))
                }
                _ => format!(
                    "{}.{}.{off:x}",
                    kind.tag(),
                    section.unwrap_or("?").trim_start_matches('.')
                ),
            };
            Unit {
                id,
                kind,
                section: section.map(str::to_string),
                file_off: off,
                len,
                va,
                sha256: sha256_hex(&file[off..off + len]),
            }
        };

        // --- région d'en-tête -------------------------------------------------
        let table_end = img.headers_end() - img.header_padding.len();
        units.push(mk(UnitKind::DosStub, None, 0, img.pe_offset, None));
        units.push(mk(
            UnitKind::PeHeaders,
            None,
            img.pe_offset,
            table_end - img.pe_offset,
            None,
        ));
        if !img.header_padding.is_empty() {
            units.push(mk(
                UnitKind::HeaderPad,
                None,
                table_end,
                img.header_padding.len(),
                None,
            ));
        }

        // --- sections ---------------------------------------------------------
        let (ranges, _stats) = pdata::scan(img);
        let mut merged = pdata::merge(&ranges);
        // Les feuilles viennent compléter `.pdata`, jamais le contredire : on
        // écarte tout ce qui recouvre une plage déjà décrite.
        if !extra.is_empty() {
            let covered = |rva: u32| {
                merged
                    .binary_search_by(|r| {
                        if rva < r.begin {
                            core::cmp::Ordering::Greater
                        } else if rva >= r.end {
                            core::cmp::Ordering::Less
                        } else {
                            core::cmp::Ordering::Equal
                        }
                    })
                    .is_ok()
            };
            let mut add: Vec<pdata::CodeRange> = extra
                .iter()
                .filter(|&&(b, l)| l > 0 && !covered(b) && !covered(b + l - 1))
                .map(|&(b, l)| pdata::CodeRange {
                    begin: b,
                    end: b + l,
                    unwind: 0,
                    chained: false,
                })
                .collect();
            merged.append(&mut add);
            merged.sort_unstable();
            merged.dedup_by_key(|r| r.begin);
        }

        let mut secs: Vec<_> = img
            .sections
            .iter()
            .filter(|s| s.size_raw > 0)
            .cloned()
            .collect();
        secs.sort_by_key(|s| s.ptr_raw);

        let mut cursor = img.headers_end();
        for s in &secs {
            let start = s.ptr_raw as usize;
            let end = start + s.size_raw as usize;
            if start < cursor {
                return Err(PeError::Cover(format!(
                    "section {} chevauche la région précédente ({start:#x} < {cursor:#x})",
                    s.name_str()
                )));
            }
            if end > total_len {
                return Err(PeError::Cover(format!(
                    "section {} déborde du fichier ({end:#x} > {total_len:#x})",
                    s.name_str()
                )));
            }
            if start > cursor {
                units.push(mk(UnitKind::Gap, None, cursor, start - cursor, None));
            }

            let name = s.name_str();
            let executable = s.characteristics & 0x2000_0020 != 0; // CNT_CODE | MEM_EXECUTE
            let in_sec: Vec<&CodeRange> = if executable {
                merged
                    .iter()
                    .filter(|r| s.contains_rva(r.begin) && !r.is_empty())
                    .collect()
            } else {
                Vec::new()
            };

            if in_sec.is_empty() {
                units.push(mk(
                    UnitKind::SectionData,
                    Some(&name),
                    start,
                    s.size_raw as usize,
                    Some(img.opt.image_base + u64::from(s.virtual_address)),
                ));
            } else {
                let ctx = SectionCtx {
                    name: &name,
                    file_start: start,
                    va: img.opt.image_base + u64::from(s.virtual_address),
                };
                let mut pos = start;
                for r in in_sec {
                    let Some(f_begin) = img.rva_to_offset(r.begin) else {
                        continue;
                    };
                    let f_end = (f_begin + r.len()).min(end);
                    if f_begin < pos || f_end <= f_begin {
                        continue; // chevauchement résiduel : déjà couvert
                    }
                    if f_begin > pos {
                        push_residue(&mut units, &mk, file, &ctx, pos, f_begin);
                    }
                    let kind = if r.chained {
                        UnitKind::CodeFragment
                    } else {
                        UnitKind::Function
                    };
                    units.push(mk(
                        kind,
                        Some(&name),
                        f_begin,
                        f_end - f_begin,
                        Some(img.opt.image_base + u64::from(r.begin)),
                    ));
                    pos = f_end;
                }
                if pos < end {
                    push_residue(&mut units, &mk, file, &ctx, pos, end);
                }
            }
            cursor = end;
        }

        if cursor < total_len {
            units.push(mk(
                UnitKind::Overlay,
                None,
                cursor,
                total_len - cursor,
                None,
            ));
        }

        let cover = Self {
            total_len,
            sha256: sha256_hex(file),
            units,
        };
        cover.validate()?;
        Ok(cover)
    }

    /// Vérifie que le recouvrement est total, contigu et sans doublon d'identifiant.
    ///
    /// # Erreurs
    /// Retourne une erreur décrivant la première rupture d'invariant rencontrée.
    pub fn validate(&self) -> Result<()> {
        let mut pos = 0usize;
        let mut seen = std::collections::HashSet::with_capacity(self.units.len());
        for u in &self.units {
            if u.file_off != pos {
                return Err(PeError::Cover(format!(
                    "trou/chevauchement à {pos:#x} : unité {} commence à {:#x}",
                    u.id, u.file_off
                )));
            }
            if u.len == 0 {
                return Err(PeError::Cover(format!("unité vide : {}", u.id)));
            }
            if !seen.insert(u.id.clone()) {
                return Err(PeError::Cover(format!("identifiant dupliqué : {}", u.id)));
            }
            pos += u.len;
        }
        if pos != self.total_len {
            return Err(PeError::Cover(format!(
                "recouvrement partiel : {pos:#x} octets couverts sur {:#x}",
                self.total_len
            )));
        }
        Ok(())
    }

    /// Réassemble le fichier en concaténant les charges utiles fournies.
    ///
    /// `fetch` doit rendre la charge utile de chaque unité ; sa longueur doit
    /// correspondre exactement à `unit.len`. Le fichier d'origine n'est jamais lu.
    ///
    /// # Erreurs
    /// Retourne une erreur si une charge utile manque ou n'a pas la bonne taille.
    pub fn assemble<F>(&self, mut fetch: F) -> Result<Vec<u8>>
    where
        F: FnMut(&Unit) -> Option<Vec<u8>>,
    {
        let mut out = Vec::with_capacity(self.total_len);
        for u in &self.units {
            let payload = fetch(u).ok_or_else(|| {
                PeError::Cover(format!("charge utile absente pour l'unité {}", u.id))
            })?;
            if payload.len() != u.len {
                return Err(PeError::Cover(format!(
                    "taille invalide pour {} : {} octets fournis, {} attendus",
                    u.id,
                    payload.len(),
                    u.len
                )));
            }
            out.extend_from_slice(&payload);
        }
        if out.len() != self.total_len {
            return Err(PeError::Cover(format!(
                "assemblage de {} octets, {} attendus",
                out.len(),
                self.total_len
            )));
        }
        Ok(out)
    }

    /// Nombre d'unités par nature.
    #[must_use]
    pub fn count_by_kind(&self, kind: UnitKind) -> usize {
        self.units.iter().filter(|u| u.kind == kind).count()
    }

    /// Masse d'octets par nature.
    #[must_use]
    pub fn bytes_by_kind(&self, kind: UnitKind) -> usize {
        self.units
            .iter()
            .filter(|u| u.kind == kind)
            .map(|u| u.len)
            .sum()
    }

    /// Retrouve une unité par identifiant.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Unit> {
        self.units.iter().find(|u| u.id == id)
    }

    /// Retrouve l'unité contenant une adresse virtuelle.
    #[must_use]
    pub fn find_va(&self, va: u64) -> Option<&Unit> {
        self.units
            .iter()
            .find(|u| u.va.is_some_and(|b| va >= b && va < b + u.len as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::PeImage;

    fn synth_image() -> PeImage {
        // Réutilise le générateur de l'unité `image` via un PE minimal à 1 section.
        let pe_off = 0x80usize;
        let opt_size = 240usize;
        let headers = 0x200usize;
        let mut b = vec![0u8; headers + 0x400];
        b[0..2].copy_from_slice(&0x5A4Du16.to_le_bytes());
        b[0x3c..0x40].copy_from_slice(&u32::try_from(pe_off).unwrap().to_le_bytes());
        b[pe_off..pe_off + 4].copy_from_slice(&0x0000_4550u32.to_le_bytes());
        let c = pe_off + 4;
        b[c..c + 2].copy_from_slice(&0x8664u16.to_le_bytes());
        b[c + 2..c + 4].copy_from_slice(&1u16.to_le_bytes());
        b[c + 16..c + 18].copy_from_slice(&u16::try_from(opt_size).unwrap().to_le_bytes());
        let o = c + 20;
        b[o..o + 2].copy_from_slice(&0x020Bu16.to_le_bytes());
        b[o + 24..o + 32].copy_from_slice(&0x1_4000_0000u64.to_le_bytes());
        b[o + 32..o + 36].copy_from_slice(&0x1000u32.to_le_bytes());
        b[o + 36..o + 40].copy_from_slice(&0x200u32.to_le_bytes());
        b[o + 60..o + 64].copy_from_slice(&u32::try_from(headers).unwrap().to_le_bytes());
        b[o + 108..o + 112].copy_from_slice(&16u32.to_le_bytes());
        let s = o + opt_size;
        b[s..s + 5].copy_from_slice(b".text");
        b[s + 8..s + 12].copy_from_slice(&0x400u32.to_le_bytes());
        b[s + 12..s + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        b[s + 16..s + 20].copy_from_slice(&0x400u32.to_le_bytes());
        b[s + 20..s + 24].copy_from_slice(&u32::try_from(headers).unwrap().to_le_bytes());
        b[s + 36..s + 40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        for (i, byte) in b[headers..headers + 0x400].iter_mut().enumerate() {
            *byte = u8::try_from(i % 251).unwrap();
        }
        PeImage::parse(b).expect("parse")
    }

    #[test]
    fn le_recouvrement_est_total_et_reassemble_a_l_identique() {
        let img = synth_image();
        let cover = Cover::split(&img).expect("split");
        cover.validate().expect("recouvrement total");
        assert_eq!(
            cover.units.iter().map(|u| u.len).sum::<usize>(),
            img.bytes.len()
        );
        let rebuilt = cover
            .assemble(|u| Some(img.bytes[u.range()].to_vec()))
            .expect("assemble");
        assert_eq!(rebuilt, img.bytes);
        assert_eq!(sha256_hex(&rebuilt), cover.sha256);
    }

    #[test]
    fn assemblage_refuse_une_charge_de_mauvaise_taille() {
        let img = synth_image();
        let cover = Cover::split(&img).expect("split");
        let err = cover
            .assemble(|u| {
                if u.kind == UnitKind::DosStub {
                    Some(vec![0u8; u.len + 1])
                } else {
                    Some(img.bytes[u.range()].to_vec())
                }
            })
            .unwrap_err();
        assert!(matches!(err, PeError::Cover(m) if m.contains("taille invalide")));
    }

    #[test]
    fn seul_le_bourrage_a_une_regle_de_regeneration() {
        let pad = Unit {
            id: "pad..text.0".into(),
            kind: UnitKind::Padding,
            section: Some(".text".into()),
            file_off: 0,
            len: 7,
            va: Some(0x1_4000_1000),
            sha256: String::new(),
        };
        assert_eq!(pad.emit_rule(), Some(vec![INT3; 7]));
        let hdrpad = Unit {
            kind: UnitKind::HeaderPad,
            ..pad.clone()
        };
        assert_eq!(hdrpad.emit_rule(), Some(vec![0u8; 7]));
        for kind in [
            UnitKind::Function,
            UnitKind::CodeResidue,
            UnitKind::SectionData,
            UnitKind::DosStub,
        ] {
            let u = Unit {
                kind,
                ..pad.clone()
            };
            assert_eq!(u.emit_rule(), None, "{kind:?} n'a pas de règle connue");
        }
    }

    #[test]
    fn une_regle_qui_se_trompe_ne_produit_rien() {
        // L'empreinte est le garde-fou : sans elle, une règle fausse gonflerait
        // la mesure sans que `build` ait son mot à dire.
        let menteuse = Unit {
            id: "pad..text.0".into(),
            kind: UnitKind::Padding,
            section: Some(".text".into()),
            file_off: 0,
            len: 4,
            va: None,
            sha256: sha256_hex(&[0x90, 0x90, 0x90, 0x90]),
        };
        assert_eq!(menteuse.emit_rule(), None);
        let fidele = Unit {
            sha256: sha256_hex(&[INT3; 4]),
            ..menteuse
        };
        assert_eq!(fidele.emit_rule(), Some(vec![INT3; 4]));
    }

    #[test]
    fn le_bourrage_se_reassemble_sans_consulter_la_reference() {
        // `.text` contenant deux corps séparés par un run d'`int3` : la règle
        // doit suffire à reconstituer le fichier à l'identique. Le bourrage
        // n'est isolé que dans une section où une plage de code est posée —
        // ici via `extra`, comme le fait l'échafaudage RE sur `nie.exe`.
        let mut img = synth_image();
        let text = img.bytes.len() - 0x400;
        for byte in img.bytes[text + 0x10..text + 0x1c].iter_mut() {
            *byte = INT3;
        }
        let img = PeImage::parse(img.bytes).expect("reparse");
        let cover = Cover::split_with(&img, &[(0x1000, 0x10)]).expect("split");
        let pads: Vec<_> = cover
            .units
            .iter()
            .filter(|u| u.kind == UnitKind::Padding)
            .collect();
        assert!(!pads.is_empty(), "le découpage doit isoler le bourrage");

        let rebuilt = cover
            .assemble(|u| {
                u.emit_rule()
                    .or_else(|| Some(img.bytes[u.range()].to_vec()))
            })
            .expect("assemble");
        assert_eq!(rebuilt, img.bytes, "identité préservée par la règle");
    }

    #[test]
    fn les_donnees_inline_scindent_le_corps_sans_trouer_le_recouvrement() {
        let img = synth_image();
        // Une fonction couvre RVA 0x1000..0x1100 ; on y declare une table de
        // sauts de 16 octets a RVA 0x1040.
        let cover =
            Cover::split_with_data(&img, &[(0x1000, 0x100)], &[(0x1040, 16)]).expect("split");
        cover.validate().expect("recouvrement total");

        let morceaux: Vec<_> = cover
            .units
            .iter()
            .filter(|u| {
                u.va.is_some_and(|v| (0x1_4000_1000..0x1_4000_1100).contains(&v))
            })
            .collect();
        let kinds: Vec<_> = morceaux.iter().map(|u| u.kind).collect();
        assert_eq!(
            kinds,
            vec![
                UnitKind::Function,
                UnitKind::InlineData,
                UnitKind::CodeFragment
            ],
            "code / donnees / reprise"
        );
        assert_eq!(morceaux[0].len, 0x40);
        assert_eq!(morceaux[1].len, 16);
        assert_eq!(morceaux[2].len, 0x100 - 0x40 - 16);
        assert_eq!(
            morceaux[0].id, "fn.140001000",
            "le premier morceau garde l'identite de l'unite"
        );
        assert!(morceaux.iter().map(|u| u.len).sum::<usize>() == 0x100);

        let rebuilt = cover
            .assemble(|u| Some(img.bytes[u.range()].to_vec()))
            .expect("assemble");
        assert_eq!(rebuilt, img.bytes);
    }

    #[test]
    fn une_plage_de_donnees_hors_du_code_est_ignoree() {
        let img = synth_image();
        // 0x8000 ne tombe dans aucune section : la plage doit etre ecartee
        // sans erreur, et le decoupage rester celui d'un split ordinaire.
        let a = Cover::split_with(&img, &[(0x1000, 0x100)]).expect("split");
        let b = Cover::split_with_data(&img, &[(0x1000, 0x100)], &[(0x8000, 16)]).expect("split");
        assert_eq!(a.units.len(), b.units.len());
    }

    #[test]
    fn validate_detecte_un_trou() {
        let img = synth_image();
        let mut cover = Cover::split(&img).expect("split");
        cover.units[0].len -= 1;
        assert!(cover.validate().is_err());
    }
}
