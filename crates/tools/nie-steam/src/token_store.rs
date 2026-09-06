//! Cache JSON des jetons d'authentification Steam par compte.
//!
//! Port de `IECODE.Core/Steam/Content/SteamTokenStore.cs` (C#).
//! Chaque entrée stocke un `refresh_token` (re-login sans mot de passe) et
//! optionnellement un `guard_data` (remembered machine, évite le 2FA répété).
//!
//! Format JSON sur disque :
//! ```json
//! {
//!   "alice": { "refreshToken": "eyJ...", "guardData": null },
//!   "Bob":   { "refreshToken": "eyJ...", "guardData": "abc" }
//! }
//! ```
//!
//! La clé est **insensible à la casse** (comme en C# avec `StringComparer.OrdinalIgnoreCase`).
//! Toutes les erreurs I/O sont silencieuses (best-effort) : l'échec de persistance
//! ne doit jamais interrompre un download.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::warn;

/// Chemin par défaut du token store niers.
/// `~/.local/share/niers/steam-tokens.json`
pub fn default_path() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("niers")
        .join("steam-tokens.json")
}

/// Jetons persistés pour un compte (refresh token + guard data optionnel).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccountTokens {
    /// Refresh token Steam (JWT opaque). Permet de se reconnecter sans mot de passe.
    #[serde(rename = "refreshToken", skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Guard data ("machine remembered"). Si présent, évite de redemander le 2FA.
    #[serde(rename = "guardData", skip_serializing_if = "Option::is_none")]
    pub guard_data: Option<String>,
}

/// Cache JSON thread-safe des jetons par compte.
///
/// Charge depuis `path` au démarrage (si le fichier existe).
/// Toute écriture est immédiatement persistée (best-effort).
/// Thread-safe via `Mutex` interne.
pub struct TokenStore {
    path: Option<PathBuf>,
    // Clés normalisées en minuscules pour l'insensibilité à la casse.
    inner: Mutex<HashMap<String, AccountTokens>>,
}

