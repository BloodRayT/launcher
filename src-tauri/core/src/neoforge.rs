use crate::download::{
    download_file, ensure_files, write_atomic, DownloadError, DownloadItem, Expected,
};
use crate::errors::LauncherError;
use crate::java::hide_console;
use crate::launch::LaunchContext;
use crate::libraries::{current_os, LibrarySet};
use crate::safety::is_sha1_hex;
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

const ARTIFACT_PATH: &str = "net/neoforged/neoforge";
const INSTALLER_METADATA_FILES: [&str; 2] = ["install_profile.json", "version.json"];
const LIBRARY_CONCURRENCY: usize = 8;
const LOG_TAIL_LINES: usize = 15;

/// Префикс версий NeoForge для версии игры: `1.21.1` → `21.1.`, `1.21` → `21.0.`, `26.1` → `26.1.`.
pub fn neoforge_prefix(minecraft: &str) -> String {
    let short = minecraft.strip_prefix("1.").unwrap_or(minecraft);
    if short.contains('.') {
        format!("{short}.")
    } else {
        format!("{short}.0.")
    }
}

fn parse_versions(metadata_xml: &str) -> Vec<String> {
    metadata_xml
        .split("<version>")
        .skip(1)
        .filter_map(|part| part.split("</version>").next())
        .map(str::to_string)
        .collect()
}

fn numeric_parts(version: &str) -> Vec<u64> {
    version
        .split('.')
        .filter_map(|part| part.parse().ok())
        .collect()
}

/// Самая новая стабильная версия: бета-версии содержат дефис.
fn latest_stable(versions: &[String], prefix: &str) -> Option<String> {
    versions
        .iter()
        .filter(|version| version.starts_with(prefix) && !version.contains('-'))
        .max_by_key(|version| numeric_parts(version))
        .cloned()
}

/// Ищет установленный NeoForge именно для этой версии игры.
pub fn find_installed_neoforge(versions_dir: &Path, minecraft: &str) -> Option<(String, PathBuf)> {
    std::fs::read_dir(versions_dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .find_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.to_lowercase().contains("neoforge") {
                return None;
            }
            let path = entry.path().join(format!("{name}.json"));
            let json: Value = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
            (json["inheritsFrom"].as_str() == Some(minecraft)).then_some((name, path))
        })
}

/// Основной сервер NeoForge доступен не везде, поэтому берётся первый ответивший из списка.
async fn find_reachable_maven(ctx: &LaunchContext) -> Result<(String, String), LauncherError> {
    let mut any_answer = false;
    for base in &ctx.endpoints.neoforge_mavens {
        let url = format!(
            "{}/{ARTIFACT_PATH}/maven-metadata.xml",
            base.trim_end_matches('/')
        );
        let Ok(response) = ctx.probe_client.get(&url).send().await else {
            ctx.reporter
                .log(format!("[NeoForge] Сервер {base} недоступен."));
            continue;
        };
        any_answer = true;
        if !response.status().is_success() {
            continue;
        }
        if let Ok(text) = response.text().await {
            ctx.reporter.log(format!("[NeoForge] Используем {base}"));
            return Ok((base.trim_end_matches('/').to_string(), text));
        }
    }
    if any_answer {
        Err(LauncherError::NeoForgeInstall(
            "серверы NeoForge не отдают список версий".to_string(),
        ))
    } else {
        Err(LauncherError::NeedsInternet {
            what: "NeoForge".to_string(),
        })
    }
}

/// Контрольная сумма необязательна: без неё файл защищён только HTTPS.
async fn fetch_sha1(ctx: &LaunchContext, url: &str) -> Option<String> {
    let response = ctx
        .probe_client
        .get(format!("{url}.sha1"))
        .send()
        .await
        .ok()?;
    let text = response.error_for_status().ok()?.text().await.ok()?;
    let hash = text.split_whitespace().next()?.to_string();
    is_sha1_hex(&hash).then_some(hash)
}

