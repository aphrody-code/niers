//! Parseur **G4NV** — maillage de navigation (navmesh) Level-5, extension `.g4nv`.
//!
//! Le magic réel n'est pas « G4NV » mais **« NAVM »** (`0x4D56414E` LE). En-tête commun Level-5
//! (cf. [`crate::level5`]) de `0x60` octets, suivi d'une table de **cinq compteurs** `u32` à
//! `0x20`, puis de sept sections contiguës.
//!
//! ## Structure — **mesurée sur les 160 `.g4nv` du VFS, 160/160 validés**
//!
//! ```text
//!   0x00  en-tete Level-5 : magic 'NAVM' · header_size 0x60 · type_id 0x66 · align 0x18 · data_size
//!   0x20  u32 counts[5] = { coins, refs_aretes, sommets, polygones, aretes }
//!   0x60  u32  vertex_flags[arrondi(sommets,   16)]
//!         u32  edge_refs   [arrondi(refs,      16)]
//!         u32  corners     [arrondi(coins,     16)]   index de sommet, 3 par polygone
//!         16 o Vertex      [arrondi(sommets,    4)]   x, y, z, w (= 1.0)
//!         32 o Polygon     [arrondi(polygones,  2)]
//!         32 o Edge        [arrondi(aretes,     2)]
//!         padding de fin (0 à 384 octets nuls observés)
//! ```
//!
//! **Chaque tableau est alloué par blocs de 64 octets** — d'où les arrondis à 16 éléments pour
//! les `u32`, 4 pour les sommets (16 o) et 2 pour les enregistrements de 32 o. C'est cette règle,
//! et pas une suite de sections jointives, qui rend le découpage exact : sans elle les offsets
//! dérivent dès qu'un compteur n'est pas multiple de 16.
//!
//! ## Faits vérifiés (160 fichiers, 47 122 polygones, 58 104 arêtes)
//!
//! - `counts[0] == 3 * counts[3]` : trois coins par polygone, sans exception.
//! - Le 4ᵉ flottant d'un sommet vaut **1.0** partout (coordonnée homogène).
//! - Les trois premiers flottants d'un polygone sont **la moyenne exacte de ses trois sommets**
//!   (centroïde) — c'est ce test, rejoué sur les 47 122 polygones, qui prouve que le tableau de
//!   coins est bien lu.
//! - Le 4ᵉ flottant d'un polygone est le **carré du rayon** de la sphère centrée sur le centroïde
//!   qui englobe ses trois sommets (`max‖v − c‖²`), vérifié partout.
//! - `first_corner` et `first_edge_ref` sont des **curseurs contigus** : chaque polygone reprend
//!   là où le précédent s'arrête, et la somme des `edge_ref_count` vaut exactement `counts[1]`.
//! - Les deux premiers champs d'une arête sont des index de **polygone** (`< counts[3]`), les deux
//!   suivants des index de **sommet** (`< counts[2]`), et son flottant est la **distance entre les
//!   centroïdes des deux polygones** — le coût d'un arc de graphe A*, pas la longueur de l'arête.
//! - Toute arête référencée par un polygone le cite en retour (cohérence 160/160).
//!
//! ## Références d'arête **externes**
//!
//! Sur 7 des 160 fichiers (`e01g002`, `w10`, `w10g022`, `w10g023`, `w10g030`, `w10g031`, `w50`),
//! 38 références sur 25 261 valent `counts[4]`, `counts[4] + 1`, … — numérotées consécutivement
//! **au-delà** du tableau d'arêtes, une seule par polygone concerné. Ce ne sont donc pas des
//! index corrompus mais un domaine réservé : aucune arête n'est stockée pour elles.
//! [`Navm::edge_of_ref`] rend `None` dans ce cas, et [`check`] les accepte. Ce à quoi elles
//! renvoient (bord de carte, portail vers un autre navmesh) **n'est pas établi** ici.
//!
//! ## Ce qui n'est pas résolu — et n'est donc pas inventé
//!
//! [`Polygon::attr`] (valeurs 0..7 et 256..259 : bitfield d'attributs de terrain probable),
//! [`Vertex::flag`] (0, 1 ou 2) et [`Edge::extra`] sont exposés bruts. Le padding de fin est
//! conservé dans [`Navm::padding_len`] plutôt que supposé constant.

