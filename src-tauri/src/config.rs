//! The launcher's fixed settings. They are compiled in from `launcher.json`, the single place to
//! change which repository, status file and trailer the launcher uses.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformConfig {
    /// End of the release asset name for this OS, e.g. `-StandaloneLinux64.zip`.
    pub asset_suffix: String,
    /// Candidate executable names inside the build, tried in order.
    pub executables: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LauncherConfig {
    pub game_name: String,
    pub github_api: String,
    pub game_repo: String,
    pub status_url: String,
    pub trailer_youtube_id: String,
    pub platforms: HashMap<String, PlatformConfig>,
}

impl LauncherConfig {
    /// Parses the bundled config. Panics at boot, before any window opens, if the file is invalid
    /// or has no entry for this OS: a launcher that cannot find its builds is better refused early.
    pub fn load() -> Self {
        let config: LauncherConfig = serde_json::from_str(include_str!("../launcher.json"))
            .unwrap_or_else(|e| panic!("src-tauri/launcher.json is invalid: {e}"));
        config.platform();
        config
    }

    pub fn platform(&self) -> &PlatformConfig {
        let os = std::env::consts::OS;
        self.platforms.get(os).unwrap_or_else(|| {
            panic!("src-tauri/launcher.json has no \"platforms\" entry for this OS ({os})")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_config_parses_and_covers_this_os() {
        let config = LauncherConfig::load();
        assert!(!config.platform().executables.is_empty());
    }
}
