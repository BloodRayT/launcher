use crate::download::write_atomic;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "launcher_config.json";
const DEFAULT_BACKEND_URL: &str = "http://fenixcraft.ru/api/launcher";
/// Адрес из прежних версий: сайт пока работает только по HTTP.
const OLD_BACKEND_URL: &str = "https://fenixcraft.ru/api/launcher";
const DEFAULT_JVM_ARGS: &str = "-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M";
const MIN_RAM_MB: u32 = 1024;
const MAX_RAM_MB: u32 = 65536;
const MIN_WINDOW_SIZE: u32 = 320;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigItem {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub ip: String,
    pub port: u16,
    pub icon: String,
    pub max_default: u32,
}

fn server(id: &str, name: &str, desc: &str, ip: &str, icon: &str, max: u32) -> ServerConfigItem {
    ServerConfigItem {
        id: id.to_string(),
        name: name.to_string(),
        desc: desc.to_string(),
        ip: ip.to_string(),
        port: 25565,
        icon: icon.to_string(),
        max_default: max,
    }
}

pub fn default_servers() -> Vec<ServerConfigItem> {
    vec![
        server(
            "fenix-main",
            "FenixCraft",
            "Выживание • Vanilla/Mods",
            "play.fenixcraft.ru",
            "🦅",
            1000,
        ),
        server(
            "fenix-mods",
            "FenixTech",
            "NeoForge • 1.21.1",
            "tech.fenixcraft.ru",
            "⚙️",
            500,
        ),
        server(
            "fenix-rpg",
            "FenixRPG",
            "Fabric + Iris • 1.21.11",
            "rpg.fenixcraft.ru",
            "⚔️",
            500,
        ),
        server(
            "fenix-pvp",
            "FenixArena",
            "Анархия • 1.21.11",
            "pvp.fenixcraft.ru",
            "⭐",
            300,
        ),
    ]
}

fn default_account_type() -> String {
    "offline".to_string()
}
fn default_ram() -> u32 {
    6144
}
fn default_width() -> u32 {
    1280
}
fn default_height() -> u32 {
    720
}
fn default_jvm_args() -> String {
    DEFAULT_JVM_ARGS.to_string()
}
fn default_selected_version() -> String {
    "latest-vanilla".to_string()
}
fn default_selected_server() -> String {
    "play.fenixcraft.ru".to_string()
}
fn default_backend_url() -> String {
    DEFAULT_BACKEND_URL.to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    #[serde(default)]
    pub username: String,
    #[serde(default = "default_account_type")]
    pub account_type: String,
    #[serde(default = "default_ram")]
    pub ram: u32,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default)]
    pub java_path: String,
    #[serde(default = "default_jvm_args")]
    pub jvm_args: String,
    #[serde(default)]
    pub show_console: bool,
    #[serde(default)]
    pub debug_mode: bool,
    #[serde(default = "default_selected_version")]
    pub selected_version: String,
    #[serde(default = "default_selected_server")]
    pub selected_server: String,
    #[serde(default = "default_backend_url")]
    pub backend_url: String,
    #[serde(default = "default_servers")]
    pub custom_servers: Vec<ServerConfigItem>,
    #[serde(default)]
    pub avatar_data: String,
    #[serde(default)]
    pub ms_username: String,
    #[serde(default)]
    pub ms_uuid: String,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            username: String::new(),
            account_type: default_account_type(),
            ram: default_ram(),
            width: default_width(),
            height: default_height(),
            java_path: String::new(),
            jvm_args: default_jvm_args(),
            show_console: false,
            debug_mode: false,
            selected_version: default_selected_version(),
            selected_server: default_selected_server(),
            backend_url: default_backend_url(),
            custom_servers: default_servers(),
            avatar_data: String::new(),
            ms_username: String::new(),
            ms_uuid: String::new(),
        }
    }
}

impl LauncherConfig {
    /// Приводит значения к допустимым. Лицензия без сохранённого профиля считается пираткой.
    pub fn normalized(self) -> Self {
        let account_type = if self.account_type == "microsoft" && !self.ms_uuid.is_empty() {
            "microsoft"
        } else {
            "offline"
        };
        let backend_is_default =
            self.backend_url.trim().is_empty() || self.backend_url == OLD_BACKEND_URL;
        Self {
            account_type: account_type.to_string(),
            ram: self.ram.clamp(MIN_RAM_MB, MAX_RAM_MB),
            width: self.width.max(MIN_WINDOW_SIZE),
            height: self.height.max(MIN_WINDOW_SIZE),
            backend_url: if backend_is_default {
                default_backend_url()
            } else {
                self.backend_url
            },
            custom_servers: if self.custom_servers.is_empty() {
                default_servers()
            } else {
                self.custom_servers
            },
            ..self
        }
    }
}

