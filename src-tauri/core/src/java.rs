use crate::download::{download_file, DownloadError, Expected};
use crate::errors::LauncherError;
use crate::launch::LaunchContext;
use crate::libraries::extract_zip;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_JAVA_MAJOR: u32 = 21;

/// На Windows скрывает окно консоли у дочернего процесса.
pub(crate) fn hide_console(command: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = command;
}

pub fn required_java_major(version: &Value) -> u32 {
    version["javaVersion"]["majorVersion"]
        .as_u64()
        .and_then(|major| u32::try_from(major).ok())
        .unwrap_or(DEFAULT_JAVA_MAJOR)
}

/// Извлекает основную версию из вывода `java -version`: `"21.0.4"` → 21, `"1.8.0_202"` → 8.
pub fn parse_java_major(output: &str) -> Option<u32> {
    let start = output.find("version \"")? + "version \"".len();
    let version = output[start..].split('"').next()?;
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

fn java_major_of(binary: &str) -> Option<u32> {
    let mut command = Command::new(binary);
    command.arg("-version");
    hide_console(&mut command);
    let output = command.output().ok()?;
    parse_java_major(&format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    ))
}

fn find_java_binary(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join("bin").join(name);
    if direct.is_file() {
        return Some(direct);
    }
    let mac_bundle = dir.join("Contents").join("Home").join("bin").join(name);
    if mac_bundle.is_file() {
        return Some(mac_bundle);
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .find_map(|path| find_java_binary(&path, name))
}

struct JavaPackage {
    link: String,
    expected: Expected,
}

fn parse_java_package(response: &Value) -> Option<JavaPackage> {
    let package = &response.as_array()?.first()?["binary"]["package"];
    Some(JavaPackage {
        link: package["link"].as_str()?.to_string(),
        expected: Expected {
            sha256: package["checksum"].as_str().map(str::to_string),
            size: package["size"].as_u64(),
            ..Expected::default()
        },
    })
}

async fn find_java_package(ctx: &LaunchContext, major: u32) -> Result<JavaPackage, LauncherError> {
    let os = match crate::libraries::current_os() {
        "osx" => "mac",
        other => other,
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    let base = ctx.endpoints.adoptium.trim_end_matches('/');
    for image in ["jre", "jdk"] {
        let url = format!(
            "{base}/assets/latest/{major}/hotspot?architecture={arch}&image_type={image}&os={os}&vendor=eclipse"
        );
        let response =
            ctx.probe_client
                .get(&url)
                .send()
                .await
                .map_err(|_| LauncherError::NeedsInternet {
                    what: format!("Java {major}"),
                })?;
        if !response.status().is_success() {
            continue;
        }
        let json: Value = response.json().await.map_err(|e| LauncherError::Java {
            major,
            reason: e.to_string(),
        })?;
        if let Some(package) = parse_java_package(&json) {
            return Ok(package);
        }
    }
    Err(LauncherError::Java {
        major,
        reason: "Adoptium не предлагает сборку для этой системы".to_string(),
    })
}

fn unpack_java(archive: &Path, target: &Path, major: u32) -> Result<(), LauncherError> {
    if archive.extension().is_some_and(|ext| ext == "zip") {
        return extract_zip(archive, target, false);
    }
    let status = Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(target)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(LauncherError::Java {
            major,
            reason: "не удалось распаковать архив".to_string(),
        })
    }
}

async fn install_java(
    ctx: &LaunchContext,
    runtime_dir: &Path,
    binary_name: &str,
    major: u32,
) -> Result<PathBuf, LauncherError> {
    ctx.reporter.log(format!(
        "[Java] Java {major} не найдена, скачиваем с Adoptium..."
    ));
    ctx.reporter
        .progress(format!("Скачивание Java {major}"), 20, 100);
    let package = find_java_package(ctx, major).await?;

    // Распаковка идёт во временную папку: прерванная установка не должна выглядеть готовой Java.
    let staging = runtime_dir.with_extension("tmp");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let extension = if cfg!(target_os = "windows") {
        "zip"
    } else {
        "tar.gz"
    };
    let archive = staging.join(format!("adoptium-java{major}.{extension}"));
    download_file(&ctx.client, &package.link, &archive, &package.expected)
        .await
        .map_err(|err| match err {
            DownloadError::Unreachable(_) | DownloadError::Network(_) => {
                LauncherError::NeedsInternet {
                    what: format!("Java {major}"),
                }
            }
            DownloadError::Hash | DownloadError::Size { .. } => LauncherError::Checksum {
                file: format!("Java {major}"),
            },
            other => LauncherError::Java {
                major,
                reason: format!("{other:?}"),
            },
        })?;

    ctx.reporter
        .progress(format!("Распаковка Java {major}"), 85, 100);
    let unpacked = unpack_java(&archive, &staging, major);
    let _ = std::fs::remove_file(&archive);
    unpacked?;

    let unpacked_binary =
        find_java_binary(&staging, binary_name).ok_or_else(|| LauncherError::Java {
            major,
            reason: "исполняемый файл не найден после распаковки".to_string(),
        })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&unpacked_binary, std::fs::Permissions::from_mode(0o755))?;
    }
    if !java_runs(&unpacked_binary) {
        return Err(LauncherError::Java {
            major,
            reason: "скачанная Java не запускается".to_string(),
        });
    }

    let _ = std::fs::remove_dir_all(runtime_dir);
    std::fs::rename(&staging, runtime_dir)?;
    find_java_binary(runtime_dir, binary_name).ok_or_else(|| LauncherError::Java {
        major,
        reason: "исполняемый файл не найден после установки".to_string(),
    })
}

