//! The only module that talks to GitHub: release listing, the maintenance status file, and
//! downloading a release asset.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
    /// `sha256:<hex>`, published by GitHub for each uploaded asset.
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Status {
    pub message: String,
    pub level: String,
}

pub fn http_client() -> reqwest::Client {
    // GitHub rejects API requests without a User-Agent.
    reqwest::Client::builder()
        .user_agent(concat!("flysim-launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("HTTP client could not be built (TLS backend unavailable)")
}

async fn get_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Result<T, String> {
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Could not reach {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("{url} answered {}", response.status()));
    }
    response
        .json()
        .await
        .map_err(|e| format!("Unexpected answer from {url}: {e}"))
}

/// Published, non-prerelease releases, newest first (GitHub's order).
pub async fn fetch_releases(
    client: &reqwest::Client,
    api: &str,
    repo: &str,
) -> Result<Vec<Release>, String> {
    let url = format!("{api}/repos/{repo}/releases?per_page=20");
    let releases: Vec<Release> = get_json(client, &url).await?;
    Ok(published_only(releases))
}

pub async fn fetch_release(
    client: &reqwest::Client,
    api: &str,
    repo: &str,
    tag: &str,
) -> Result<Release, String> {
    get_json(client, &format!("{api}/repos/{repo}/releases/tags/{tag}")).await
}

/// `None` when the status file has an empty message, meaning no banner.
pub async fn fetch_status(client: &reqwest::Client, url: &str) -> Result<Option<Status>, String> {
    let status: Status = get_json(client, url).await?;
    Ok(Some(status).filter(|s| !s.message.trim().is_empty()))
}

/// Streams the asset to `dest`, reporting bytes written so far, and returns its SHA-256 hex.
pub async fn download_asset(
    client: &reqwest::Client,
    asset: &Asset,
    dest: &Path,
    mut on_progress: impl FnMut(u64),
) -> Result<String, String> {
    let url = &asset.browser_download_url;
    let mut response = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Download of {} failed: {e}", asset.name))?;
    let mut file = std::fs::File::create(dest)
        .map_err(|e| format!("Cannot write {}: {e}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut written = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Download of {} was interrupted: {e}", asset.name))?
    {
        file.write_all(&chunk)
            .map_err(|e| format!("Cannot write {}: {e}", dest.display()))?;
        hasher.update(&chunk);
        written += chunk.len() as u64;
        on_progress(written);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

pub fn find_asset<'a>(release: &'a Release, suffix: &str) -> Option<&'a Asset> {
    release.assets.iter().find(|a| a.name.ends_with(suffix))
}

pub fn expected_sha256(asset: &Asset) -> Option<&str> {
    asset.digest.as_deref()?.strip_prefix("sha256:")
}

fn published_only(releases: Vec<Release>) -> Vec<Release> {
    releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, draft: bool, prerelease: bool, assets: &[&str]) -> Release {
        Release {
            tag_name: tag.into(),
            name: None,
            body: None,
            html_url: String::new(),
            published_at: None,
            draft,
            prerelease,
            assets: assets
                .iter()
                .map(|n| Asset {
                    name: (*n).into(),
                    size: 1,
                    browser_download_url: String::new(),
                    digest: Some("sha256:abc".into()),
                })
                .collect(),
        }
    }

    #[test]
    fn drafts_and_prereleases_are_not_offered() {
        let all = vec![
            release("v3", true, false, &[]),
            release("v2", false, true, &[]),
            release("v1", false, false, &[]),
        ];
        let tags: Vec<_> = published_only(all).into_iter().map(|r| r.tag_name).collect();
        assert_eq!(tags, ["v1"]);
    }

    #[test]
    fn asset_is_matched_by_platform_suffix() {
        let r = release(
            "v1",
            false,
            false,
            &["FlySim-v1-StandaloneWindows64.zip", "FlySim-v1-StandaloneLinux64.zip"],
        );
        let asset = find_asset(&r, "-StandaloneLinux64.zip").unwrap();
        assert_eq!(asset.name, "FlySim-v1-StandaloneLinux64.zip");
        assert!(find_asset(&r, "-StandaloneOSX.zip").is_none());
    }

    /// Serves one fixed HTTP response on a local port, then stops.
    fn serve_once(body: &'static [u8]) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.read(&mut [0u8; 4096]);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
        });
        format!("http://{address}/build.zip")
    }

    #[test]
    fn download_writes_file_reports_progress_and_hashes() {
        let url = serve_once(b"hello");
        let asset = Asset { name: "build.zip".into(), size: 5, browser_download_url: url, digest: None };
        let dest = std::env::temp_dir().join(format!("flysim-download-{}", std::process::id()));
        let mut last_progress = 0;
        let hash = tauri::async_runtime::block_on(download_asset(&http_client(), &asset, &dest, |done| {
            last_progress = done;
        }))
        .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
        let _ = std::fs::remove_file(&dest);
        assert_eq!(last_progress, 5);
        // sha256("hello")
        assert_eq!(hash, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }

    #[test]
    fn digest_prefix_is_stripped() {
        let r = release("v1", false, false, &["a.zip"]);
        assert_eq!(expected_sha256(&r.assets[0]), Some("abc"));
    }
}
