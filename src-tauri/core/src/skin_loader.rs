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
}
