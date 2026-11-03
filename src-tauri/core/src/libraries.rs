use crate::download::{DownloadItem, Expected};
use crate::errors::LauncherError;
use crate::safety::safe_join;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn current_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn maven_to_rel_path(coord: &str) -> Option<String> {
    let clean = coord.split('@').next().unwrap_or(coord);
    let parts: Vec<&str> = clean.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let (artifact, version) = (parts[1], parts[2]);
    let file_name = match parts.get(3) {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    Some(format!("{group}/{artifact}/{version}/{file_name}"))
}

fn arch_matches(arch: &str) -> bool {
    if cfg!(target_arch = "x86_64") {
        matches!(arch, "x64" | "x86_64" | "amd64")
    } else if cfg!(target_arch = "aarch64") {
        matches!(arch, "arm64" | "aarch64")
    } else {
        arch == "x86"
    }
}

fn is_lib_allowed(lib: &Value, os: &str) -> bool {
    let Some(rules) = lib["rules"].as_array() else {
        return true;
    };
    let mut allowed = false;
    for rule in rules {
        let os_match = rule["os"]["name"].as_str().is_none_or(|name| name == os);
        let arch_match = rule["os"]["arch"].as_str().is_none_or(arch_matches);
        if os_match && arch_match {
            allowed = rule["action"].as_str().unwrap_or("allow") == "allow";
        }
    }
    allowed
}

/// Правила для аргументов запуска. Условия по функциям (демо-режим и т.п.) у нас не выполняются.
pub fn argument_allowed(item: &Value, os: &str) -> bool {
    let uses_features = item["rules"]
        .as_array()
        .is_some_and(|rules| rules.iter().any(|rule| rule.get("features").is_some()));
    !uses_features && is_lib_allowed(item, os)
}

fn is_wrong_native_arch(rel_path: &str, os: &str) -> bool {
    let lower = rel_path.to_lowercase();
    let foreign_os = (os != "osx"
        && (lower.contains("java-objc-bridge")
            || lower.contains("natives-macos")
            || lower.contains("natives-osx")))
        || (os != "linux" && lower.contains("natives-linux"))
        || (os != "windows" && lower.contains("natives-windows"));
    let foreign_arch = cfg!(target_arch = "x86_64")
        && (lower.contains("arm64")
            || lower.contains("aarch64")
            || lower.contains("natives-windows-x86")
            || lower.contains("natives-linux-arm"));
    foreign_os || foreign_arch
}

fn expected_of(source: &Value) -> Expected {
    Expected {
        sha1: source["sha1"].as_str().map(str::to_string),
        size: source["size"].as_u64(),
        ..Expected::default()
    }
}

/// Набор библиотек версии: classpath, файлы для докачки и файлы, которые негде взять.
#[derive(Debug, Default)]
pub struct LibrarySet {
    pub classpath: Vec<PathBuf>,
    pub downloads: Vec<DownloadItem>,
    pub unresolved: Vec<PathBuf>,
    unsafe_paths: Vec<String>,
    seen: HashSet<String>,
}

impl LibrarySet {
    pub fn seeded(classpath: &[PathBuf]) -> Self {
        Self {
            seen: classpath.iter().map(|p| key_of(p)).collect(),
            ..Self::default()
        }
    }

    fn add(&mut self, path: PathBuf, url: Option<String>, expected: Expected) {
        if !self.seen.insert(key_of(&path)) {
            return;
        }
        self.classpath.push(path.clone());
        match url.filter(|url| !url.is_empty()) {
            Some(url) => self.downloads.push(DownloadItem {
                url,
                dest: path,
                expected,
            }),
            None if !path.exists() => self.unresolved.push(path),
            None => {}
        }
    }

    pub fn collect(&mut self, version: &Value, libs_dir: &Path, os: &str) {
        let Some(libraries) = version["libraries"].as_array() else {
            return;
        };
        for lib in libraries.iter().filter(|lib| is_lib_allowed(lib, os)) {
            self.collect_artifact(lib, libs_dir, os);
            self.collect_classifier(lib, libs_dir, os);
        }
    }

    fn collect_artifact(&mut self, lib: &Value, libs_dir: &Path, os: &str) {
        let artifact = &lib["downloads"]["artifact"];
        let (rel, url, expected) = match artifact["path"].as_str() {
            Some(path) => (
                Some(path.to_string()),
                artifact["url"].as_str().map(str::to_string),
                expected_of(artifact),
            ),
            None => {
                let rel = lib["name"].as_str().and_then(maven_to_rel_path);
                let url = match (lib["url"].as_str(), &rel) {
                    (Some(base), Some(rel)) => {
                        Some(format!("{}/{rel}", base.trim_end_matches('/')))
                    }
                    _ => None,
                };
                (rel, url, expected_of(lib))
            }
        };
        if let Some(rel) = rel.filter(|rel| !is_wrong_native_arch(rel, os)) {
            self.add_checked(libs_dir, &rel, url, expected);
        }
    }

    /// Пути приходят из внешних JSON, поэтому не должны выходить за папку библиотек.
    fn add_checked(&mut self, libs_dir: &Path, rel: &str, url: Option<String>, expected: Expected) {
        match safe_join(libs_dir, rel) {
            Some(path) => self.add(path, url, expected),
            None => self.unsafe_paths.push(rel.to_string()),
        }
    }

    fn collect_classifier(&mut self, lib: &Value, libs_dir: &Path, os: &str) {
        let key = match os {
            "windows" => "natives-windows",
            "osx" => "natives-macos",
            _ => "natives-linux",
        };
        let native = &lib["downloads"]["classifiers"][key];
        if let (Some(path), Some(url)) = (native["path"].as_str(), native["url"].as_str()) {
            if !is_wrong_native_arch(path, os) {
                self.add_checked(libs_dir, path, Some(url.to_string()), expected_of(native));
            }
        }
    }

    pub fn ensure_resolved(&self) -> Result<(), LauncherError> {
        if let Some(rel) = self.unsafe_paths.first() {
            return Err(LauncherError::BadData(format!(
                "недопустимый путь библиотеки «{rel}»"
            )));
        }
        match self.unresolved.first() {
            None => Ok(()),
            Some(path) => Err(LauncherError::BadData(format!(
                "нет библиотеки {} и адреса для её загрузки",
                path.file_name().unwrap_or_default().to_string_lossy()
            ))),
        }
    }
}

fn key_of(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

pub fn extract_zip(archive: &Path, target: &Path, skip_meta: bool) -> Result<(), LauncherError> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| LauncherError::BadData(e.to_string()))?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|e| LauncherError::BadData(e.to_string()))?;
        // enclosed_name отбрасывает записи с выходом за пределы папки.
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let name = rel.to_string_lossy();
        if skip_meta
            && (name.starts_with("META-INF") || name.ends_with(".git") || name.ends_with(".sha1"))
        {
            continue;
        }
        let out = target.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out_file = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut out_file)?;
    }
    Ok(())
}

