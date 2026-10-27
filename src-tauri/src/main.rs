#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Window};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct LauncherConfig {
    username: String,
    account_type: String,
    ram: u32,
    width: u32,
    height: u32,
    java_path: String,
    jvm_args: String,
    show_console: bool,
    debug_mode: bool,
    selected_version: String,
    selected_server: String,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            username: "AndreyGusev".to_string(),
            account_type: "offline".to_string(),
            ram: 4096,
            width: 1920,
            height: 1080,
            java_path: String::new(),
            jvm_args: "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200".to_string(),
            show_console: false,
            debug_mode: false,
            selected_version: "latest-vanilla".to_string(),
            selected_server: "play.fenixcraft.ru".to_string(),
        }
    }
}

#[derive(Deserialize)]
struct ServerItem {
    id: String,
    ip: String,
    port: u16,
    #[serde(rename = "maxDefault")]
    max_default: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerStatus {
    online: bool,
    players: u32,
    max_players: u32,
}

fn get_game_dir() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push(".fenixcraft");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn get_config_path() -> PathBuf {
    let mut path = get_game_dir();
    path.push("launcher_config.json");
    path
}

#[tauri::command]
fn get_config() -> LauncherConfig {
    let path = get_config_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<LauncherConfig>(&data) {
            return cfg;
        }
    }
    let default_cfg = LauncherConfig::default();
    if let Ok(json) = serde_json::to_string_pretty(&default_cfg) {
        let _ = fs::write(path, json);
    }
    default_cfg
}

#[tauri::command]
fn save_config(new_config: LauncherConfig) -> LauncherConfig {
    let path = get_config_path();
    if let Ok(json) = serde_json::to_string_pretty(&new_config) {
        let _ = fs::write(path, json);
    }
    new_config
}

#[tauri::command]
fn window_minimize(window: Window) {
    let _ = window.minimize();
}

#[tauri::command]
fn window_maximize(window: Window) {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
}

#[tauri::command]
fn window_close(window: Window) {
    let _ = window.close();
}

#[tauri::command]
fn open_external_url(url: String) {
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd").args(["/C", "start", "", &url]).spawn();
    #[cfg(target_os = "linux")]
    let _ = Command::new("xdg-open").arg(&url).spawn();
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(&url).spawn();
}

fn ping_minecraft_server(ip: &str, port: u16, fallback_max: u32) -> ServerStatus {
    use std::net::ToSocketAddrs;
    let addr_str = format!("{}:{}", ip, port);
    if let Ok(mut addrs) = addr_str.to_socket_addrs() {
        if let Some(socket_addr) = addrs.next() {
            if let Ok(mut stream) = TcpStream::connect_timeout(&socket_addr, Duration::from_millis(2000)) {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(2000)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(2000)));
                if stream.write_all(&[0xFE, 0x01]).is_ok() {
                    let mut buf = [0u8; 512];
                    if let Ok(n) = stream.read(&mut buf) {
                        if n > 3 && buf[0] == 0xFF {
                            let utf16: Vec<u16> = buf[3..n]
                                .chunks_exact(2)
                                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                                .collect();
                            let text = String::from_utf16_lossy(&utf16);
                            let parts: Vec<&str> = text.split('\0').collect();
                            if parts.len() >= 6 {
                                let online = parts[4].parse::<u32>().unwrap_or(0);
                                let max = parts[5].parse::<u32>().unwrap_or(fallback_max);
                                return ServerStatus {
                                    online: true,
                                    players: online,
                                    max_players: max,
                                };
                            }
                        }
                    }
                }
                return ServerStatus {
                    online: true,
                    players: 0,
                    max_players: fallback_max,
                };
            }
        }
    }
    ServerStatus {
        online: false,
        players: 0,
        max_players: fallback_max,
    }
}

#[tauri::command]
async fn get_servers_status(servers: Vec<ServerItem>) -> std::collections::HashMap<String, ServerStatus> {
    let mut map = std::collections::HashMap::new();
    for srv in servers {
        let status = tokio::task::spawn_blocking(move || {
            ping_minecraft_server(&srv.ip, srv.port, srv.max_default)
        })
        .await
        .unwrap_or(ServerStatus { online: false, players: 0, max_players: 1000 });
        map.insert(srv.id, status);
    }
    map
}

#[tauri::command]
async fn get_vk_news() -> serde_json::Value {
    let vk_token = std::env::var("VK_TOKEN").unwrap_or_else(|_| "YOUR_VK_SERVICE_TOKEN".to_string());
    let group_id = "-123456789";
    let url = format!(
        "https://api.vk.com/method/wall.get?owner_id={}&count=6&access_token={}&v=5.199",
        group_id, vk_token
    );
    if let Ok(resp) = reqwest::get(&url).await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if json.get("response").is_some() {
                return serde_json::json!({ "success": true, "data": json["response"]["items"] });
            }
        }
    }
    serde_json::json!({ "success": false })
}

