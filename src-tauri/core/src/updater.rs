use crate::download::{download_file, Expected};
use crate::errors::LauncherError;
use crate::safety::{is_newer_version, is_trusted_update_url};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/BloodRayT/launcher/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/BloodRayT/launcher/releases";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub update_available: bool,
    pub latest_version: String,
    pub download_url: String,
    pub changelog: String,
}

impl UpdateInfo {
    fn none(current: &str) -> Self {
        Self {
            update_available: false,
            latest_version: current.to_string(),
            download_url: String::new(),
            changelog: String::new(),
        }
    }
}

fn find_installer(release: &Value) -> Option<String> {
    release["assets"]
        .as_array()?
        .iter()
        .filter_map(|asset| asset["browser_download_url"].as_str())
        .find(|url| is_trusted_update_url(url))
        .map(str::to_string)
}

/// Разбирает ответ GitHub. На Windows адрес указывает на установщик, на остальных системах — на страницу релиза.
pub fn parse_release(release: &Value, current: &str, windows: bool) -> UpdateInfo {
    let Some(tag) = release["tag_name"].as_str() else {
        return UpdateInfo::none(current);
    };
    if !is_newer_version(tag, current) {
        return UpdateInfo::none(current);
    }
    let installer = if windows {
        find_installer(release)
    } else {
        None
    };
    UpdateInfo {
        update_available: true,
        latest_version: tag.trim_start_matches('v').to_string(),
        download_url: installer.unwrap_or_else(|| RELEASES_PAGE.to_string()),
        changelog: release["body"].as_str().unwrap_or_default().to_string(),
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

/// Скачивает установщик. Адрес проверяется здесь, потому что он приходит из интерфейса.
pub async fn download_installer(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
) -> Result<(), LauncherError> {
    if !is_trusted_update_url(url) {
        return Err(LauncherError::Download {
            file: "обновление".to_string(),
            reason: "адрес не относится к релизам FenixCraft".to_string(),
        });
    }
    download_file(client, url, dest, &Expected::default())
        .await
        .map_err(|err| LauncherError::Download {
            file: "обновление".to_string(),
            reason: format!("{err:?}"),
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

        let result = download_installer(&client, "https://evil.example/x-setup.exe", &dest).await;

        assert!(matches!(result, Err(LauncherError::Download { .. })));
        assert!(!dest.exists());
    }
}
