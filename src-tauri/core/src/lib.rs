//! Логика лаунчера без зависимости от Tauri: скачивание, проверка файлов, подготовка и запуск игры.

pub mod assets;
pub mod backend_sync;
pub mod config;
pub mod download;
pub mod ely;
pub mod errors;
pub mod instances;
pub mod java;
pub mod launch;
pub mod libraries;
pub mod microsoft;
pub mod mods;
pub mod neoforge;
pub mod network;
pub mod news;
pub mod ping;
pub mod reporter;
pub mod safety;
pub mod skin_loader;
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
    pub neoforge_mavens: Vec<String>,
    pub forge_mavens: Vec<String>,
    pub ely_authserver: String,
    /// Последний выпуск authlib-injector на GitHub.
    pub authlib_injector_release: String,
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
            forge_mavens: vec!["https://maven.minecraftforge.net".to_string()],
            ely_authserver: "https://authserver.ely.by".to_string(),
            authlib_injector_release:
                "https://api.github.com/repos/yushijinhun/authlib-injector/releases/latest"
                    .to_string(),
            neoforge_mavens: vec![
                "https://maven.neoforged.net/releases".to_string(),
                "https://maven.creeperhost.net".to_string(),
            ],
        }
    }
}
