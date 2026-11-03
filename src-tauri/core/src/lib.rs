//! Логика лаунчера без зависимости от Tauri: скачивание, проверка файлов, подготовка и запуск игры.

pub mod assets;
pub mod auth;
pub mod backend_sync;
pub mod config;
pub mod download;
pub mod errors;
pub mod java;
pub mod launch;
pub mod libraries;
pub mod mods;
pub mod network;
pub mod reporter;
pub mod safety;
pub mod updater;
pub mod versions;

/// Адреса внешних сервисов. В тестах подменяются локальным сервером.
#[derive(Clone, Debug)]
pub struct Endpoints {
    pub version_manifest: String,
    pub assets_base: String,
    pub fabric_meta: String,
    pub modrinth: String,
    pub adoptium: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            version_manifest: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
                .to_string(),
            assets_base: "https://resources.download.minecraft.net".to_string(),
            fabric_meta: "https://meta.fabricmc.net/v2".to_string(),
            modrinth: "https://api.modrinth.com/v2".to_string(),
            adoptium: "https://api.adoptium.net/v3".to_string(),
        }
    }
}