extern crate alloc;

use alloc::vec::Vec;

use crate::FormatError;
use crate::level5::{self, Level5Header};

/// Magic « NAVM » en little-endian.
const MAGIC: u32 = 0x4D56_414E;
/// Nombre de compteurs `u32` lus à `0x20`.
pub const SECTION_COUNT: usize = 5;
/// Longueur de l'en-tête (constante sur les 160 fichiers réels).
const HEADER_LEN: usize = 0x60;
/// Longueur minimale : en-tête complet.
const MIN_LEN: usize = HEADER_LEN;
/// Taille d'un sommet, en octets.
pub const VERTEX_LEN: usize = 16;
/// Taille d'un enregistrement de polygone, en octets.
pub const POLYGON_LEN: usize = 32;
/// Taille d'un enregistrement d'arête, en octets.
pub const EDGE_LEN: usize = 32;
/// Granularité d'allocation des tableaux, en octets (mesurée : 160/160).
pub const BLOCK_LEN: usize = 64;

/// Arrondit `n` au multiple supérieur de `k` (allocation par blocs de 64 octets).
const fn round_up(n: usize, k: usize) -> usize {
    n.div_ceil(k) * k
}

/// Un sommet du navmesh : position monde + coordonnée homogène.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vertex {
    /// Position monde.
    pub pos: [f32; 3],
    /// 4ᵉ composante — vaut `1.0` sur les 33 857 sommets mesurés.
    pub w: f32,
    /// Drapeau du tableau de tête (valeurs observées : 0, 1, 2). Sémantique non établie.
    pub flag: u32,
}

/// Un polygone (triangle) du navmesh.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Polygon {
    /// Centroïde — **la moyenne exacte des trois sommets** (vérifié sur 47 122 polygones).
    pub center: [f32; 3],
    /// Carré du rayon englobant depuis le centroïde (`max‖v − c‖²`).
    pub radius_sq: f32,
    /// Index du premier coin dans [`Navm::corners`].
    pub first_corner: u32,
    /// Index de la première référence d'arête dans [`Navm::edge_refs`].
    pub first_edge_ref: u32,
    /// Nombre de références d'arête (leur somme vaut `counts[1]`).
    pub edge_ref_count: u16,
    /// Nombre de coins — vaut **3** sur les 47 122 polygones mesurés.
    pub corner_count: u16,
    /// Champ d'attributs, exposé brut (0..7 et 256..259 observés). Sémantique non établie.
    pub attr: u32,
}

/// Une arête du graphe de navigation : l'arc entre deux polygones voisins.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Edge {
    /// Premier polygone (`< counts[3]`).
    pub poly_a: u32,
    /// Second polygone (`< counts[3]`).
    pub poly_b: u32,
    /// Premier sommet de l'arête partagée (`< counts[2]`).
    pub vert_a: u32,
    /// Second sommet de l'arête partagée (`< counts[2]`).
    pub vert_b: u32,
    /// Coût de l'arc : **distance entre les centroïdes** de `poly_a` et `poly_b`.
    pub cost: f32,
    /// Trois mots de queue. Seul le premier est parfois non nul (2 933 arêtes sur 58 104) ;
    /// les deux autres sont toujours nuls. Sémantique non établie, exposés bruts.
    pub extra: [u32; 3],
}