pub fn extract_natives(classpath: &[PathBuf], natives_dir: &Path) -> Result<(), LauncherError> {
    let _ = std::fs::remove_dir_all(natives_dir);
    std::fs::create_dir_all(natives_dir)?;
    for jar in classpath {
        let name = jar
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let is_native =
            name.contains("natives") && !name.contains("arm64") && !name.contains("x86");
        if is_native && jar.exists() {
            extract_zip(jar, natives_dir, true)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maven_coordinates_become_paths() {
        assert_eq!(
            maven_to_rel_path("net.fabricmc:fabric-loader:0.16.10").as_deref(),
            Some("net/fabricmc/fabric-loader/0.16.10/fabric-loader-0.16.10.jar")
        );
        assert_eq!(
            maven_to_rel_path("org.lwjgl:lwjgl:3.3.3:natives-windows@jar").as_deref(),
            Some("org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar")
        );
        assert_eq!(maven_to_rel_path("broken"), None);
    }

    #[test]
    fn rules_are_applied_in_order() {
        let only_windows = json!({ "rules": [{ "action": "allow", "os": { "name": "windows" } }] });
        assert!(is_lib_allowed(&only_windows, "windows"));
        assert!(!is_lib_allowed(&only_windows, "linux"));

        let not_osx = json!({ "rules": [
            { "action": "allow" },
            { "action": "disallow", "os": { "name": "osx" } }
        ] });
        assert!(is_lib_allowed(&not_osx, "linux"));
        assert!(!is_lib_allowed(&not_osx, "osx"));
        assert!(is_lib_allowed(&json!({}), "linux"));
    }

    #[test]
    fn collect_keeps_hashes_and_removes_duplicates() {
        let version = json!({ "libraries": [
            { "downloads": { "artifact": { "path": "a/a.jar", "url": "https://x/a.jar", "sha1": "s1", "size": 5 } } },
            { "downloads": { "artifact": { "path": "a/a.jar", "url": "https://x/a.jar", "sha1": "s1", "size": 5 } } },
            { "name": "b.c:d:1", "url": "https://maven.test/", "sha1": "s2" }
        ] });
        let mut set = LibrarySet::default();

        set.collect(&version, Path::new("libs"), "linux");

        assert_eq!(set.classpath.len(), 2);
        assert_eq!(set.downloads.len(), 2);
        assert_eq!(set.downloads[0].expected.sha1.as_deref(), Some("s1"));
        assert_eq!(set.downloads[0].expected.size, Some(5));
        assert_eq!(set.downloads[1].url, "https://maven.test/b/c/d/1/d-1.jar");
        assert_eq!(set.downloads[1].expected.sha1.as_deref(), Some("s2"));
    }

    #[test]
    fn missing_library_without_a_source_is_reported() {
        let version = json!({ "libraries": [{ "name": "b.c:patched:1" }] });
        let dir = tempfile::tempdir().unwrap();
        let mut set = LibrarySet::default();

        set.collect(&version, dir.path(), "linux");

        assert!(matches!(
            set.ensure_resolved(),
            Err(LauncherError::BadData(_))
        ));
    }

    #[test]
    fn library_path_cannot_leave_the_libraries_folder() {
        let version = json!({ "libraries": [
            { "downloads": { "artifact": { "path": "../../evil.jar", "url": "https://x/e.jar" } } }
        ] });
        let mut set = LibrarySet::default();

        set.collect(&version, Path::new("libs"), "linux");

        assert!(set.downloads.is_empty());
        assert!(set.classpath.is_empty());
        assert!(matches!(
            set.ensure_resolved(),
            Err(LauncherError::BadData(_))
        ));
    }

    #[test]
    fn natives_for_other_systems_are_skipped() {
        let version = json!({ "libraries": [
            { "downloads": { "artifact": { "path": "x/lwjgl-3-natives-linux.jar", "url": "https://x/l.jar" } } },
            { "downloads": { "artifact": { "path": "x/lwjgl-3-natives-windows.jar", "url": "https://x/w.jar" } } }
        ] });
        let mut set = LibrarySet::default();

        set.collect(&version, Path::new("libs"), "linux");

        assert_eq!(set.downloads.len(), 1);
        assert_eq!(set.downloads[0].url, "https://x/l.jar");
    }
}
