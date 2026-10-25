# Fly Simulator Launcher

Desktop launcher (Tauri 2) that installs, updates and starts Fly Simulator from the GitHub
Releases of [`17med/FlySim_Game`](https://github.com/17med/FlySim_Game). Runs on Windows and Linux.

## How it decides what to show

| Installed | Latest release | Main button |
|---|---|---|
| nothing | `v0.2.0` | **Install** |
| `v0.1.0` | `v0.2.0` | **Update** |
| `v0.2.0` | `v0.2.0` | **Play** |
| `v0.2.0` | GitHub unreachable | **Play** (offline) |
| nothing | no release yet | disabled |

- **Latest** = the newest published, non-prerelease release. Drafts are never offered.
- **Installed** = `installed.json` inside the game folder, written only after a build is fully
  unpacked and swapped in.
- The download for this OS is the release asset ending in `-StandaloneLinux64.zip` or
  `-StandaloneWindows64.zip`, the names the game's `release.yml` uploads.
- Settings (gear): Check for updates, Repair (reinstall the installed version), Open folder,
  Uninstall, and the install folder.

## Install / update steps

1. Download the zip to `<install folder>/FlySim.download.zip`, checking it against the SHA-256
   that GitHub publishes for the asset. A mismatch aborts the install.
2. Unzip into `FlySim.staging/`. Entries that would escape the folder are refused.
3. Make the executable runnable (Linux), rename `FlySim/` → `FlySim.previous/` and
   `FlySim.staging/` → `FlySim/`, then write `installed.json`.
4. If any step fails, the previously installed version stays playable.

Default install folder: the launcher's data folder (`~/.local/share/com.flysim.launcher` on Linux,
`%APPDATA%\com.flysim.launcher` on Windows).

## Configuration

`src-tauri/launcher.json` is compiled in:

| Key | Meaning |
|---|---|
| `game_name` | Folder name of the installed build (`FlySim/`); changing it orphans existing installs |
| `display_name` | Name shown in the launcher |
| `game_repo` | Repository whose releases are installed |
| `status_url` | Raw URL of the maintenance banner file (see below) |
| `trailer_youtube_id` | YouTube video id for the trailer (`""` shows a placeholder) |
| `platforms.<os>` | Asset suffix and executable names per OS |

## Maintenance banner

Edit `status.json` in this repository and push to `main`. The launcher shows it at startup:

```json
{ "message": "Maintenance at 12:00 UTC. The server will be unavailable for a while.", "level": "warning" }
```

`level` is `warning` (yellow), `info` (blue) or `critical` (red). An empty `message` hides the
banner. The repository must be public for players to read it.

## Develop

Needs Node, Rust and the Tauri Linux libraries
(`libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`).

```
npm ci
npm run dev          # run the launcher
npm run build        # installers in src-tauri/target/release/bundle/
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
```

## CI and releases

- Every push / pull request to `main` runs `.github/workflows/ci.yml`: clippy, tests, then Linux
  and Windows builds. The installers are attached to the run under **Artifacts**.
- `git tag v0.1.0 && git push origin v0.1.0` runs `.github/workflows/release.yml`: the same
  checks, then Linux and Windows builds published as a GitHub Release.
