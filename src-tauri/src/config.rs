//! The launcher's fixed settings. They are compiled in from `launcher.json`, the single place to
//! change which repository, status file and trailer the launcher uses.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A person credited in Settings → Devs.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Developer {
    pub name: String,
    /// GitHub username; the Devs dialog links to `https://github.com/<github>`.
    pub github: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformConfig {
    /// End of the release asset name for this OS, e.g. `-StandaloneLinux64.zip`.
    pub asset_suffix: String,
    /// Candidate executable names inside the build, tried in order.
    pub executables: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LauncherConfig {
    /// Folder name of the installed build. Kept separate from `display_name` so renaming the
    /// game does not move the folder and orphan existing installs.
    pub game_name: String,
    /// Name shown to players.
    pub display_name: String,
    pub github_api: String,
    pub game_repo: String,
    pub status_url: String,
    pub trailer_youtube_id: String,
    pub developers: Vec<Developer>,
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

    #[test]
    fn bundled_developers_have_plain_github_usernames() {
        // The username is pasted into a URL, so it must not carry `/`, `?` or other URL syntax.
        let config = LauncherConfig::load();
        assert!(!config.developers.is_empty());
        for dev in &config.developers {
            assert!(!dev.github.is_empty(), "{} has no GitHub username", dev.name);
            assert!(
                dev.github.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
                "{} is not a plain GitHub username",
                dev.github
            );
        }
    }
}
