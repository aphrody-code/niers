//! CRILAYLA — détection magic + décompresseur complet.
//!
//! Port du décodeur C# `IECODE.Core.Formats.Criware.CriFs.Compression.CriLayla`
//! (source : `iecode/src/IECODE.Core/Formats/Criware/CriFs/Compression/CriLayla.cs`).
//!
//! ## Format de fichier
//!
//! ```text
//! Offset  Taille  Champ
//!  0x00     8     Magic ASCII "CRILAYLA"
//!  0x08     4     uncompressed_size  (u32 LE) — taille des données compressées décompressées
//!  0x0C     4     header_offset      (u32 LE) — offset du bloc raw depuis 0x10
//!  0x10     …     bitstream compressé (LZ, lu depuis la FIN du bitstream vers le début)
//!  0x10 + header_offset   0x100  bloc non-compressé (→ début du résultat)
//! ```
//!
//! La sortie totale a pour taille `uncompressed_size + 0x100`.
//! Les 0x100 premiers octets du résultat = bloc raw (copié tel quel depuis la fin du fichier).
//! Le reste est reconstruit par le bitstream LZ lu à rebours (MSB-first par octet).
//!
//! ## Algorithme LZ
//!
//! Le bitstream est lu de droite à gauche (on recule dans le fichier).
//! Chaque octet est consommé du MSB vers le LSB.
//!
//! Chaque symbole :
//! - flag bit = 0 → octet littéral (8 bits)
//! - flag bit = 1 → backref : 13 bits d'offset + longueur Fibonacci (2+3+5+8+…)
//!
//! L'écriture de la sortie va de la FIN vers le début (écriture à rebours dans output).
//! La copie backref utilise des positions PLUS GRANDES que write_pos (déjà écrites).
//!
//! ## Port de `CriLayla.cs`
//!
//! La sémantique exacte des helpers C# est portée :
//! - `GetNextBit` : recule l'octet courant si épuisé, retourne 1 bit.
//! - `Read8` : recule TOUJOURS d'un octet, puis lit 8 bits (éventuellement à cheval).
//! - `Read2` : lit 2 bits (fast-path si ≥ 2 bits dispo, sinon croise 2 octets).
//! - `ReadMax8(n)` : lit n bits (1 ≤ n ≤ 7) depuis l'octet courant avec avance si épuisé.
//! - `Read13` : lit 13 bits en 2-3 octets.
//!
//! ## `std`
//!
//! Ce module utilise `alloc` (via `Vec`). Compatible no_std+alloc.

extern crate alloc;
use alloc::vec::Vec;

use crate::FormatError;

/// Préfixe magic CRILAYLA (8 octets ASCII).
pub const MAGIC: &[u8; 8] = b"CRILAYLA";

/// Taille du bloc non-compressé en fin de fichier (toujours 0x100 octets).
pub const RAW_HEADER_SIZE: usize = 0x100;

/// Longueur minimale d'une copie LZ.
pub const MIN_COPY_LEN: usize = 3;

/// Taille minimale d'un fichier CRILAYLA valide :
/// 8 (magic) + 4 (uncompressed_size) + 4 (header_offset) + 0x100 (raw block) = 0x110.
pub const MIN_FILE_SIZE: usize = 0x10 + RAW_HEADER_SIZE;

// ---------------------------------------------------------------------------
// API publique
// ---------------------------------------------------------------------------

/// Vrai si `buf` commence par le magic CRILAYLA.
#[must_use]
pub fn has_magic(buf: &[u8]) -> bool {
    buf.starts_with(MAGIC)
}

