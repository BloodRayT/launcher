use crate::errors::LauncherError;
use crate::reporter::Reporter;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

const USER_AGENT: &str = concat!(
    "FenixCraftLauncher/",
    env!("CARGO_PKG_VERSION"),
    " (fenixcraft.ru)"
);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_ATTEMPTS: u64 = 3;
const RETRY_DELAY_MS: u64 = 200;
const HASH_BUFFER_SIZE: usize = 64 * 1024;

/// Клиент для файлов: ограничено ожидание данных, а не общее время, поэтому большие файлы не обрываются.
pub fn download_client() -> Result<reqwest::Client, LauncherError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(|e| LauncherError::Io(e.to_string()))
}

/// Клиент для коротких запросов, от которых не должен зависеть запуск игры.
pub fn probe_client() -> Result<reqwest::Client, LauncherError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT.min(PROBE_TIMEOUT))
        .timeout(PROBE_TIMEOUT)
        .build()
        .map_err(|e| LauncherError::Io(e.to_string()))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Expected {
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub url: String,
    pub dest: PathBuf,
    pub expected: Expected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    /// Не удалось соединиться с сервером: обычно это отсутствие интернета.
    Unreachable(String),
    /// Соединение было, но передача оборвалась.
    Network(String),
    Status(u16),
    Size {
        expected: u64,
        actual: u64,
    },
    Hash,
    Io(String),
}

fn hash_file<D: Digest>(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = D::new();
    let mut buffer = vec![0u8; HASH_BUFFER_SIZE];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            return Some(hex::encode(hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

pub fn sha1_file(path: &Path) -> Option<String> {
    hash_file::<Sha1>(path)
}

pub fn sha256_file(path: &Path) -> Option<String> {
    hash_file::<Sha256>(path)
}

/// Проверяет файл на диске. Без ожидаемых значений файл считается целым, если он существует и не пуст.
pub fn file_matches(path: &Path, expected: &Expected) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || expected.size.is_some_and(|size| size != meta.len()) {
        return false;
    }
    if expected.size.is_none() && meta.len() == 0 {
        return false;
    }
    let sha1_ok = expected
        .sha1
        .as_deref()
        .is_none_or(|want| sha1_file(path).is_some_and(|got| got.eq_ignore_ascii_case(want)));
    sha1_ok
        && expected
            .sha256
            .as_deref()
            .is_none_or(|want| sha256_file(path).is_some_and(|got| got.eq_ignore_ascii_case(want)))
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().map(OsString::from).unwrap_or_default();
    name.push(".part");
    dest.with_file_name(name)
}

fn check_download(
    expected: &Expected,
    actual_size: u64,
    actual_sha1: &str,
    actual_sha256: &str,
) -> Result<(), DownloadError> {
    if let Some(size) = expected.size.filter(|size| *size != actual_size) {
        return Err(DownloadError::Size {
            expected: size,
            actual: actual_size,
        });
    }
    let sha1_ok = expected
        .sha1
        .as_deref()
        .is_none_or(|want| want.eq_ignore_ascii_case(actual_sha1));
    let sha256_ok = expected
        .sha256
        .as_deref()
        .is_none_or(|want| want.eq_ignore_ascii_case(actual_sha256));
    if sha1_ok && sha256_ok {
        Ok(())
    } else {
        Err(DownloadError::Hash)
    }
}

async fn stream_to_part(
    mut response: reqwest::Response,
    part: &Path,
) -> Result<(u64, String, String), DownloadError> {
    let mut file = tokio::fs::File::create(part)
        .await
        .map_err(|e| DownloadError::Io(e.to_string()))?;
    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut size = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| DownloadError::Network(e.to_string()))?
    {
        sha1.update(&chunk);
        sha256.update(&chunk);
        size += chunk.len() as u64;
        file.write_all(&chunk)
            .await
            .map_err(|e| DownloadError::Io(e.to_string()))?;
    }
    file.flush()
        .await
        .map_err(|e| DownloadError::Io(e.to_string()))?;
    Ok((
        size,
        hex::encode(sha1.finalize()),
        hex::encode(sha256.finalize()),
    ))
}

/// Скачивает файл во временный `.part` и заменяет итоговый файл только после всех проверок.
pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected: &Expected,
) -> Result<(), DownloadError> {
    let response = client.get(url).send().await.map_err(|e| {
        if e.is_connect() || e.is_timeout() {
            DownloadError::Unreachable(e.to_string())
        } else {
            DownloadError::Network(e.to_string())
        }
    })?;
    if !response.status().is_success() {
        return Err(DownloadError::Status(response.status().as_u16()));
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| DownloadError::Io(e.to_string()))?;
    }

    let part = part_path(dest);
    let verified = match stream_to_part(response, &part).await {
        Ok((size, sha1, sha256)) => check_download(expected, size, &sha1, &sha256),
        Err(err) => Err(err),
    };
    if let Err(err) = verified {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(err);
    }
    tokio::fs::rename(&part, dest)
        .await
        .map_err(|e| DownloadError::Io(e.to_string()))
}

