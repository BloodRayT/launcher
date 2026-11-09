use reqwest::Url;
use std::path::{Component, Path, PathBuf};

const WINDOWS_RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Присоединяет путь из внешних данных к корню. Возвращает `None`, если он может выйти за корень.
pub fn safe_join(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || relative.contains(['\\', ':', '\0']) {
        return None;
    }
    let path = Path::new(relative);
    for component in path.components() {
        let Component::Normal(segment) = component else {
            return None;
        };
        let segment = segment.to_str()?;
        // Windows отбрасывает завершающие точки и пробелы, поэтому ".. " превратилось бы в "..".
        if segment.ends_with(['.', ' ']) {
            return None;
        }
        let stem = segment.split('.').next().unwrap_or(segment);
        if WINDOWS_RESERVED_NAMES.contains(&stem.to_ascii_uppercase().as_str()) {
            return None;
        }
    }
    Some(root.join(path))
}

pub fn is_valid_username(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn is_safe_profile_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && !id.ends_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

pub fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_sha1_hex(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Разрешает открывать во внешнем браузере только обычные веб-адреса.
pub fn parse_external_url(raw: &str) -> Option<Url> {
    let url = Url::parse(raw).ok()?;
    let is_web = matches!(url.scheme(), "http" | "https");
    let has_credentials = !url.username().is_empty() || url.password().is_some();
    (is_web && url.host_str().is_some() && !has_credentials).then_some(url)
}

/// Установщик обновления можно скачивать только из релизов этого репозитория.
pub fn is_trusted_update_url(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.port().is_none()
        && url
            .path()
            .starts_with("/BloodRayT/launcher/releases/download/")
        && url.path().ends_with("-setup.exe")
}

/// Журнал пишется только в файл `.log` или `.txt`: команда получает путь из интерфейса.
pub fn is_log_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("log") || extension.eq_ignore_ascii_case("txt")
        })
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().trim_start_matches('v').split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    parts.next().is_none().then_some((major, minor, patch))
}

/// Обновление предлагается только на более новую версию, а не на любую другую.
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    matches!(
        (parse_version(candidate), parse_version(current)),
        (Some(candidate), Some(current)) if candidate > current
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_log_can_only_be_a_log_or_text_file() {
        assert!(is_log_file(Path::new("C:/Users/me/fenixcraft-debug.log")));
        assert!(is_log_file(Path::new("report.TXT")));
        assert!(!is_log_file(Path::new("C:/Users/me/run.bat")));
        assert!(!is_log_file(Path::new("launcher_config.json")));
        assert!(!is_log_file(Path::new("no-extension")));
    }

    #[test]
    fn safe_join_accepts_nested_relative_paths() {
        let joined = safe_join(Path::new("root"), "mods/sodium.jar").unwrap();
        assert_eq!(joined, Path::new("root").join("mods/sodium.jar"));
    }

    #[test]
    fn safe_join_rejects_paths_that_escape_the_root() {
        for bad in [
            "",
            "../evil.jar",
            "mods/../../evil.jar",
            "/etc/passwd",
            "C:/Windows/evil.dll",
            "C:evil.dll",
            "mods\\..\\evil.jar",
            "./mods/a.jar",
            "mods/.. /evil.jar",
            "mods/evil.jar:stream",
            "mods/NUL.jar",
            "mods/com1",
        ] {
            assert!(
                safe_join(Path::new("root"), bad).is_none(),
                "accepted {bad:?}"
            );
        }
    }

    #[test]
    fn username_follows_minecraft_rules() {
        assert!(is_valid_username("Fenix_123"));
        for bad in ["", "ab", "seventeen_chars__", "Игрок", "with space", "a-b"] {
            assert!(!is_valid_username(bad), "accepted {bad:?}");
        }
    }

    #[test]
    fn profile_id_cannot_be_a_path() {
        assert!(is_safe_profile_id("1.21.1-neoforge"));
        for bad in ["", "..", ".hidden", "a/b", "a\\b", "trailing."] {
            assert!(!is_safe_profile_id(bad), "accepted {bad:?}");
        }
    }

    #[test]
    fn hash_checks_need_exact_hex_length() {
        assert!(is_sha1_hex(&"a".repeat(40)));
        assert!(!is_sha1_hex(&"a".repeat(39)));
        assert!(!is_sha1_hex(&format!("../{}", "a".repeat(37))));
        assert!(is_sha256_hex(&"F".repeat(64)));
        assert!(!is_sha256_hex(&"g".repeat(64)));
    }

    #[test]
    fn external_urls_are_limited_to_the_web() {
        assert!(parse_external_url("https://vk.ru/fenixcraft").is_some());
        for bad in [
            "file:///C:/Windows/System32/calc.exe",
            "javascript:alert(1)",
            "ms-settings:",
            "calc.exe",
            "https://user:pass@example.com",
        ] {
            assert!(parse_external_url(bad).is_none(), "accepted {bad:?}");
        }
    }

    #[test]
    fn only_own_release_installers_are_trusted() {
        let good = "https://github.com/BloodRayT/launcher/releases/download/v1.1.0/FenixCraft_1.1.0_x64-setup.exe";
        assert!(is_trusted_update_url(good));
        for bad in [
            "http://github.com/BloodRayT/launcher/releases/download/v1/a-setup.exe",
            "https://evil.example/BloodRayT/launcher/releases/download/v1/a-setup.exe",
            "https://github.com/someone/launcher/releases/download/v1/a-setup.exe",
            "https://github.com/BloodRayT/launcher/releases/download/../../../x/a-setup.exe",
            "https://github.com/BloodRayT/launcher/releases/download/v1/a.zip",
            "https://github.com.evil.example/BloodRayT/launcher/releases/download/v1/a-setup.exe",
        ] {
            assert!(!is_trusted_update_url(bad), "accepted {bad:?}");
        }
    }

    #[test]
    fn update_is_offered_only_for_a_newer_version() {
        assert!(is_newer_version("v1.0.13", "1.0.12"));
        assert!(is_newer_version("1.0.10", "1.0.9"));
        assert!(is_newer_version("2.0", "1.9.9"));
        assert!(!is_newer_version("1.0.12", "1.0.12"));
        assert!(!is_newer_version("1.0.9", "1.0.10"));
        assert!(!is_newer_version("nightly", "1.0.0"));
    }
}