fn read_installer_json(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<Value> {
    let mut entry = archive.by_name(name).ok()?;
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    serde_json::from_str(&text).ok()
}

fn point_to_mirror(url: &str, mirrors: &[String], base: &str) -> String {
    mirrors
        .iter()
        .find_map(|mirror| url.strip_prefix(mirror.trim_end_matches('/')))
        .map_or_else(|| url.to_string(), |path| format!("{base}{path}"))
}

/// Библиотеки установщика с адресами на доступном сервере. Если заранее положить их на диск,
/// установщику не придётся обращаться к недоступным серверам.
fn installer_libraries(
    installer: &Path,
    libs_dir: &Path,
    mirrors: &[String],
    base: &str,
) -> Result<Vec<DownloadItem>, LauncherError> {
    let file = std::fs::File::open(installer)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| LauncherError::NeoForgeInstall(format!("повреждённый установщик: {e}")))?;
    let mut libraries = LibrarySet::default();
    let mut found_metadata = false;
    for name in INSTALLER_METADATA_FILES {
        if let Some(json) = read_installer_json(&mut archive, name) {
            found_metadata = true;
            libraries.collect(&json, libs_dir, current_os());
        }
    }
    if !found_metadata {
        return Err(LauncherError::NeoForgeInstall(
            "в установщике нет списка библиотек".to_string(),
        ));
    }
    Ok(libraries
        .downloads
        .into_iter()
        .map(|item| DownloadItem {
            url: point_to_mirror(&item.url, mirrors, base),
            ..item
        })
        .collect())
}

fn run_installer(java: &str, installer: &Path, root: &Path) -> Result<String, LauncherError> {
    let profiles = root.join("launcher_profiles.json");
    if !profiles.exists() {
        // Установщик требует этот файл, потому что рассчитан на официальный лаунчер.
        write_atomic(&profiles, b"{\"profiles\":{}}")?;
    }
    let mut command = Command::new(java);
    command
        .arg("-jar")
        .arg(installer)
        .arg("--install-client")
        .arg(root)
        .current_dir(root);
    hide_console(&mut command);
    let output = command
        .output()
        .map_err(|e| LauncherError::NeoForgeInstall(format!("не удалось запустить Java: {e}")))?;
    let log = String::from_utf8_lossy(&output.stdout).to_string();
    if output.status.success() {
        return Ok(log);
    }
    let lines: Vec<&str> = log.lines().collect();
    let tail = lines[lines.len().saturating_sub(LOG_TAIL_LINES)..].join(" | ");
    Err(LauncherError::NeoForgeInstall(format!(
        "установщик завершился с кодом {}: {tail}",
        output.status.code().unwrap_or(-1)
    )))
}

