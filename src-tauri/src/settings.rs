//! Per-user launcher preferences, stored as JSON in the OS config folder.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Folder the game is installed under; `None` means the launcher's data folder.
    #[serde(default)]
    pub install_dir: Option<PathBuf>,
    /// Steam-style launch options, e.g. `prime-run %command%`; empty starts the game directly.
    #[serde(default)]
    pub launch_options: String,
}

pub fn load(path: &Path) -> Settings {
    // Fails open to defaults: a missing or unreadable file only means the default install folder
    // is used, and the player can set it again from the settings panel.
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Loads the settings, applies `change` and saves them, so saving one setting keeps the others.
pub fn update(path: &Path, change: impl FnOnce(&mut Settings)) -> Result<(), String> {
    let mut settings = load(path);
    change(&mut settings);
    save(path, &settings)
}

fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(settings).expect("Settings always serialise");
    fs::write(path, text).map_err(|e| format!("Cannot save {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_from_older_launchers_still_load() {
        let settings: Settings = serde_json::from_str(r#"{"install_dir":"/games"}"#).unwrap();
        assert_eq!(settings.install_dir, Some(PathBuf::from("/games")));
        assert_eq!(settings.launch_options, "");
    }

    #[test]
    fn updating_one_setting_keeps_the_other() {
        let dir = std::env::temp_dir().join(format!("flysim-launcher-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        update(&path, |s| s.install_dir = Some(PathBuf::from("/games"))).unwrap();
        update(&path, |s| s.launch_options = "prime-run %command%".into()).unwrap();
        let loaded = load(&path);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(loaded.install_dir, Some(PathBuf::from("/games")));
        assert_eq!(loaded.launch_options, "prime-run %command%");
    }
}