/// Décompresse un tampon CRILAYLA et retourne les données brutes.
///
/// `input` doit commencer exactement au magic `CRILAYLA` (offset 0x00).
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si le tampon est plus court que le minimum attendu.
/// - [`FormatError::BadMagic`] si les 8 premiers octets ne sont pas `CRILAYLA`.
/// - [`FormatError::Corrupt`] si les offsets internes dépassent les bornes.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, FormatError> {
    // Vérification de longueur minimale.
    if input.len() < MIN_FILE_SIZE {
        return Err(FormatError::TooShort {
            got: input.len(),
            need: MIN_FILE_SIZE,
        });
    }

    // Vérification du magic.
    if !has_magic(input) {
        return Err(FormatError::BadMagic { format: "CRILAYLA" });
    }

    // Lecture du header (LE).
    let uncompressed_size = u32::from_le_bytes([input[8], input[9], input[10], input[11]]) as usize;
    let header_offset = u32::from_le_bytes([input[12], input[13], input[14], input[15]]) as usize;

    // Le bloc raw (0x100 octets) se trouve à input[0x10 + header_offset].
    let raw_block_start = 0x10usize
        .checked_add(header_offset)
        .ok_or(FormatError::Corrupt("CRILAYLA : overflow header_offset"))?;
    let raw_block_end = raw_block_start
        .checked_add(RAW_HEADER_SIZE)
        .ok_or(FormatError::Corrupt("CRILAYLA : overflow raw block end"))?;
    if raw_block_end > input.len() {
        return Err(FormatError::Corrupt("CRILAYLA : raw block hors limites"));
    }

    // Allocation du tampon résultat.
    let total_size = RAW_HEADER_SIZE
        .checked_add(uncompressed_size)
        .ok_or(FormatError::Corrupt("CRILAYLA : overflow total_size"))?;
    let mut output = alloc::vec![0u8; total_size];

    // Les 0x100 premiers octets = bloc raw (copié au début de la sortie).
    output[..RAW_HEADER_SIZE].copy_from_slice(&input[raw_block_start..raw_block_end]);

    if uncompressed_size == 0 {
        return Ok(output);
    }

    // -----------------------------------------------------------------------
    // Bitstream : lit depuis `raw_block_start - 1` en remontant vers `0x10`.
    // -----------------------------------------------------------------------
    //
    // `br.ptr` = index de l'OCTET COURANT dans `input` (on commence juste AVANT le raw block).
    // `br.bits_left` = bits disponibles dans l'octet courant (MSB-first, de 0 à 7).
    //
    // GetNextBit : si bits_left == 0 → recule d'un octet (charge le nouveau), bits_left=7 ;
    //              sinon bits_left--. Retourne (byte >> bits_left) & 1.
    //
    // Read8 : recule TOUJOURS d'un octet. Si bits_left == 0 → 8 bits directs du nouvel octet.
    //         Sinon → reconstruction à cheval (high = bits restants de l'ancien octet,
    //                 low = premiers bits du nouvel octet).
    //
    // Read2, ReadMax8(n), Read13 : opérations composites documentées inline.

    let mut br = BitReader::new(raw_block_start, input);

    // `write_pos` : prochain index à écrire dans output (descend de total_size-1 vers RAW_HEADER_SIZE).
    let mut write_pos = total_size;

    while write_pos > RAW_HEADER_SIZE {
        let flag = br.get_next_bit(input)?;
        if flag == 0 {
            // Octet littéral.
            let byte = br.read8(input)?;
            write_pos -= 1;
            output[write_pos] = byte;
        } else {
            // Backref LZ.
            // offset (13 bits) : distance depuis write_pos vers la source (>= MIN_COPY_LEN).
            let raw_offset = br.read13(input)? as usize;
            let offset = raw_offset + MIN_COPY_LEN;

            // Longueur Fibonacci (2 + 3 + 5 + 8+...).
            let mut length = MIN_COPY_LEN;

            let lvl2 = br.read_max8(2, input)? as usize;
            length += lvl2;
            if lvl2 == (1 << 2) - 1 {
                let lvl3 = br.read_max8(3, input)? as usize;
                length += lvl3;
                if lvl3 == (1 << 3) - 1 {
                    let lvl5 = br.read_max8(5, input)? as usize;
                    length += lvl5;
                    if lvl5 == (1 << 5) - 1 {
                        loop {
                            let lvl8 = br.read8(input)? as usize;
                            length += lvl8;
                            if lvl8 != 0xFF {
                                break;
                            }
                        }
                    }
                }
            }

            // Copie LZ : les octets source sont à (write_pos - 1) + offset (déjà écrits).
            //
            // Sémantique : dans le C# `*writePtr = writePtr[offset]; writePtr--`.
            // En Rust, `write_pos` représente la prochaine position D'ÉCRITURE (celle qui
            // sera décrémentée en premier), équivalent à `writePtr + 1` du C#.
            // Donc la source correcte est `(write_pos - 1) + offset`, soit `write_pos - 1`
            // APRÈS décrément de `write_pos`.
            //
            // Les bornes se vérifient **une fois pour le bloc** et non à chaque octet : la
            // version par octet payait trois tests (fin de sortie, débordement d'addition,
            // source hors limites) pour un seul octet copié.
            if length > write_pos - RAW_HEADER_SIZE {
                return Err(FormatError::Corrupt(
                    "CRILAYLA : backref dépasse le début de la sortie",
                ));
            }
            let dst_start = write_pos - length;
            let src_end = write_pos
                .checked_add(offset)
                .ok_or(FormatError::Corrupt("CRILAYLA : overflow backref src"))?;
            if src_end > total_size {
                return Err(FormatError::Corrupt(
                    "CRILAYLA : backref source hors limites",
                ));
            }

            if offset >= length {
                // Source et destination disjointes : aucun octet lu n'est réécrit pendant la
                // copie, `copy_within` (memmove) est donc équivalent à la boucle — et bien
                // plus rapide sur les longues copies.
                output.copy_within(src_end - length..src_end, dst_start);
            } else {
                // Recouvrement : la copie est *périodique* (l'octet lu vient d'être écrit),
                // ce qu'un memmove ne reproduit pas. L'ordre décroissant est la sémantique.
                for i in (dst_start..write_pos).rev() {
                    output[i] = output[i + offset];
                }
            }
            write_pos = dst_start;
        }
    }

    Ok(output)
}