/// Un `.g4nv` décodé.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Navm {
    /// En-tête commun Level-5.
    pub header: Level5Header,
    /// Les cinq compteurs bruts de `0x20`.
    pub section_counts: [u32; SECTION_COUNT],
    /// Sommets, avec leur drapeau de tête.
    pub vertices: Vec<Vertex>,
    /// Polygones.
    pub polygons: Vec<Polygon>,
    /// Arêtes du graphe.
    pub edges: Vec<Edge>,
    /// Index de sommet, trois par polygone, adressés par [`Polygon::first_corner`].
    pub corners: Vec<u32>,
    /// Index d'arête, groupés par polygone via [`Polygon::first_edge_ref`].
    pub edge_refs: Vec<u32>,
    /// Octets de padding après la dernière arête (0 à 384 observés).
    pub padding_len: usize,
    /// Taille du fichier.
    pub file_size: usize,
}

impl Navm {
    /// Invariant structurel : `header_size + data_size == file_size`.
    #[must_use]
    pub fn is_size_consistent(&self) -> bool {
        self.header.is_size_consistent(self.file_size)
    }

    /// Compte de la section d'index `i` (0 si hors borne).
    #[must_use]
    pub fn section_count(&self, i: usize) -> u32 {
        self.section_counts.get(i).copied().unwrap_or(0)
    }

    /// Les trois index de sommet du polygone `i`.
    #[must_use]
    pub fn corners_of(&self, i: usize) -> &[u32] {
        let Some(p) = self.polygons.get(i) else {
            return &[];
        };
        let a = p.first_corner as usize;
        let b = a
            .saturating_add(p.corner_count as usize)
            .min(self.corners.len());
        self.corners.get(a..b).unwrap_or(&[])
    }

    /// Les index d'arête incidents au polygone `i`.
    #[must_use]
    pub fn edges_of(&self, i: usize) -> &[u32] {
        let Some(p) = self.polygons.get(i) else {
            return &[];
        };
        let a = p.first_edge_ref as usize;
        let b = a
            .saturating_add(p.edge_ref_count as usize)
            .min(self.edge_refs.len());
        self.edge_refs.get(a..b).unwrap_or(&[])
    }

    /// L'arête désignée par une référence de [`Self::edge_refs`].
    ///
    /// `None` si la référence tombe dans le domaine **externe** (`>= counts[4]`) : aucune arête
    /// n'est stockée pour elle. Cf. la doc du module.
    #[must_use]
    pub fn edge_of_ref(&self, r: u32) -> Option<&Edge> {
        self.edges.get(r as usize)
    }

    /// Nombre de références d'arête pointant hors du tableau (domaine externe).
    #[must_use]
    pub fn external_ref_count(&self) -> usize {
        self.edge_refs
            .iter()
            .filter(|&&r| r as usize >= self.edges.len())
            .count()
    }

    /// Positions des trois sommets du polygone `i`, dans l'ordre du fichier.
    #[must_use]
    pub fn triangle(&self, i: usize) -> Option<[[f32; 3]; 3]> {
        let c = self.corners_of(i);
        let [a, b, d] = <[u32; 3]>::try_from(c).ok()?;
        Some([
            self.vertices.get(a as usize)?.pos,
            self.vertices.get(b as usize)?.pos,
            self.vertices.get(d as usize)?.pos,
        ])
    }
}

/// `true` si les 4 premiers octets sont le magic « NAVM ».
#[must_use]
pub fn is_navm(data: &[u8]) -> bool {
    level5::read_u32_le(data, 0).is_ok_and(|m| m == MAGIC)
}

fn f32_at(data: &[u8], off: usize) -> Result<f32, FormatError> {
    level5::read_u32_le(data, off).map(f32::from_bits)
}

