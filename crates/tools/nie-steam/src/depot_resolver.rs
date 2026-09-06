//! Sélection des depots Steam depuis la section KeyValue `depots` d'une app info PICS.
//!
//! Port fidèle de `IECODE.Core/Steam/Content/SteamDepotResolver.cs` (C#, SteamKit2).
//! Logique **pure**, sans I/O — testable unitairement sans connexion Steam.
//!
//! La représentation [`KeyValue`] vient de [`steamroom::types::key_value`].
//! Contrairement à SteamKit2, les enfants sont un [`BTreeMap<String, KeyValue>`]
//! (accès par `.get("clé")`), non une liste indexée.

use steamroom::types::key_value::{KeyValue, KvValue};

/// Valeur de sentinel : manifest absent / introuvable.
pub const INVALID_MANIFEST: u64 = 0;

/// Sélectionne les depot IDs éligibles depuis la section `depots` de l'app info.
///
/// Si `explicit_depots` est non vide, il est renvoyé tel quel (dédupliqué, même
/// comportement que le C#). Sinon parcourt tous les enfants numériques et filtre
/// par OS/arch/langue/lowviolence (sauf si `all_platforms` est vrai).
///
/// # Paramètres
/// - `depots` : le nœud KV `app_info["depots"]`
/// - `explicit_depots` : IDs imposés par l'utilisateur (court-circuit si non vide)
/// - `all_platforms` : ignorer les filtres OS/arch/langue si `true`
/// - `os` : `"windows"`, `"macos"` ou `"linux"`
/// - `arch` : `Some("64")`, `Some("32")` ou `None` (pas de filtre arch)
/// - `language` : ex. `"english"`, `"french"` …
pub fn select_depot_ids(
    depots: &KeyValue,
    explicit_depots: &[u32],
    all_platforms: bool,
    os: &str,
    arch: Option<&str>,
    language: &str,
) -> Vec<u32> {
    if !explicit_depots.is_empty() {
        // Déduplique en conservant l'ordre de première apparition.
        let mut seen = std::collections::HashSet::new();
        return explicit_depots
            .iter()
            .filter(|&&id| seen.insert(id))
            .copied()
            .collect();
    }

    let KvValue::Children(ref map) = depots.value else {
        return vec![];
    };

    let mut found = Vec::new();
    for (key, depot) in map {
        // N'accepter que les entrées dont la clé est un entier (depot ID).
        let Ok(id) = key.parse::<u32>() else { continue };

        // Un depot sans enfants est un placeholder — ignorer.
        if !has_children(depot) {
            continue;
        }

        if !all_platforms && !is_depot_eligible(depot, os, arch, language) {
            continue;
        }

        found.push(id);
    }
    found
}

/// Teste l'éligibilité d'un nœud depot vis-à-vis des filtres OS/arch/langue/lowviolence.
///
/// Port exact de `SteamDepotResolver.IsDepotEligible` :
/// - `config` absent → éligible (pas de restriction)
/// - `oslist` non vide → `os` doit en faire partie (CSV)
/// - `osarch` non vide + `arch` fourni → doit correspondre exactement
/// - `language` non vide → doit correspondre exactement
/// - `lowviolence` présent et vrai → exclu
pub fn is_depot_eligible(depot: &KeyValue, os: &str, arch: Option<&str>, language: &str) -> bool {
    let Some(config) = depot.get("config") else {
        return true;
    };

    // oslist : liste CSV d'OS autorisés.
    if let Some(oslist_str) = kv_string(config, "oslist")
        && !oslist_str.trim().is_empty()
    {
        let in_list = oslist_str.split(',').any(|entry| entry.trim() == os);
        if !in_list {
            return false;
        }
    }

    // osarch : architecture exacte si spécifiée.
    if let Some(arch_filter) = arch
        && let Some(osarch_str) = kv_string(config, "osarch")
        && !osarch_str.trim().is_empty()
        && osarch_str.trim() != arch_filter
    {
        return false;
    }

    // language : langue exacte.
    if let Some(lang_str) = kv_string(config, "language")
        && !lang_str.trim().is_empty()
        && lang_str.trim() != language
    {
        return false;
    }

    // lowviolence : présent et truthy → exclu.
    if let Some(lv) = config.get("lowviolence")
        && kv_as_bool(lv)
    {
        return false;
    }

    true
}

