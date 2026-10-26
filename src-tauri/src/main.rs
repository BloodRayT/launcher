#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::{Emitter, Window};

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
            selected_version: "1.21.8-neoforge".to_string(),
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

#[derive(Deserialize)]
struct ManifestFile {
    path: String,
    sha256: String,
    url: String,
}

#[derive(Deserialize)]
struct ServerManifest {
    files: Vec<ManifestFile>,
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

fn compute_sha256(path: &PathBuf) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

async fn verify_and_sync_files(window: &Window, profile: &str) -> Result<PathBuf, String> {
    let mut instance_dir = get_game_dir();
    instance_dir.push("instances");
    instance_dir.push(profile);

    let mut mods_dir = instance_dir.clone();
    mods_dir.push("mods");
    fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;

    let _ = window.emit(
        "game:log",
        format!("[Guard-Rust] Проверка SHA-256 для профиля {}...", profile),
    );

    let manifest_url = format!("https://fenixcraft.ru/api/launcher/manifest_{}.json", profile);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    if let Ok(resp) = client.get(&manifest_url).send().await {
        if resp.status().is_success() {
            if let Ok(manifest) = resp.json::<ServerManifest>().await {
                let mut valid_hashes = std::collections::HashSet::new();
                let total = manifest.files.len().max(1);

                for (idx, item) in manifest.files.iter().enumerate() {
                    valid_hashes.insert(item.sha256.to_lowercase());
                    let target_path = instance_dir.join(&item.path);

                    let need_download = if target_path.exists() {
                        match compute_sha256(&target_path) {
                            Ok(local_hash) => local_hash.to_lowercase() != item.sha256.to_lowercase(),
                            Err(_) => true,
                        }
                    } else {
                        true
                    };

                    if need_download {
                        let _ = window.emit(
                            "game:log",
                            format!("[Guard-Rust] Скачивание файла: {}", item.path),
                        );
                        if let Some(parent) = target_path.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        if let Ok(dl) = client.get(&item.url).send().await {
                            if let Ok(bytes) = dl.bytes().await {
                                let _ = fs::write(&target_path, &bytes);
                            }
                        }
                    }

                    let _ = window.emit(
                        "game:progress",
                        serde_json::json!({
                            "type": "SHA-256 Sync",
                            "task": idx + 1,
                            "total": total
                        }),
                    );
                }

                if let Ok(entries) = fs::read_dir(&mods_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            if let Ok(h) = compute_sha256(&p) {
                                if !valid_hashes.is_empty() && !valid_hashes.contains(&h.to_lowercase()) {
                                    let _ = window.emit(
                                        "game:log",
                                        format!("[Guard-Rust] Удален сторонний мод: {:?}", p.file_name().unwrap_or_default()),
                                    );
                                    let _ = fs::remove_file(p);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let _ = window.emit("game:log", "[Guard-Rust] Проверка контрольных сумм завершена.");
    Ok(instance_dir)
}

#[tauri::command]
async fn launch_game(window: Window) -> serde_json::Value {
    let config = get_config();

    let instance_dir = match verify_and_sync_files(&window, &config.selected_version).await {
        Ok(dir) => dir,
        Err(e) => {
            let _ = window.emit("game:log", format!("[ERROR] Ошибка проверки: {}", e));
            return serde_json::json!({ "status": "error", "message": e });
        }
    };

    let java_bin = if !config.java_path.trim().is_empty() && PathBuf::from(&config.java_path).exists() {
        config.java_path.clone()
    } else {
        let mut bundled_java = get_game_dir();
        bundled_java.push("runtime");
        bundled_java.push("java-21");
        bundled_java.push("bin");
        bundled_java.push(if cfg!(windows) { "javaw.exe" } else { "java" });

        if bundled_java.exists() {
            bundled_java.to_string_lossy().to_string()
        } else {
            "java".to_string()
        }
    };

    let offline_uuid = uuid::Uuid::new_v3(
        &uuid::Uuid::NAMESPACE_OID,
        format!("OfflinePlayer:{}", config.username).as_bytes(),
    )
    .to_string();

    let parts: Vec<&str> = config.selected_version.split('-').collect();
    let mc_ver = parts.first().copied().unwrap_or("1.21.8");

    let mut args: Vec<String> = Vec::new();
    args.push(format!("-Xmx{}M", config.ram));
    args.push("-Xms2048M".to_string());

    for custom_arg in config.jvm_args.split_whitespace() {
        args.push(custom_arg.to_string());
    }

    if config.debug_mode {
        args.push("-Dfabric.log.level=debug".to_string());
        args.push("-Dfml.debugExit=true".to_string());
    }

    args.push("-cp".to_string());
    args.push(format!("{}/*", instance_dir.join("libraries").display()));
    args.push("net.minecraft.client.main.Main".to_string());

    args.extend_from_slice(&[
        "--username".to_string(),
        config.username.clone(),
        "--version".to_string(),
        mc_ver.to_string(),
        "--gameDir".to_string(),
        instance_dir.to_string_lossy().to_string(),
        "--assetsDir".to_string(),
        get_game_dir().join("assets").to_string_lossy().to_string(),
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

    let _ = window.emit(
        "game:log",
        format!(
            "[Launcher-Rust] Старт JVM ({}) | Профиль: {} | Сервер: {}",
            java_bin, config.selected_version, config.selected_server
        ),
    );

    let win_clone = window.clone();
    tokio::spawn(async move {
        let child = Command::new(&java_bin)
            .args(&args)
            .current_dir(&instance_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        match child {
            Ok(mut proc) => {
                let status = proc.wait();
                let _ = win_clone.emit("game:log", format!("[Launcher-Rust] Завершено: {:?}", status));
            }
            Err(err) => {
                let _ = win_clone.emit(
                    "game:log",
                    format!("[Launcher-Rust] Ожидание файлов клиента с сервера: {}", err),
                );
            }
        }
        let _ = win_clone.emit("game:closed", ());
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
            get_servers_status,
            get_vk_news,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("Ошибка при запуске Tauri v2 лаунчера FenixCraft");
}