async fn fetch_with_retries(
    client: &reqwest::Client,
    item: &DownloadItem,
    offline: &AtomicBool,
) -> Result<(), DownloadError> {
    let (dest, expected) = (item.dest.clone(), item.expected.clone());
    let already_valid = tokio::task::spawn_blocking(move || file_matches(&dest, &expected))
        .await
        .unwrap_or(false);
    if already_valid {
        return Ok(());
    }

    let mut last_error = DownloadError::Unreachable("сеть недоступна".to_string());
    for attempt in 1..=MAX_ATTEMPTS {
        if offline.load(Ordering::Relaxed) {
            return Err(last_error);
        }
        match download_file(client, &item.url, &item.dest, &item.expected).await {
            Ok(()) => return Ok(()),
            Err(DownloadError::Status(code)) if code < 500 => {
                return Err(DownloadError::Status(code));
            }
            Err(err @ DownloadError::Io(_)) => return Err(err),
            Err(err) => last_error = err,
        }
        tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS * attempt)).await;
    }
    // Если связи нет, остальным файлам незачем по очереди ждать таймаут.
    if matches!(last_error, DownloadError::Unreachable(_)) {
        offline.store(true, Ordering::Relaxed);
    }
    Err(last_error)
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn describe_failure(error: DownloadError, file: String) -> LauncherError {
    match error {
        DownloadError::Hash | DownloadError::Size { .. } => LauncherError::Checksum { file },
        DownloadError::Status(code) => LauncherError::Download {
            file,
            reason: format!("сервер ответил {code}"),
        },
        DownloadError::Unreachable(reason)
        | DownloadError::Network(reason)
        | DownloadError::Io(reason) => LauncherError::Download { file, reason },
    }
}

/// Докачивает недостающие и повреждённые файлы.
/// Ошибки соединения объединяются в `NeedsInternet`, остальные сообщаются для конкретного файла.
pub async fn ensure_files(
    client: &reqwest::Client,
    items: Vec<DownloadItem>,
    reporter: &Reporter,
    label: &str,
    concurrency: usize,
) -> Result<(), LauncherError> {
    if items.is_empty() {
        return Ok(());
    }
    let total = items.len();
    let progress_step = (total / 100).max(1);
    let semaphore = Arc::new(Semaphore::new(concurrency.max(1)));
    let completed = Arc::new(AtomicUsize::new(0));
    let offline = Arc::new(AtomicBool::new(false));

    let mut handles = Vec::with_capacity(total);
    for item in items {
        let (client, reporter, label) = (client.clone(), reporter.clone(), label.to_string());
        let (semaphore, completed, offline) =
            (semaphore.clone(), completed.clone(), offline.clone());
        handles.push(tokio::spawn(async move {
            let _permit = semaphore.acquire_owned().await;
            let result = fetch_with_retries(&client, &item, &offline).await;
            let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
            if done % progress_step == 0 || done == total {
                reporter.progress(label, done, total);
            }
            (display_name(&item.dest), result)
        }));
    }

    let mut network_failures = 0usize;
    let mut first_failure = None;
    for handle in handles {
        match handle.await {
            Ok((_, Ok(()))) => {}
            Ok((_, Err(DownloadError::Unreachable(_)))) => network_failures += 1,
            Ok((file, Err(error))) => {
                first_failure.get_or_insert_with(|| describe_failure(error, file));
            }
            Err(error) => {
                first_failure.get_or_insert(LauncherError::Io(error.to_string()));
            }
        }
    }
    if let Some(failure) = first_failure {
        return Err(failure);
    }
    if network_failures > 0 {
        return Err(LauncherError::NeedsInternet {
            what: format!("{label}: {network_failures} шт."),
        });
    }
    Ok(())
}

/// Читает ответ не больше указанного размера: чужой сервер не должен исчерпать память лаунчера.
pub async fn read_limited(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if body.len() + chunk.len() > limit {
            return Err(format!("ответ больше {limit} байт"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Атомарная запись: при сбое на диске остаётся прежний файл, а не обрезанный.
pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let part = part_path(path);
    std::fs::write(&part, contents)?;
    std::fs::rename(&part, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO_SHA1: &str = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";
    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    #[test]
    fn hashes_match_known_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.txt");
        std::fs::write(&path, "hello").unwrap();

        assert_eq!(sha1_file(&path).as_deref(), Some(HELLO_SHA1));
        assert_eq!(sha256_file(&path).as_deref(), Some(HELLO_SHA256));
    }

    #[test]
    fn file_matches_checks_presence_size_and_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.txt");
        let any = Expected::default();
        assert!(!file_matches(&path, &any), "missing file");

        std::fs::write(&path, "").unwrap();
        assert!(!file_matches(&path, &any), "empty file");

        std::fs::write(&path, "hello").unwrap();
        assert!(file_matches(&path, &any));
        assert!(file_matches(
            &path,
            &Expected {
                size: Some(5),
                ..any.clone()
            }
        ));
        assert!(!file_matches(
            &path,
            &Expected {
                size: Some(6),
                ..any.clone()
            }
        ));
        assert!(file_matches(
            &path,
            &Expected {
                sha1: Some(HELLO_SHA1.to_uppercase()),
                ..any.clone()
            }
        ));
        assert!(!file_matches(
            &path,
            &Expected {
                sha1: Some("0".repeat(40)),
                ..any.clone()
            }
        ));
        assert!(file_matches(
            &path,
            &Expected {
                sha256: Some(HELLO_SHA256.to_string()),
                ..any.clone()
            }
        ));
        assert!(!file_matches(
            &path,
            &Expected {
                sha256: Some("0".repeat(64)),
                ..any
            }
        ));
    }

    #[test]
    fn write_atomic_replaces_the_file_and_leaves_no_part() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.json");

        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        assert!(!part_path(&path).exists());
    }
}