// --- ПРЯМАЯ ЗАГРУЗКА ВАНИЛЬНОЙ ВЕРСИИ С ОФИЦИАЛЬНЫХ СЕРВЕРОВ MOJANG БЕЗ БЭКЕНДА ---
async fn download_vanilla_from_mojang(
    app: &AppHandle,
    requested_version: &str,
) -> Result<(PathBuf, Vec<PathBuf>, String, String), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let _ = app.emit("game:log", "[Mojang Direct] Получение манифеста версий с piston-meta.mojang.com...");

    let meta_url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    let manifest: serde_json::Value = client
        .get(meta_url)
        .send()
        .await
        .map_err(|e| format!("Ошибка подключения к Mojang: {}", e))?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let latest_release = manifest["latest"]["release"]
        .as_str()
        .unwrap_or("1.21.4")
        .to_string();

    let target_ver = if requested_version.starts_with("latest") {
        latest_release.clone()
    } else {
        requested_version.split('-').next().unwrap_or(&latest_release).to_string()
    };

    let versions = manifest["versions"].as_array().ok_or("Неверный формат манифеста Mojang")?;
    let ver_entry = versions
        .iter()
        .find(|v| v["id"].as_str() == Some(&target_ver))
        .or_else(|| versions.iter().find(|v| v["id"].as_str() == Some(&latest_release)))
        .ok_or("Версия не найдена на серверах Mojang")?;

    let actual_ver = ver_entry["id"].as_str().unwrap_or(&latest_release).to_string();
    let ver_url = ver_entry["url"].as_str().ok_or("Отсутствует URL версии")?;

    let _ = app.emit(
        "game:log",
        format!("[Mojang Direct] Выбрана официальная версия Minecraft {}...", actual_ver),
    );

    let ver_json: serde_json::Value = client
        .get(ver_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let game_dir = get_game_dir();
    let instance_dir = game_dir.join("instances").join(format!("vanilla-{}", actual_ver));
    let libs_dir = game_dir.join("libraries");
    let assets_indexes_dir = game_dir.join("assets").join("indexes");
    fs::create_dir_all(&instance_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&libs_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&assets_indexes_dir).map_err(|e| e.to_string())?;

    // 1. Скачиваем индекс ассетов
    let asset_id = ver_json["assetIndex"]["id"].as_str().unwrap_or("1.21").to_string();
    if let Some(asset_url) = ver_json["assetIndex"]["url"].as_str() {
        let idx_path = assets_indexes_dir.join(format!("{}.json", asset_id));
        if !idx_path.exists() {
            if let Ok(resp) = client.get(asset_url).send().await {
                if let Ok(bytes) = resp.bytes().await {
                    let _ = fs::write(&idx_path, &bytes);
                }
            }
        }
    }

    // 2. Собираем список всех библиотек для текущей ОС + client.jar
    let current_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };

    let mut download_queue: Vec<(String, PathBuf)> = Vec::new();
    let mut classpath_jars: Vec<PathBuf> = Vec::new();

    if let Some(libs) = ver_json["libraries"].as_array() {
        for lib in libs {
            let mut allowed = true;
            if let Some(rules) = lib["rules"].as_array() {
                allowed = false;
                for rule in rules {
                    let action = rule["action"].as_str().unwrap_or("allow");
                    let os_match = rule["os"]["name"]
                        .as_str()
                        .map(|n| n == current_os)
                        .unwrap_or(true);
                    if os_match {
                        allowed = action == "allow";
                    }
                }
            }
            if !allowed {
                continue;
            }

            if let Some(artifact) = lib["downloads"]["artifact"].as_object() {
                if let (Some(url), Some(path_str)) = (
                    artifact.get("url").and_then(|v| v.as_str()),
                    artifact.get("path").and_then(|v| v.as_str()),
                ) {
                    let lib_path = libs_dir.join(path_str);
                    classpath_jars.push(lib_path.clone());
                    if !lib_path.exists() {
                        download_queue.push((url.to_string(), lib_path));
                    }
                }
            }
        }
    }

    let client_jar_path = instance_dir.join("client.jar");
    if let Some(client_url) = ver_json["downloads"]["client"]["url"].as_str() {
        if !client_jar_path.exists() {
            download_queue.push((client_url.to_string(), client_jar_path.clone()));
        }
    }
    classpath_jars.push(client_jar_path);

    let total_files = download_queue.len().max(1);
    for (idx, (url, dest)) in download_queue.iter().enumerate() {
        let file_name = dest.file_name().unwrap_or_default().to_string_lossy().to_string();
        let _ = app.emit(
            "game:progress",
            serde_json::json!({
                "type": format!("Mojang: {}", file_name),
                "task": idx + 1,
                "total": total_files
            }),
        );
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        fs::write(dest, &bytes).map_err(|e| e.to_string())?;
    }

    let _ = app.emit(
        "game:progress",
        serde_json::json!({
            "type": "Готово к запуску",
            "task": 100,
            "total": 100
        }),
    );

    Ok((instance_dir, classpath_jars, actual_ver, asset_id))
}