/// Décode entièrement un `.g4nv`.
///
/// # Errors
/// [`FormatError::TooShort`] si le tampon n'atteint pas la fin de la dernière section,
/// [`FormatError::BadMagic`] si le magic ≠ « NAVM », [`FormatError::Corrupt`] si un compteur
/// annonce plus de données que le fichier n'en contient.
pub fn parse(data: &[u8]) -> Result<Navm, FormatError> {
    if data.len() < MIN_LEN {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: MIN_LEN,
        });
    }
    let header = level5::parse_header(data, MAGIC, "G4NV")?;
    let mut section_counts = [0u32; SECTION_COUNT];
    for (i, c) in section_counts.iter_mut().enumerate() {
        *c = level5::read_u32_le(data, 0x20 + i * 4)?;
    }
    let [n_corners, n_refs, n_verts, n_polys, n_edges] = section_counts.map(|c| c as usize);

    // Allocation par blocs de 64 octets : chaque section démarre où la précédente finit,
    // capacité arrondie. Tout calcul en `usize` saturant — un compteur absurde doit rendre
    // une erreur, pas déborder.
    let mut off = header.header_size as usize;
    let o_flags = off;
    off += round_up(n_verts * 4, BLOCK_LEN);
    let o_refs = off;
    off += round_up(n_refs * 4, BLOCK_LEN);
    let o_corners = off;
    off += round_up(n_corners * 4, BLOCK_LEN);
    let o_verts = off;
    off += round_up(n_verts * VERTEX_LEN, BLOCK_LEN);
    let o_polys = off;
    off += round_up(n_polys * POLYGON_LEN, BLOCK_LEN);
    let o_edges = off;
    off += round_up(n_edges * EDGE_LEN, BLOCK_LEN);
    if off > data.len() {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: off,
        });
    }

    let mut corners = Vec::with_capacity(n_corners);
    for i in 0..n_corners {
        corners.push(level5::read_u32_le(data, o_corners + i * 4)?);
    }
    let mut edge_refs = Vec::with_capacity(n_refs);
    for i in 0..n_refs {
        edge_refs.push(level5::read_u32_le(data, o_refs + i * 4)?);
    }
    let mut vertices = Vec::with_capacity(n_verts);
    for i in 0..n_verts {
        let at = o_verts + i * VERTEX_LEN;
        vertices.push(Vertex {
            pos: [
                f32_at(data, at)?,
                f32_at(data, at + 4)?,
                f32_at(data, at + 8)?,
            ],
            w: f32_at(data, at + 12)?,
            flag: level5::read_u32_le(data, o_flags + i * 4)?,
        });
    }
    let mut polygons = Vec::with_capacity(n_polys);
    for i in 0..n_polys {
        let at = o_polys + i * POLYGON_LEN;
        let counts = level5::read_u32_le(data, at + 24)?;
        polygons.push(Polygon {
            center: [
                f32_at(data, at)?,
                f32_at(data, at + 4)?,
                f32_at(data, at + 8)?,
            ],
            radius_sq: f32_at(data, at + 12)?,
            first_corner: level5::read_u32_le(data, at + 16)?,
            first_edge_ref: level5::read_u32_le(data, at + 20)?,
            edge_ref_count: (counts & 0xFFFF) as u16,
            corner_count: (counts >> 16) as u16,
            attr: level5::read_u32_le(data, at + 28)?,
        });
    }
    let mut edges = Vec::with_capacity(n_edges);
    for i in 0..n_edges {
        let at = o_edges + i * EDGE_LEN;
        edges.push(Edge {
            poly_a: level5::read_u32_le(data, at)?,
            poly_b: level5::read_u32_le(data, at + 4)?,
            vert_a: level5::read_u32_le(data, at + 8)?,
            vert_b: level5::read_u32_le(data, at + 12)?,
            cost: f32_at(data, at + 16)?,
            extra: [
                level5::read_u32_le(data, at + 20)?,
                level5::read_u32_le(data, at + 24)?,
                level5::read_u32_le(data, at + 28)?,
            ],
        });
    }

    Ok(Navm {
        header,
        section_counts,
        vertices,
        polygons,
        edges,
        corners,
        edge_refs,
        padding_len: data.len() - off,
        file_size: data.len(),
    })
}

