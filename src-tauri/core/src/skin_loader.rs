//! Скины и плащи из лаунчера в игре: мод CustomSkinLoader берёт их с бекенда по нику игрока.
use crate::download::write_atomic;
use crate::errors::LauncherError;
use crate::launch::LaunchContext;
use crate::mods::{has_mod, install_mod};
use serde_json::json;
use std::path::{Path, PathBuf};

const MOD_SLUG: &str = "customskinloader";
/// Версия формата конфига; мод обновит его сам, сохранив список источников.
const CONFIG_VERSION: &str = "15.1";

/// Если будущая версия игры сломает один из патчей мода, игра запустится без скинов, а не вылетит.
pub const IGNORE_PATCH_FAILURE_FLAG: &str = "-Dcustomskinloader.ignorePatchFailure=true";
const VERSION_FILE: &str = "version.json";

/// Маленький jar с одним `version.json` из ванильного клиента.
///
/// CustomSkinLoader узнаёт версию игры по `version.json` на системном classpath. Ванильный
/// клиент для NeoForge и Forge на classpath не кладётся (иначе два модуля minecraft), и без
/// этого файла мод считает версию нулевой и ломает запуск патчами для старых версий.
pub fn version_marker_jar(client_jar: &Path) -> Result<PathBuf, LauncherError> {
    let stem = client_jar
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| LauncherError::BadData("у клиента игры нет имени".to_string()))?;
    let marker = client_jar.with_file_name(format!("{stem}-version.jar"));
    let fresh = match (std::fs::metadata(&marker), std::fs::metadata(client_jar)) {
        (Ok(marker_meta), Ok(client_meta)) => {
            matches!((marker_meta.modified(), client_meta.modified()), (Ok(m), Ok(c)) if m >= c)
        }
        _ => false,
    };
    if fresh {
        return Ok(marker);
    }
    let bad = |error: zip::result::ZipError| LauncherError::BadData(error.to_string());
    let mut client = zip::ZipArchive::new(std::fs::File::open(client_jar)?).map_err(bad)?;
    let mut version = Vec::new();
    std::io::Read::read_to_end(
        &mut client.by_name(VERSION_FILE).map_err(bad)?,
        &mut version,
    )?;

    let mut jar = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut jar);
        writer
            .start_file(VERSION_FILE, zip::write::SimpleFileOptions::default())
            .map_err(bad)?;
        std::io::Write::write_all(&mut writer, &version)?;
        writer.finish().map_err(bad)?;
    }
    write_atomic(&marker, jar.get_ref())?;
    Ok(marker)
}

/// Где мод ищет свой конфиг внутри папки сборки.
pub fn config_path(game_dir: &Path) -> PathBuf {
    game_dir
        .join("CustomSkinLoader")
        .join("CustomSkinLoader.json")
}

