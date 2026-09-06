//! **Decrypt / Encrypt Criware** — le cinquième bouton de Viola.
//!
//! Le calcul lui-même n'a pas été porté : c'est [`nie_formats::cpk::decrypt_block`], port fidèle
//! de `CriwareCrypt.DecryptBlock`. Ce module lui donne une façade fichier et exploite deux
//! propriétés que l'amont n'utilise pas :
//!
//! * le XOR est **involutif** — chiffrer et déchiffrer sont le même calcul, donc une seule
//!   fonction suffit là où Viola en a deux (`CEnc`/`CDec`) ;
//! * il est **positionnel** (`file_offset`) — on peut donc traiter le fichier **par tranches**
//!   au lieu de le charger entièrement. `CCriwareCrypt` lit tout le fichier en mémoire ; un
//!   `.awb` d'IEVR pèse plusieurs Gio.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use nie_formats::cpk::{VIOLA_FIXED_KEY, decrypt_block, key_from_filename};

/// Tranche de traitement. Assez grande pour amortir les appels système, assez petite pour que
/// l'occupation mémoire reste négligeable quelle que soit la taille du fichier.
const TRANCHE: usize = 8 * 1024 * 1024;

/// Origine de la clé de (dé)chiffrement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CriwareKey {
    /// Clé fixe Viola (`0x1717_E18E`) — celle des `cfg.bin` enveloppés.
    Viola,
    /// Clé dérivée du nom de fichier (CRC32), comme pour les packs CPK.
    DuNom(String),
    /// Clé fournie telle quelle, en hexadécimal dans l'interface.
    Explicite(u32),
}

impl CriwareKey {
    /// Valeur numérique effective de la clé.
    #[must_use]
    pub fn valeur(&self) -> u32 {
        match self {
            CriwareKey::Viola => VIOLA_FIXED_KEY,
            CriwareKey::DuNom(nom) => key_from_filename(nom),
            CriwareKey::Explicite(k) => *k,
        }
    }

    /// Lit une clé écrite en hexadécimal (`1717E18E` ou `0x1717E18E`).
    ///
    /// # Errors
    /// Si le texte n'est pas un entier hexadécimal 32 bits.
    pub fn depuis_hex(texte: &str) -> Result<Self, String> {
        let t = texte.trim();
        let t = t
            .strip_prefix("0x")
            .or_else(|| t.strip_prefix("0X"))
            .unwrap_or(t);
        u32::from_str_radix(t, 16)
            .map(CriwareKey::Explicite)
            .map_err(|_| format!("clé hexadécimale invalide : « {texte} »"))
    }
}

/// (Dé)chiffre un tampon en place, à partir de l'offset `depart` dans le fichier d'origine.
///
/// Une seule fonction pour les deux sens : l'opération est involutive.
pub fn crypt_bytes(tampon: &mut [u8], depart: u64, cle: &CriwareKey) {
    decrypt_block(tampon, depart, cle.valeur());
}

/// (Dé)chiffre un fichier vers un autre, **par tranches**, sans jamais le charger en entier.
///
/// Renvoie le nombre d'octets traités.
///
/// # Errors
/// Toute erreur d'entrée/sortie sur la source ou la destination.
pub fn crypt_file(source: &Path, destination: &Path, cle: &CriwareKey) -> Result<u64, String> {
    let mut entree =
        std::fs::File::open(source).map_err(|e| format!("{} : {e}", source.display()))?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
    }
    let mut sortie = std::fs::File::create(destination)
        .map_err(|e| format!("{} : {e}", destination.display()))?;

    let mut tampon = vec![0u8; TRANCHE];
    let mut position = 0u64;
    loop {
        let lu = entree
            .read(&mut tampon)
            .map_err(|e| format!("lecture : {e}"))?;
        if lu == 0 {
            break;
        }
        // L'offset absolu est indispensable : la clé dérive de la position dans le fichier, une
        // tranche traitée comme si elle commençait à zéro produirait du charabia.
        crypt_bytes(&mut tampon[..lu], position, cle);
        sortie
            .write_all(&tampon[..lu])
            .map_err(|e| format!("écriture : {e}"))?;
        position += lu as u64;
    }
    sortie.flush().map_err(|e| format!("vidage : {e}"))?;
    Ok(position)
}

/// Devine si un fichier est chiffré, en regardant si son en-tête devient un magique connu une
/// fois la clé appliquée.
///
/// Rendu **explicitement faillible** : c'est une aide à la saisie dans l'interface, pas une
/// détection sûre. Viola, elle, demande la clé à l'utilisatrice sans jamais vérifier.
///
/// # Errors
/// Si le fichier est illisible.
pub fn deviner_chiffre(source: &Path, cle: &CriwareKey) -> Result<bool, String> {
    let mut f = std::fs::File::open(source).map_err(|e| format!("{} : {e}", source.display()))?;
    let mut tete = [0u8; 4];
    let lu = f.read(&mut tete).map_err(|e| format!("lecture : {e}"))?;
    if lu < 4 {
        return Ok(false);
    }
    // Magiques rencontrés en clair sur les conteneurs concernés.
    const CLAIRS: [&[u8; 4]; 4] = [b"CPK ", b"@UTF", b"AFS2", b"\x40UTF"];
    if CLAIRS.contains(&&tete) {
        return Ok(false);
    }
    f.seek(SeekFrom::Start(0))
        .map_err(|e| format!("repositionnement : {e}"))?;
    let mut essai = tete;
    crypt_bytes(&mut essai, 0, cle);
    Ok(CLAIRS.contains(&&essai))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cle_hexadecimale_accepte_les_deux_ecritures() {
        assert_eq!(
            CriwareKey::depuis_hex("1717E18E")
                .expect("sans préfixe")
                .valeur(),
            VIOLA_FIXED_KEY
        );
        assert_eq!(
            CriwareKey::depuis_hex("0x1717e18e")
                .expect("avec préfixe")
                .valeur(),
            VIOLA_FIXED_KEY
        );
        assert!(CriwareKey::depuis_hex("pas une clé").is_err());
    }

    #[test]
    fn le_traitement_par_tranches_donne_le_meme_resultat_qu_en_un_bloc() {
        // C'est LA propriété qui autorise le découpage : sans elle, un fichier de plusieurs Gio
        // serait chiffré différemment selon la taille du tampon.
        let cle = CriwareKey::Viola;
        let original: Vec<u8> = (0..(TRANCHE + 12345)).map(|i| (i % 251) as u8).collect();

        let mut bloc = original.clone();
        crypt_bytes(&mut bloc, 0, &cle);

        let mut tranches = original.clone();
        let (a, b) = tranches.split_at_mut(TRANCHE);
        crypt_bytes(a, 0, &cle);
        crypt_bytes(b, TRANCHE as u64, &cle);

        assert_eq!(bloc, tranches, "le découpage doit être transparent");
    }

    #[test]
    fn chiffrer_deux_fois_rend_l_original() {
        let cle = CriwareKey::DuNom("un_fichier.awb".to_string());
        let original: Vec<u8> = (0..1000u32).map(|i| (i % 256) as u8).collect();
        let mut buf = original.clone();
        crypt_bytes(&mut buf, 0, &cle);
        assert_ne!(buf, original, "le chiffrement doit changer quelque chose");
        crypt_bytes(&mut buf, 0, &cle);
        assert_eq!(buf, original, "l'opération est involutive");
    }
}
