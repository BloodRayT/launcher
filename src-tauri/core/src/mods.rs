use crate::download::{ensure_files, DownloadItem, Expected};
use crate::errors::LauncherError;
use crate::launch::LaunchContext;
use crate::safety::safe_join;
use serde_json::Value;
use std::path::Path;

fn has_mod(mods_dir: &Path, slug: &str) -> bool {
    std::fs::read_dir(mods_dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            name.contains(slug) && name.ends_with(".jar")
        })
}

/// Основной файл последней версии мода из ответа Modrinth.
fn pick_mod_file(mods_dir: &Path, versions: &Value) -> Option<DownloadItem> {
    let files = versions.as_array()?.first()?["files"].as_array()?;
    let file = files
        .iter()
        .find(|file| file["primary"].as_bool() == Some(true))
        .or_else(|| files.first())?;
    Some(DownloadItem {
        url: file["url"].as_str()?.to_string(),
        dest: safe_join(mods_dir, file["filename"].as_str()?)?,
        expected: Expected {
            sha1: file["hashes"]["sha1"].as_str().map(str::to_string),
            size: file["size"].as_u64(),
            ..Expected::default()
        },
    })
}

async fn install_mod(
    ctx: &LaunchContext,
    mods_dir: &Path,
    slug: &str,
    minecraft: &str,
    loader: &str,
) -> Result<(), LauncherError> {
    let base = ctx.endpoints.modrinth.trim_end_matches('/');
    let url = format!(
        "{base}/project/{slug}/version?loaders=[\"{loader}\"]&game_versions=[\"{minecraft}\"]"
    );
    let unavailable = || LauncherError::NeedsInternet {
        what: format!("мод {slug}"),
    };
    let response = ctx
        .probe_client
        .get(&url)
        .send()
        .await
        .map_err(|_| unavailable())?;
    if !response.status().is_success() {
        return Err(LauncherError::Download {
            file: format!("мод {slug}"),
            reason: format!("Modrinth ответил {}", response.status().as_u16()),
        });
    }
    let versions: Value = response.json().await.map_err(|_| unavailable())?;
    let item = pick_mod_file(mods_dir, &versions).ok_or_else(|| {
        LauncherError::BadData(format!("мод {slug} недоступен для {minecraft} ({loader})"))
    })?;
    ensure_files(
        &ctx.client,
        vec![item],
        &ctx.reporter,
        &format!("Мод {slug}"),
        1,
    )
    .await
}

/// Ставит Sodium и Iris, а для Fabric ещё и Fabric API.
/// Подходит только точная версия игры: мод для другой версии привёл бы к вылету.
pub async fn ensure_shader_mods(
    ctx: &LaunchContext,
    minecraft: &str,
    loader: &str,
) -> Result<(), LauncherError> {
    let mods_dir = ctx.root.join("mods");
    std::fs::create_dir_all(&mods_dir)?;
    let mut slugs = vec!["sodium", "iris"];
    if loader == "fabric" {
        slugs.push("fabric-api");
    }
    for slug in slugs {
        if !has_mod(&mods_dir, slug) {
            ctx.reporter.log(format!(
                "[Моды] Устанавливаем {slug} для {minecraft} ({loader})..."
            ));
            install_mod(ctx, &mods_dir, slug, minecraft, loader).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn primary_file_is_preferred_and_keeps_its_hash() {
        let response = json!([{ "files": [
            { "url": "https://x/sources.jar", "filename": "sources.jar", "primary": false },
            { "url": "https://x/iris.jar", "filename": "iris.jar", "primary": true,
              "hashes": { "sha1": "abc" }, "size": 7 }
        ] }]);

        let item = pick_mod_file(Path::new("mods"), &response).unwrap();

        assert_eq!(item.url, "https://x/iris.jar");
        assert_eq!(item.expected.sha1.as_deref(), Some("abc"));
        assert_eq!(item.expected.size, Some(7));
    }

    #[test]
    fn file_name_cannot_leave_the_mods_folder() {
        let response = json!([{ "files": [
            { "url": "https://x/a.jar", "filename": "../../evil.jar" }
        ] }]);
        assert!(pick_mod_file(Path::new("mods"), &response).is_none());
    }

    #[test]
    fn no_compatible_version_gives_nothing() {
        assert!(pick_mod_file(Path::new("mods"), &json!([])).is_none());
    }
}