// ---------------------------------------------------------------------------
// BitReader — port exact des helpers C# de CriLayla.cs
// ---------------------------------------------------------------------------

/// Lecteur de bitstream inversé (lit depuis la fin du fichier vers le début, MSB-first par octet).
///
/// Port de `CriLayla.cs` : `compressedDataPtr` = `ptr`, `bitsLeft` = `bits_left`.
struct BitReader {
    /// Index de l'octet courant dans `input`.
    ptr: usize,
    /// Nombre de bits encore disponibles dans l'octet courant (de 0 à 7).
    bits_left: u32,
}

impl BitReader {
    /// Initialise le lecteur juste avant `raw_block_start`.
    ///
    /// Dans le C# : `compressedDataPtr = uncompressedDataPtr` (pointe sur le raw block),
    /// `bitsTillNextByte = 0`. Les premiers appels reculeront vers `raw_block_start - 1`.
    fn new(raw_block_start: usize, _input: &[u8]) -> Self {
        Self {
            ptr: raw_block_start,
            bits_left: 0,
        }
    }

    /// `GetNextBit` : lit 1 bit. Si épuisé (`bits_left == 0`), recule et charge l'octet suivant.
    fn get_next_bit(&mut self, input: &[u8]) -> Result<u8, FormatError> {
        if self.bits_left == 0 {
            if self.ptr == 0x10 {
                return Err(FormatError::Corrupt(
                    "CRILAYLA : bitstream épuisé (get_next_bit)",
                ));
            }
            self.ptr -= 1;
            self.bits_left = 7;
        } else {
            self.bits_left -= 1;
        }
        Ok((input[self.ptr] >> self.bits_left) & 1)
    }

    /// `Read8` : recule TOUJOURS d'un octet, lit 8 bits (éventuellement à cheval sur 2 octets).
    ///
    /// Port exact de `CriLayla.cs::Read8` :
    /// ```text
    /// compressedDataPtr--;
    /// if (bitsLeft != 0) {
    ///     int extraBitCount = 8 - bitsLeft;
    ///     int high = *(compressedDataPtr + 1) & ((1 << bitsLeft) - 1);
    ///     var low = (*(compressedDataPtr) >> (8 - extraBitCount)) & ((1 << extraBitCount) - 1);
    ///     return (byte)(high << extraBitCount | low);
    /// }
    /// return *compressedDataPtr;
    /// ```
    fn read8(&mut self, input: &[u8]) -> Result<u8, FormatError> {
        if self.ptr == 0x10 {
            return Err(FormatError::Corrupt(
                "CRILAYLA : bitstream épuisé (read8 avant recul)",
            ));
        }
        self.ptr -= 1;
        if self.bits_left != 0 {
            // `bits_left` bits disponibles dans l'ancien octet (ptr+1) → high.
            // `extra = 8 - bits_left` bits depuis les MSBs du nouvel octet (ptr) → low.
            // Note : `(input[ptr] >> (8 - extra)) = (input[ptr] >> bits_left)`.
            let extra = 8 - self.bits_left;
            let high = (input[self.ptr + 1] & ((1u8 << self.bits_left) - 1)) as u32;
            // low = (input[ptr] >> (8 - extra)) & ((1 << extra) - 1)
            //     = (input[ptr] >> bits_left) & ((1 << extra) - 1)
            let low = (input[self.ptr] >> self.bits_left) as u32 & ((1u32 << extra) - 1);
            Ok(((high << extra) | low) as u8)
        } else {
            // Aligné : 8 bits directs de l'octet courant.
            Ok(input[self.ptr])
        }
    }

    /// `Read2` : lit 2 bits.
    ///
    /// Fast-path si ≥ 2 bits disponibles (ou si bits_left == 0 → charge un nouvel octet).
    /// Slow-path si bits_left == 1 (croisement d'octets).
    ///
    /// Conservé pour complétion documentaire (le décompresseur utilise `read_max8(2, …)`
    /// qui a la même sémantique mais est générique).
    #[allow(dead_code)]
    fn read2(&mut self, input: &[u8]) -> Result<u8, FormatError> {
        const N: u32 = 2;
        if self.bits_left >= N || self.bits_left == 0 {
            // Fast-path (branchless dans le C#, branché ici pour la lisibilité).
            if self.bits_left == 0 {
                if self.ptr == 0x10 {
                    return Err(FormatError::Corrupt("CRILAYLA : bitstream épuisé (read2)"));
                }
                self.ptr -= 1;
                self.bits_left = 8;
            }
            self.bits_left -= N;
            Ok((input[self.ptr] >> self.bits_left) & 0b11)
        } else {
            // bits_left == 1 : 1 bit de l'octet courant + 1 bit du suivant.
            let high = input[self.ptr] & 1;
            if self.ptr == 0x10 {
                return Err(FormatError::Corrupt(
                    "CRILAYLA : bitstream épuisé (read2 cross)",
                ));
            }
            self.ptr -= 1;
            let low = (input[self.ptr] >> 7) & 1;
            self.bits_left = 7;
            Ok((high << 1) | low)
        }
    }