impl TokenStore {
    /// Charge le token store depuis `path`.
    ///
    /// Si `path` est `None` ou que le fichier n'existe pas, renvoie un store vide
    /// non persistant (comportement identique au C# `SteamTokenStore.Load(null)`).
    pub fn load(path: Option<&Path>) -> Self {
        let Some(p) = path else {
            return Self {
                path: None,
                inner: Mutex::new(HashMap::new()),
            };
        };

        let map = std::fs::read_to_string(p)
            .ok()
            .and_then(|text| {
                serde_json::from_str::<HashMap<String, AccountTokens>>(&text)
                    .map_err(|e| {
                        warn!(
                            "token-store {}: JSON invalide ({e}), réinitialisation",
                            p.display()
                        );
                        e
                    })
                    .ok()
            })
            .unwrap_or_default();

        // Normalise les clés en minuscules.
        let normalized: HashMap<String, AccountTokens> = map
            .into_iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v))
            .collect();

        Self {
            path: Some(p.to_owned()),
            inner: Mutex::new(normalized),
        }
    }

    /// Retourne les jetons pour `account`. Ne renvoie jamais d'erreur :
    /// si le compte est inconnu, renvoie des `AccountTokens` par défaut (tous `None`).
    pub fn get(&self, account: &str) -> AccountTokens {
        let key = account.to_ascii_lowercase();
        self.inner
            .lock()
            .expect("token-store lock poisoned")
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }

    /// Met à jour (ou crée) les jetons du compte, puis persiste best-effort.
    ///
    /// Seuls les champs `Some(...)` écrasent la valeur existante ; un `None`
    /// laisse la valeur en place (même sémantique que le C# `Set`).
    pub fn set(&self, account: &str, refresh_token: Option<&str>, guard_data: Option<&str>) {
        if self.path.is_none() {
            return;
        }

        let key = account.to_ascii_lowercase();
        let mut guard = self.inner.lock().expect("token-store lock poisoned");
        let entry = guard.entry(key).or_default();
        if let Some(t) = refresh_token {
            entry.refresh_token = if t.is_empty() {
                None
            } else {
                Some(t.to_owned())
            };
        }
        if let Some(g) = guard_data {
            entry.guard_data = if g.is_empty() {
                None
            } else {
                Some(g.to_owned())
            };
        }
        self.save_locked(&guard);
    }

    /// Persiste le store sur disque (best-effort, silencieux en cas d'erreur).
    fn save_locked(&self, data: &HashMap<String, AccountTokens>) {
        let Some(ref path) = self.path else { return };

        // Réhydrate avec les clés telles qu'elles sont dans la map.
        let serializable: &HashMap<String, AccountTokens> = data;

        let result = (|| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_string_pretty(serializable).map_err(std::io::Error::other)?;
            std::fs::write(path, json)?;
            Ok::<(), std::io::Error>(())
        })();

        if let Err(e) = result {
            warn!("token-store {}: échec de sauvegarde ({e})", path.display());
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    /// Ecrit un JSON de token store dans un fichier temporaire et le charge.
    fn write_and_load(json: &str) -> (TokenStore, PathBuf) {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_owned();
        std::fs::write(&path, json).unwrap();
        // Garde le NamedTempFile en vie via `keep` pour que le path reste valide.
        let _ = file.keep();
        let store = TokenStore::load(Some(&path));
        (store, path)
    }

    #[test]
    fn load_empty_when_path_none() {
        let store = TokenStore::load(None);
        let tokens = store.get("alice");
        assert!(tokens.refresh_token.is_none());
        assert!(tokens.guard_data.is_none());
    }

    #[test]
    fn load_nonexistent_file_returns_empty() {
        let path = PathBuf::from("/tmp/nie-steam-tests-nonexistent-99999.json");
        let store = TokenStore::load(Some(&path));
        assert!(store.get("alice").refresh_token.is_none());
    }

    #[test]
    fn round_trip_set_save_load_get() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();
        let _ = tmp.keep();

        let store = TokenStore::load(Some(&path));
        store.set("Alice", Some("eyJ_refresh"), Some("guard_abc"));

        // Reload depuis le fichier sauvegardé.
        let store2 = TokenStore::load(Some(&path));
        let tokens = store2.get("alice"); // insensible casse
        assert_eq!(tokens.refresh_token.as_deref(), Some("eyJ_refresh"));
        assert_eq!(tokens.guard_data.as_deref(), Some("guard_abc"));
    }

    #[test]
    fn case_insensitive_get() {
        let json = r#"{"ALICE":{"refreshToken":"tok_a"}}"#;
        let (store, _) = write_and_load(json);
        // Quel que soit la casse, on doit trouver le token.
        assert_eq!(store.get("alice").refresh_token.as_deref(), Some("tok_a"));
        assert_eq!(store.get("ALICE").refresh_token.as_deref(), Some("tok_a"));
        assert_eq!(store.get("Alice").refresh_token.as_deref(), Some("tok_a"));
    }

    #[test]
    fn set_partial_update_preserves_existing() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();
        let _ = tmp.keep();

        let store = TokenStore::load(Some(&path));
        store.set("bob", Some("initial_token"), Some("initial_guard"));

        // Met à jour seulement le refresh_token, guard_data doit rester.
        store.set("bob", Some("new_token"), None);
        let tokens = store.get("bob");
        assert_eq!(tokens.refresh_token.as_deref(), Some("new_token"));
        assert_eq!(tokens.guard_data.as_deref(), Some("initial_guard"));
    }

    #[test]
    fn empty_string_clears_token() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();
        let _ = tmp.keep();

        let store = TokenStore::load(Some(&path));
        store.set("charlie", Some("tok"), None);
        store.set("charlie", Some(""), None); // vide → efface
        assert!(store.get("charlie").refresh_token.is_none());
    }

    #[test]
    fn unknown_account_returns_default() {
        let store = TokenStore::load(None);
        let tokens = store.get("nobody");
        assert!(tokens.refresh_token.is_none());
        assert!(tokens.guard_data.is_none());
    }

    #[test]
    fn persists_after_set_reload() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();
        let _ = tmp.keep();

        {
            let store = TokenStore::load(Some(&path));
            store.set("dave", Some("dave_tok"), Some("dave_guard"));
        }

        // Nouvel objet store chargé depuis le même fichier.
        let store2 = TokenStore::load(Some(&path));
        let t = store2.get("dave");
        assert_eq!(t.refresh_token.as_deref(), Some("dave_tok"));
        assert_eq!(t.guard_data.as_deref(), Some("dave_guard"));
    }
}
