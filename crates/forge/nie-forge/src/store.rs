//! Persistance du recouvrement et résolution des charges utiles.
//!
//! Le « magasin » de la forge ne stocke **aucun octet du jeu** : il conserve le
//! découpage (`cover.json` : offsets, longueurs, empreintes) et va chercher les
//! octets non encore portés dans le binaire de référence fourni par l'utilisateur
//! — exactement comme un projet de décompilation exige la ROM d'origine. Les
//! assets et le binaire sont © LEVEL-5 et ne quittent jamais la machine.

use anyhow::{Context, bail};
use nie_pe::{Cover, PeImage, Unit, sha256_hex};
use std::path::{Path, PathBuf};

/// Nom du fichier de recouvrement dans le répertoire de forge.
pub const COVER_FILE: &str = "cover.json";

/// Recouvrement persistant + accès au binaire de référence.
#[derive(Debug)]
pub struct ForgeStore {
    /// Répertoire de travail de la forge (hors dépôt Git).
    pub root: PathBuf,
    /// Recouvrement total du binaire cible.
    pub cover: Cover,
}

impl ForgeStore {
    /// Chemin du fichier de recouvrement.
    #[must_use]
    pub fn cover_path(root: &Path) -> PathBuf {
        root.join(COVER_FILE)
    }

    /// Découpe un binaire et écrit le recouvrement.
    ///
    /// # Erreurs
    /// Retourne une erreur de lecture, de parsing PE ou d'écriture disque.
    pub fn split_from(exe: &Path, root: &Path) -> anyhow::Result<Self> {
        Self::split_from_with(exe, root, &[])
    }

    /// Découpe un binaire en tenant compte de fonctions supplémentaires
    /// `(adresse virtuelle, taille)` — les feuilles de l'échafaudage RE, que
    /// `.pdata` ne décrit pas.
    ///
    /// # Erreurs
    /// Mêmes conditions que [`ForgeStore::split_from`].
    pub fn split_from_with(exe: &Path, root: &Path, extra: &[(u64, u32)]) -> anyhow::Result<Self> {
        let bytes = std::fs::read(exe).with_context(|| format!("lecture de {}", exe.display()))?;
        let img = PeImage::parse(bytes).with_context(|| format!("parsing de {}", exe.display()))?;
        let base = img.opt.image_base;
        let extra_rva: Vec<(u32, u32)> = extra
            .iter()
            .filter_map(|&(va, len)| {
                u32::try_from(va.checked_sub(base)?)
                    .ok()
                    .map(|rva| (rva, len))
            })
            .collect();
        let cover = Cover::split_with(&img, &extra_rva).context("découpage en unités")?;
        std::fs::create_dir_all(root)?;
        let path = Self::cover_path(root);
        std::fs::write(&path, serde_json::to_vec(&cover)?)
            .with_context(|| format!("écriture de {}", path.display()))?;
        Ok(Self {
            root: root.to_path_buf(),
            cover,
        })
    }

    /// Écrit un recouvrement déjà calculé.
    ///
    /// Le découpage complet demande deux passes — la seconde isole les données
    /// inline repérées sur la première — et l'image n'a aucune raison d'être
    /// relue et reparsée entre les deux.
    ///
    /// # Erreurs
    /// Retourne une erreur si le recouvrement est incohérent ou si l'écriture
    /// échoue.
    pub fn persist(root: &Path, cover: Cover) -> anyhow::Result<Self> {
        cover.validate().context("recouvrement incohérent")?;
        std::fs::create_dir_all(root)?;
        let path = Self::cover_path(root);
        std::fs::write(&path, serde_json::to_vec(&cover)?)
            .with_context(|| format!("écriture de {}", path.display()))?;
        Ok(Self {
            root: root.to_path_buf(),
            cover,
        })
    }