/// Сначала скин, выбранный в лаунчере, затем официальный скин Mojang, затем локальные файлы.
pub fn loader_config(backend_url: &str) -> String {
    let base = backend_url.trim_end_matches('/');
    let config = json!({
        "version": CONFIG_VERSION,
        "loadlist": [
            {
                "name": "FenixCraft",
                "type": "Legacy",
                "checkPNG": false,
                "skin": format!("{base}/skins/player/{{USERNAME}}.png"),
                "model": "auto",
                "cape": format!("{base}/skins/cape/{{USERNAME}}.png"),
                "elytra": format!("{base}/skins/cape/{{USERNAME}}.png"),
            },
            // Скины игроков Ely.by, которые не выбрали скин в лаунчере.
            { "name": "ElyBy", "type": "ElyByAPI", "root": "http://skinsystem.ely.by/textures/" },
            { "name": "Mojang", "type": "MojangAPI" },
            {
                "name": "LocalSkin",
                "type": "Legacy",
                "checkPNG": false,
                "skin": "LocalSkin/skins/{USERNAME}.png",
                "model": "auto",
                "cape": "LocalSkin/capes/{USERNAME}.png",
                "elytra": "LocalSkin/elytras/{USERNAME}.png",
            },
        ],
        "enableDynamicSkull": true,
        "enableTransparentSkin": true,
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

/// Случайный ключ установки для сервера скинов: 32 шестнадцатеричных символа.
pub fn new_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Конфиг пишется при каждом запуске: адрес бекенда мог смениться.
pub fn write_loader_config(game_dir: &Path, backend_url: &str) -> Result<(), LauncherError> {
    write_atomic(
        &config_path(game_dir),
        loader_config(backend_url).as_bytes(),
    )?;
    Ok(())
}

/// Ставит мод в сборку, которую не раздаёт сервер сборок. Без мода игра просто запустится со скинами Mojang.
pub async fn ensure_skin_mod(ctx: &LaunchContext, game_dir: &Path, minecraft: &str, loader: &str) {
    let mods_dir = game_dir.join("mods");
    if has_mod(&mods_dir, MOD_SLUG) {
        return;
    }
    ctx.reporter.log(format!(
        "[Скины] Устанавливаем CustomSkinLoader для {minecraft} ({loader})..."
    ));
    if let Err(err) = std::fs::create_dir_all(&mods_dir) {
        ctx.reporter
            .log(format!("[Скины] Папка модов недоступна: {err}"));
        return;
    }
    if let Err(err) = install_mod(ctx, &mods_dir, MOD_SLUG, minecraft, loader).await {
        ctx.reporter.log(format!(
            "[Скины] CustomSkinLoader не установлен ({err}), в игре будут обычные скины."
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn launcher_skins_come_first_then_mojang() {
        let config: Value =
            serde_json::from_str(&loader_config("http://fenixcraft.ru/api/launcher/")).unwrap();
        let list = config["loadlist"].as_array().unwrap();

        assert_eq!(list[0]["type"], "Legacy");
        assert_eq!(
            list[0]["skin"],
            "http://fenixcraft.ru/api/launcher/skins/player/{USERNAME}.png"
        );
        assert_eq!(
            list[0]["cape"],
            "http://fenixcraft.ru/api/launcher/skins/cape/{USERNAME}.png"
        );
        assert_eq!(list[1]["type"], "ElyByAPI");
        assert_eq!(list[2]["type"], "MojangAPI");
    }

    #[test]
    fn config_lands_where_the_mod_reads_it() {
        let dir = tempfile::tempdir().unwrap();
        write_loader_config(dir.path(), "http://backend.test").unwrap();

        let written = std::fs::read_to_string(
            dir.path()
                .join("CustomSkinLoader")
                .join("CustomSkinLoader.json"),
        )
        .unwrap();
        assert!(written.contains("http://backend.test/skins/player/{USERNAME}.png"));
    }

    fn client_jar(dir: &Path) -> PathBuf {
        let path = dir.join("26.2.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("net/minecraft/Main.class", options)
            .unwrap();
        std::io::Write::write_all(&mut writer, b"class").unwrap();
        writer.start_file(VERSION_FILE, options).unwrap();
        std::io::Write::write_all(&mut writer, br#"{"id":"26.2","protocol_version":775}"#).unwrap();
        writer.finish().unwrap();
        path
    }

    #[test]
    fn version_marker_holds_only_the_game_version_file() {
        let dir = tempfile::tempdir().unwrap();
        let client = client_jar(dir.path());

        let marker = version_marker_jar(&client).unwrap();

        assert_eq!(marker, dir.path().join("26.2-version.jar"));
        let mut jar = zip::ZipArchive::new(std::fs::File::open(&marker).unwrap()).unwrap();
        assert_eq!(jar.len(), 1);
        let mut text = String::new();
        std::io::Read::read_to_string(&mut jar.by_name(VERSION_FILE).unwrap(), &mut text).unwrap();
        assert!(text.contains("\"id\":\"26.2\""));
        assert_eq!(version_marker_jar(&client).unwrap(), marker);
    }

    #[test]
    fn client_without_version_file_gives_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let client = dir.path().join("old.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&client).unwrap());
        writer
            .start_file("a.class", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap();

        assert!(version_marker_jar(&client).is_err());
        assert!(!dir.path().join("old-version.jar").exists());
    }
}