    /// `ReadMax8(bit_count)` : lit `bit_count` bits (1 ≤ n ≤ 7).
    ///
    /// Si `bits_left == 0`, charge un nouvel octet. Peut croiser 2 octets si nécessaire.
    fn read_max8(&mut self, bit_count: u32, input: &[u8]) -> Result<u8, FormatError> {
        debug_assert!((1..=7).contains(&bit_count));
        // Charger si épuisé.
        if self.bits_left == 0 {
            if self.ptr == 0x10 {
                return Err(FormatError::Corrupt(
                    "CRILAYLA : bitstream épuisé (read_max8 init)",
                ));
            }
            self.ptr -= 1;
            self.bits_left = 8;
        }

        let take1 = self.bits_left.min(bit_count);
        self.bits_left -= take1;
        let result = (input[self.ptr] >> self.bits_left) & ((1u8 << take1) - 1);
        let remaining = bit_count - take1;

        if remaining == 0 {
            return Ok(result);
        }

        // Croisement nécessaire (bits_left était < bit_count).
        if self.ptr == 0x10 {
            return Err(FormatError::Corrupt(
                "CRILAYLA : bitstream épuisé (read_max8 cross)",
            ));
        }
        self.ptr -= 1;
        self.bits_left = 8;

        let take2 = remaining;
        self.bits_left -= take2;
        let low = (input[self.ptr] >> self.bits_left) & ((1u8 << take2) - 1);
        Ok((result << take2) | low)
    }