/// Lit le GID du manifest d'un depot pour une branche donnée.
///
/// Chemin KV : `depot_child["manifests"][branch]["gid"]`.
/// Renvoie [`INVALID_MANIFEST`] si la clé est absente ou non parsable.
pub fn read_manifest_gid(depot_child: &KeyValue, branch: &str) -> u64 {
    depot_child
        .get("manifests")
        .and_then(|m| m.get(branch))
        .and_then(|b| b.get("gid"))
        .and_then(|g| {
            // gid peut être stocké comme String ou UInt64.
            match &g.value {
                KvValue::String(s) => s.parse::<u64>().ok(),
                KvValue::UInt64(v) => Some(*v),
                KvValue::Int64(v) => u64::try_from(*v).ok(),
                _ => None,
            }
        })
        .unwrap_or(INVALID_MANIFEST)
}

/// Indique si le depot est proxié vers une autre app (`depotfromapp`).
///
/// Renvoie l'app ID cible si le champ `depotfromapp` existe et que `manifests`
/// est absent (comportement identique au C#). Renvoie `0` sinon.
pub fn proxied_from_app(depot_child: &KeyValue) -> u32 {
    // Si le depot a ses propres manifests, il n'est pas un proxy.
    if depot_child.get("manifests").is_some() {
        return 0;
    }
    depot_child
        .get("depotfromapp")
        .and_then(kv_as_u32)
        .unwrap_or(0)
}

/// Lit le `buildid` d'une branche dans la section `depots`.
///
/// Chemin KV : `depots["branches"][branch]["buildid"]`.
/// Sert à détecter si une nouvelle version du jeu est disponible (buildid change
/// à chaque mise à jour). Renvoie `0` si le buildid est absent.
pub fn read_branch_build_id(depots: &KeyValue, branch: &str) -> u32 {
    depots
        .get("branches")
        .and_then(|b| b.get(branch))
        .and_then(|b| b.get("buildid"))
        .and_then(|v| match &v.value {
            KvValue::String(s) => s.parse::<u32>().ok(),
            KvValue::Int32(n) => u32::try_from(*n).ok(),
            KvValue::UInt64(n) => u32::try_from(*n).ok(),
            _ => None,
        })
        .unwrap_or(0)
}

// ─── Helpers internes ─────────────────────────────────────────────────────────

/// `true` si le nœud KV a au moins un enfant (Children non vide).
fn has_children(kv: &KeyValue) -> bool {
    matches!(&kv.value, KvValue::Children(m) if !m.is_empty())
}

/// Retourne la valeur String d'un champ enfant, ou `None`.
fn kv_string<'a>(parent: &'a KeyValue, key: &str) -> Option<&'a str> {
    parent.get(key).and_then(|v| v.as_str())
}

/// Interprète un nœud KV comme booléen : "1" / non-nul → vrai.
fn kv_as_bool(kv: &KeyValue) -> bool {
    match &kv.value {
        KvValue::String(s) => s.trim() == "1" || s.trim().eq_ignore_ascii_case("true"),
        KvValue::Int32(n) => *n != 0,
        KvValue::UInt64(n) => *n != 0,
        _ => false,
    }
}

/// Interprète un nœud KV comme `u32`.
fn kv_as_u32(kv: &KeyValue) -> Option<u32> {
    match &kv.value {
        KvValue::String(s) => s.parse::<u32>().ok(),
        KvValue::Int32(n) => u32::try_from(*n).ok(),
        KvValue::UInt64(n) => u32::try_from(*n).ok(),
        _ => None,
    }
}

// ─── Constructeur de KeyValue de test ─────────────────────────────────────────

#[cfg(test)]
mod kv_builder {
    use std::collections::BTreeMap;
    use steamroom::types::key_value::{KeyValue, KvValue};

    /// Crée un nœud KV enfants (section / object).
    pub fn children(key: &str, pairs: Vec<(&str, KeyValue)>) -> KeyValue {
        let mut map = BTreeMap::new();
        for (k, v) in pairs {
            map.insert(k.to_string(), v);
        }
        KeyValue {
            key: key.to_string(),
            value: KvValue::Children(map),
        }
    }

