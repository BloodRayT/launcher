use crate::download::{ensure_files, sha256_file, write_atomic, DownloadItem, Expected};
use crate::errors::LauncherError;
use crate::launch::LaunchContext;
use crate::reporter::Reporter;
use crate::safety::{is_safe_profile_id, is_sha256_hex, safe_join};
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

const SYNC_CONCURRENCY: usize = 6;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub profile: String,
    #[serde(default)]
    pub enforce_mods: bool,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOutcome {
    /// Использован свежий манифест сервера.
    Fresh,
    /// Сервер недоступен, использован последний сохранённый манифест.
    Cached,
    /// Для этой сборки на сервере нет манифеста.
    NoProfile,
    /// Сервер недоступен и сохранённого манифеста нет.
    Unavailable,
}

/// Манифест приходит извне, поэтому проверяется целиком до изменения любых файлов.
pub fn validate_manifest(manifest: &Manifest, root: &Path) -> Result<(), LauncherError> {
    for file in &manifest.files {
        if safe_join(root, &file.path).is_none() {
            return Err(LauncherError::BadManifest(format!(
                "недопустимый путь «{}»",
                file.path
            )));
        }
        if !is_sha256_hex(&file.sha256) {
            return Err(LauncherError::BadManifest(format!(
                "нет контрольной суммы у «{}»",
                file.path
            )));
        }
    }
    Ok(())
}

/// Адрес файла из манифеста. HTTP допустим только если сам бекенд работает по HTTP (локальная отладка).
fn resolve_file_url(base: &str, raw: &str) -> Result<String, LauncherError> {
    let invalid = || LauncherError::BadManifest(format!("недопустимый адрес файла «{raw}»"));
    let resolved = match Url::parse(raw) {
        Ok(absolute) => absolute,
        Err(_) => Url::parse(&format!(
            "{}/{}",
            base.trim_end_matches('/'),
            raw.trim_start_matches('/')
        ))
        .map_err(|_| invalid())?,
    };
    let allowed = match resolved.scheme() {
        "https" => true,
        "http" => base.starts_with("http://"),
        _ => false,
    };
    if allowed {
        Ok(resolved.to_string())
    } else {
        Err(invalid())
    }
}

/// Настройки игрока не перезаписываются, если они уже существуют.
fn is_player_owned(path: &str) -> bool {
    path.starts_with("config/") || path == "options.txt"
}

fn plan_downloads(
    manifest: &Manifest,
    root: &Path,
    base: &str,
) -> Result<Vec<DownloadItem>, LauncherError> {
    let mut items = Vec::new();
    for file in &manifest.files {
        let dest = safe_join(root, &file.path)
            .ok_or_else(|| LauncherError::BadManifest(file.path.clone()))?;
        if is_player_owned(&file.path) && dest.exists() {
            continue;
        }
        items.push(DownloadItem {
            url: resolve_file_url(base, &file.url)?,
            dest,
            expected: Expected {
                sha256: Some(file.sha256.clone()),
                size: Some(file.size),
                ..Expected::default()
            },
        });
    }
    Ok(items)
}

/// Удаляет посторонние `.jar` из папки модов. Вызывается только после успешной загрузки всех файлов.
fn remove_foreign_mods(root: &Path, manifest: &Manifest, reporter: &Reporter) {
    let allowed: HashSet<String> = manifest
        .files
        .iter()
        .filter(|file| file.path.starts_with("mods/"))
        .map(|file| file.sha256.to_lowercase())
        .collect();
    if !manifest.enforce_mods || allowed.is_empty() {
        return;
    }
    for entry in std::fs::read_dir(root.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let path = entry.path();
        let is_jar = path.is_file() && path.extension().is_some_and(|ext| ext == "jar");
        if is_jar
            && sha256_file(&path).is_some_and(|hash| !allowed.contains(&hash.to_lowercase()))
            && std::fs::remove_file(&path).is_ok()
        {
            reporter.log(format!(
                "[Сборка] Удалён посторонний мод: {}",
                entry.file_name().to_string_lossy()
            ));
        }
    }
}

enum Fetched {
    Manifest(Manifest),
    NoProfile,
    Unreachable(String),
}

async fn fetch_manifest(ctx: &LaunchContext, url: &str) -> Fetched {
    let response = match ctx.probe_client.get(url).send().await {
        Ok(response) => response,
        Err(err) => return Fetched::Unreachable(err.to_string()),
    };
    if response.status() == StatusCode::NOT_FOUND {
        return Fetched::NoProfile;
    }
    if !response.status().is_success() {
        return Fetched::Unreachable(format!("сервер ответил {}", response.status().as_u16()));
    }
    match response.json::<Manifest>().await {
        Ok(manifest) => Fetched::Manifest(manifest),
        Err(err) => Fetched::Unreachable(format!("неверный ответ: {err}")),
    }
}