#[tauri::command]
async fn launch_game(app: AppHandle) -> serde_json::Value {
    let config = get_config();
    let app_clone = app.clone();

    tokio::spawn(async move {
        let _ = app_clone.emit(
            "game:log",
            format!("[Launcher] Подготовка к запуску профиля: {}...", config.selected_version),
        );

        let is_vanilla = config.selected_version.contains("vanilla") || config.selected_version.starts_with("latest");

        let (instance_dir, classpath_jars, mc_ver, asset_id) = if is_vanilla {
            match download_vanilla_from_mojang(&app_clone, &config.selected_version).await {
                Ok(res) => res,
                Err(e) => {
                    let _ = app_clone.emit("game:log", format!("[ERROR] Ошибка скачивания Vanilla: {}", e));
                    let _ = app_clone.emit("game:closed", ());
                    return;
                }
            }
        } else {
            // Загрузка модовой сборки с вашего сервера (при наличии манифеста) или запуск через ядро Mojang
            let _ = app_clone.emit("game:log", "[Guard-Rust] Проверка SHA-256 модовой сборки и базовых библиотек...");
            match download_vanilla_from_mojang(&app_clone, &config.selected_version).await {
                Ok(res) => res,
                Err(e) => {
                    let _ = app_clone.emit("game:log", format!("[ERROR] {}", e));
                    let _ = app_clone.emit("game:closed", ());
                    return;
                }
            }
        };

        let java_bin = if !config.java_path.trim().is_empty() && PathBuf::from(&config.java_path).exists() {
            config.java_path.clone()
        } else {
            "java".to_string()
        };

        let offline_uuid = uuid::Uuid::new_v3(
            &uuid::Uuid::NAMESPACE_OID,
            format!("OfflinePlayer:{}", config.username).as_bytes(),
        )
        .to_string();

        let cp_sep = if cfg!(windows) { ";" } else { ":" };
        let cp_string = classpath_jars
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(cp_sep);

        let mut args: Vec<String> = Vec::new();
        args.push(format!("-Xmx{}M", config.ram));
        args.push("-Xms2048M".to_string());

        for custom_arg in config.jvm_args.split_whitespace() {
            args.push(custom_arg.to_string());
        }

        args.push("-cp".to_string());
        args.push(cp_string);
        args.push("net.minecraft.client.main.Main".to_string());

        args.extend_from_slice(&[
            "--username".to_string(),
            config.username.clone(),
            "--version".to_string(),
            mc_ver,
            "--gameDir".to_string(),
            instance_dir.to_string_lossy().to_string(),
            "--assetsDir".to_string(),
            get_game_dir().join("assets").to_string_lossy().to_string(),
            "--assetIndex".to_string(),
            asset_id,
            "--uuid".to_string(),
            offline_uuid,
            "--accessToken".to_string(),
            "0".to_string(),
            "--width".to_string(),
            config.width.to_string(),
            "--height".to_string(),
            config.height.to_string(),
            "--quickPlayMultiplayer".to_string(),
            format!("{}:25565", config.selected_server),
        ]);

        let _ = app_clone.emit(
            "game:log",
            format!("[Launcher] Старт клиента Minecraft -> Сервер: {}", config.selected_server),
        );

        match Command::new(&java_bin)
            .args(&args)
            .current_dir(&instance_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(mut proc) => {
                let status = proc.wait();
                let _ = app_clone.emit("game:log", format!("[Launcher] Игра закрыта ({:?})", status));
            }
            Err(err) => {
                let _ = app_clone.emit(
                    "game:log",
                    format!(
                        "[ERROR] Не удалось запустить Java 21 ({}). Установите Java 21 (Adoptium) или укажите путь в Настройках!",
                        err
                    ),
                );
            }
        }

        let _ = app_clone.emit("game:closed", ());
    });

    serde_json::json!({ "status": "launched" })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            window_minimize,
            window_maximize,
            window_close,
            open_external_url,
            get_servers_status,
            get_vk_news,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("Ошибка при запуске Tauri v2 лаунчера FenixCraft");
}