    /// `Read13` : lit 13 bits.
    ///
    /// Port exact de `CriLayla.cs::Read13`. Toujours consomme 2 ou 3 octets.
    fn read13(&mut self, input: &[u8]) -> Result<u16, FormatError> {
        let mut bit_count = 13u32;

        // Premier octet.
        if self.bits_left == 0 {
            if self.ptr == 0x10 {
                return Err(FormatError::Corrupt(
                    "CRILAYLA : bitstream épuisé (read13 init)",
                ));
            }
            self.ptr -= 1;
            self.bits_left = 8;
        }

        let take1 = self.bits_left.min(bit_count);
        // take1 peut valoir 8 : utiliser u16 pour éviter l'overflow de (1u8 << 8).
        let mask1 = if take1 >= 8 {
            0xFFu8
        } else {
            (1u8 << take1) - 1
        };
        let result1 = (input[self.ptr] >> (self.bits_left - take1)) & mask1;
        bit_count -= take1;

        // Avancer vers l'octet suivant.
        self.bits_left = 8;
        if self.ptr == 0x10 {
            return Err(FormatError::Corrupt(
                "CRILAYLA : bitstream épuisé (read13 octet 2)",
            ));
        }
        self.ptr -= 1;

        let take2 = self.bits_left.min(bit_count);
        let mask2 = if take2 >= 8 {
            0xFFu8
        } else {
            (1u8 << take2) - 1
        };
        let result2 = (input[self.ptr] >> (self.bits_left - take2)) & mask2;
        bit_count -= take2;

        let mut result = ((result1 as u16) << take2) | result2 as u16;

        if bit_count == 0 {
            self.bits_left -= take2;
            return Ok(result);
        }

        // Troisième octet si nécessaire.
        self.bits_left = 8;
        if self.ptr == 0x10 {
            return Err(FormatError::Corrupt(
                "CRILAYLA : bitstream épuisé (read13 octet 3)",
            ));
        }
        self.ptr -= 1;

        let take3 = bit_count;
        let mask3: u16 = if take3 >= 16 {
            0xFFFF
        } else {
            (1u16 << take3) - 1
        };
        result = (result << take3) | ((input[self.ptr] >> (self.bits_left - take3)) as u16 & mask3);
        self.bits_left -= take3;
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Détection magic --

    #[test]
    fn magic_detection() {
        assert!(has_magic(b"CRILAYLA"));
        assert!(has_magic(b"CRILAYLA\x00\x01\x02\x03"));
        assert!(!has_magic(b"CRILAYL"));
        assert!(!has_magic(b""));
        assert!(!has_magic(b"crilayla"));
    }

    // -- Erreurs de format --

    #[test]
    fn trop_court_renvoie_erreur() {
        let result = decompress(b"CRILAYLA\x00\x00\x00\x00\x00\x00\x00\x00");
        assert!(matches!(result, Err(FormatError::TooShort { .. })));
    }

    #[test]
    fn mauvais_magic_renvoie_erreur() {
        let mut buf = alloc::vec![0u8; 0x110];
        buf[0..8].copy_from_slice(b"NOTMAGIC");
        let result = decompress(&buf);
        assert!(matches!(result, Err(FormatError::BadMagic { .. })));
    }

    #[test]
    fn uncompressed_size_zero_renvoie_raw_block() {
        // Fichier CRILAYLA avec uncompressed_size = 0, header_offset = 0.
        let mut buf = alloc::vec![0u8; 0x110];
        buf[0..8].copy_from_slice(b"CRILAYLA");
        buf[8..12].copy_from_slice(&0u32.to_le_bytes()); // uncompressed_size = 0
        buf[12..16].copy_from_slice(&0u32.to_le_bytes()); // header_offset = 0
        for b in &mut buf[0x10..0x110] {
            *b = 0xAB;
        }
        let result = decompress(&buf).expect("uncompressed_size=0 doit réussir");
        assert_eq!(result.len(), 0x100);
        assert!(result.iter().all(|&b| b == 0xAB));
    }

    #[test]
    fn header_offset_hors_limites_renvoie_erreur() {
        let mut buf = alloc::vec![0u8; 0x110];
        buf[0..8].copy_from_slice(b"CRILAYLA");
        buf[8..12].copy_from_slice(&0u32.to_le_bytes());
        buf[12..16].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        let result = decompress(&buf);
        assert!(matches!(result, Err(FormatError::Corrupt(..))));
    }

    // -- Tests round-trip avec vecteur synthétique --
    //
    // On construit un fichier CRILAYLA encodant uniquement des octets LITERAUX
    // (flag bit = 0, suivi de 8 bits de valeur), ce qui évite de devoir construire
    // un compresseur complet.
    //
    // Encodage du bitstream (inverse, MSB-first par octet) :
    //   Pour chaque octet `b` du payload (du dernier au premier) :
    //     émettre bit 0 (flag = littéral)
    //     émettre les 8 bits de `b` (MSB en premier)
    //   = 9 bits par octet de payload.
    //
    // Le BitWriter ci-dessous construit les octets dans l'ordre
    // "du premier bit émis vers le dernier" — ce sera le bitstream tel qu'il
    // sera LU par le décompresseur (qui lit depuis la fin du bitstream).
    //
    // IMPORTANT : le décompresseur lit le bitstream EN REMONTANT depuis raw_block_start-1
    // vers 0x10. L'octet à raw_block_start-1 est le PREMIER lu. Donc le DERNIER octet
    // produit par le BitWriter doit être à raw_block_start-1, et les octets s'étendent
    // vers les positions basses.
    //
    // Avec BitWriter qui empile dans Vec dans l'ordre d'émission, la séquence binaire
    // produite doit être INVERSÉE (bit 0 de payload[-1] devient le premier bit lu).
    //
    // Pour simplifier : on encode les octets du payload du DERNIER au PREMIER.
    // Le premier groupe 9 bits (flag=0 + byte[-1]) devient les 9 bits les plus "anciens"
    // dans le bitstream (lus EN DERNIER). Mais le décompresseur lit DU PLUS RÉCENT AU PLUS ANCIEN.
    //
    // Architecture du bitstream dans le fichier :
    //   input[0x10 .. raw_block_start]  : bitstream, MSB-first par octet
    //   décompresseur commence à ptr = raw_block_start, bits_left = 0
    //   → première opération : ptr-- → ptr = raw_block_start-1, bits_left = 7
    //
    // Donc input[raw_block_start-1] contient les PREMIERS bits émis.
    // Le BitWriter doit les placer là.
    //
    // Stratégie : BitWriter accumule les bits dans un Vec, du premier au dernier.
    // Le premier octet produit va en tête. On le place à input[0x10], et le
    // dernier octet produit va à input[raw_block_start-1].
    // Le décompresseur lit input[raw_block_start-1] en premier → il voit le DERNIER
    // groupe émis en premier.
    //
    // Conclusion : il faut encoder le payload dans l'ordre NATUREL (premier octet en premier),
    // et le bitstream sera placé dans input[0x10..raw_block_start] dans cet ordre.
    // Le décompresseur qui lit à rebours verra le dernier octet du payload EN PREMIER
    // et écrira output[total_size-1] = payload[-1], puis remonte vers payload[0].
    // Ce qui est cohérent : writePtr descend de total_size-1 → RAW_HEADER_SIZE.

    struct BitWriter {
        bytes: Vec<u8>,
        current: u8,
        bits_used: u32, // nombre de bits déjà dans `current`, de gauche à droite
    }

    impl BitWriter {
        fn new() -> Self {
            Self {
                bytes: Vec::new(),
                current: 0,
                bits_used: 0,
            }
        }

        /// Émet un bit (MSB-first dans chaque octet).
        fn push_bit(&mut self, bit: u8) {
            self.current = (self.current << 1) | (bit & 1);
            self.bits_used += 1;
            if self.bits_used == 8 {
                self.bytes.push(self.current);
                self.current = 0;
                self.bits_used = 0;
            }
        }

        /// Émet `n` bits de `val` (MSB en premier).
        fn push_bits(&mut self, val: u32, n: u32) {
            for i in (0..n).rev() {
                self.push_bit(((val >> i) & 1) as u8);
            }
        }

        /// Finalise (pad à droite avec des 0 si nécessaire).
        fn finish(mut self) -> Vec<u8> {
            if self.bits_used > 0 {
                self.current <<= 8 - self.bits_used;
                self.bytes.push(self.current);
            }
            self.bytes
        }
    }

    /// Construit un fichier CRILAYLA avec `payload` encodé en octets littéraux.
    /// Le raw block (0x100 octets) est rempli de `raw_fill`.
    ///
    /// ## Ordre d'encodage
    ///
    /// Le décompresseur lit le bitstream EN REMONTANT : il commence à `raw_block_start - 1`
    /// et recule vers `0x10`. Chaque octet est consommé MSB-first.
    ///
    /// Donc l'octet à `raw_block_start - 1` contient les PREMIERS bits lus.
    ///
    /// Le décompresseur écrit la sortie à rebours (write_pos descend de total_size-1
    /// vers RAW_HEADER_SIZE). Donc output[total_size-1] = payload[-1] (dernier octet),
    /// ..., output[RAW_HEADER_SIZE] = payload[0] (premier octet).
    ///
    /// Pour produire ce résultat avec des littéraux :
    /// le DERNIER groupe 9 bits encodé (pour payload[-1]) doit être lu EN PREMIER.
    /// → on encode payload du DERNIER au PREMIER octet.
    /// → les bits produits vont dans le BitWriter dans cet ordre.
    /// → le BitWriter produit les octets dans l'ordre ; le PREMIER octet produit doit
    ///   être à `raw_block_start - 1` (lu en premier).
    /// → le bitstream est placé dans le fichier EN ORDRE INVERSE (dernier octet du
    ///   BitWriter à input[0x10], premier octet à input[raw_block_start-1]).
    fn build_literal_crilayla(payload: &[u8], raw_fill: u8) -> Vec<u8> {
        // Encoder les octets du payload du DERNIER au PREMIER.
        let mut bw = BitWriter::new();
        for &byte in payload.iter().rev() {
            bw.push_bit(0); // flag = 0 → littéral
            bw.push_bits(byte as u32, 8);
        }
        let mut bitstream = bw.finish();

        // Inverser pour que le premier groupe émis soit à raw_block_start-1.
        bitstream.reverse();

        let header_offset = bitstream.len() as u32;
        let uncompressed_size = payload.len() as u32;

        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"CRILAYLA");
        out.extend_from_slice(&uncompressed_size.to_le_bytes());
        out.extend_from_slice(&header_offset.to_le_bytes());
        // Bitstream à input[0x10..0x10+bitstream.len()].
        out.extend_from_slice(&bitstream);
        // Raw block (0x100 octets) rempli de `raw_fill`.
        out.extend(alloc::vec![raw_fill; 0x100]);

        out
    }

