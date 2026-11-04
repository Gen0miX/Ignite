//! API keys and tokens, stored in the OS vault (Windows Credential Manager).
//!
//! Rules (CLAUDE.md): secrets never reach the frontend, the logs or SQLite.
//! The frontend can only set, clear, or ask whether a secret is present;
//! reading a value is reserved to backend code (`SecretStore::get`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Service name under which every credential is filed in the vault.
/// Entries appear as `<kind>.ludotheque` in Credential Manager.
pub const SERVICE: &str = "ludotheque";

/// Longest secret accepted (Credential Manager caps blobs at 2560 bytes).
const MAX_SECRET_LEN: usize = 2048;

/// Every secret the app knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKind {
    SteamWebApiKey,
    SteamGridDbApiKey,
    IgdbClientId,
    IgdbClientSecret,
    RetroAchievementsApiKey,
}

impl SecretKind {
    pub const ALL: &'static [SecretKind] = &[
        SecretKind::SteamWebApiKey,
        SecretKind::SteamGridDbApiKey,
        SecretKind::IgdbClientId,
        SecretKind::IgdbClientSecret,
        SecretKind::RetroAchievementsApiKey,
    ];

    /// Account name used in the vault.
    pub fn account(self) -> &'static str {
        match self {
            SecretKind::SteamWebApiKey => "steam_web_api_key",
            SecretKind::SteamGridDbApiKey => "steamgriddb_api_key",
            SecretKind::IgdbClientId => "igdb_client_id",
            SecretKind::IgdbClientSecret => "igdb_client_secret",
            SecretKind::RetroAchievementsApiKey => "retroachievements_api_key",
        }
    }
}

/// Storage backend for secrets. Implemented by the OS vault and by an
/// in-memory store used in tests.
pub trait SecretStore: Send + Sync {
    fn set(&self, kind: SecretKind, value: &str) -> AppResult<()>;
    /// Backend-only: never expose this through a Tauri command.
    fn get(&self, kind: SecretKind) -> AppResult<Option<String>>;
    /// Removing a missing secret is not an error.
    fn clear(&self, kind: SecretKind) -> AppResult<()>;

    fn has(&self, kind: SecretKind) -> AppResult<bool> {
        Ok(self.get(kind)?.is_some())
    }
}

/// Trims and validates a secret typed by the user.
pub fn normalize(value: &str) -> AppResult<&str> {
    let v = value.trim();
    if v.is_empty() {
        return Err(AppError::Invalid("secret is empty".into()));
    }
    if v.len() > MAX_SECRET_LEN {
        return Err(AppError::Invalid("secret is too long".into()));
    }
    if v.chars().any(char::is_control) {
        return Err(AppError::Invalid(
            "secret contains control characters".into(),
        ));
    }
    Ok(v)
}

/// Shared handle stored in Tauri state.
pub type Secrets = Arc<dyn SecretStore>;

/// Secret store backed by the OS credential vault through `keyring-core`.
pub struct KeyringStore {
    store: Arc<keyring_core::CredentialStore>,
    service: String,
    // Credential Manager does not reliably order concurrent operations on the
    // same entry (see windows-native-keyring-store README): serialize them.
    lock: Mutex<()>,
}

impl KeyringStore {
    #[cfg(windows)]
    pub fn new() -> AppResult<Self> {
        Self::with_service(SERVICE)
    }

    /// Separate service name, used by the integration test so it never touches
    /// the user's real entries.
    #[cfg(windows)]
    pub fn with_service(service: &str) -> AppResult<Self> {
        let store = windows_native_keyring_store::Store::new().map_err(vault_error)?;
        Ok(Self {
            store,
            service: service.to_string(),
            lock: Mutex::new(()),
        })
    }

    fn entry(&self, kind: SecretKind) -> AppResult<keyring_core::Entry> {
        self.store
            .build(&self.service, kind.account(), None)
            .map_err(vault_error)
    }
}