    /// Crée un nœud KV feuille String.
    pub fn str_val(key: &str, val: &str) -> KeyValue {
        KeyValue {
            key: key.to_string(),
            value: KvValue::String(val.to_string()),
        }
    }

    /// Crée un nœud KV feuille Int32.
    pub fn int_val(key: &str, val: i32) -> KeyValue {
        KeyValue {
            key: key.to_string(),
            value: KvValue::Int32(val),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use kv_builder::*;

    /// Construit un nœud `depots` minimal avec un seul depot.
    /// Le depot a un nœud `config` avec les champs voulus.
    fn make_depot_with_config(
        depot_id: u32,
        oslist: Option<&str>,
        osarch: Option<&str>,
        language: Option<&str>,
        lowviolence: Option<&str>,
    ) -> KeyValue {
        let mut config_children = vec![];
        if let Some(v) = oslist {
            config_children.push(("oslist", str_val("oslist", v)));
        }
        if let Some(v) = osarch {
            config_children.push(("osarch", str_val("osarch", v)));
        }
        if let Some(v) = language {
            config_children.push(("language", str_val("language", v)));
        }
        if let Some(v) = lowviolence {
            config_children.push(("lowviolence", str_val("lowviolence", v)));
        }

        let config = children("config", config_children);
        let depot = children(&depot_id.to_string(), vec![("config", config)]);
        children("depots", vec![(&depot_id.to_string(), depot)])
    }

    /// Construit un nœud `depots` avec un depot qui a un nœud `manifests`.
    fn make_depot_with_manifest(depot_id: u32, branch: &str, gid: u64) -> KeyValue {
        let gid_node = str_val("gid", &gid.to_string());
        let branch_node = children(branch, vec![("gid", gid_node)]);
        let manifests = children("manifests", vec![(branch, branch_node)]);
        let depot = children(&depot_id.to_string(), vec![("manifests", manifests)]);
        children("depots", vec![(&depot_id.to_string(), depot)])
    }

    // ── select_depot_ids ─────────────────────────────────────────────────────

    #[test]
    fn explicit_depots_returned_deduped() {
        let depots = children("depots", vec![]);
        let result = select_depot_ids(
            &depots,
            &[10, 20, 10, 30],
            false,
            "windows",
            None,
            "english",
        );
        assert_eq!(result, vec![10, 20, 30]);
    }

    #[test]
    fn eligible_depot_selected_windows() {
        let depots = make_depot_with_config(12345, Some("windows"), None, None, None);
        let ids = select_depot_ids(&depots, &[], false, "windows", None, "english");
        assert_eq!(ids, vec![12345]);
    }

    #[test]
    fn ineligible_depot_filtered_wrong_os() {
        let depots = make_depot_with_config(12345, Some("macos"), None, None, None);
        let ids = select_depot_ids(&depots, &[], false, "windows", None, "english");
        assert!(ids.is_empty());
    }

    #[test]
    fn all_platforms_bypasses_os_filter() {
        let depots = make_depot_with_config(12345, Some("macos"), None, None, None);
        let ids = select_depot_ids(&depots, &[], true, "windows", None, "english");
        assert_eq!(ids, vec![12345]);
    }

    #[test]
    fn depot_without_config_is_eligible() {
        // Un depot sans nœud "config" est toujours éligible.
        let manifest_gid = str_val("gid", "99999");
        let branch_node = children("public", vec![("gid", manifest_gid)]);
        let manifests = children("manifests", vec![("public", branch_node)]);
        let depot = children("9999", vec![("manifests", manifests)]);
        let depots = children("depots", vec![("9999", depot)]);

        let ids = select_depot_ids(&depots, &[], false, "windows", None, "english");
        assert_eq!(ids, vec![9999]);
    }

    // ── is_depot_eligible ────────────────────────────────────────────────────

    #[test]
    fn eligible_when_no_config() {
        let depot = children("42", vec![]);
        assert!(is_depot_eligible(&depot, "windows", None, "english"));
    }

    #[test]
    fn ineligible_lowviolence_true() {
        let config = children("config", vec![("lowviolence", str_val("lowviolence", "1"))]);
        let depot = children("42", vec![("config", config)]);
        assert!(!is_depot_eligible(&depot, "windows", None, "english"));
    }

    #[test]
    fn ineligible_lowviolence_zero_is_ok() {
        let config = children("config", vec![("lowviolence", str_val("lowviolence", "0"))]);
        let depot = children("42", vec![("config", config)]);
        assert!(is_depot_eligible(&depot, "windows", None, "english"));
    }

    #[test]
    fn ineligible_wrong_language() {
        let config = children("config", vec![("language", str_val("language", "french"))]);
        let depot = children("42", vec![("config", config)]);
        assert!(!is_depot_eligible(&depot, "windows", None, "english"));
    }

    #[test]
    fn eligible_correct_language() {
        let config = children("config", vec![("language", str_val("language", "english"))]);
        let depot = children("42", vec![("config", config)]);
        assert!(is_depot_eligible(&depot, "windows", None, "english"));
    }

    #[test]
    fn ineligible_wrong_arch() {
        let config = children("config", vec![("osarch", str_val("osarch", "32"))]);
        let depot = children("42", vec![("config", config)]);
        assert!(!is_depot_eligible(&depot, "windows", Some("64"), "english"));
    }

    #[test]
    fn arch_none_skips_arch_filter() {
        let config = children("config", vec![("osarch", str_val("osarch", "32"))]);
        let depot = children("42", vec![("config", config)]);
        // arch=None → aucun filtre arch
        assert!(is_depot_eligible(&depot, "windows", None, "english"));
    }

    // ── read_manifest_gid ────────────────────────────────────────────────────

    #[test]
    fn manifest_gid_read_correctly() {
        let depots = make_depot_with_manifest(5555, "public", 123456789012345);
        let depot_child = depots.get("5555").unwrap();
        assert_eq!(read_manifest_gid(depot_child, "public"), 123456789012345);
    }

    #[test]
    fn manifest_gid_absent_returns_invalid() {
        let depot = children("5555", vec![]);
        assert_eq!(read_manifest_gid(&depot, "public"), INVALID_MANIFEST);
    }

    #[test]
    fn manifest_gid_wrong_branch() {
        let depots = make_depot_with_manifest(5555, "beta", 42);
        let depot_child = depots.get("5555").unwrap();
        assert_eq!(read_manifest_gid(depot_child, "public"), INVALID_MANIFEST);
    }

    // ── proxied_from_app ────────────────────────────────────────────────────

    #[test]
    fn proxied_from_app_returns_target() {
        // Pas de "manifests", mais "depotfromapp" présent.
        let depot = children(
            "42",
            vec![("depotfromapp", str_val("depotfromapp", "9999"))],
        );
        assert_eq!(proxied_from_app(&depot), 9999);
    }

    #[test]
    fn proxied_from_app_zero_when_has_manifests() {
        let gid = str_val("gid", "1");
        let branch = children("public", vec![("gid", gid)]);
        let manifests = children("manifests", vec![("public", branch)]);
        let depot = children(
            "42",
            vec![
                ("manifests", manifests),
                ("depotfromapp", str_val("depotfromapp", "9999")),
            ],
        );
        // Manifests présents → pas un proxy
        assert_eq!(proxied_from_app(&depot), 0);
    }

    #[test]
    fn proxied_from_app_zero_absent() {
        let depot = children("42", vec![]);
        assert_eq!(proxied_from_app(&depot), 0);
    }

    // ── read_branch_build_id ─────────────────────────────────────────────────

    #[test]
    fn build_id_read_correctly() {
        let build_node = str_val("buildid", "7654321");
        let branch_node = children("public", vec![("buildid", build_node)]);
        let branches = children("branches", vec![("public", branch_node)]);
        let depots = children("depots", vec![("branches", branches)]);
        assert_eq!(read_branch_build_id(&depots, "public"), 7654321);
    }

    #[test]
    fn build_id_absent_returns_zero() {
        let depots = children("depots", vec![]);
        assert_eq!(read_branch_build_id(&depots, "public"), 0);
    }

    #[test]
    fn build_id_int32_variant() {
        // Si le buildid est stocké en Int32 plutôt qu'en String.
        let build_node = int_val("buildid", 42);
        let branch_node = children("public", vec![("buildid", build_node)]);
        let branches = children("branches", vec![("public", branch_node)]);
        let depots = children("depots", vec![("branches", branches)]);
        assert_eq!(read_branch_build_id(&depots, "public"), 42);
    }
}
