//! Everything on disk: where the game lives, which version is installed, unpacking a build and
//! swapping it in so a failed update never breaks the version already installed.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Layout under the install root: `<game>/` is the live build, the others exist only mid-install.
pub struct GameDirs {
    root: PathBuf,
    name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Installed {
    pub version: String,
}

const MARKER: &str = "installed.json";

impl GameDirs {
    pub fn new(root: PathBuf, game_name: &str) -> Self {
        GameDirs { root, name: game_name.to_string() }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn game(&self) -> PathBuf {
        self.root.join(&self.name)
    }
    pub fn download(&self) -> PathBuf {
        self.root.join(format!("{}.download.zip", self.name))
    }
    fn staging(&self) -> PathBuf {
        self.root.join(format!("{}.staging", self.name))
    }
    fn previous(&self) -> PathBuf {
        self.root.join(format!("{}.previous", self.name))
    }
}

/// The marker is written last during an install, so its presence means the build is complete.
pub fn read_installed(dirs: &GameDirs) -> Option<Installed> {
    let text = fs::read_to_string(dirs.game().join(MARKER)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn find_executable(dirs: &GameDirs, names: &[String]) -> Option<PathBuf> {
    find_in(&dirs.game(), names)
}

fn find_in(folder: &Path, names: &[String]) -> Option<PathBuf> {
    names.iter().map(|n| folder.join(n)).find(|p| p.is_file())
}

/// Unpacks `zip_path` into a fresh staging folder.
pub fn extract_to_staging(dirs: &GameDirs, zip_path: &Path) -> Result<(), String> {
    let staging = dirs.staging();
    remove_if_exists(&staging)?;
    fs::create_dir_all(&staging).map_err(|e| io_error("create", &staging, e))?;
    let file = fs::File::open(zip_path).map_err(|e| io_error("open", zip_path, e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Downloaded build is not a valid zip: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Downloaded build is damaged (entry {i}): {e}"))?;
        // enclosed_name() rejects absolute paths and `..`, so a hostile zip cannot write outside staging.
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| format!("Downloaded build has an unsafe path: {}", entry.name()))?;
        let target = staging.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| io_error("create", &target, e))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| io_error("create", parent, e))?;
        }
        let mut out = fs::File::create(&target).map_err(|e| io_error("write", &target, e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| io_error("write", &target, e))?;
        apply_unix_mode(&target, entry.unix_mode());
    }
    Ok(())
}

/// Makes the first matching executable runnable. Zips made on Linux usually keep the bit, but
/// not every archiver records it, and the game cannot start without it.
pub fn ensure_executable(dirs: &GameDirs, names: &[String]) -> Result<(), String> {
    let exe = find_in(&dirs.staging(), names).ok_or_else(|| {
        format!("The downloaded build contains none of these executables: {}", names.join(", "))
    })?;
    apply_unix_mode(&exe, Some(0o755));
    Ok(())
}

/// Replaces the live build with the staged one. If the swap fails half way, the previous build is
/// moved back, so the player keeps a working game rather than none.
pub fn activate_staging(dirs: &GameDirs, version: &str) -> Result<(), String> {
    let (game, staging, previous) = (dirs.game(), dirs.staging(), dirs.previous());
    remove_if_exists(&previous)?;
    let had_previous = game.exists();
    if had_previous {
        fs::rename(&game, &previous).map_err(|e| io_error("move", &game, e))?;
    }
    if let Err(e) = fs::rename(&staging, &game) {
        // Best-effort rollback; the swap error is what gets reported. If this rename also fails,
        // the old build is still intact under `<game>.previous` for a manual restore.
        if had_previous {
            let _ = fs::rename(&previous, &game);
        }
        return Err(io_error("move", &staging, e));
    }
    let marker = serde_json::to_string(&Installed { version: version.to_string() })
        .expect("Installed always serialises");
    fs::write(game.join(MARKER), marker).map_err(|e| io_error("write", &game.join(MARKER), e))?;
    remove_if_exists(&previous)
}

pub fn uninstall(dirs: &GameDirs) -> Result<(), String> {
    for path in [dirs.game(), dirs.staging(), dirs.previous(), dirs.download()] {
        remove_if_exists(&path)?;
    }
    Ok(())
}

pub fn remove_if_exists(path: &Path) -> Result<(), String> {
    let result = if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
    match result {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_error("remove", path, e)),
        _ => Ok(()),
    }
}

fn io_error(action: &str, path: &Path, e: std::io::Error) -> String {
    format!("Cannot {action} {}: {e}", path.display())
}

#[cfg(unix)]
fn apply_unix_mode(path: &Path, mode: Option<u32>) {
    use std::os::unix::fs::PermissionsExt;
    // Fails open: a missing permission bit only matters for the executable, which
    // ensure_executable checks separately and reports.
    if let Some(mode) = mode {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777));
    }
}

#[cfg(not(unix))]
fn apply_unix_mode(_path: &Path, _mode: Option<u32>) {}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