/// Приводит файлы сборки к манифесту бекенда. Сбой бекенда не мешает играть:
/// используется последний проверенный манифест, а посторонние моды в этом случае не удаляются.
pub async fn sync_profile(
    ctx: &LaunchContext,
    profile: &str,
    backend_url: &str,
) -> Result<SyncOutcome, LauncherError> {
    if !is_safe_profile_id(profile) {
        return Err(LauncherError::BadData(format!(
            "недопустимое имя сборки «{profile}»"
        )));
    }
    let base = backend_url.trim_end_matches('/');
    let cache = ctx
        .root
        .join("profiles")
        .join(profile)
        .join("manifest.json");
    let url = format!("{base}/manifest/{profile}");

    let (manifest, fresh) = match fetch_manifest(ctx, &url).await {
        Fetched::Manifest(manifest) => {
            validate_manifest(&manifest, &ctx.root)?;
            if let Ok(json) = serde_json::to_vec_pretty(&manifest) {
                let _ = write_atomic(&cache, &json);
            }
            (manifest, true)
        }
        Fetched::NoProfile => {
            ctx.reporter.log(format!(
                "[Сборка] На сервере нет файлов для «{profile}», это штатная ситуация."
            ));
            return Ok(SyncOutcome::NoProfile);
        }
        Fetched::Unreachable(reason) => {
            ctx.reporter
                .log(format!("[Сборка] Сервер сборки недоступен ({reason})."));
            let cached = std::fs::read(&cache)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Manifest>(&bytes).ok());
            let Some(manifest) = cached else {
                return Ok(SyncOutcome::Unavailable);
            };
            validate_manifest(&manifest, &ctx.root)?;
            ctx.reporter
                .log("[Сборка] Проверяем файлы по сохранённому манифесту.");
            (manifest, false)
        }
    };

    let items = plan_downloads(&manifest, &ctx.root, base)?;
    ensure_files(
        &ctx.client,
        items,
        &ctx.reporter,
        "Файлы сборки",
        SYNC_CONCURRENCY,
    )
    .await?;
    if !fresh {
        return Ok(SyncOutcome::Cached);
    }

    let (root, reporter) = (ctx.root.clone(), ctx.reporter.clone());
    tokio::task::spawn_blocking(move || remove_foreign_mods(&root, &manifest, &reporter))
        .await
        .map_err(|e| LauncherError::Io(e.to_string()))?;
    Ok(SyncOutcome::Fresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> ManifestFile {
        ManifestFile {
            path: path.to_string(),
            sha256: "a".repeat(64),
            size: 1,
            url: "https://x/f".to_string(),
        }
    }

    fn manifest(files: Vec<ManifestFile>) -> Manifest {
        Manifest {
            profile: "p".to_string(),
            enforce_mods: true,
            files,
        }
    }

    #[test]
    fn valid_manifest_is_accepted() {
        let good = manifest(vec![file("mods/a.jar"), file("options.txt")]);
        assert!(validate_manifest(&good, Path::new("root")).is_ok());
    }

    #[test]
    fn traversal_and_missing_hashes_are_rejected() {
        let traversal = manifest(vec![file("../../AppData/Startup/evil.exe")]);
        assert!(matches!(
            validate_manifest(&traversal, Path::new("root")),
            Err(LauncherError::BadManifest(_))
        ));

        let no_hash = manifest(vec![ManifestFile {
            sha256: String::new(),
            ..file("mods/a.jar")
        }]);
        assert!(matches!(
            validate_manifest(&no_hash, Path::new("root")),
            Err(LauncherError::BadManifest(_))
        ));
    }

    #[test]
    fn file_urls_are_resolved_and_restricted() {
        assert_eq!(
            resolve_file_url("https://b.test/api", "files/a.jar").unwrap(),
            "https://b.test/api/files/a.jar"
        );
        assert_eq!(
            resolve_file_url("https://b.test/api", "https://cdn.test/a.jar").unwrap(),
            "https://cdn.test/a.jar"
        );
        assert!(resolve_file_url("https://b.test/api", "http://cdn.test/a.jar").is_err());
        assert!(resolve_file_url("http://127.0.0.1:8000", "http://127.0.0.1:8000/a.jar").is_ok());
        assert!(resolve_file_url("https://b.test/api", "ftp://cdn.test/a.jar").is_err());
    }

    #[test]
    fn existing_player_settings_are_not_replaced() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("options.txt"), "mine").unwrap();
        let m = manifest(vec![
            file("options.txt"),
            file("config/new.toml"),
            file("mods/a.jar"),
        ]);

        let items = plan_downloads(&m, dir.path(), "https://b.test").unwrap();

        let names: Vec<_> = items.iter().map(|item| item.dest.clone()).collect();
        assert_eq!(
            names,
            vec![
                dir.path().join("config/new.toml"),
                dir.path().join("mods/a.jar")
            ]
        );
    }

    #[test]
    fn foreign_mods_are_removed_only_when_enforced_and_listed() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        let keep = mods.join("keep.jar");
        let foreign = mods.join("foreign.jar");
        let note = mods.join("note.txt");
        std::fs::write(&keep, "hello").unwrap();
        std::fs::write(&foreign, "other").unwrap();
        std::fs::write(&note, "text").unwrap();

        let mut m = manifest(vec![ManifestFile {
            sha256: sha256_file(&keep).unwrap(),
            ..file("mods/keep.jar")
        }]);
        m.enforce_mods = false;
        remove_foreign_mods(dir.path(), &m, &Reporter::silent());
        assert!(foreign.exists(), "enforcement is off");

        m.enforce_mods = true;
        remove_foreign_mods(dir.path(), &m, &Reporter::silent());
        assert!(keep.exists());
        assert!(note.exists());
        assert!(!foreign.exists());
    }
}
