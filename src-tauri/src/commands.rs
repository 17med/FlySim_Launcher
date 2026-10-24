//! The commands the UI can call. Each one is a thin step over `github` and `install`.

use crate::config::LauncherConfig;
use crate::github::{self, Asset, Release, Status};
use crate::install::{self, GameDirs};
use crate::settings;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, State};

pub struct AppState {
    config: LauncherConfig,
    client: reqwest::Client,
    settings_path: PathBuf,
    default_root: PathBuf,
    /// Set while an install or uninstall runs, so two can never touch the game folder at once.
    busy: AtomicBool,
}

impl AppState {
    pub fn new(config: LauncherConfig, settings_path: PathBuf, default_root: PathBuf) -> Self {
        AppState {
            config,
            client: github::http_client(),
            settings_path,
            default_root,
            busy: AtomicBool::new(false),
        }
    }

    fn dirs(&self) -> GameDirs {
        let root = settings::load(&self.settings_path)
            .install_dir
            .unwrap_or_else(|| self.default_root.clone());
        GameDirs::new(root, &self.config.game_name)
    }
}

struct BusyGuard<'a>(&'a AtomicBool);

impl<'a> BusyGuard<'a> {
    fn acquire(flag: &'a AtomicBool) -> Result<Self, String> {
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "Another install or uninstall is already running.".to_string())?;
        Ok(BusyGuard(flag))
    }
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherInfo {
    game_name: String,
    game_repo: String,
    trailer_youtube_id: String,
    install_dir: String,
    installed_version: Option<String>,
    can_play: bool,
}

#[derive(Serialize, Clone)]
struct Progress {
    stage: &'static str,
    done: u64,
    total: u64,
}

#[tauri::command]
pub fn get_info(state: State<'_, AppState>) -> LauncherInfo {
    let dirs = state.dirs();
    let config = &state.config;
    LauncherInfo {
        game_name: config.game_name.clone(),
        game_repo: config.game_repo.clone(),
        trailer_youtube_id: config.trailer_youtube_id.clone(),
        install_dir: dirs.root().display().to_string(),
        installed_version: install::read_installed(&dirs).map(|i| i.version),
        can_play: install::find_executable(&dirs, &config.platform().executables).is_some(),
    }
}

