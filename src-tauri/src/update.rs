//! In-app updates from GitHub Releases. The feed and its minisign public key are in
//! `tauri.conf.json`; every download is verified against that key before it is installed.

use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// A downloaded, verified update waiting for the user to restart.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<(Update, Vec<u8>)>>);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
}

/// Checks the feed and, if there is a newer version, downloads and verifies it. Emits
/// `update-ready` once it can be installed.
pub async fn check_and_download(app: &tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    if let Some((update, _)) = app.state::<PendingUpdate>().0.lock().unwrap().as_ref() {
        return Ok(Some(info(update)));
    }
    let Some(update) = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    let info = info(&update);
    *app.state::<PendingUpdate>().0.lock().unwrap() = Some((update, bytes));
    let _ = app.emit("update-ready", &info);
    Ok(Some(info))
}

fn info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        version: update.version.clone(),
        notes: update.body.clone().unwrap_or_default(),
    }
}

/// Replaces the app bundle with the downloaded update and relaunches.
pub fn install_and_restart(app: &tauri::AppHandle) -> Result<(), String> {
    let pending = app.state::<PendingUpdate>().0.lock().unwrap().take();
    let Some((update, bytes)) = pending else {
        return Err("No update has been downloaded.".into());
    };
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format_matches_typescript_type() {
        let v = serde_json::to_value(UpdateInfo {
            version: "0.2.0".into(),
            notes: "Fixes".into(),
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({"version": "0.2.0", "notes": "Fixes"}));
    }
}
