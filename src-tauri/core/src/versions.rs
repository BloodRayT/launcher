use crate::download::{ensure_files, write_atomic, DownloadItem, Expected};
use crate::errors::LauncherError;
use crate::launch::LaunchContext;
use crate::safety::is_safe_profile_id;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionEntry {
    pub id: String,
    pub url: String,
    pub sha1: Option<String>,
}

/// Конкретная версия из идентификатора сборки: `1.21.11-fabric` → `1.21.11`, `latest-vanilla` → нет.
pub fn base_version(requested: &str) -> Option<&str> {
    if requested.starts_with("latest") {
        None
    } else {
        requested.split('-').next().filter(|id| !id.is_empty())
    }
}

pub fn select_version(manifest: &Value, requested: &str) -> Result<VersionEntry, LauncherError> {
    let target = match base_version(requested) {
        Some(id) => id,
        None => manifest["latest"]["release"]
            .as_str()
            .ok_or_else(|| LauncherError::BadData("в списке версий нет последней".to_string()))?,
    };
    let entry = manifest["versions"]
        .as_array()
        .and_then(|versions| versions.iter().find(|v| v["id"].as_str() == Some(target)))
        .ok_or_else(|| LauncherError::VersionNotFound(target.to_string()))?;
    let url = entry["url"]
        .as_str()
        .ok_or_else(|| LauncherError::BadData(format!("у версии {target} нет адреса")))?;
    Ok(VersionEntry {
        id: target.to_string(),
        url: url.to_string(),
        sha1: entry["sha1"].as_str().map(str::to_string),
    })
}

fn version_json_path(root: &Path, id: &str) -> PathBuf {
    root.join("versions").join(id).join(format!("{id}.json"))
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Берёт свежий список версий, а если он недоступен — последний сохранённый.
async fn load_manifest(ctx: &LaunchContext) -> Option<Value> {
    let cache = ctx.root.join("versions").join("version_manifest_v2.json");
    let response = ctx
        .probe_client
        .get(&ctx.endpoints.version_manifest)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status);
    let fresh = match response {
        Ok(response) => response.bytes().await.ok(),
        Err(_) => None,
    };
    if let Some(bytes) = fresh {
        if let Ok(manifest) = serde_json::from_slice::<Value>(&bytes) {
            let _ = write_atomic(&cache, &bytes);
            return Some(manifest);
        }
    }
    ctx.reporter
        .log("[Сеть] Список версий Mojang недоступен, используем сохранённые данные.");
    read_json(&cache)
}

/// Возвращает id версии и её описание. Для установленной версии интернет не нужен.
pub async fn load_version_json(
    ctx: &LaunchContext,
    requested: &str,
) -> Result<(String, Value), LauncherError> {
    let manifest = load_manifest(ctx).await;
    let installed = |id: &str| version_json_path(&ctx.root, id).exists();

    let entry = match &manifest {
        Some(manifest) => match select_version(manifest, requested) {
            Ok(entry) => Some(entry),
            Err(_) if base_version(requested).is_some_and(installed) => None,
            Err(err) => return Err(err),
        },
        None => None,
    };
    let id = match (&entry, base_version(requested)) {
        (Some(entry), _) => entry.id.clone(),
        (None, Some(id)) if installed(id) => id.to_string(),
        _ => {
            return Err(LauncherError::NeedsInternet {
                what: "список версий Minecraft".to_string(),
            })
        }
    };
    if !is_safe_profile_id(&id) {
        return Err(LauncherError::BadData(format!(
            "недопустимый id версии «{id}»"
        )));
    }

    let path = version_json_path(&ctx.root, &id);
    if let Some(entry) = entry {
        let item = DownloadItem {
            url: entry.url,
            dest: path.clone(),
            expected: Expected {
                sha1: entry.sha1,
                ..Expected::default()
            },
        };
        let updated =
            ensure_files(&ctx.client, vec![item], &ctx.reporter, "Описание версии", 1).await;
        if let Err(err) = updated {
            if read_json(&path).is_none() {
                return Err(err);
            }
            ctx.reporter
                .log(format!("[Сеть] Описание версии {id} не обновлено: {err}"));
        }
    }
    let json = read_json(&path)
        .ok_or_else(|| LauncherError::BadData(format!("описание версии {id} повреждено")))?;
    Ok((id, json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest() -> Value {
        json!({
            "latest": { "release": "1.21.11" },
            "versions": [
                { "id": "1.21.11", "url": "https://x/1.21.11.json", "sha1": "abc" },
                { "id": "1.21.1", "url": "https://x/1.21.1.json" }
            ]
        })
    }

    #[test]
    fn base_version_extracts_the_minecraft_version() {
        assert_eq!(base_version("latest-vanilla"), None);
        assert_eq!(base_version("1.21.11-iris-fabric"), Some("1.21.11"));
        assert_eq!(base_version("1.21.1"), Some("1.21.1"));
    }

    #[test]
    fn latest_resolves_to_the_release() {
        let entry = select_version(&manifest(), "latest-vanilla").unwrap();
        assert_eq!(entry.id, "1.21.11");
        assert_eq!(entry.sha1.as_deref(), Some("abc"));
    }

    #[test]
    fn specific_version_is_selected_exactly() {
        assert_eq!(
            select_version(&manifest(), "1.21.1-neoforge").unwrap().id,
            "1.21.1"
        );
    }

    #[test]
    fn unknown_version_is_an_error_not_a_silent_substitution() {
        assert_eq!(
            select_version(&manifest(), "9.9.9-vanilla"),
            Err(LauncherError::VersionNotFound("9.9.9".to_string()))
        );
    }
}
