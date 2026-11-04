//! Secret commands. Write-only from the frontend's point of view: there is
//! deliberately no command returning a secret value.

use serde::Serialize;

use super::CommandResult;
use crate::secrets::{SecretKind, Secrets};

#[derive(Debug, Serialize, specta::Type)]
pub struct SecretStatus {
    pub kind: SecretKind,
    pub present: bool,
}

/// Which secrets are stored (never their values).
#[tauri::command]
#[specta::specta]
pub fn secrets_status(store: tauri::State<'_, Secrets>) -> CommandResult<Vec<SecretStatus>> {
    SecretKind::ALL
        .iter()
        .map(|&kind| {
            Ok(SecretStatus {
                kind,
                present: store.has(kind)?,
            })
        })
        .collect()
}

#[tauri::command]
#[specta::specta]
pub fn secrets_set(
    store: tauri::State<'_, Secrets>,
    kind: SecretKind,
    value: String,
) -> CommandResult<()> {
    store.set(kind, &value)?;
    tracing::info!(?kind, "secret stored");
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn secrets_clear(store: tauri::State<'_, Secrets>, kind: SecretKind) -> CommandResult<()> {
    store.clear(kind)?;
    tracing::info!(?kind, "secret cleared");
    Ok(())
}
