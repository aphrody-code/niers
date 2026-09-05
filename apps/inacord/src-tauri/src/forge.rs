//! La **forge** vue depuis l'explorateur — façade IPC au-dessus de `nie-forge`.
//!
//! Le dépôt ne vise pas seulement à rejouer le jeu : il vise à **produire
//! `nie.exe`**, et la forge est le juge qui mesure, à l'octet, la part
//! réellement générée. Cet onglet rend cette mesure consultable sans quitter
//! l'explorateur, et surtout **rejouable** : les mêmes fichiers que la CLI
//! (`var/forge/cover.json`, `forge/registry.json`, `forge/asm/*.s`) sont lus
//! ici, pas un rapport figé recopié.
//!
//! ## Pourquoi `nie-forge` est lié sans sa feature `redb`
//!
//! Son module `redb` lit `var/niers.sqlite` via `rusqlite`, qui porte
//! `links = "sqlite3"`. Cargo interdit deux copies d'une bibliothèque `links`,
//! et l'explorateur en a déjà une (le `sqlx-sqlite` de `tauri-plugin-sql`).
//! Le reste du crate — recouvrement, registre, source assembleur, rapport —
//! est pur, d'où `default-features = false`. Vérifié par
//! `cargo tree -i libsqlite3-sys` : une seule occurrence.

use serde::Serialize;

/// Part produite par le dépôt, par source.
#[derive(Serialize, specta::Type)]
pub struct ForgeBucketDto {
    /// Unités concernées.
    pub units: u32,
    /// Octets concernés.
    pub bytes: u32,
}

/// Mesure de production de la forge, telle que `nie-forge report` la calcule.
#[derive(Serialize, specta::Type)]
pub struct ForgeReportDto {
    /// Racine du dépôt effectivement utilisée.
    pub root: String,
    /// Taille du binaire cible.
    pub total_bytes: u32,
    /// Nombre total d'unités du recouvrement.
    pub total_units: u32,
    /// Octets de code (`.text` et assimilés).
    pub code_bytes: u32,
    /// Unités de fonction du recouvrement.
    pub functions: u32,
    /// En-têtes PE recalculés par `nie-pe`.
    pub emitted: ForgeBucketDto,
    /// Corps réassemblés depuis `forge/asm/*.s` par `nie-asm`.
    pub assembled: ForgeBucketDto,
    /// Fonctions dont le codegen Rust coïncide avec les octets d'origine.
    pub matched_bytes: ForgeBucketDto,
    /// Portages validés sémantiquement — **jamais** comptés comme produits.
    pub matched_semantic: ForgeBucketDto,
    /// Portages en cours, non validés.
    pub wip: ForgeBucketDto,
    /// Octets produits par le dépôt.
    pub produced_bytes: u32,
    /// Part du fichier produite par le dépôt, en pourcentage.
    pub produced_pct: f64,
    /// Part du `.text` produite par le dépôt, en pourcentage.
    pub code_pct: f64,
    /// Entrées du registre sans unité correspondante.
    pub orphan_entries: u32,
}

impl From<&nie_forge::report::Bucket> for ForgeBucketDto {
    fn from(b: &nie_forge::report::Bucket) -> Self {
        Self {
            units: u32::try_from(b.units).unwrap_or(u32::MAX),
            bytes: u32::try_from(b.bytes).unwrap_or(u32::MAX),
        }
    }
}

/// Remonte depuis `start` jusqu'au premier ancêtre portant `forge/registry.json`.
///
/// L'explorateur peut être lancé de n'importe où ; la forge, elle, vit à la
/// racine du dépôt. On la cherche plutôt que de la supposer.
fn find_repo_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if dir.join("forge").join("registry.json").is_file() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

