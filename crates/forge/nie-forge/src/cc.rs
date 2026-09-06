//! Pont **MSVC** : compiler du C et exiger les octets de `nie.exe`.
//!
//! ## Pourquoi c'est le chemin principal
//!
//! `nie.exe` est lié par MSVC **14.44**. Ce toolset est installé sur la machine
//! (`cl.exe` 19.44.35228), donc le binaire peut être reconstruit par **le
//! compilateur qui l'a produit**, à partir de code source. Vérifié dès le premier
//! essai : `unsigned f(void){return 0xefec8a0dU;}` compilé en `/O2 /GS- /Gy /Zl`
//! donne `b8 0d 8a ec ef c3` — exactement les octets de la fonction
//! `0x1411194b0` du jeu.
//!
//! C'est ce qui sépare ce projet d'une transcription : la source est du C
//! lisible, réécrit à partir du reverse, et le juge est le compilateur. Là où
//! [`crate::lift`] doit encoder les instructions à la main (dialecte restreint),
//! ici on écrit la **sémantique** et MSVC produit la forme.
//!
//! ## Convention de source
//!
//! Chaque fonction porte l'adresse qu'elle prétend reproduire :
//!
//! ```c
//! /* @nie 0x1411194b0 */
//! unsigned int type_id_1411194b0(void) { return 0xefec8a0dU; }
//! ```
//!
//! `nie-forge cc` compile l'unité de traduction, extrait chaque symbole annoté de
//! l'objet COFF et le compare **byte-à-byte** à l'unité correspondante du binaire
//! (champs relogés masqués). Rien n'est enregistré sans cette égalité.

use anyhow::{Context, bail};
use std::path::{Path, PathBuf};

/// Options de compilation MSVC reproduisant celles du binaire cible.
///
/// `/O2` optimisation vitesse, `/GS-` sans contrôle de débordement de pile,
/// `/Gy` fonctions en COMDAT (une section par fonction → étendue exacte),
/// `/Zl` sans nom de bibliothèque par défaut (objet autonome).
pub const DEFAULT_FLAGS: &[&str] = &["/nologo", "/c", "/O2", "/GS-", "/Gy", "/Zl"];

/// Une fonction annotée dans une source C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotated {
    /// Adresse virtuelle revendiquée.
    pub va: u64,
    /// Symbole C correspondant.
    pub symbol: String,
    /// Ligne de l'annotation (diagnostics).
    pub line: usize,
}

/// Extrait les annotations `@nie <va>` d'une source C.
///
/// Le symbole est le dernier identifiant précédant `(` sur la première ligne de
/// définition qui suit l'annotation.
///
/// # Erreurs
/// Retourne une erreur si une annotation n'est suivie d'aucune définition
/// exploitable, ou si l'adresse est invalide.
pub fn parse_annotations(src: &str) -> anyhow::Result<Vec<Annotated>> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let Some(pos) = l.find("@nie") else { continue };
        let rest = l[pos + 4..].trim();
        let tok = rest.split_whitespace().next().unwrap_or_default();
        // Une annotation porte TOUJOURS une adresse `0x…`. Les mentions en prose
        // (« annotée `@nie <adresse>` » dans un en-tête de fichier) sont ignorées
        // plutôt que de faire échouer la compilation de tout le fichier.
        let Some(hex) = tok.strip_prefix("0x").or_else(|| tok.strip_prefix("0X")) else {
            continue;
        };
        let va = u64::from_str_radix(hex, 16)
            .with_context(|| format!("ligne {} : adresse invalide `{tok}`", i + 1))?;

        let sym = lines[i + 1..]
            .iter()
            .find(|s| s.contains('('))
            .and_then(|s| {
                let head = &s[..s.find('(')?];
                head.rsplit(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .find(|t| !t.is_empty())
                    .map(str::to_string)
            })
            .with_context(|| {
                format!(
                    "ligne {} : annotation @nie {va:#x} sans définition de fonction ensuite",
                    i + 1
                )
            })?;
        out.push(Annotated {
            va,
            symbol: sym,
            line: i + 1,
        });
    }
    Ok(out)
}

