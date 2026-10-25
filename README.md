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
  Uninstall, the install folder and the launch options. **Devs** lists the people who built the
  launcher, each linking to their GitHub profile.

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
| `developers` | `{ "name", "github" }` entries shown in Settings → Devs; only these GitHub profiles can be opened from it |
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
- The Arch package never updates itself (see [Arch Linux](#arch-linux)).

### Signing key (one-time setup)

The private key lives outside the repository, in `~/.tauri/flysim-launcher.key` on the machine
that generated it. Never commit it. Add it to the repository secrets:

```
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/flysim-launcher.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --body ""
```

Keep a backup of the key. Lose it and installed launchers can never update again: you would have to
ship a new public key and have every player reinstall by hand.

## Arch Linux

Every release carries a pacman package, `fly-simulator-launcher-bin-<version>-1-x86_64.pkg.tar.zst`:

```
sudo pacman -U https://github.com/17med/FlySim_Launcher/releases/download/v0.3.5/fly-simulator-launcher-bin-0.3.5-1-x86_64.pkg.tar.zst
```

It is not in the AUR, which takes no new accounts, so nothing tells pacman about a new version:
players install each new release with `pacman -U` the same way. The AppImage also runs on Arch and
keeps updating itself.

The release workflow builds it from `packaging/aur/PKGBUILD` in an Arch container: `bump.sh`
points the PKGBUILD at the release's `.deb` (checked against its sha256), then `makepkg`
repackages it. It depends on `webkit2gtk-4.1` and `gtk3`; `nvidia-prime` and `gamemode` are
optional, for the `prime-run` / `gamemoderun` launch options.

The package installs `/usr/share/flysim-launcher/package-managed`. While that file exists the
launcher refuses to update itself, since the `.deb` binary would try `pkexec dpkg -i`, which Arch
does not have; Settings → **Check launcher updates** says to install the new package with
`pacman -U`.

### Publishing to the AUR later

If the AUR opens registration again, after the GitHub release of a new version is published:

```
packaging/aur/bump.sh 0.3.3          # sets pkgver, pkgrel=1 and the sha256 in PKGBUILD and .SRCINFO
git commit -am "AUR: 0.3.3"
cp packaging/aur/PKGBUILD packaging/aur/.SRCINFO ~/aur/fly-simulator-launcher-bin/
cd ~/aur/fly-simulator-launcher-bin && git commit -am "0.3.3" && git push
```

One-time setup: an account on aur.archlinux.org with your SSH public key added, then
`git clone ssh://aur@aur.archlinux.org/fly-simulator-launcher-bin.git ~/aur/fly-simulator-launcher-bin`
(an empty clone creates the package on the first push). Add an email to the `# Maintainer:` line
of the PKGBUILD first; the AUR shows it publicly.

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
  `package.json`: the release stops before building if the tag differs from `tauri.conf.json` or
  `package.json`, and `cargo test` checks `Cargo.toml` against `tauri.conf.json`. Bump all three
  (and `Cargo.lock`), commit, then tag.

## Developers

- Ahmed Dhia Ben Achour ([@17med](https://github.com/17med))
- Abdelkarim Selmi ([@abdelkarimse](https://github.com/abdelkarimse))
- Abdelhedi Rziga ([@rzig1](https://github.com/rzig1))