    #[test]
    fn decode_un_seul_octet() {
        // payload = [0x42] → 1 octet littéral.
        let buf = build_literal_crilayla(&[0x42], 0xAA);
        let result = decompress(&buf).expect("1 octet littéral doit décompresser");
        assert_eq!(result.len(), 0x100 + 1);
        assert!(
            result[..0x100].iter().all(|&b| b == 0xAA),
            "raw block incorrect"
        );
        assert_eq!(result[0x100], 0x42, "octet littéral incorrect");
    }

    #[test]
    fn decode_literals_connus() {
        let payload = &[0x01u8, 0x02, 0x03, 0x04];
        let buf = build_literal_crilayla(payload, 0xAA);
        let result = decompress(&buf).expect("4 octets littéraux doivent décompresser");

        assert_eq!(result.len(), 0x100 + 4);
        assert!(
            result[..0x100].iter().all(|&b| b == 0xAA),
            "raw block incorrect"
        );
        assert_eq!(
            &result[0x100..],
            payload as &[u8],
            "payload décompressé incorrect"
        );
    }

    #[test]
    fn decode_vide_payload() {
        let buf = build_literal_crilayla(&[], 0xAA);
        let result = decompress(&buf).expect("payload vide doit réussir");
        assert_eq!(result.len(), 0x100);
        assert!(result.iter().all(|&b| b == 0xAA));
    }

    #[test]
    fn taille_totale_correcte() {
        let payload: Vec<u8> = (0..64).collect();
        let buf = build_literal_crilayla(&payload, 0xBB);
        let result = decompress(&buf).expect("64 octets littéraux OK");
        assert_eq!(result.len(), 0x100 + 64);
        assert!(result[..0x100].iter().all(|&b| b == 0xBB));
        assert_eq!(&result[0x100..], payload.as_slice());
    }

    #[test]
    fn decode_valeurs_variees() {
        // Payload avec valeurs 0x00 à 0xFF.
        let payload: Vec<u8> = (0u8..=255).collect();
        let buf = build_literal_crilayla(&payload, 0x00);
        let result = decompress(&buf).expect("256 octets littéraux OK");
        assert_eq!(&result[0x100..], payload.as_slice());
    }