/// Скачивает и запускает установщик NeoForge для указанной версии игры.
pub async fn install_neoforge(
    ctx: &LaunchContext,
    minecraft: &str,
    java: &str,
) -> Result<(), LauncherError> {
    let (base, metadata) = find_reachable_maven(ctx).await?;
    let prefix = neoforge_prefix(minecraft);
    let version = latest_stable(&parse_versions(&metadata), &prefix).ok_or_else(|| {
        LauncherError::NeoForgeInstall(format!("нет стабильной версии для Minecraft {minecraft}"))
    })?;
    ctx.reporter
        .log(format!("[NeoForge] Устанавливаем {version}..."));
    ctx.reporter.progress("Установка NeoForge", 10, 100);

    let url = format!("{base}/{ARTIFACT_PATH}/{version}/neoforge-{version}-installer.jar");
    let installer = ctx
        .root
        .join("cache")
        .join("neoforge")
        .join(format!("neoforge-{version}-installer.jar"));
    let expected = Expected {
        sha1: fetch_sha1(ctx, &url).await,
        ..Expected::default()
    };
    download_file(&ctx.client, &url, &installer, &expected)
        .await
        .map_err(|err| match err {
            DownloadError::Unreachable(_) | DownloadError::Network(_) => {
                LauncherError::NeedsInternet {
                    what: "установщик NeoForge".to_string(),
                }
            }
            DownloadError::Hash | DownloadError::Size { .. } => LauncherError::Checksum {
                file: "установщик NeoForge".to_string(),
            },
            other => LauncherError::NeoForgeInstall(format!("{other:?}")),
        })?;

    let libs_dir = ctx.root.join("libraries");
    let items = installer_libraries(&installer, &libs_dir, &ctx.endpoints.neoforge_mavens, &base)?;
    ensure_files(
        &ctx.client,
        items,
        &ctx.reporter,
        "Библиотеки NeoForge",
        LIBRARY_CONCURRENCY,
    )
    .await?;

    ctx.reporter.progress("Установка NeoForge", 80, 100);
    let (java, installer_path, root) = (java.to_string(), installer.clone(), ctx.root.clone());
    let log = tokio::task::spawn_blocking(move || run_installer(&java, &installer_path, &root))
        .await
        .map_err(|e| LauncherError::Io(e.to_string()))??;
    for line in log
        .lines()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        ctx.reporter.log(format!("[NeoForge] {line}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn prefix_follows_the_neoforge_numbering() {
        assert_eq!(neoforge_prefix("1.21.1"), "21.1.");
        assert_eq!(neoforge_prefix("1.21"), "21.0.");
        assert_eq!(neoforge_prefix("26.1"), "26.1.");
    }

    #[test]
    fn newest_stable_version_is_chosen_numerically() {
        let xml = "<versions><version>21.1.9</version><version>21.1.100</version>\
                   <version>21.1.200-beta</version><version>21.4.1</version></versions>";
        let versions = parse_versions(xml);

        assert_eq!(versions.len(), 4);
        assert_eq!(
            latest_stable(&versions, "21.1.").as_deref(),
            Some("21.1.100")
        );
        assert_eq!(latest_stable(&versions, "20.1."), None);
    }

    #[test]
    fn installed_neoforge_must_match_the_game_version() {
        let dir = tempfile::tempdir().unwrap();
        let version = dir.path().join("neoforge-21.1.5");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(
            version.join("neoforge-21.1.5.json"),
            json!({ "inheritsFrom": "1.21.1" }).to_string(),
        )
        .unwrap();

        let found = find_installed_neoforge(dir.path(), "1.21.1").unwrap();
        assert_eq!(found.0, "neoforge-21.1.5");
        assert!(find_installed_neoforge(dir.path(), "1.21.4").is_none());
    }

    #[test]
    fn library_addresses_move_to_the_reachable_server() {
        let mirrors = vec![
            "https://maven.neoforged.net/releases".to_string(),
            "https://maven.creeperhost.net".to_string(),
        ];
        let base = "https://maven.creeperhost.net";

        assert_eq!(
            point_to_mirror(
                "https://maven.neoforged.net/releases/a/b.jar",
                &mirrors,
                base
            ),
            "https://maven.creeperhost.net/a/b.jar"
        );
        assert_eq!(
            point_to_mirror("https://libraries.minecraft.net/a/b.jar", &mirrors, base),
            "https://libraries.minecraft.net/a/b.jar"
        );
    }

    #[test]
    fn libraries_are_read_from_the_installer_with_their_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let installer = dir.path().join("installer.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&installer).unwrap());
        writer
            .start_file("install_profile.json", SimpleFileOptions::default())
            .unwrap();
        let profile = json!({ "libraries": [
            { "name": "a:b:1", "downloads": { "artifact": {
                "path": "a/b/1/b-1.jar",
                "url": "https://maven.neoforged.net/releases/a/b/1/b-1.jar",
                "sha1": "abc", "size": 3
            } } },
            { "name": "x:patched:1", "downloads": { "artifact": {
                "path": "x/patched/1/patched-1.jar", "url": ""
            } } },
            { "name": "evil:lib:1", "downloads": { "artifact": {
                "path": "../../evil.jar", "url": "https://x/e.jar"
            } } }
        ] });
        writer.write_all(profile.to_string().as_bytes()).unwrap();
        writer.finish().unwrap();
        let mirrors = vec!["https://maven.neoforged.net/releases".to_string()];

        let items = installer_libraries(
            &installer,
            Path::new("libs"),
            &mirrors,
            "https://mirror.test",
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://mirror.test/a/b/1/b-1.jar");
        assert_eq!(items[0].expected.sha1.as_deref(), Some("abc"));
    }

    #[test]
    fn broken_installer_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let installer = dir.path().join("installer.jar");
        std::fs::write(&installer, "not a zip").unwrap();

        let result = installer_libraries(&installer, Path::new("libs"), &[], "https://m");

        assert!(matches!(result, Err(LauncherError::NeoForgeInstall(_))));
    }
}