    /// Charge un recouvrement déjà calculé.
    ///
    /// # Erreurs
    /// Retourne une erreur si le fichier est absent ou mal formé.
    pub fn load(root: &Path) -> anyhow::Result<Self> {
        let path = Self::cover_path(root);
        let raw = std::fs::read(&path).with_context(|| {
            format!(
                "recouvrement absent : {} (lancer `nie-forge split` d'abord)",
                path.display()
            )
        })?;
        let cover: Cover = serde_json::from_slice(&raw)
            .with_context(|| format!("recouvrement illisible : {}", path.display()))?;
        cover.validate().context("recouvrement incohérent")?;
        Ok(Self {
            root: root.to_path_buf(),
            cover,
        })
    }
}

/// Binaire de référence : source des unités non encore produites par Rust.
#[derive(Debug)]
pub struct ReferenceBinary {
    /// Contenu intégral.
    pub bytes: Vec<u8>,
}

impl ReferenceBinary {
    /// Charge et vérifie le binaire de référence contre l'empreinte du recouvrement.
    ///
    /// # Erreurs
    /// Retourne une erreur si le fichier diffère du binaire ayant servi au découpage.
    pub fn load_checked(path: &Path, cover: &Cover) -> anyhow::Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("lecture de {}", path.display()))?;
        if bytes.len() != cover.total_len {
            bail!(
                "référence {} : {} octets, {} attendus (recouvrement d'un autre build ?)",
                path.display(),
                bytes.len(),
                cover.total_len
            );
        }
        let sha = sha256_hex(&bytes);
        if sha != cover.sha256 {
            bail!(
                "référence {} : sha256={sha}, recouvrement calculé sur {}",
                path.display(),
                cover.sha256
            );
        }
        Ok(Self { bytes })
    }

    /// Charge sans vérifier l'empreinte (comparaison de deux builds).
    ///
    /// # Erreurs
    /// Retourne une erreur de lecture disque.
    pub fn load_raw(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            bytes: std::fs::read(path).with_context(|| format!("lecture de {}", path.display()))?,
        })
    }

    /// Charge utile de référence d'une unité.
    #[must_use]
    pub fn payload(&self, u: &Unit) -> Option<Vec<u8>> {
        self.bytes.get(u.range()).map(<[u8]>::to_vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_pe() -> Vec<u8> {
        let pe_off = 0x80usize;
        let opt_size = 240usize;
        let headers = 0x200usize;
        let mut b = vec![0u8; headers + 0x200];
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
        b[s + 8..s + 12].copy_from_slice(&0x200u32.to_le_bytes());
        b[s + 12..s + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        b[s + 16..s + 20].copy_from_slice(&0x200u32.to_le_bytes());
        b[s + 20..s + 24].copy_from_slice(&u32::try_from(headers).unwrap().to_le_bytes());
        b[s + 36..s + 40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        b
    }

    #[test]
    fn split_puis_load_puis_reassemblage_identique() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("faux.exe");
        std::fs::write(&exe, synth_pe()).unwrap();
        let forge = dir.path().join("forge");

        let store = ForgeStore::split_from(&exe, &forge).expect("split");
        assert!(!store.cover.units.is_empty());

        let reloaded = ForgeStore::load(&forge).expect("load");
        assert_eq!(reloaded.cover, store.cover);

        let reference = ReferenceBinary::load_checked(&exe, &reloaded.cover).expect("référence");
        let rebuilt = reloaded
            .cover
            .assemble(|u| reference.payload(u))
            .expect("assemblage");
        assert_eq!(sha256_hex(&rebuilt), reloaded.cover.sha256);
    }

    #[test]
    fn reference_d_un_autre_build_est_rejetee() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("faux.exe");
        std::fs::write(&exe, synth_pe()).unwrap();
        let forge = dir.path().join("forge");
        let store = ForgeStore::split_from(&exe, &forge).unwrap();

        let mut altered = synth_pe();
        let n = altered.len();
        altered[n - 1] ^= 0xFF;
        let other = dir.path().join("autre.exe");
        std::fs::write(&other, altered).unwrap();

        let err = ReferenceBinary::load_checked(&other, &store.cover).unwrap_err();
        assert!(err.to_string().contains("sha256"));
    }
}