    // -- Tests avec backref (régression bug : calcul src avant/après décrément) --
    //
    // Constructeur CRILAYLA avec un backref encodé manuellement.
    //
    // Payload cible : [0x41, 0x42, 0x43, 0x44, 0x41, 0x42, 0x43, 0x44]
    // (4 littéraux puis une copie des 4 précédents avec offset=4, longueur=4)
    //
    // Ordre de décompression (write_pos descend de total_size-1 vers 0x100) :
    //   Lit d'abord les symboles encodant les positions les PLUS HAUTES.
    //   output[total_size-1] → output[0x100] de droite à gauche.
    //
    // Pour output[0x100..0x108] = [0x41,0x42,0x43,0x44,0x41,0x42,0x43,0x44] :
    //   positions 0x100..0x104 = [0x41,0x42,0x43,0x44] écrites EN DERNIER (write_pos décroît)
    //   positions 0x104..0x108 = [0x41,0x42,0x43,0x44] écrites EN PREMIER
    //
    // Donc le bitstream encode (de gauche à droite = premier lu) :
    //   1. backref (off_raw=1, len=4) → écrit output[0x107..0x104], source à +4
    //      soit off_raw=4-3=1, lvl2=1 (3+1=4)
    //   2. lit 0x44 → output[0x103]
    //   3. lit 0x43 → output[0x102]
    //   4. lit 0x42 → output[0x101]
    //   5. lit 0x41 → output[0x100]
    //
    // Note : la copie backref à write_pos=0x107 et offset=4 doit lire output[0x107+4=0x10B].
    // Mais output ne fait que 0x108 octets → hors limites ?
    //
    // C'est le bug corrigé ! Le décompresseur décrémente write_pos D'ABORD,
    // puis lit src = write_pos + offset = 0x106 + 4 = 0x10A > total_size = 0x108.
    // Toujours hors limites...
    //
    // Reprenons la sémantique : avec write_pos = total_size au départ,
    // après le décrément D'ABORD, write_pos = total_size - 1 = 0x107.
    // src = 0x107 + 4 = 0x10B ≥ 0x108 → erreur.
    //
    // Cas valide : la copie backref ne peut référencer que des positions
    // DÉJÀ ÉCRITES (> write_pos courant). Donc pour un offset=4 à write_pos=0x107 :
    // src = 0x10B > total_out. Impossible pour le premier symbole.
    //
    // Un backref valide initial : après 4 littéraux, write_pos = 0x103.
    // src = 0x103 + 4 = 0x107 < total_size = 0x108. Valide !
    //
    // Donc la séquence correcte d'encodage :
    //   1. lit 0x41 → output[0x107] (write_pos descend de 0x108 → 0x107)
    //   2. lit 0x42 → output[0x106]
    //   3. lit 0x43 → output[0x105]
    //   4. lit 0x44 → output[0x104]
    //   5. backref off_raw=1, len=4 → écrit output[0x103..0x100], src=write_pos+4
    //      write_pos=0x103 → src=0x107=output[0x107]=0x41 ✓
    //      write_pos=0x102 → src=0x106=output[0x106]=0x42 ✓
    //      write_pos=0x101 → src=0x105=output[0x105]=0x43 ✓
    //      write_pos=0x100 → src=0x104=output[0x104]=0x44 ✓
    //
    // Résultat final : output[0x100..0x108] = [0x44,0x43,0x42,0x41,0x41,0x42,0x43,0x44]
    // (les littéraux écrits en ordre décroissant de write_pos donnent l'ordre croissant)