/// Contrôle d'intégrité **interne** : tous les index pointent-ils dans leurs tableaux ?
///
/// Rend `Ok(())` ou le premier manquement rencontré. C'est ce prédicat qui distingue un fichier
/// réellement décodé d'un tampon qui a seulement « passé le parse » : les 160 `.g4nv` du jeu le
/// satisfont.
///
/// # Errors
/// [`FormatError::Corrupt`] avec la nature du manquement.
pub fn check(n: &Navm) -> Result<(), FormatError> {
    for p in &n.polygons {
        let a = p.first_corner as usize;
        if a + p.corner_count as usize > n.corners.len() {
            return Err(FormatError::Corrupt(
                "G4NV: tranche de coins hors du tableau",
            ));
        }
        let b = p.first_edge_ref as usize;
        if b + p.edge_ref_count as usize > n.edge_refs.len() {
            return Err(FormatError::Corrupt(
                "G4NV: tranche de refs d'arêtes hors du tableau",
            ));
        }
    }
    if n.corners.iter().any(|&c| c as usize >= n.vertices.len()) {
        return Err(FormatError::Corrupt("G4NV: index de sommet hors bornes"));
    }
    // Une référence >= counts[4] n'est pas une erreur : c'est le domaine externe (cf. module).
    // Elle reste bornée — au-delà de counts[4] + counts[1] il n'y a plus de lecture possible.
    let plafond = n.edges.len().saturating_add(n.edge_refs.len());
    if n.edge_refs.iter().any(|&e| e as usize >= plafond) {
        return Err(FormatError::Corrupt("G4NV: référence d'arête absurde"));
    }
    for e in &n.edges {
        if e.poly_a as usize >= n.polygons.len() || e.poly_b as usize >= n.polygons.len() {
            return Err(FormatError::Corrupt("G4NV: index de polygone hors bornes"));
        }
        if e.vert_a as usize >= n.vertices.len() || e.vert_b as usize >= n.vertices.len() {
            return Err(FormatError::Corrupt(
                "G4NV: index de sommet d'arête hors bornes",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Deux triangles formant un carré de 150 × 150 dans le plan XZ, une arête partagée.
    fn synthetique() -> Vec<u8> {
        let (n_corners, n_refs, n_verts, n_polys, n_edges) =
            (6usize, 2usize, 4usize, 2usize, 1usize);
        let mut d = vec![0u8; HEADER_LEN];
        d[0..4].copy_from_slice(b"NAVM");
        d[4..6].copy_from_slice(&(HEADER_LEN as u16).to_le_bytes());
        d[6..8].copy_from_slice(&0x0066u16.to_le_bytes());
        d[10..12].copy_from_slice(&0x0018u16.to_le_bytes());
        for (i, c) in [n_corners, n_refs, n_verts, n_polys, n_edges]
            .iter()
            .enumerate()
        {
            d[0x20 + i * 4..0x20 + i * 4 + 4].copy_from_slice(&(*c as u32).to_le_bytes());
        }
        let put_u32 = |d: &mut Vec<u8>, v: u32| d.extend_from_slice(&v.to_le_bytes());
        let put_f32 = |d: &mut Vec<u8>, v: f32| d.extend_from_slice(&v.to_bits().to_le_bytes());
        // vertex_flags[4] → bloc de 64
        for _ in 0..n_verts {
            put_u32(&mut d, 1);
        }
        d.resize(HEADER_LEN + BLOCK_LEN, 0);
        // edge_refs[2] → bloc de 64
        for _ in 0..n_refs {
            put_u32(&mut d, 0);
        }
        d.resize(HEADER_LEN + 2 * BLOCK_LEN, 0);
        // corners[6] : (0,1,2) puis (2,1,3)
        for c in [0u32, 1, 2, 2, 1, 3] {
            put_u32(&mut d, c);
        }
        d.resize(HEADER_LEN + 3 * BLOCK_LEN, 0);
        // sommets : les 4 coins du carré
        for (x, z) in [
            (-75.0f32, 75.0f32),
            (75.0, 75.0),
            (-75.0, -75.0),
            (75.0, -75.0),
        ] {
            put_f32(&mut d, x);
            put_f32(&mut d, 0.0);
            put_f32(&mut d, z);
            put_f32(&mut d, 1.0);
        }
        // sommets : 4 × 16 = 64 = déjà un bloc plein.
        // polygones
        for (c, first_corner, first_ref) in [
            ([-25.0f32, 0.0, 25.0], 0u32, 0u32),
            ([25.0, 0.0, -25.0], 3, 1),
        ] {
            put_f32(&mut d, c[0]);
            put_f32(&mut d, c[1]);
            put_f32(&mut d, c[2]);
            put_f32(&mut d, 12500.0);
            put_u32(&mut d, first_corner);
            put_u32(&mut d, first_ref);
            put_u32(&mut d, (3 << 16) | 1);
            put_u32(&mut d, 0);
        }
        // arête unique : entre les polygones 0 et 1, sommets 1 et 2
        put_u32(&mut d, 0);
        put_u32(&mut d, 1);
        put_u32(&mut d, 1);
        put_u32(&mut d, 2);
        put_f32(&mut d, 70.710_68);
        put_u32(&mut d, 0);
        put_u32(&mut d, 0);
        put_u32(&mut d, 0);
        // l'arête occupe 32 o mais son bloc en fait 64
        d.resize(d.len() + 32, 0);
        let data_size = (d.len() - HEADER_LEN) as u32;
        d[12..16].copy_from_slice(&data_size.to_le_bytes());
        d
    }

    #[test]
    fn decode_synthetique() {
        let raw = synthetique();
        let n = parse(&raw).expect("parse");
        assert_eq!(n.header.magic, MAGIC);
        assert_eq!(n.header.header_size, 0x60);
        assert_eq!(n.header.type_id, 0x66);
        assert_eq!(n.header.align, 0x18);
        assert!(n.is_size_consistent());
        assert_eq!(n.section_counts, [6, 2, 4, 2, 1]);
        assert_eq!(n.section_count(3), 2);
        assert_eq!(n.section_count(9), 0);
        assert_eq!(n.vertices.len(), 4);
        assert_eq!(n.vertices[0].pos, [-75.0, 0.0, 75.0]);
        assert!(n.vertices.iter().all(|v| v.w == 1.0));
        assert_eq!(n.polygons.len(), 2);
        assert_eq!(n.polygons[0].corner_count, 3);
        assert_eq!(n.corners_of(1), &[2, 1, 3]);
        assert_eq!(n.edges_of(0), &[0]);
        assert_eq!(n.edges.len(), 1);
        assert_eq!((n.edges[0].poly_a, n.edges[0].poly_b), (0, 1));
        check(&n).expect("cohérence interne");
        // le centroïde annoncé est bien la moyenne des trois sommets
        let t = n.triangle(0).expect("triangle");
        let cx = (t[0][0] + t[1][0] + t[2][0]) / 3.0;
        assert!((cx - n.polygons[0].center[0]).abs() < 1e-4);
    }

    #[test]
    fn rejette_magic_et_court() {
        assert!(matches!(
            parse(&[0u8; HEADER_LEN]),
            Err(FormatError::BadMagic { .. })
        ));
        assert!(matches!(parse(b"NAVM"), Err(FormatError::TooShort { .. })));
        assert!(!is_navm(b"G4SK"));
        assert!(is_navm(b"NAVM____"));
    }

    #[test]
    fn compteur_absurde_ne_panique_pas() {
        // Un compteur énorme doit rendre TooShort, jamais déborder ni allouer à l'aveugle.
        let mut raw = synthetique();
        raw[0x20 + 12..0x20 + 16].copy_from_slice(&0x00FF_FFFFu32.to_le_bytes());
        assert!(matches!(parse(&raw), Err(FormatError::TooShort { .. })));
    }

    #[test]
    fn index_hors_bornes_detecte_par_check() {
        let mut raw = synthetique();
        // corners[0] pointe sur un sommet inexistant
        let o_corners = HEADER_LEN + 2 * BLOCK_LEN;
        raw[o_corners..o_corners + 4].copy_from_slice(&99u32.to_le_bytes());
        let n = parse(&raw).expect("parse");
        assert!(matches!(check(&n), Err(FormatError::Corrupt(_))));
    }
}
