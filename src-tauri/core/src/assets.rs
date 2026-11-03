use crate::download::{ensure_files, file_matches, sha1_file, DownloadItem, Expected};
use crate::errors::LauncherError;
use crate::launch::LaunchContext;
use crate::reporter::Reporter;
use crate::safety::{is_safe_profile_id, is_sha1_hex};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const ASSET_CONCURRENCY: usize = 16;

fn read_index(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Уникальные объекты индекса. Записи с некорректным хешем отбрасываются: хеш становится частью пути.
fn unique_objects(index: &Value) -> Vec<(String, Option<u64>)> {
    let mut seen = HashSet::new();
    index["objects"]
        .as_object()
        .into_iter()
        .flat_map(|objects| objects.values())
        .filter_map(|object| {
            let hash = object["hash"].as_str().filter(|hash| is_sha1_hex(hash))?;
            seen.insert(hash.to_string())
                .then(|| (hash.to_string(), object["size"].as_u64()))
        })
        .collect()
}

fn object_path(objects_dir: &Path, hash: &str) -> PathBuf {
    objects_dir.join(&hash[..2]).join(hash)
}

/// Быстрая проверка перед запуском: файл есть и его размер совпадает.
/// Если у официального лаунчера файл уже есть, он копируется после проверки хеша.
fn plan_downloads(
    index: &Value,
    objects_dir: &Path,
    official_objects_dir: &Path,
    assets_base: &str,
) -> Vec<DownloadItem> {
    unique_objects(index)
        .into_iter()
        .filter_map(|(hash, size)| {
            let dest = object_path(objects_dir, &hash);
            if file_matches(
                &dest,
                &Expected {
                    size,
                    ..Expected::default()
                },
            ) {
                return None;
            }
            let expected = Expected {
                sha1: Some(hash.clone()),
                size,
                ..Expected::default()
            };
            let official = object_path(official_objects_dir, &hash);
            if file_matches(&official, &expected) {
                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::copy(&official, &dest).is_ok() {
                    return None;
                }
            }
            Some(DownloadItem {
                url: format!("{assets_base}/{}/{hash}", &hash[..2]),
                dest,
                expected,
            })
        })
        .collect()
}

/// Подготавливает ресурсы версии и возвращает id индекса.
pub async fn ensure_assets(ctx: &LaunchContext, version: &Value) -> Result<String, LauncherError> {
    let info = &version["assetIndex"];
    let asset_id = info["id"]
        .as_str()
        .or_else(|| version["assets"].as_str())
        .filter(|id| is_safe_profile_id(id))
        .ok_or_else(|| LauncherError::BadData("у версии нет индекса ресурсов".to_string()))?
        .to_string();

    let assets_dir = ctx.root.join("assets");
    let index_path = assets_dir.join("indexes").join(format!("{asset_id}.json"));
    if let Some(url) = info["url"].as_str() {
        let item = DownloadItem {
            url: url.to_string(),
            dest: index_path.clone(),
            expected: Expected {
                sha1: info["sha1"].as_str().map(str::to_string),
                ..Expected::default()
            },
        };
        let updated =
            ensure_files(&ctx.client, vec![item], &ctx.reporter, "Индекс ресурсов", 1).await;
        if let Err(err) = updated {
            if read_index(&index_path).is_none() {
                return Err(err);
            }
        }
    }
    let index = read_index(&index_path)
        .ok_or_else(|| LauncherError::BadData(format!("индекс ресурсов {asset_id} повреждён")))?;

    let objects_dir = assets_dir.join("objects");
    let official_dir = ctx.official_mc_dir.join("assets").join("objects");
    let base = ctx.endpoints.assets_base.trim_end_matches('/').to_string();
    let missing = tokio::task::spawn_blocking(move || {
        plan_downloads(&index, &objects_dir, &official_dir, &base)
    })
    .await
    .map_err(|e| LauncherError::Io(e.to_string()))?;

    if !missing.is_empty() {
        ctx.reporter.log(format!(
            "[Ресурсы] Нужно докачать файлов: {}",
            missing.len()
        ));
    }
    ensure_files(
        &ctx.client,
        missing,
        &ctx.reporter,
        "Ресурсы игры",
        ASSET_CONCURRENCY,
    )
    .await?;
    Ok(asset_id)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssetAudit {
    pub ok: bool,
    pub total: usize,
    pub valid: usize,
    pub missing: usize,
    pub corrupt: usize,
    pub percent: usize,
}

/// Полная проверка всех ресурсов по SHA-1. Работает долго, вызывается вне потока запуска.
pub fn verify_assets(root: &Path, reporter: &Reporter) -> AssetAudit {
    let indexes_dir = root.join("assets").join("indexes");
    let objects_dir = root.join("assets").join("objects");
    let mut seen = HashSet::new();
    let (mut valid, mut missing, mut corrupt) = (0usize, 0usize, 0usize);

    reporter.log("[Проверка] Сверяем ресурсы игры по контрольным суммам...");
    for entry in std::fs::read_dir(&indexes_dir)
        .into_iter()
        .flatten()
        .flatten()
    {
        let Some(index) = read_index(&entry.path()) else {
            continue;
        };
        for (hash, _) in unique_objects(&index) {
            if !seen.insert(hash.clone()) {
                continue;
            }
            let path = object_path(&objects_dir, &hash);
            match sha1_file(&path) {
                None => missing += 1,
                Some(actual) if actual.eq_ignore_ascii_case(&hash) => valid += 1,
                Some(_) => corrupt += 1,
            }
        }
    }

    let total = seen.len();
    let percent = (valid * 100).checked_div(total).unwrap_or(0);
    let ok = total > 0 && valid == total;
    reporter.log(if total == 0 {
        "[Проверка] Ресурсы ещё не скачаны. Нажмите «Играть» для первой загрузки.".to_string()
    } else if ok {
        format!("[Проверка] Все ресурсы целы: {valid} из {total}.")
    } else {
        format!(
            "[Проверка] Целых файлов: {valid} из {total} ({percent}%). Нет: {missing}, повреждено: {corrupt}."
        )
    });
    AssetAudit {
        ok,
        total,
        valid,
        missing,
        corrupt,
        percent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const HELLO_SHA1: &str = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";

    fn index() -> Value {
        json!({ "objects": {
            "a": { "hash": HELLO_SHA1, "size": 5 },
            "copy-of-a": { "hash": HELLO_SHA1, "size": 5 },
            "bad-hash": { "hash": "../../evil", "size": 5 }
        } })
    }

    #[test]
    fn duplicates_and_unsafe_hashes_are_dropped() {
        assert_eq!(
            unique_objects(&index()),
            vec![(HELLO_SHA1.to_string(), Some(5))]
        );
    }

    #[test]
    fn missing_object_is_planned_for_download() {
        let dir = tempfile::tempdir().unwrap();

        let plan = plan_downloads(&index(), dir.path(), &dir.path().join("none"), "https://a");

        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].url, format!("https://a/aa/{HELLO_SHA1}"));
        assert_eq!(plan[0].expected.sha1.as_deref(), Some(HELLO_SHA1));
    }

    #[test]
    fn object_from_the_official_launcher_is_reused_only_when_valid() {
        let dir = tempfile::tempdir().unwrap();
        let official = dir.path().join("official");
        let objects = dir.path().join("objects");
        let source = object_path(&official, HELLO_SHA1);
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();

        std::fs::write(&source, "HELLO").unwrap();
        assert_eq!(
            plan_downloads(&index(), &objects, &official, "https://a").len(),
            1
        );

        std::fs::write(&source, "hello").unwrap();
        assert!(plan_downloads(&index(), &objects, &official, "https://a").is_empty());
        assert!(object_path(&objects, HELLO_SHA1).exists());
    }

    #[test]
    fn audit_tells_missing_from_corrupt_files() {
        let dir = tempfile::tempdir().unwrap();
        let indexes = dir.path().join("assets").join("indexes");
        std::fs::create_dir_all(&indexes).unwrap();
        let hello_two = "b".repeat(40);
        std::fs::write(
            indexes.join("1.json"),
            json!({ "objects": {
                "a": { "hash": HELLO_SHA1, "size": 5 },
                "b": { "hash": hello_two, "size": 5 }
            } })
            .to_string(),
        )
        .unwrap();
        let objects = dir.path().join("assets").join("objects");
        let good = object_path(&objects, HELLO_SHA1);
        std::fs::create_dir_all(good.parent().unwrap()).unwrap();

        std::fs::write(&good, "WRONG").unwrap();
        let audit = verify_assets(dir.path(), &Reporter::silent());
        assert_eq!((audit.valid, audit.missing, audit.corrupt), (0, 1, 1));
        assert!(!audit.ok);

        std::fs::write(&good, "hello").unwrap();
        let audit = verify_assets(dir.path(), &Reporter::silent());
        assert_eq!(
            (audit.valid, audit.missing, audit.corrupt, audit.percent),
            (1, 1, 0, 50)
        );
    }
}