#[tauri::command]
pub async fn list_releases(state: State<'_, AppState>) -> Result<Vec<Release>, String> {
    let config = &state.config;
    github::fetch_releases(&state.client, &config.github_api, &config.game_repo).await
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<Option<Status>, String> {
    github::fetch_status(&state.client, &state.config.status_url).await
}

/// Installs `tag`, or updates/repairs to it: download, verify, unpack, then swap in.
#[tauri::command]
pub async fn install(app: AppHandle, state: State<'_, AppState>, tag: String) -> Result<(), String> {
    let _guard = BusyGuard::acquire(&state.busy)?;
    let (config, dirs) = (&state.config, state.dirs());
    std::fs::create_dir_all(dirs.root())
        .map_err(|e| format!("Cannot create {}: {e}", dirs.root().display()))?;
    // Re-read the release from GitHub rather than trusting a URL handed over by the UI.
    let release = github::fetch_release(&state.client, &config.github_api, &config.game_repo, &tag).await?;
    let platform = config.platform();
    let asset = github::find_asset(&release, &platform.asset_suffix)
        .ok_or_else(|| format!("Release {tag} has no build for this system ({}).", platform.asset_suffix))?;
    download_verified(&app, &state.client, asset, &dirs.download()).await?;

    emit_progress(&app, "extract", 0, 1);
    let names = platform.executables.clone();
    let unpacked = tauri::async_runtime::spawn_blocking(move || {
        let result = install::extract_to_staging(&dirs, &dirs.download())
            .and_then(|_| install::ensure_executable(&dirs, &names))
            .and_then(|_| install::activate_staging(&dirs, &tag));
        let _ = install::remove_if_exists(&dirs.download());
        result
    })
    .await
    .map_err(|e| format!("Install task stopped unexpectedly: {e}"))?;
    emit_progress(&app, "done", 1, 1);
    unpacked
}

async fn download_verified(
    app: &AppHandle,
    client: &reqwest::Client,
    asset: &Asset,
    dest: &Path,
) -> Result<(), String> {
    let total = asset.size.max(1);
    let mut last_percent = u64::MAX;
    let downloaded = github::download_asset(client, asset, dest, |done| {
        let percent = done * 100 / total;
        if percent != last_percent {
            last_percent = percent;
            emit_progress(app, "download", done, total);
        }
    })
    .await;
    let actual = downloaded.inspect_err(|_| {
        let _ = install::remove_if_exists(dest);
    })?;
    match github::expected_sha256(asset) {
        // Fails closed: a mismatch means the file is damaged or not what GitHub published.
        Some(expected) if !expected.eq_ignore_ascii_case(&actual) => {
            let _ = install::remove_if_exists(dest);
            Err(format!("{} failed its checksum check. Try again.", asset.name))
        }
        // Fails open when GitHub lists no digest: the file still came over HTTPS from GitHub.
        _ => Ok(()),
    }
}

fn emit_progress(app: &AppHandle, stage: &'static str, done: u64, total: u64) {
    // Fails open: a lost progress event only leaves the progress ring behind, never the install.
    let _ = app.emit("install-progress", Progress { stage, done, total });
}

#[tauri::command]
pub fn play(state: State<'_, AppState>) -> Result<(), String> {
    let dirs = state.dirs();
    let exe = install::find_executable(&dirs, &state.config.platform().executables)
        .ok_or("The game is not installed. Press Install first.")?;
    Command::new(&exe)
        .current_dir(dirs.game())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not start {}: {e}", exe.display()))
}

#[tauri::command]
pub async fn uninstall(state: State<'_, AppState>) -> Result<(), String> {
    let _guard = BusyGuard::acquire(&state.busy)?;
    let dirs = state.dirs();
    tauri::async_runtime::spawn_blocking(move || install::uninstall(&dirs))
        .await
        .map_err(|e| format!("Uninstall task stopped unexpectedly: {e}"))?
}

#[tauri::command]
pub fn set_install_dir(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let _guard = BusyGuard::acquire(&state.busy)?;
    let path = PathBuf::from(path.trim());
    if !path.is_absolute() {
        return Err(format!("{} is not a full path (e.g. /home/you/Games).", path.display()));
    }
    // Moving the folder under an installed game would orphan it; the player uninstalls first.
    if install::read_installed(&state.dirs()).is_some() {
        return Err("Uninstall the game before changing the install folder.".into());
    }
    let new_settings = settings::Settings { install_dir: Some(path) };
    settings::save(&state.settings_path, &new_settings)
}

#[tauri::command]
pub fn open_install_folder(state: State<'_, AppState>) -> Result<(), String> {
    let dirs = state.dirs();
    std::fs::create_dir_all(dirs.root())
        .map_err(|e| format!("Cannot create {}: {e}", dirs.root().display()))?;
    open_with_system(&dirs.root().display().to_string())
}

/// Opens a page of the game's GitHub repository in the browser. Other URLs are refused so the
/// UI cannot be used to launch arbitrary programs or sites.
#[tauri::command]
pub fn open_repo_page(state: State<'_, AppState>, url: String) -> Result<(), String> {
    let allowed = format!("https://github.com/{}/", state.config.game_repo);
    if !url.starts_with(&allowed) {
        return Err(format!("Refusing to open {url}: only {allowed} pages are allowed."));
    }
    open_with_system(&url)
}

fn open_with_system(target: &str) -> Result<(), String> {
    let program = if cfg!(windows) { "explorer" } else { "xdg-open" };
    Command::new(program)
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open {target}: {e}"))
}