/// Localise `cl.exe` x64 : `--cl`, puis `$NIE_CL`, puis `vswhere`, puis les
/// emplacements d'installation usuels.
///
/// # Erreurs
/// Retourne une erreur si aucun compilateur n'est trouvé.
pub fn find_cl(explicit: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Some(p) = explicit {
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
        bail!("compilateur introuvable : {}", p.display());
    }
    if let Ok(p) = std::env::var("NIE_CL") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Ok(p);
        }
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    let vswhere =
        PathBuf::from(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe");
    if vswhere.is_file()
        && let Ok(out) = std::process::Command::new(&vswhere)
            .args([
                "-all",
                "-products",
                "*",
                "-format",
                "value",
                "-property",
                "installationPath",
            ])
            .output()
        && out.status.success()
    {
        for l in String::from_utf8_lossy(&out.stdout).lines() {
            let l = l.trim();
            if !l.is_empty() {
                roots.push(PathBuf::from(l));
            }
        }
    }

    // Le binaire cible est lié en 14.44 : privilégier ce toolset s'il est présent.
    let mut found: Vec<PathBuf> = Vec::new();
    for r in roots {
        let tools = r.join("VC").join("Tools").join("MSVC");
        let Ok(rd) = std::fs::read_dir(&tools) else {
            continue;
        };
        for e in rd.flatten() {
            let cl = e
                .path()
                .join("bin")
                .join("Hostx64")
                .join("x64")
                .join("cl.exe");
            if cl.is_file() {
                found.push(cl);
            }
        }
    }
    if found.is_empty() {
        bail!(
            "cl.exe introuvable — installer les Build Tools MSVC, ou passer --cl / définir NIE_CL"
        );
    }
    found.sort();
    let preferred = found
        .iter()
        .find(|p| p.to_string_lossy().contains("14.44"))
        .cloned();
    Ok(preferred.unwrap_or_else(|| found[0].clone()))
}

/// Version affichée par le compilateur (première ligne de sa bannière).
#[must_use]
pub fn cl_version(cl: &Path) -> String {
    std::process::Command::new(cl)
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stderr).into_owned()
                + &String::from_utf8_lossy(&o.stdout);
            s.lines()
                .find(|l| l.contains("19.") || l.to_lowercase().contains("version"))
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .unwrap_or_default()
}

/// Compile une source C en objet COFF.
///
/// # Erreurs
/// Retourne une erreur si `cl.exe` échoue, en propageant sa sortie.
pub fn compile(cl: &Path, src: &Path, out_dir: &Path, flags: &[String]) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unite".into());
    let obj = out_dir.join(format!("{stem}.obj"));

    let mut cmd = std::process::Command::new(cl);
    for f in flags {
        cmd.arg(f);
    }
    // `cl.exe` tourne dans le répertoire de sortie (il y écrit ses fichiers
    // intermédiaires) : la source et l'objet doivent donc être des chemins
    // absolus, sinon ils sont résolus depuis ce répertoire.
    // `std::path::absolute` et non `canonicalize` : ce dernier rend un chemin
    // UNC `\\?\C:\…` que `cl.exe` ne sait pas ouvrir.
    let src_abs = std::path::absolute(src).unwrap_or_else(|_| src.to_path_buf());
    let obj_abs = std::path::absolute(&obj).unwrap_or_else(|_| obj.clone());
    cmd.arg(format!("/Fo{}", obj_abs.display()));
    cmd.arg(&src_abs);
    cmd.current_dir(out_dir);

    let out = cmd
        .output()
        .with_context(|| format!("exécution de {}", cl.display()))?;
    if !out.status.success() {
        bail!(
            "cl.exe a échoué sur {} :\n{}{}",
            src.display(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    if !obj.is_file() {
        bail!("cl.exe n'a pas produit {}", obj.display());
    }
    Ok(obj)
}

/// Flags par défaut, en `String` (pratique pour clap).
#[must_use]
pub fn default_flags() -> Vec<String> {
    DEFAULT_FLAGS.iter().map(|s| (*s).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrait_les_annotations() {
        let src = r#"
/* @nie 0x1411194b0 */
unsigned int type_id_1411194b0(void) { return 0xefec8a0dU; }

// @nie 0x140057350
void *ret_this_140057350(void *p)
{
    return p;
}
"#;
        let a = parse_annotations(src).expect("annotations");
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].va, 0x1_4111_94b0);
        assert_eq!(a[0].symbol, "type_id_1411194b0");
        assert_eq!(a[1].va, 0x1_4005_7350);
        assert_eq!(a[1].symbol, "ret_this_140057350");
    }

    #[test]
    fn annotation_orpheline_est_une_erreur() {
        let e = parse_annotations("/* @nie 0x140001000 */\n").unwrap_err();
        assert!(e.to_string().contains("sans définition"), "{e}");
        let e = parse_annotations("/* @nie 0xzz */\nint f(void){return 0;}").unwrap_err();
        assert!(e.to_string().contains("adresse invalide"), "{e}");
    }

    #[test]
    fn mention_en_prose_ignoree() {
        // Un en-tête de fichier qui *parle* de l'annotation ne doit pas être lu
        // comme une annotation — sinon un commentaire casse toute la compilation.
        let a = parse_annotations(
            "/* chaque fonction annotée @nie <adresse> */\nint f(void){return 0;}",
        )
        .expect("prose ignorée");
        assert!(a.is_empty());
    }

    #[test]
    fn les_flags_par_defaut_reproduisent_le_binaire_cible() {
        // Régression : ces options sont celles qui ont fait coïncider les octets.
        // Les changer sans re-valider casserait toutes les correspondances.
        let f = default_flags();
        for expected in ["/c", "/O2", "/GS-", "/Gy", "/Zl"] {
            assert!(
                f.iter().any(|x| x == expected),
                "flag manquant : {expected}"
            );
        }
    }
}
