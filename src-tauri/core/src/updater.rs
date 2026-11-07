use crate::download::{download_file, Expected};
use crate::errors::LauncherError;
use crate::safety::{is_newer_version, is_trusted_update_url};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/BloodRayT/launcher/releases/latest";
const RELEASES_API: &str = "https://api.github.com/repos/BloodRayT/launcher/releases?per_page=20";
pub const RELEASES_PAGE: &str = "https://github.com/BloodRayT/launcher/releases";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReleaseNote {
    pub version: String,
    pub name: String,
    pub date: String,
    pub body: String,
    pub url: String,
}

/// История обновлений: черновики и предварительные версии игрокам не показываются.
pub fn parse_release_notes(releases: &Value) -> Vec<ReleaseNote> {
    releases
        .as_array()
        .into_iter()
        .flatten()
        .filter(|release| {
            release["draft"].as_bool() != Some(true)
                && release["prerelease"].as_bool() != Some(true)
        })
        .filter_map(|release| {
            let tag = release["tag_name"].as_str()?;
            Some(ReleaseNote {
                version: tag.trim_start_matches('v').to_string(),
                name: release["name"].as_str().unwrap_or(tag).to_string(),
                date: release["published_at"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                body: release["body"].as_str().unwrap_or_default().to_string(),
                url: release["html_url"]
                    .as_str()
                    .unwrap_or(RELEASES_PAGE)
                    .to_string(),
            })
        })
        .collect()
}

pub async fn fetch_release_notes(client: &reqwest::Client) -> Result<Vec<ReleaseNote>, String> {
    let response = client
        .get(RELEASES_API)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let body: Value = response.json().await.map_err(|error| error.to_string())?;
    Ok(parse_release_notes(&body))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub update_available: bool,
    pub latest_version: String,
    pub download_url: String,
    pub changelog: String,
    /// SHA-256 установщика из ответа GitHub. Без неё копии на сервере FenixCraft не доверяем.
    pub sha256: String,
}

impl UpdateInfo {
    pub fn none(current: &str) -> Self {
        Self {
            update_available: false,
            latest_version: current.to_string(),
            download_url: String::new(),
            changelog: String::new(),
            sha256: String::new(),
        }
    }
}

/// GitHub указывает у файла релиза `digest: "sha256:<hex>"`.
fn asset_sha256(asset: &Value) -> String {
    asset["digest"]
        .as_str()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

fn find_installer(release: &Value) -> Option<(String, String)> {
    release["assets"].as_array()?.iter().find_map(|asset| {
        let url = asset["browser_download_url"].as_str()?;
        is_trusted_update_url(url).then(|| (url.to_string(), asset_sha256(asset)))
    })
}

/// Копия установщика на сервере FenixCraft: сервер сам скачивает её с GitHub после каждого релиза.
pub fn mirror_url(backend_url: &str, installer_url: &str) -> Option<String> {
    if !is_trusted_update_url(installer_url) {
        return None;
    }
    let backend = reqwest::Url::parse(backend_url).ok()?;
    if !matches!(backend.scheme(), "http" | "https") {
        return None;
    }
    let name = installer_url.rsplit('/').next()?;
    Some(format!(
        "{}/updates/{name}",
        backend_url.trim_end_matches('/')
    ))
}

/// Разбирает ответ GitHub. На Windows адрес указывает на установщик, на остальных системах — на страницу релиза.
pub fn parse_release(release: &Value, current: &str, windows: bool) -> UpdateInfo {
    let Some(tag) = release["tag_name"].as_str() else {
        return UpdateInfo::none(current);
    };
    if !is_newer_version(tag, current) {
        return UpdateInfo::none(current);
    }
    let (download_url, sha256) = windows
        .then(|| find_installer(release))
        .flatten()
        .unwrap_or_else(|| (RELEASES_PAGE.to_string(), String::new()));
    UpdateInfo {
        update_available: true,
        latest_version: tag.trim_start_matches('v').to_string(),
        download_url,
        changelog: release["body"].as_str().unwrap_or_default().to_string(),
        sha256,
    }
}

/// Сбой проверки обновлений не должен мешать работе лаунчера.
pub async fn check_update(client: &reqwest::Client, current: &str) -> UpdateInfo {
    let release: Option<Value> = async {
        client
            .get(LATEST_RELEASE_API)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()
    }
    .await;
    match release {
        Some(release) => parse_release(&release, current, cfg!(windows)),
        None => UpdateInfo::none(current),
    }
}

fn update_error(reason: String) -> LauncherError {
    LauncherError::Download {
        file: "обновление".to_string(),
        reason,
    }
}

/// Скачивает установщик с GitHub, а если GitHub недоступен — копию с сервера FenixCraft.
/// Копия принимается, только если её SHA-256 совпала с указанной на GitHub.
/// Адрес проверяется здесь, потому что он приходит из интерфейса.
pub async fn download_installer(
    client: &reqwest::Client,
    url: &str,
    sha256: &str,
    mirror: Option<&str>,
    dest: &Path,
) -> Result<(), LauncherError> {
    if !is_trusted_update_url(url) {
        return Err(update_error(
            "адрес не относится к релизам FenixCraft".to_string(),
        ));
    }
    let expected = Expected {
        sha256: (!sha256.is_empty()).then(|| sha256.to_string()),
        ..Expected::default()
    };
    let Err(github_error) = download_file(client, url, dest, &expected).await else {
        return Ok(());
    };
    let Some(mirror) = mirror.filter(|_| expected.sha256.is_some()) else {
        return Err(update_error(format!("{github_error:?}")));
    };
    download_file(client, mirror, dest, &expected)
        .await
        .map_err(|mirror_error| {
            update_error(format!(
                "GitHub: {github_error:?}; сервер FenixCraft: {mirror_error:?}"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const INSTALLER: &str =
        "https://github.com/BloodRayT/launcher/releases/download/v1.2.0/FenixCraft_1.2.0_x64-setup.exe";

    fn release(tag: &str) -> Value {
        json!({
            "tag_name": tag,
            "body": "Что нового",
            "assets": [
                { "browser_download_url": "https://github.com/BloodRayT/launcher/releases/download/v1.2.0/a.msi" },
                { "browser_download_url": INSTALLER }
            ]
        })
    }

    #[test]
    fn release_notes_skip_drafts_and_prereleases() {
        let releases = json!([
            { "tag_name": "v1.2.0", "name": "FenixCraft 1.2.0", "published_at": "2026-11-05T10:00:00Z",
              "body": "Что нового", "html_url": "https://github.com/x/1.2.0" },
            { "tag_name": "v1.3.0", "draft": true },
            { "tag_name": "v1.3.0-beta", "prerelease": true },
            { "name": "без тега" }
        ]);

        let notes = parse_release_notes(&releases);

        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].version, "1.2.0");
        assert_eq!(notes[0].name, "FenixCraft 1.2.0");
        assert_eq!(notes[0].body, "Что нового");
        assert_eq!(notes[0].date, "2026-11-05T10:00:00Z");
        assert!(parse_release_notes(&json!({ "message": "rate limit" })).is_empty());
    }

    #[test]
    fn newer_release_offers_the_installer_on_windows() {
        let info = parse_release(&release("v1.2.0"), "1.1.0", true);

        assert!(info.update_available);
        assert_eq!(info.latest_version, "1.2.0");
        assert_eq!(info.download_url, INSTALLER);
        assert_eq!(info.changelog, "Что нового");
    }

    #[test]
    fn other_systems_get_the_release_page() {
        let info = parse_release(&release("v1.2.0"), "1.1.0", false);
        assert_eq!(info.download_url, RELEASES_PAGE);
    }

    #[test]
    fn same_or_older_release_is_not_an_update() {
        assert!(!parse_release(&release("v1.2.0"), "1.2.0", true).update_available);
        assert!(!parse_release(&release("v1.0.12"), "1.2.0", true).update_available);
        assert!(!parse_release(&json!({}), "1.2.0", true).update_available);
    }

    #[test]
    fn foreign_assets_are_not_used_as_the_installer() {
        let foreign = json!({
            "tag_name": "v9.0.0",
            "assets": [{ "browser_download_url": "https://evil.example/x-setup.exe" }]
        });
        assert_eq!(
            parse_release(&foreign, "1.0.0", true).download_url,
            RELEASES_PAGE
        );
    }

    #[tokio::test]
    async fn untrusted_installer_url_is_refused_without_a_request() {
        let client = reqwest::Client::new();
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("setup.exe");

        let result = download_installer(
            &client,
            "https://evil.example/x-setup.exe",
            "",
            Some("http://fenixcraft.ru/api/launcher/updates/x-setup.exe"),
            &dest,
        )
        .await;

        assert!(matches!(result, Err(LauncherError::Download { .. })));
        assert!(!dest.exists());
    }

    #[test]
    fn installer_checksum_comes_from_the_github_digest() {
        let digest = "93CD3E7EDE74BF4F132EFC0531CE6689C95B00A53356E51C165D24B3F2669E37";
        let with_digest = json!({
            "tag_name": "v1.2.0",
            "assets": [{ "browser_download_url": INSTALLER, "digest": format!("sha256:{digest}") }]
        });

        let info = parse_release(&with_digest, "1.1.0", true);

        assert_eq!(info.sha256, digest.to_ascii_lowercase());
        assert_eq!(parse_release(&release("v1.2.0"), "1.1.0", true).sha256, "");
        let bad = json!({ "browser_download_url": INSTALLER, "digest": "sha256:zz" });
        assert_eq!(asset_sha256(&bad), "");
    }

    #[test]
    fn mirror_keeps_the_installer_name_on_the_fenixcraft_server() {
        assert_eq!(
            mirror_url("http://fenixcraft.ru/api/launcher/", INSTALLER).as_deref(),
            Some("http://fenixcraft.ru/api/launcher/updates/FenixCraft_1.2.0_x64-setup.exe")
        );
        assert_eq!(
            mirror_url(
                "http://fenixcraft.ru/api/launcher",
                "https://evil.example/x-setup.exe"
            ),
            None
        );
        assert_eq!(mirror_url("file:///C:/x", INSTALLER), None);
    }
}