fn java_runs(binary: &Path) -> bool {
    java_major_of(&binary.to_string_lossy()).is_some()
}

/// Находит подходящую Java. Скачивание нужно только тогда, когда локальной Java этой версии нет.
pub async fn ensure_java(
    ctx: &LaunchContext,
    custom_path: &str,
    major: u32,
) -> Result<String, LauncherError> {
    let custom = custom_path.trim();
    if !custom.is_empty() {
        if Path::new(custom).is_file() {
            if java_major_of(custom).is_some_and(|found| found != major) {
                ctx.reporter.log(format!(
                    "[Java] Внимание: для этой версии рекомендуется Java {major}, а в настройках указана другая."
                ));
            }
            return Ok(custom.to_string());
        }
        ctx.reporter.log(format!(
            "[Java] Путь из настроек не найден: {custom}. Подбираем Java автоматически."
        ));
    }

    let binary_name = if cfg!(windows) { "java.exe" } else { "java" };
    let runtime_dir = ctx.root.join("runtime").join(format!("java-{major}"));
    if let Some(found) = find_java_binary(&runtime_dir, binary_name) {
        if java_runs(&found) {
            return Ok(found.to_string_lossy().to_string());
        }
        ctx.reporter
            .log("[Java] Установленная Java повреждена, устанавливаем заново.");
    }
    let legacy = ctx
        .root
        .join("java")
        .join(format!("jdk-{major}"))
        .join("bin")
        .join(binary_name);
    if legacy.is_file() {
        return Ok(legacy.to_string_lossy().to_string());
    }
    if java_major_of("java") == Some(major) {
        return Ok("java".to_string());
    }

    let binary = install_java(ctx, &runtime_dir, binary_name, major).await?;
    Ok(binary.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn java_version_output_is_parsed() {
        let modern = "openjdk version \"21.0.4\" 2024-07-16\nOpenJDK Runtime Environment";
        assert_eq!(parse_java_major(modern), Some(21));
        assert_eq!(parse_java_major("java version \"1.8.0_202\""), Some(8));
        assert_eq!(
            parse_java_major("openjdk version \"25\" 2025-09-16"),
            Some(25)
        );
        assert_eq!(parse_java_major("no java here"), None);
    }

    #[test]
    fn required_java_comes_from_the_version_file() {
        assert_eq!(
            required_java_major(&json!({ "javaVersion": { "majorVersion": 25 } })),
            25
        );
        assert_eq!(required_java_major(&json!({})), DEFAULT_JAVA_MAJOR);
    }

    #[test]
    fn adoptium_package_keeps_its_checksum() {
        let response = json!([{ "binary": { "package": {
            "link": "https://x/jre.zip", "checksum": "abc", "size": 10
        } } }]);

        let package = parse_java_package(&response).unwrap();

        assert_eq!(package.link, "https://x/jre.zip");
        assert_eq!(package.expected.sha256.as_deref(), Some("abc"));
        assert_eq!(package.expected.size, Some(10));
        assert!(parse_java_package(&json!([])).is_none());
    }

    #[test]
    fn java_binary_is_found_inside_an_unpacked_folder() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("jdk-21.0.4-jre").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("java"), "").unwrap();

        assert_eq!(find_java_binary(dir.path(), "java"), Some(bin.join("java")));
        assert_eq!(find_java_binary(dir.path(), "missing"), None);
    }
}