/// Résout la racine du dépôt : celle donnée, sinon le répertoire courant ou un
/// de ses ancêtres.
fn resolve_root(root: Option<String>) -> Result<std::path::PathBuf, String> {
    if let Some(r) = root.filter(|r| !r.trim().is_empty()) {
        let p = std::path::PathBuf::from(r);
        return find_repo_root(&p)
            .ok_or_else(|| format!("aucun `forge/registry.json` sous {}", p.display()));
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    find_repo_root(&cwd).ok_or_else(|| {
        format!(
            "racine du dépôt introuvable depuis {} — indiquer le chemin du dépôt niers",
            cwd.display()
        )
    })
}

/// Mesure de production de la forge, recalculée depuis les artefacts.
///
/// Les trois entrées sont celles de la CLI : le recouvrement
/// (`var/forge/cover.json`, produit par `nie-forge split`), le registre
/// (`forge/registry.json`) et la source assembleur (`forge/asm/*.s`, produite
/// par `nie-forge lift`). Rien n'est mis en cache : la valeur rendue est celle
/// de l'état du disque au moment de l'appel.
///
/// # Errors
///
/// Échoue si la racine du dépôt est introuvable, si le recouvrement n'a pas
/// encore été produit, ou si un artefact est illisible.
#[tauri::command]
#[specta::specta]
pub async fn forge_report(root: Option<String>) -> Result<ForgeReportDto, String> {
    let root = resolve_root(root)?;
    // `cover.json` pèse ~40 Mo : le chargement part sur un thread bloquant pour
    // ne pas figer l'IPC.
    tokio::task::spawn_blocking(move || {
        let store = nie_forge::ForgeStore::load(&root.join("var").join("forge"))
            .map_err(|e| format!("recouvrement absent ou illisible ({e}) — lancer `nie-forge split`"))?;
        let registry = nie_forge::Registry::load(&root.join("forge").join("registry.json"))
            .map_err(|e| e.to_string())?;
        let asm = nie_forge::AsmSource::load_dir(&root.join("forge").join("asm"))
            .map_err(|e| e.to_string())?;
        let mut r = nie_forge::Report::build(&store.cover, &registry, &asm)
            .map_err(|e| e.to_string())?;
        // Les sections-tables (`.pdata`, `.reloc`) sont **regenerees** par
        // `nie-pe`, pas recopiees : elles comptent comme produites, exactement
        // comme dans `nie-forge report`. Omettre cette etape sous-declarait de
        // 1 427 968 octets — 4,2 points — et faisait diverger cet onglet de la
        // CLI. C'est la meme fonction des deux cotes, pour que ca ne puisse
        // plus arriver.
        // `src-tauri` n'est pas en edition 2024 : pas de let-chain ici.
        if let Ok(bytes) = std::fs::read(root.join("nie.exe")) {
            if let Ok(img) = nie_pe::PeImage::parse(bytes) {
                r.add_emitted_tables(&store.cover, &img);
            }
        }
        Ok(ForgeReportDto {
            root: root.display().to_string(),
            total_bytes: u32::try_from(r.total_bytes).unwrap_or(u32::MAX),
            total_units: u32::try_from(r.total_units).unwrap_or(u32::MAX),
            code_bytes: u32::try_from(r.code_bytes).unwrap_or(u32::MAX),
            functions: u32::try_from(r.functions).unwrap_or(u32::MAX),
            emitted: (&r.emitted).into(),
            assembled: (&r.assembled).into(),
            matched_bytes: (&r.matched_bytes).into(),
            matched_semantic: (&r.matched_semantic).into(),
            wip: (&r.wip).into(),
            produced_bytes: u32::try_from(r.produced_bytes()).unwrap_or(u32::MAX),
            produced_pct: r.produced_pct(),
            code_pct: r.code_pct(),
            orphan_entries: u32::try_from(r.orphan_entries).unwrap_or(u32::MAX),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Une cause de blocage du relevé, avec ce qu'elle coûte.
#[derive(Serialize, specta::Type)]
pub struct ForgeBlockerDto {
    /// Mnémonique ou nature du blocage (`gs:`, `encodage:mov`, `invalide`…).
    pub cause: String,
    /// Unités bloquées par cette cause.
    pub units: u32,
    /// Octets bloqués par cette cause — le gain d'un déblocage.
    pub bytes: u32,
    /// Exemple désassemblé, avec son adresse.
    pub sample: String,
}

/// Ce qui empêche encore la forge de produire, trié par octets bloqués.
///
/// C'est la **liste de travail** : chaque ligne dit combien d'octets un
/// élargissement du dialecte rapporterait, et donne l'instruction fautive
/// désassemblée. C'est ce diagnostic — pas l'intuition — qui a fait passer la
/// part produite de 51,86 % à 69,53 % du fichier.
///
/// # Errors
///
/// Mêmes conditions que [`forge_report`].
#[tauri::command]
#[specta::specta]
pub async fn forge_blockers(
    root: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<ForgeBlockerDto>, String> {
    let root = resolve_root(root)?;
    let limit = limit.unwrap_or(30) as usize;
    tokio::task::spawn_blocking(move || {
        let store = nie_forge::ForgeStore::load(&root.join("var").join("forge"))
            .map_err(|e| format!("recouvrement absent ou illisible ({e}) — lancer `nie-forge split`"))?;
        let exe = root.join("nie.exe");
        let bytes = std::fs::read(&exe)
            .map_err(|e| format!("lecture de {} : {e}", exe.display()))?;
        // L'agregation vit dans `nie-forge`, partagee avec `nie-forge lift` :
        // une boucle recopiee des deux cotes finit par diverger sans que rien
        // ne le signale — c'est exactement ce qui etait arrive a la mesure.
        let out: Vec<ForgeBlockerDto> = nie_forge::lift::blockers(&store.cover, &bytes, 0)
            .into_iter()
            .take(limit)
            .map(|b| ForgeBlockerDto {
                cause: b.cause,
                units: u32::try_from(b.units).unwrap_or(u32::MAX),
                bytes: u32::try_from(b.bytes).unwrap_or(u32::MAX),
                sample: b.sample,
            })
            .collect();
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_racine_est_trouvee_en_remontant() {
        let dir = std::env::temp_dir().join("nie_forge_root_test");
        let deep = dir.join("a").join("b").join("c");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(dir.join("forge")).unwrap();
        std::fs::write(dir.join("forge").join("registry.json"), "{}").unwrap();

        // Depuis un sous-répertoire profond, on remonte jusqu'au dépôt.
        assert_eq!(find_repo_root(&deep).as_deref(), Some(dir.as_path()));
        // La racine elle-même convient.
        assert_eq!(find_repo_root(&dir).as_deref(), Some(dir.as_path()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn une_arborescence_sans_forge_ne_ment_pas() {
        let dir = std::env::temp_dir().join("nie_forge_absent_test");
        std::fs::create_dir_all(&dir).unwrap();
        // Un `forge/` sans registre ne compte pas : c'est le registre qui
        // atteste d'un dépôt niers, pas le nom du répertoire.
        std::fs::create_dir_all(dir.join("forge")).unwrap();
        assert!(resolve_root(Some(dir.display().to_string())).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