pub fn game_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fenixcraft");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn official_minecraft_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".minecraft")
}

pub fn config_path(root: &Path) -> PathBuf {
    root.join(CONFIG_FILE)
}

/// Читает конфиг. Повреждённый файл не перезаписывается, а сохраняется рядом для восстановления.
pub fn load_config(path: &Path) -> LauncherConfig {
    let Ok(data) = std::fs::read_to_string(path) else {
        return LauncherConfig::default();
    };
    match serde_json::from_str::<LauncherConfig>(&data) {
        Ok(config) => {
            let config = config.normalized();
            // Прежние версии хранили здесь токены входа в открытом виде: их нужно убрать с диска.
            if data.contains("licenseToken") {
                let _ = save_config(path, config.clone());
            }
            config
        }
        Err(_) => {
            let _ = std::fs::rename(path, path.with_extension("json.corrupt"));
            LauncherConfig::default()
        }
    }
}

pub fn save_config(path: &Path, config: LauncherConfig) -> std::io::Result<LauncherConfig> {
    let config = config.normalized();
    let json = serde_json::to_string_pretty(&config)?;
    write_atomic(path, json.as_bytes())?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load_config(&config_path(dir.path())),
            LauncherConfig::default()
        );
    }

    #[test]
    fn saved_config_is_read_back_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = config_path(dir.path());
        let config = LauncherConfig {
            username: "Fenix".to_string(),
            avatar_data: "data:image/webp;base64,AAAA".to_string(),
            backend_url: "https://example.test/api".to_string(),
            ..LauncherConfig::default()
        };

        let saved = save_config(&path, config.clone()).unwrap();

        assert_eq!(saved, config);
        assert_eq!(load_config(&path), config);
    }

    #[test]
    fn corrupt_config_is_kept_for_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = config_path(dir.path());
        std::fs::write(&path, "{ not json").unwrap();

        assert_eq!(load_config(&path), LauncherConfig::default());

        let backup = path.with_extension("json.corrupt");
        assert_eq!(std::fs::read_to_string(backup).unwrap(), "{ not json");
    }

    #[test]
    fn old_login_data_is_removed_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = config_path(dir.path());
        std::fs::write(
            &path,
            r#"{"username":"Fenix","accountType":"mojang","licenseLogin":"me@example.test","licenseToken":"secret","licenseUuid":"id"}"#,
        )
        .unwrap();

        let config = load_config(&path);

        assert_eq!(config.username, "Fenix");
        assert_eq!(config.account_type, "offline");
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!on_disk.contains("secret"), "{on_disk}");
        assert!(!on_disk.contains("me@example.test"), "{on_disk}");
    }

    #[test]
    fn licensed_account_needs_a_saved_profile() {
        let without_profile = LauncherConfig {
            account_type: "microsoft".to_string(),
            ..LauncherConfig::default()
        }
        .normalized();
        let with_profile = LauncherConfig {
            account_type: "microsoft".to_string(),
            ms_username: "RealPlayer".to_string(),
            ms_uuid: "abc".to_string(),
            ..LauncherConfig::default()
        }
        .normalized();

        assert_eq!(without_profile.account_type, "offline");
        assert_eq!(with_profile.account_type, "microsoft");
    }

    #[test]
    fn old_https_backend_address_moves_to_http() {
        let old = LauncherConfig {
            backend_url: OLD_BACKEND_URL.to_string(),
            ..LauncherConfig::default()
        }
        .normalized();
        let custom = LauncherConfig {
            backend_url: "https://my.backend/api".to_string(),
            ..LauncherConfig::default()
        }
        .normalized();

        assert_eq!(old.backend_url, DEFAULT_BACKEND_URL);
        assert_eq!(custom.backend_url, "https://my.backend/api");
    }

    #[test]
    fn out_of_range_values_are_corrected() {
        let config = LauncherConfig {
            account_type: "microsoft".to_string(),
            ram: 1,
            width: 0,
            height: 0,
            backend_url: "  ".to_string(),
            custom_servers: Vec::new(),
            ..LauncherConfig::default()
        }
        .normalized();

        assert_eq!(config.account_type, "offline");
        assert_eq!(config.ram, MIN_RAM_MB);
        assert_eq!(config.width, MIN_WINDOW_SIZE);
        assert_eq!(config.backend_url, DEFAULT_BACKEND_URL);
        assert_eq!(config.custom_servers, default_servers());
    }
}
