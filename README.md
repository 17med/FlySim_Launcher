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
  Uninstall, the install folder and the launch options.

## Launch options

Settings → **Launch options** works like Steam's, e.g. for a laptop with an NVIDIA GPU on Linux:

| Launch options | Runs |
|---|---|
| *(empty)* | `FlySim` |
| `prime-run %command%` | `prime-run FlySim` |
| `__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia %command%` | `FlySim` with those variables set |
| `gamemoderun prime-run %command% -screen-fullscreen 0` | `gamemoderun prime-run FlySim -screen-fullscreen 0` |
| `-screen-fullscreen 0` | `FlySim -screen-fullscreen 0` (no `%command%`: words go to the game) |

The text is split on spaces and never run through a shell, so quotes, `;`, `|` and `$(...)` have
no special meaning and an argument cannot contain a space. Saved as `launch_options` in
`settings.json` in the launcher's config folder.

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

## Launcher self-update

The launcher updates itself from the releases of this repository through Tauri's updater plugin
(`src-tauri/src/self_update.rs`).

- At startup it reads `https://github.com/17med/FlySim_Launcher/releases/latest/download/latest.json`
  (set in `tauri.conf.json > plugins > updater > endpoints`). If that lists a newer version, a
  "New launcher available" bar appears; **Restart to update** downloads it, checks its signature,
  installs it and restarts. Settings → **Check launcher updates** runs the same check by hand.
- Every download must be signed by the key whose public half is in
  `tauri.conf.json > plugins > updater > pubkey`. Unsigned or wrongly signed files are refused.
- Works for the Windows installers, the AppImage, and the `.deb` / `.rpm` packages (those ask for
  the admin password, since they install system-wide).
- The update is refused while a game install or uninstall is running, and the game buttons are
  disabled while the launcher updates.
- A failed check is silent at startup (the launcher keeps working at its current version) and shown
  as a message when started from Settings.
- Launchers older than v0.2.0 have no updater; those players must install v0.2.0 by hand once.

### Signing key (one-time setup)

The private key lives outside the repository, in `~/.tauri/flysim-launcher.key` on the machine
that generated it. Never commit it. Add it to the repository secrets:

```
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/flysim-launcher.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --body ""
```

Keep a backup of the key. Lose it and installed launchers can never update again: you would have to
ship a new public key and have every player reinstall by hand.

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
npm run build -- --no-sign   # installers in src-tauri/target/release/bundle/, without updater signatures
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
```

## CI and releases

- Every push / pull request to `main` runs `.github/workflows/ci.yml`: clippy, tests, then Linux
  and Windows builds. The installers are attached to the run under **Artifacts**.
- `git tag v0.2.0 && git push origin v0.2.0` runs `.github/workflows/release.yml`: the same
  checks, then signed Linux and Windows builds published as a GitHub Release together with
  `latest.json`. The tag must match `version` in `tauri.conf.json`, `Cargo.toml` and
  `package.json`; `cargo test` checks the first two agree.