fn vault_error(e: keyring_core::Error) -> AppError {
    // keyring errors describe the failure, never the secret value.
    AppError::Secret(e.to_string())
}

impl SecretStore for KeyringStore {
    fn set(&self, kind: SecretKind, value: &str) -> AppResult<()> {
        let value = normalize(value)?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.entry(kind)?.set_password(value).map_err(vault_error)
    }

    fn get(&self, kind: SecretKind) -> AppResult<Option<String>> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        match self.entry(kind)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(vault_error(e)),
        }
    }

    fn clear(&self, kind: SecretKind) -> AppResult<()> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        match self.entry(kind)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(vault_error(e)),
        }
    }
}

/// In-memory store for tests and non-Windows dev builds.
#[derive(Default)]
pub struct MemoryStore {
    values: Mutex<HashMap<SecretKind, String>>,
}

impl SecretStore for MemoryStore {
    fn set(&self, kind: SecretKind, value: &str) -> AppResult<()> {
        let value = normalize(value)?;
        self.values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(kind, value.to_string());
        Ok(())
    }

    fn get(&self, kind: SecretKind) -> AppResult<Option<String>> {
        Ok(self
            .values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&kind)
            .cloned())
    }

    fn clear(&self, kind: SecretKind) -> AppResult<()> {
        self.values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&kind);
        Ok(())
    }
}

/// The store used by the app: the OS vault on Windows.
pub fn default_store() -> AppResult<Secrets> {
    #[cfg(windows)]
    {
        Ok(Arc::new(KeyringStore::new()?))
    }
    #[cfg(not(windows))]
    {
        tracing::warn!("no OS vault backend on this platform: secrets kept in memory only");
        Ok(Arc::new(MemoryStore::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exercise(store: &dyn SecretStore) {
        let kind = SecretKind::SteamGridDbApiKey;
        store.clear(kind).unwrap();
        assert!(!store.has(kind).unwrap());
        assert_eq!(store.get(kind).unwrap(), None);

        store.set(kind, "  abc123  ").unwrap();
        assert!(store.has(kind).unwrap());
        assert_eq!(store.get(kind).unwrap().as_deref(), Some("abc123"));

        store.set(kind, "def456").unwrap();
        assert_eq!(store.get(kind).unwrap().as_deref(), Some("def456"));
        assert!(!store.has(SecretKind::IgdbClientId).unwrap());

        store.clear(kind).unwrap();
        store.clear(kind).unwrap(); // idempotent
        assert!(!store.has(kind).unwrap());
    }

    #[test]
    fn memory_store_round_trip() {
        exercise(&MemoryStore::default());
    }

    #[test]
    fn rejects_invalid_values() {
        let store = MemoryStore::default();
        let kind = SecretKind::SteamWebApiKey;
        assert!(matches!(store.set(kind, "   "), Err(AppError::Invalid(_))));
        assert!(matches!(store.set(kind, "a\nb"), Err(AppError::Invalid(_))));
        assert!(matches!(
            store.set(kind, &"x".repeat(MAX_SECRET_LEN + 1)),
            Err(AppError::Invalid(_))
        ));
        assert!(!store.has(kind).unwrap());
    }

    #[test]
    fn accounts_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for k in SecretKind::ALL {
            assert!(seen.insert(k.account()));
        }
    }

    #[test]
    fn errors_do_not_leak_the_value() {
        let store = MemoryStore::default();
        let err = store
            .set(SecretKind::SteamWebApiKey, "secret\u{7}value")
            .unwrap_err();
        assert!(!err.to_string().contains("secret\u{7}value"));
    }

    /// Writes to the real Windows Credential Manager under a test-only service
    /// name, then removes it. Run with `cargo test -- --ignored`.
    #[cfg(windows)]
    #[test]
    #[ignore = "touches the real Windows Credential Manager"]
    fn keyring_store_round_trip_on_windows() {
        let store = KeyringStore::with_service("ludotheque-test").unwrap();
        exercise(&store);
    }
}
