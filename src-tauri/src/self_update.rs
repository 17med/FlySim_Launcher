//! Updates the launcher itself from the releases of its own repository, through Tauri's updater
//! plugin. The endpoint and the public key live in `tauri.conf.json > plugins > updater`; the
//! plugin refuses any download whose signature does not match that key.

use crate::commands::{AppState, BusyGuard};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// The update found by the last check. Installing uses this one rather than checking again, so the
/// player gets exactly the version they were shown.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

impl PendingUpdate {
    fn replace(&self, update: Option<Update>) {
        *self.0.lock().expect("pending launcher update lock poisoned") = update;
    }

    fn current(&self) -> Option<Update> {
        self.0.lock().expect("pending launcher update lock poisoned").clone()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherUpdate {
    version: String,
    current_version: String,
    notes: Option<String>,
}

#[derive(Serialize, Clone)]
struct Progress {
    done: u64,
    total: Option<u64>,
}

/// Installed by the Arch package (packaging/aur/PKGBUILD). Its binary is the one from the .deb,
/// whose updater runs `pkexec dpkg -i`: Arch has no dpkg, and pacman must own the files anyway.
const PACKAGE_MANAGED_MARKER: &str = "/usr/share/flysim-launcher/package-managed";

/// Fails closed: a launcher installed by a package manager never replaces its own files.
fn refuse_if_package_managed(marker: &Path) -> Result<(), String> {
    if marker.exists() {
        return Err("This launcher was installed with pacman: download the new .pkg.tar.zst from GitHub Releases and install it with sudo pacman -U.".into());
    }
    Ok(())
}

/// `None` when this launcher is the newest one published.
#[tauri::command]
pub async fn check_launcher_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<LauncherUpdate>, String> {
    refuse_if_package_managed(Path::new(PACKAGE_MANAGED_MARKER))?;
    let update = app
        .updater()
        .map_err(|e| format!("The launcher updater is not set up: {e}"))?
        .check()
        .await
        .map_err(|e| format!("Could not check for a new launcher: {e}"))?;
    let summary = update.as_ref().map(|u| LauncherUpdate {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone(),
    });
    pending.replace(update);
    Ok(summary)
}

/// Downloads, verifies and installs the pending update, then restarts into the new launcher.
/// On Windows the installer closes and reopens the launcher itself, so this never returns there.
#[tauri::command]
pub async fn install_launcher_update(
    app: AppHandle,
    state: State<'_, AppState>,
    pending: State<'_, PendingUpdate>,
) -> Result<(), String> {
    refuse_if_package_managed(Path::new(PACKAGE_MANAGED_MARKER))?;
    // Fails closed: restarting in the middle of a game install would throw that install away.
    let _guard = BusyGuard::acquire(&state.busy)?;
    let update = pending
        .current()
        .ok_or("No launcher update is waiting. Check for updates again.")?;
    let mut done = 0u64;
    let mut last_percent = None;
    update
        .download_and_install(
            |chunk, total| {
                done += chunk as u64;
                let percent = total.map(|t| done * 100 / t.max(1));
                if percent != last_percent {
                    last_percent = percent;
                    emit_progress(&app, done, total);
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("Launcher update to {} failed: {e}", update.version))?;
    app.restart()
}

fn emit_progress(app: &AppHandle, done: u64, total: Option<u64>) {
    // Fails open: a lost progress event only leaves the progress bar behind, never the update.
    let _ = app.emit("launcher-update-progress", Progress { done, total });
}

#[cfg(test)]
mod tests {
    use super::refuse_if_package_managed;
    use serde_json::Value;

    fn tauri_conf() -> Value {
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json parses")
    }

    /// The updater compares the version in tauri.conf.json with latest.json. If it drifts from the
    /// crate version, releases get built under one number and advertised under another.
    #[test]
    fn app_version_matches_crate_version() {
        assert_eq!(tauri_conf()["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn updater_has_a_key_and_only_https_endpoints() {
        let updater = &tauri_conf()["plugins"]["updater"];
        assert!(!updater["pubkey"].as_str().unwrap_or_default().is_empty());
        let endpoints = updater["endpoints"].as_array().expect("endpoints is a list");
        assert!(!endpoints.is_empty());
        assert!(endpoints.iter().all(|e| e.as_str().is_some_and(|u| u.starts_with("https://"))));
    }

    /// Release builds must produce the signed files that latest.json points at.
    #[test]
    fn release_builds_create_updater_artifacts() {
        assert_eq!(tauri_conf()["bundle"]["createUpdaterArtifacts"], true);
    }

    #[test]
    fn package_managed_install_refuses_to_update_itself() {
        let marker = std::env::temp_dir().join(format!("flysim-launcher-marker-{}", std::process::id()));
        assert!(refuse_if_package_managed(&marker).is_ok());
        std::fs::write(&marker, "").unwrap();
        let refused = refuse_if_package_managed(&marker);
        let _ = std::fs::remove_file(&marker);
        assert!(refused.unwrap_err().contains("pacman -U"));
    }
}
