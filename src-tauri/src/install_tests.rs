use super::*;
use std::io::Write;
use zip::write::SimpleFileOptions;

/// A fresh, empty folder per test, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("flysim-launcher-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_zip(path: &Path, entries: &[(&str, &str)]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, content) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

fn exe_names() -> Vec<String> {
    vec!["FlySim.x86_64".into(), "FlySim.exe".into()]
}

fn install(dirs: &GameDirs, zip: &Path, version: &str) -> Result<(), String> {
    extract_to_staging(dirs, zip)?;
    ensure_executable(dirs, &exe_names())?;
    activate_staging(dirs, version)
}

#[test]
fn install_then_update_replaces_build_and_records_version() {
    let scratch = Scratch::new("update");
    let dirs = GameDirs::new(scratch.0.clone(), "FlySim");
    let zip = scratch.0.join("build.zip");

    write_zip(&zip, &[("FlySim.x86_64", "v1"), ("FlySim_Data/old.txt", "old")]);
    install(&dirs, &zip, "v1").unwrap();
    assert_eq!(read_installed(&dirs), Some(Installed { version: "v1".into() }));

    write_zip(&zip, &[("FlySim.x86_64", "v2")]);
    install(&dirs, &zip, "v2").unwrap();
    assert_eq!(read_installed(&dirs).unwrap().version, "v2");
    assert!(!dirs.game().join("FlySim_Data/old.txt").exists(), "old files must not linger");
    assert!(find_executable(&dirs, &exe_names()).is_some());
}

#[test]
fn bad_build_leaves_installed_version_untouched() {
    let scratch = Scratch::new("bad-build");
    let dirs = GameDirs::new(scratch.0.clone(), "FlySim");
    let zip = scratch.0.join("build.zip");
    write_zip(&zip, &[("FlySim.x86_64", "v1")]);
    install(&dirs, &zip, "v1").unwrap();

    write_zip(&zip, &[("readme.txt", "no executable here")]);
    assert!(install(&dirs, &zip, "v2").is_err());
    assert_eq!(read_installed(&dirs).unwrap().version, "v1");
}

#[test]
fn zip_entries_escaping_the_folder_are_refused() {
    let scratch = Scratch::new("zip-slip");
    let dirs = GameDirs::new(scratch.0.join("root"), "FlySim");
    let zip = scratch.0.join("evil.zip");
    write_zip(&zip, &[("../escaped.txt", "x")]);
    assert!(extract_to_staging(&dirs, &zip).is_err());
    assert!(!scratch.0.join("escaped.txt").exists());
}

#[test]
fn uninstall_removes_game_and_marker() {
    let scratch = Scratch::new("uninstall");
    let dirs = GameDirs::new(scratch.0.clone(), "FlySim");
    let zip = scratch.0.join("build.zip");
    write_zip(&zip, &[("FlySim.x86_64", "v1")]);
    install(&dirs, &zip, "v1").unwrap();

    uninstall(&dirs).unwrap();
    assert!(read_installed(&dirs).is_none());
    assert!(!dirs.game().exists());
}

#[cfg(unix)]
#[test]
fn executable_bit_is_set_even_if_zip_lacks_it() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("exec-bit");
    let dirs = GameDirs::new(scratch.0.clone(), "FlySim");
    let zip = scratch.0.join("build.zip");
    write_zip(&zip, &[("FlySim.x86_64", "v1")]);
    install(&dirs, &zip, "v1").unwrap();

    let exe = find_executable(&dirs, &exe_names()).unwrap();
    assert_ne!(fs::metadata(exe).unwrap().permissions().mode() & 0o111, 0);
}