    fn build_backref_crilayla() -> Vec<u8> {
        // Construit le bitstream encodant :
        //   4 littéraux 0x41,0x42,0x43,0x44 puis un backref(off=4, len=4).
        //
        // Ordre d'encodage : ce qui est lu EN PREMIER doit correspondre
        // à ce qui est écrit EN PREMIER (write_pos le plus haut).
        // Premier écrit = lit 0x41 → encodé EN PREMIER dans le bitstream.
        // Dernier écrit = backref → encodé EN DERNIER.
        // Le bitstream est inversé avant insertion dans le fichier.

        let mut bw = BitWriter::new();

        // Lit 0x41 (encode en premier → lu en premier → écrit en premier)
        bw.push_bit(0);
        bw.push_bits(0x41, 8);
        // Lit 0x42
        bw.push_bit(0);
        bw.push_bits(0x42, 8);
        // Lit 0x43
        bw.push_bit(0);
        bw.push_bits(0x43, 8);
        // Lit 0x44
        bw.push_bit(0);
        bw.push_bits(0x44, 8);
        // Backref off_raw=1 (offset=4), len=4 (lvl2=1)
        bw.push_bit(1); // flag = backref
        bw.push_bits(1, 13); // off_raw = 1 → offset = 4
        bw.push_bits(1, 2); // lvl2 = 1 → length = 3 + 1 = 4 (< max 3, pas lvl3)

        let mut bitstream = bw.finish();
        // Inverser : le premier bit émis doit être lu EN PREMIER (à raw_block_start-1).
        bitstream.reverse();

        let header_offset = bitstream.len() as u32;
        let uncompressed_size = 8u32; // 4 lits + 4 backref

        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"CRILAYLA");
        out.extend_from_slice(&uncompressed_size.to_le_bytes());
        out.extend_from_slice(&header_offset.to_le_bytes());
        out.extend_from_slice(&bitstream);
        out.extend(alloc::vec![0xBBu8; 0x100]);
        out
    }

    #[test]
    fn decode_backref_simple() {
        // Vérification de la copie LZ backref : 4 littéraux + backref(offset=4, len=4).
        let buf = build_backref_crilayla();
        let result = decompress(&buf).expect("backref simple doit décompresser");

        // output[0x100..0x108] attendu :
        //   [0x44, 0x43, 0x42, 0x41, 0x41, 0x42, 0x43, 0x44]
        // (4 lits écrits à rebours aux positions 0x107..0x104, puis backref aux positions 0x103..0x100)
        assert_eq!(result.len(), 0x100 + 8, "taille incorrecte");
        assert!(
            result[..0x100].iter().all(|&b| b == 0xBB),
            "raw block incorrect"
        );
        let payload = &result[0x100..];
        // Les 4 littéraux sont écrits D'ABORD (write_pos décroît depuis total_size-1) :
        //   write_pos=0x107 → lit 0x41 ; [0x106] → 0x42 ; [0x105] → 0x43 ; [0x104] → 0x44
        // Donc payload[7]=0x41, [6]=0x42, [5]=0x43, [4]=0x44.
        assert_eq!(payload[7], 0x41, "lit 0x41 écrit à write_pos=0x107");
        assert_eq!(payload[6], 0x42, "lit 0x42 écrit à write_pos=0x106");
        assert_eq!(payload[5], 0x43, "lit 0x43 écrit à write_pos=0x105");
        assert_eq!(payload[4], 0x44, "lit 0x44 écrit à write_pos=0x104");
        // Le backref (offset=4, len=4) copie ensuite aux positions 0x103..0x100 :
        //   write_pos=0x103 → src=0x107 → 0x41 ; etc.
        assert_eq!(payload[3], 0x41, "backref[3] copie output[0x107]=0x41");
        assert_eq!(payload[2], 0x42, "backref[2] copie output[0x106]=0x42");
        assert_eq!(payload[1], 0x43, "backref[1] copie output[0x105]=0x43");
        assert_eq!(payload[0], 0x44, "backref[0] copie output[0x104]=0x44");
    }

    #[test]
    fn decode_repetition_run() {
        // Test run-length : répétition d'un octet via backref(offset=3, len=MIN_COPY=3).
        // Backref minimal offset=3 : source = write_pos+3.
        // Si les 3 octets à write_pos+1..+3 sont identiques, la copie reproduit le run.
        //
        // Payload = [0xAB, 0xAB, 0xAB, 0xAB, 0xAB, 0xAB] (6 fois 0xAB)
        // Encodage : 3 lits 0xAB, puis backref(off_raw=0, len=3)
        //   → output[0x105..0x103] = lits (0xAB écrit 3 fois), backref aux 0x102..0x100
        //   backref à write_pos=0x102, src=0x102+3=0x105, output[0x105]=0xAB ✓

        let mut bw = BitWriter::new();
        // 3 lits 0xAB (encodés dans l'ordre : premier lit = lit le plus haut write_pos)
        bw.push_bit(0);
        bw.push_bits(0xAB, 8);
        bw.push_bit(0);
        bw.push_bits(0xAB, 8);
        bw.push_bit(0);
        bw.push_bits(0xAB, 8);
        // Backref off_raw=0 (offset=3), len=3 (lvl2=0)
        bw.push_bit(1);
        bw.push_bits(0, 13); // off_raw=0 → offset=3
        bw.push_bits(0, 2); // lvl2=0 → length=3+0=3

        let mut bs = bw.finish();
        bs.reverse();
        let ho = bs.len() as u32;

        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(b"CRILAYLA");
        buf.extend_from_slice(&6u32.to_le_bytes()); // uncompressed_size = 6
        buf.extend_from_slice(&ho.to_le_bytes());
        buf.extend_from_slice(&bs);
        buf.extend(alloc::vec![0xCCu8; 0x100]);

        let result = decompress(&buf).expect("run-length via backref doit décompresser");
        assert_eq!(result.len(), 0x100 + 6);
        assert!(result[..0x100].iter().all(|&b| b == 0xCC));
        assert!(
            result[0x100..].iter().all(|&b| b == 0xAB),
            "run-length incorrect: {:?}",
            &result[0x100..]
        );
    }
}
