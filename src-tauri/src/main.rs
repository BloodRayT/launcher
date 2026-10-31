#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Window};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ServerConfigItem {
    id: String,
    name: String,
    desc: String,
    ip: String,
    port: u16,
    icon: String,
    max_default: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct LauncherConfig {
    #[serde(default)]
    username: String,
    #[serde(default = "default_account_type")]
    account_type: String,
    #[serde(default = "default_ram")]
    ram: u32,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default)]
    java_path: String,
    #[serde(default)]
    jvm_args: String,
    #[serde(default)]
    show_console: bool,
    #[serde(default)]
    debug_mode: bool,
    #[serde(default = "default_selected_version")]
    selected_version: String,
    #[serde(default = "default_selected_server")]
    selected_server: String,
    #[serde(default)]
    vk_token: String,
    #[serde(default = "default_backend_url")]
    backend_url: String,
    #[serde(default = "default_servers_list")]
    custom_servers: Vec<ServerConfigItem>,
    #[serde(default)]
    avatar_data: String,
    #[serde(default)]
    license_login: String,
    #[serde(default)]
    license_token: String,
    #[serde(default)]
    license_uuid: String,
}

fn default_account_type() -> String { "offline".to_string() }
fn default_ram() -> u32 { 6144 }
fn default_width() -> u32 { 1280 }
fn default_height() -> u32 { 720 }
fn default_selected_version() -> String { "latest-vanilla".to_string() }
fn default_selected_server() -> String { "play.fenixcraft.ru".to_string() }

fn default_backend_url() -> String {
    "https://fenixcraft.ru/api/launcher".to_string()
}

fn default_servers_list() -> Vec<ServerConfigItem> {
    vec![
        ServerConfigItem {
            id: "fenix-main".to_string(),
            name: "FenixCraft".to_string(),
            desc: "Выживание • Vanilla/Mods".to_string(),
            ip: "play.fenixcraft.ru".to_string(),
            port: 25565,
            icon: "🦅".to_string(),
            max_default: 1000,
        },
        ServerConfigItem {
            id: "fenix-mods".to_string(),
            name: "FenixTech".to_string(),
            desc: "NeoForge • 1.21.1".to_string(),
            ip: "tech.fenixcraft.ru".to_string(),
            port: 25565,
            icon: "⚙️".to_string(),
            max_default: 500,
        },
        ServerConfigItem {
            id: "fenix-rpg".to_string(),
            name: "FenixRPG".to_string(),
            desc: "Fabric + Iris • 1.21.11".to_string(),
            ip: "rpg.fenixcraft.ru".to_string(),
            port: 25565,
            icon: "⚔️".to_string(),
            max_default: 500,
        },
        ServerConfigItem {
            id: "fenix-pvp".to_string(),
            name: "FenixArena".to_string(),
            desc: "Анархия • 1.21.11".to_string(),
            ip: "pvp.fenixcraft.ru".to_string(),
            port: 25565,
            icon: "⭐".to_string(),
            max_default: 300,
        },
    ]
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            username: "".to_string(),
            account_type: "offline".to_string(),
            ram: 6144,
            width: 1280,
            height: 720,
            java_path: String::new(),
            jvm_args: "-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M".to_string(),
            show_console: false,
            debug_mode: false,
            selected_version: "latest-vanilla".to_string(),
            selected_server: "play.fenixcraft.ru".to_string(),
            vk_token: String::new(),
            backend_url: default_backend_url(),
            custom_servers: default_servers_list(),
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
struct BackendManifestFile {
    path: String,
    sha256: String,
    #[allow(dead_code)]
    size: u64,
    url: String,
}

#[derive(Deserialize)]
struct BackendManifest {
    profile: String,
    #[serde(default)]
    enforce_mods: bool,
    files: Vec<BackendManifestFile>,
}

fn extract_zip_archive(archive_path: &PathBuf, target_dir: &PathBuf, skip_meta_inf: bool) -> Result<(), String> {
    let file = fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let outpath = match entry.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        let out_str = outpath.to_string_lossy();
        if skip_meta_inf && (out_str.starts_with("META-INF") || out_str.ends_with(".git") || out_str.ends_with(".sha1")) {
            continue;
        }

        let full_out = target_dir.join(&outpath);
        if entry.name().ends_with('/') {
            let _ = fs::create_dir_all(&full_out);
        } else {
            if let Some(p) = full_out.parent() {
                let _ = fs::create_dir_all(p);
            }
            if let Ok(mut outfile) = fs::File::create(&full_out) {
                let _ = std::io::copy(&mut entry, &mut outfile);
            }
        }
    }
    Ok(())
}

fn get_game_dir() -> PathBuf {
    let mut dir = dirs::config_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    dir.push(".fenixcraft");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn get_official_mc_dir() -> PathBuf {
    let mut dir = dirs::config_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    dir.push(".minecraft");
    dir
}

fn get_config_path() -> PathBuf {
    get_game_dir().join("launcher_config.json")
}

#[tauri::command]
fn get_config() -> LauncherConfig {
    let path = get_config_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(mut cfg) = serde_json::from_str::<LauncherConfig>(&data) {
            if cfg.custom_servers.is_empty() {
                cfg.custom_servers = default_servers_list();
            }
            if cfg.backend_url.is_empty() {
                cfg.backend_url = default_backend_url();
            }
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
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(0x08000000)
            .spawn();
    }
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

fn strip_html_tags(input: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let replaced = input.replace("<br>", "\n").replace("<br/>", "\n");
    for c in replaced.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(c);
        }
    }
    out.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&#33;", "!")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

#[tauri::command]
async fn check_twitch_live() -> serde_json::Value {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_default();

    let gql_body = serde_json::json!({
        "query": "query { user(login: \"bloodrayt\") { stream { id type } } }"
    });

    if let Ok(resp) = client
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", "kimne78kx3ncx6brgo4mv6wki5h1ko")
        .json(&gql_body)
        .send()
        .await
    {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            let is_live = !json["data"]["user"]["stream"].is_null()
                && json["data"]["user"]["stream"]["type"].as_str() == Some("live");
            return serde_json::json!({ "live": is_live });
        }
    }

    serde_json::json!({ "live": false })
}

#[tauri::command]
async fn get_vk_news() -> serde_json::Value {
    let cfg = get_config();
    let vk_token = if !cfg.vk_token.trim().is_empty() {
        cfg.vk_token.trim().to_string()
    } else {
        std::env::var("VK_TOKEN").unwrap_or_default()
    };

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    if !vk_token.is_empty() && vk_token != "YOUR_VK_SERVICE_TOKEN" {
        let api_url = format!(
            "https://api.vk.ru/method/wall.get?domain=fenixcraft&count=6&access_token={}&v=5.199",
            vk_token
        );
        if let Ok(resp) = client.get(&api_url).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(items) = json["response"]["items"].as_array() {
                    if !items.is_empty() {
                        return serde_json::json!({ "success": true, "source": "api", "data": items });
                    }
                }
            }
        }
    }

    for url in ["https://m.vk.ru/fenixcraft", "https://vk.ru/fenixcraft"] {
        if let Ok(resp) = client.get(url).header("Accept-Language", "ru-RU,ru;q=0.9").send().await {
            if let Ok(html) = resp.text().await {
                let mut parsed_posts = Vec::new();
                let blocks: Vec<&str> = if html.contains("wall_post_text") {
                    html.split("wall_post_text").skip(1).collect()
                } else {
                    html.split("pi_text").skip(1).collect()
                };

                for (idx, block) in blocks.iter().take(6).enumerate() {
                    if let Some(start_gt) = block.find('>') {
                        let rest = &block[start_gt + 1..];
                        if let Some(end_div) = rest.find("</div>") {
                            let clean_text = strip_html_tags(&rest[..end_div]);
                            if clean_text.chars().count() < 5 {
                                continue;
                            }

                            let mut img_url = "https://images.unsplash.com/photo-1607513746994-51f730a44832?w=600&q=80".to_string();
                            let search_area = &rest[..rest.len().min(4500)];
                            for prefix in ["https://sun", "https://pp.userapi.com"] {
                                if let Some(pos) = search_area.find(prefix) {
                                    let sub = &search_area[pos..];
                                    let end_pos = sub
                                        .find(|c: char| c == '"' || c == '\'' || c == ')' || c == ' ' || c == '&')
                                        .unwrap_or(sub.len());
                                    let candidate = &sub[..end_pos];
                                    if candidate.len() > 20 {
                                        img_url = candidate.to_string();
                                        break;
                                    }
                                }
                            }

                            let title_line = clean_text
                                .lines()
                                .next()
                                .unwrap_or("Новость FenixCraft")
                                .chars()
                                .take(55)
                                .collect::<String>();

                            parsed_posts.push(serde_json::json!({
                                "id": idx + 1,
                                "tag": "VK.RU/FENIXCRAFT",
                                "title": title_line,
                                "text": clean_text,
                                "date": "Стена сообщества ВК",
                                "image": img_url,
                                "url": "https://vk.ru/fenixcraft"
                            }));
                        }
                    }
                }

                if !parsed_posts.is_empty() {
                    return serde_json::json!({
                        "success": true,
                        "source": "public_wall",
                        "posts": parsed_posts
                    });
                }
            }
        }
    }

    serde_json::json!({ "success": false })
}

#[tauri::command]
async fn verify_assets_command(app: AppHandle) -> serde_json::Value {
    let root_dir = get_game_dir();
    let indexes_dir = root_dir.join("assets").join("indexes");
    let objects_dir = root_dir.join("assets").join("objects");

    let _ = app.emit("game:log", "====================================================");
    let _ = app.emit("game:log", "[Assets Audit] Старт 100% проверки игровых ассетов...");

    if !indexes_dir.exists() {
        let _ = app.emit("game:log", "[Assets Audit] Папка assets/indexes еще пуста. Нажмите «Играть» для первичной загрузки.");
        return serde_json::json!({ "ok": false, "total": 0, "valid": 0, "missing": 0 });
    }

    let mut total_checked = 0usize;
    let mut valid_count = 0usize;
    let mut missing_count = 0usize;

    if let Ok(entries) = fs::read_dir(&indexes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let idx_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(objects) = json["objects"].as_object() {
                            let mut seen = HashSet::new();
                            for (_k, obj) in objects {
                                if let Some(hash) = obj["hash"].as_str() {
                                    let expected_size = obj["size"].as_u64().unwrap_or(0);
                                    if hash.len() >= 2 && seen.insert(hash.to_string()) {
                                        total_checked += 1;
                                        let file_path = objects_dir.join(&hash[0..2]).join(hash);
                                        if let Ok(meta) = fs::metadata(&file_path) {
                                            if meta.len() > 0 && (expected_size == 0 || meta.len() == expected_size) {
                                                valid_count += 1;
                                            } else {
                                                missing_count += 1;
                                            }
                                        } else {
                                            missing_count += 1;
                                        }
                                    }
                                }
                            }
                            let _ = app.emit(
                                "game:log",
                                format!(
                                    "[Assets Audit] Индекс {}: проверено уникальных объектов = {}, валидных = {}, битых/нет = {}",
                                    idx_name,
                                    seen.len(),
                                    valid_count,
                                    missing_count
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    let percent = if total_checked > 0 {
        (valid_count * 100) / total_checked
    } else {
        0
    };

    if total_checked > 0 && missing_count == 0 {
        let _ = app.emit(
            "game:log",
            format!("[Assets Audit] ✅ ИТОГ: 100% АССЕТОВ НА МЕСТЕ ({} из {} файлов проверены по размеру)!", valid_count, total_checked),
        );
    } else {
        let _ = app.emit(
            "game:log",
            format!(
                "[Assets Audit] ⚠️ ИТОГ: Готовность {}% ({} из {}). Недостает {} файлов — они автоматически докачаются при нажатии «Играть».",
                percent, valid_count, total_checked, missing_count
            ),
        );
    }
    let _ = app.emit("game:log", "====================================================");

    serde_json::json!({
        "ok": total_checked > 0 && missing_count == 0,
        "total": total_checked,
        "valid": valid_count,
        "missing": missing_count,
        "percent": percent
    })
}


#[tauri::command]
async fn mojang_authenticate(username: String, password: String) -> serde_json::Value {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .unwrap_or_default();

    let body = serde_json::json!({
        "agent": {
            "name": "Minecraft",
            "version": 1
        },
        "username": username,
        "password": password,
        "requestUser": true
    });

    match client.post("https://authserver.mojang.com/authenticate")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let user_name = json["selectedProfile"]["name"].as_str().unwrap_or("Player").to_string();
                    let uuid = json["selectedProfile"]["id"].as_str().unwrap_or("").to_string();
                    let token = json["accessToken"].as_str().unwrap_or("").to_string();
                    return serde_json::json!({
                        "success": true,
                        "username": user_name,
                        "uuid": uuid,
                        "token": token
                    });
                }
            } else {
                let err_msg = if let Ok(err_json) = resp.json::<serde_json::Value>().await {
                    err_json["errorMessage"].as_str().unwrap_or("Неверный логин или пароль").to_string()
                } else {
                    "Ошибка авторизации Mojang (401)".to_string()
                };
                return serde_json::json!({ "success": false, "error": err_msg });
            }
        }
        Err(e) => {
            return serde_json::json!({ "success": false, "error": format!("Ошибка соединения с сервером Mojang: {}", e) });
        }
    }
    serde_json::json!({ "success": false, "error": "Неизвестный ответ сервера" })
}

fn append_to_debug_file(line: &str) {
    let root = get_game_dir();
    let logs_dir = root.join("logs");
    let _ = fs::create_dir_all(&logs_dir);
    let log_file = logs_dir.join("fenixcraft-debug.log");
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&log_file) {
        let _ = writeln!(f, "{}", line);
    }
    let root_log = root.join("fenixcraft-debug.log");
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&root_log) {
        let _ = writeln!(f, "{}", line);
    }
}

#[tauri::command]
fn save_debug_log(console_text: String, custom_path: Option<String>) -> serde_json::Value {
    let root = get_game_dir();
    let logs_dir = root.join("logs");
    let _ = fs::create_dir_all(&logs_dir);

    let target_path = if let Some(p) = custom_path.filter(|s| !s.trim().is_empty()) {
        PathBuf::from(p)
    } else {
        root.join("fenixcraft-debug.log")
    };

    let mut full_report = String::new();
    full_report.push_str("===== FENIXCRAFT LAUNCHER v1.0 DEBUG LOG =====\n");
    full_report.push_str(&console_text);
    full_report.push_str("\n==============================================\n");

    for extra_name in ["debug.log", "latest.log"] {
        let p = logs_dir.join(extra_name);
        if p.exists() {
            if let Ok(content) = fs::read_to_string(&p) {
                let tail: Vec<&str> = content.lines().rev().take(150).collect();
                full_report.push_str(&format!("\n--- Последние 150 строк из logs/{} ---\n", extra_name));
                for l in tail.into_iter().rev() {
                    full_report.push_str(l);
                    full_report.push('\n');
                }
            }
        }
    }

    match fs::write(&target_path, &full_report) {
        Ok(_) => {
            let _ = fs::write(logs_dir.join("fenixcraft-debug.log"), &full_report);
            #[cfg(target_os = "windows")]
            {
                let _ = Command::new("explorer.exe")
                    .arg(format!("/select,{}", target_path.to_string_lossy()))
                    .spawn();
            }
            serde_json::json!({
                "success": true,
                "path": target_path.to_string_lossy().to_string()
            })
        }
        Err(e) => serde_json::json!({
            "success": false,
            "error": e.to_string()
        }),
    }
}

fn ensure_default_game_options(root_dir: &PathBuf) {
    let options_path = root_dir.join("options.txt");
    if !options_path.exists() {
        let mut packs = vec!["\"vanilla\"".to_string(), "\"mod_resources\"".to_string()];
        let rp_dir = root_dir.join("resourcepacks");
        if let Ok(entries) = fs::read_dir(&rp_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".zip") {
                    packs.push(format!("\"file/{}\"", name));
                }
            }
        }
        let default_opts = format!(
            "lang:ru_ru\nnarrator:0\nguiScale:0\nmaxFps:260\nenableVsync:false\nrenderDistance:12\nsimulationDistance:10\ngamma:0.5\ntutorialStep:none\njoinedFirstServer:true\nresourcePacks:[{}]\n",
            packs.join(",")
        );
        let _ = fs::write(&options_path, default_opts);
    } else if let Ok(existing) = fs::read_to_string(&options_path) {
        let mut lines: Vec<String> = Vec::new();
        let mut has_narrator = false;
        for line in existing.lines() {
            if line.starts_with("narrator:") {
                lines.push("narrator:0".to_string());
                has_narrator = true;
            } else {
                lines.push(line.to_string());
            }
        }
        if !has_narrator {
            lines.push("narrator:0".to_string());
        }
        let _ = fs::write(&options_path, lines.join("\n") + "\n");
    }

    let config_dir = root_dir.join("config");
    let _ = fs::create_dir_all(&config_dir);
    let iris_props = config_dir.join("iris.properties");
    if !iris_props.exists() {
        let mut first_shader = String::new();
        let sp_dir = root_dir.join("shaderpacks");
        let _ = fs::create_dir_all(&sp_dir);
        if let Ok(entries) = fs::read_dir(&sp_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".zip") {
                    first_shader = name;
                    break;
                }
            }
        }
        let iris_content = if !first_shader.is_empty() {
            format!("enableShaders=true\nshaderPack={}\n", first_shader)
        } else {
            "enableShaders=true\n".to_string()
        };
        let _ = fs::write(&iris_props, iris_content);
    }
}

fn compute_sha256_file(path: &PathBuf) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hex::encode(hasher.finalize()))
}

async fn sync_with_custom_backend(app: &AppHandle, root_dir: &PathBuf, profile: &str, backend_url: &str) {
    let base = backend_url.trim_end_matches('/');
    let manifest_url = format!("{}/manifest/{}", base, profile);

    let client = match reqwest::Client::builder().timeout(Duration::from_secs(15)).build() {
        Ok(c) => c,
        Err(_) => return,
    };

    let resp = match client.get(&manifest_url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => {
            let _ = app.emit(
                "game:log",
                format!("[FenixBackend] Серверный манифест {} не активен, используем локальный/автономный режим.", manifest_url),
            );
            return;
        }
    };

    let manifest: BackendManifest = match resp.json().await {
        Ok(m) => m,
        Err(_) => return,
    };

    let _ = app.emit(
        "game:log",
        format!(
            "[FenixBackend] Получен манифест профиля '{}': проверка {} файлов (моды, текстуры, конфиги)...",
            manifest.profile,
            manifest.files.len()
        ),
    );

    let mut allowed_mod_hashes: HashSet<String> = HashSet::new();
    let total = manifest.files.len().max(1);

    for (idx, item) in manifest.files.iter().enumerate() {
        if item.path.starts_with("mods/") {
            allowed_mod_hashes.insert(item.sha256.to_lowercase());
        }

        let target_path = root_dir.join(&item.path);
        let need_download = if target_path.exists() {
            if item.path.starts_with("config/") || item.path == "options.txt" {
                false
            } else {
                match compute_sha256_file(&target_path) {
                    Some(h) => h.to_lowercase() != item.sha256.to_lowercase(),
                    None => true,
                }
            }
        } else {
            true
        };

        if need_download {
            let _ = app.emit(
                "game:progress",
                serde_json::json!({
                    "type": format!("Сервер: {}", item.path),
                    "task": idx + 1,
                    "total": total
                }),
            );
            if let Some(p) = target_path.parent() {
                let _ = fs::create_dir_all(p);
            }
            let file_url = if item.url.starts_with("http") {
                item.url.clone()
            } else {
                format!("{}/{}", base, item.url.trim_start_matches('/'))
            };
            if let Ok(dl) = client.get(&file_url).send().await {
                if dl.status().is_success() {
                    if let Ok(bytes) = dl.bytes().await {
                        let _ = fs::write(&target_path, &bytes);
                        let _ = app.emit("game:log", format!("[FenixBackend] Загружен файл: {}", item.path));
                    }
                }
            }
        }
    }

    if manifest.enforce_mods && !allowed_mod_hashes.is_empty() {
        let mods_dir = root_dir.join("mods");
        if let Ok(entries) = fs::read_dir(&mods_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("jar") {
                    if let Some(h) = compute_sha256_file(&p) {
                        if !allowed_mod_hashes.contains(&h.to_lowercase()) {
                            let _ = app.emit(
                                "game:log",
                                format!("[Guard SHA-256] Удален сторонний мод: {:?}", p.file_name().unwrap_or_default()),
                            );
                            let _ = fs::remove_file(&p);
                        }
                    }
                }
            }
        }
    }
}

async fn ensure_iris_and_sodium_mods(
    app: &AppHandle,
    root_dir: &PathBuf,
    mc_ver: &str,
    loader: &str,
) -> Result<(), String> {
    let mods_dir = root_dir.join("mods");
    fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;

    let client = reqwest::Client::builder()
        .user_agent("FenixCraftLauncher/1.0 (fenixcraft.ru)")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    for project_slug in ["sodium", "iris"] {
        let mut already_installed = false;
        if let Ok(entries) = fs::read_dir(&mods_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_lowercase();
                if fname.contains(project_slug) && fname.ends_with(".jar") {
                    already_installed = true;
                    break;
                }
            }
        }
        if already_installed {
            let _ = app.emit("game:log", format!("[Iris Installer] Мод '{}' уже установлен в папке mods.", project_slug));
            continue;
        }

        let _ = app.emit(
            "game:log",
            format!("[Iris Installer] Поиск последней версии '{}' для {} ({})...", project_slug, mc_ver, loader),
        );

        for candidate_ver in [mc_ver, "1.21.4", "1.21.1"] {
            let url = format!(
                "https://api.modrinth.com/v2/project/{}/version?loaders=[\"{}\"]&game_versions=[\"{}\"]",
                project_slug, loader, candidate_ver
            );
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(arr) = resp.json::<serde_json::Value>().await {
                    if let Some(first_ver) = arr.as_array().and_then(|a| a.first()) {
                        if let Some(file_obj) = first_ver["files"].as_array().and_then(|f| f.first()) {
                            if let (Some(dl_url), Some(filename)) = (
                                file_obj["url"].as_str(),
                                file_obj["filename"].as_str(),
                            ) {
                                let _ = app.emit(
                                    "game:progress",
                                    serde_json::json!({
                                        "type": format!("Установка шейдер-мода: {}", filename),
                                        "task": 50,
                                        "total": 100
                                    }),
                                );
                                if let Ok(dl_resp) = client.get(dl_url).send().await {
                                    if let Ok(bytes) = dl_resp.bytes().await {
                                        let dest = mods_dir.join(filename);
                                        let _ = fs::write(&dest, &bytes);
                                        let _ = app.emit(
                                            "game:log",
                                            format!("[Iris Installer] ✅ Установлен {} ({} байт)", filename, bytes.len()),
                                        );
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if loader == "fabric" {
        let mut has_fapi = false;
        if let Ok(entries) = fs::read_dir(&mods_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_lowercase();
                if fname.contains("fabric-api") && fname.ends_with(".jar") {
                    has_fapi = true;
                    break;
                }
            }
        }
        if !has_fapi {
            for candidate_ver in [mc_ver, "1.21.4", "1.21.1"] {
                let url = format!(
                    "https://api.modrinth.com/v2/project/fabric-api/version?loaders=[\"fabric\"]&game_versions=[\"{}\"]",
                    candidate_ver
                );
                if let Ok(resp) = client.get(&url).send().await {
                    if let Ok(arr) = resp.json::<serde_json::Value>().await {
                        if let Some(first_ver) = arr.as_array().and_then(|a| a.first()) {
                            if let Some(file_obj) = first_ver["files"].as_array().and_then(|f| f.first()) {
                                if let (Some(dl_url), Some(filename)) = (
                                    file_obj["url"].as_str(),
                                    file_obj["filename"].as_str(),
                                ) {
                                    if let Ok(dl_resp) = client.get(dl_url).send().await {
                                        if let Ok(bytes) = dl_resp.bytes().await {
                                            let _ = fs::write(mods_dir.join(filename), &bytes);
                                            let _ = app.emit("game:log", format!("[Fabric Installer] ✅ Установлен {}", filename));
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn find_java_binary_in_dir(dir: &PathBuf, target_name: &str) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }
    let direct = dir.join("bin").join(target_name);
    if direct.exists() {
        return Some(direct);
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let candidate = p.join("bin").join(target_name);
                if candidate.exists() {
                    return Some(candidate);
                }
                let mac_candidate = p.join("Contents").join("Home").join("bin").join(target_name);
                if mac_candidate.exists() {
                    return Some(mac_candidate);
                }
                if let Some(sub) = find_java_binary_in_dir(&p, target_name) {
                    return Some(sub);
                }
            }
        }
    }
    None
}

fn is_system_java_exact_major(required_major: u32) -> bool {
    if let Ok(out) = Command::new("java").arg("-version").output() {
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
        let p1 = format!("version \"{}.", required_major);
        let p2 = format!("version \"{}\"", required_major);
        let p3 = format!("openjdk {}.", required_major);
        let p4 = format!("openjdk {}", required_major);
        let p5 = format!("java {}.", required_major);
        if combined.contains(&p1)
            || combined.contains(&p2)
            || combined.contains(&p3)
            || combined.contains(&p4)
            || combined.contains(&p5)
        {
            return true;
        }
    }
    false
}

fn detect_required_java_version(mc_version_id: &str, ver_json_opt: Option<&serde_json::Value>) -> u32 {
    if let Some(vj) = ver_json_opt {
        if let Some(major) = vj["javaVersion"]["majorVersion"].as_u64() {
            if major >= 25 {
                return 25;
            }
            if major == 21 {
                return 21;
            }
        }
    }

    let clean = mc_version_id.trim().to_lowercase();
    if let Some(first_num_str) = clean.split(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty()) {
        if let Ok(first_num) = first_num_str.parse::<u32>() {
            if first_num >= 26 {
                return 25;
            }
        }
    }

    21
}

async fn ensure_java_runtime(
    app: &AppHandle,
    custom_path: &str,
    required_major: u32,
    mc_version_label: &str,
) -> Result<String, String> {
    let _ = app.emit(
        "game:log",
        format!(
            "[Java Scanner] Версия Minecraft '{}' -> Рекомендуемая среда выполнения: Java {}",
            mc_version_label, required_major
        ),
    );

    if !custom_path.trim().is_empty() {
        let p = PathBuf::from(custom_path.trim());
        if p.exists() {
            let _ = app.emit(
                "game:log",
                format!("[Java Scanner] Используется пользовательский путь Java: {}", custom_path.trim()),
            );
            return Ok(custom_path.trim().to_string());
        }
    }

    let bin_name = if cfg!(windows) { "java.exe" } else { "java" };
    let root_dir = get_game_dir();

    if required_major == 21 {
        let existing_jdk21 = root_dir.join("java").join("jdk-21").join("bin").join(bin_name);
        if existing_jdk21.exists() {
            let path_str = existing_jdk21.to_string_lossy().to_string();
            let _ = app.emit("game:log", format!("[Java Scanner] Найдена локальная Java 21: {}", path_str));
            return Ok(path_str);
        }
    }

    let runtime_dir = root_dir.join("runtime").join(format!("java-{}", required_major));
    if let Some(found) = find_java_binary_in_dir(&runtime_dir, bin_name) {
        let path_str = found.to_string_lossy().to_string();
        let _ = app.emit(
            "game:log",
            format!("[Java Scanner] Найдена портативная Java {}: {}", required_major, path_str),
        );
        return Ok(path_str);
    }

    if is_system_java_exact_major(required_major) {
        let _ = app.emit(
            "game:log",
            format!("[Java Scanner] Системная Java точно соответствует версии Java {}.", required_major),
        );
        return Ok("java".to_string());
    }

    let _ = app.emit(
        "game:log",
        format!(
            "[Java Auto-Installer] Требуемая Java {} не найдена локально. Скачиваем портативную Java {} (Adoptium)...",
            required_major, required_major
        ),
    );
    fs::create_dir_all(&runtime_dir).map_err(|e| e.to_string())?;

    let os_name = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch_name = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x64" };
    let ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };
    let archive_path = runtime_dir.join(format!("adoptium-java{}.{}", required_major, ext));

    let client = reqwest::Client::builder()
        .user_agent("FenixCraftLauncher/1.0")
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;

    let candidate_urls = [
        format!(
            "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jre/hotspot/normal/eclipse",
            required_major, os_name, arch_name
        ),
        format!(
            "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/eclipse",
            required_major, os_name, arch_name
        ),
        format!(
            "https://api.adoptium.net/v3/binary/latest/{}/ea/{}/{}/jdk/hotspot/normal/eclipse",
            required_major, os_name, arch_name
        ),
    ];

    let mut downloaded = false;
    for (idx, url) in candidate_urls.iter().enumerate() {
        let _ = app.emit(
            "game:progress",
            serde_json::json!({
                "type": format!("Скачивание портативной Java {} (Adoptium)", required_major),
                "task": 20 + idx * 15,
                "total": 100
            }),
        );
        if let Ok(resp) = client.get(url).send().await {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    if bytes.len() > 5_000_000 {
                        fs::write(&archive_path, &bytes).map_err(|e| e.to_string())?;
                        downloaded = true;
                        break;
                    }
                }
            }
        }
    }

    if !downloaded {
        return Err(format!("Не удалось скачать портативную Java {} с серверов Adoptium", required_major));
    }

    let _ = app.emit(
        "game:progress",
        serde_json::json!({
            "type": format!("Распаковка Java {}...", required_major),
            "task": 85,
            "total": 100
        }),
    );

    if cfg!(target_os = "windows") {
        let _ = extract_zip_archive(&archive_path, &runtime_dir, false);
    } else {
        let _ = Command::new("tar").arg("-xf").arg(&archive_path).arg("-C").arg(&runtime_dir).status();
    }
    let _ = fs::remove_file(&archive_path);

    if let Some(found) = find_java_binary_in_dir(&runtime_dir, bin_name) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&found) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&found, perms);
            }
        }
        let path_str = found.to_string_lossy().to_string();
        let _ = app.emit(
            "game:log",
            format!("[Java Auto-Installer] ✅ Портативная Java {} успешно установлена: {}", required_major, path_str),
        );
        return Ok(path_str);
    }

    Err(format!("Исполняемый файл Java {} не найден после распаковки", required_major))
}

async fn ensure_all_assets_downloaded(
    app: &AppHandle,
    _client: &reqwest::Client,
    root_dir: &PathBuf,
    asset_id: &str,
    asset_url_opt: Option<&str>,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .pool_max_idle_per_host(16)
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())?;

    let indexes_dir = root_dir.join("assets").join("indexes");
    let objects_dir = root_dir.join("assets").join("objects");
    fs::create_dir_all(&indexes_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&objects_dir).map_err(|e| e.to_string())?;

    let idx_path = indexes_dir.join(format!("{}.json", asset_id));
    if !idx_path.exists() || fs::metadata(&idx_path).map(|m| m.len() == 0).unwrap_or(true) {
        if let Some(url) = asset_url_opt {
            let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
            let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
            fs::write(&idx_path, &bytes).map_err(|e| e.to_string())?;
        }
    }

    if !idx_path.exists() {
        return Ok(());
    }

    let idx_str = fs::read_to_string(&idx_path).map_err(|e| e.to_string())?;
    let idx_json: serde_json::Value = serde_json::from_str(&idx_str).map_err(|e| e.to_string())?;

    let official_objects_dir = get_official_mc_dir().join("assets").join("objects");
    let mut missing_items: Vec<(String, u64)> = Vec::new();
    let mut seen_hashes: HashSet<String> = HashSet::new();
    let mut total_unique = 0usize;

    if let Some(objects) = idx_json["objects"].as_object() {
        for (_name, obj) in objects {
            if let Some(hash) = obj["hash"].as_str() {
                let expected_size = obj["size"].as_u64().unwrap_or(0);
                if hash.len() >= 2 && seen_hashes.insert(hash.to_string()) {
                    total_unique += 1;
                    let prefix = &hash[0..2];
                    let target_file = objects_dir.join(prefix).join(hash);

                    let is_valid = if let Ok(meta) = fs::metadata(&target_file) {
                        meta.len() > 0 && (expected_size == 0 || meta.len() == expected_size)
                    } else {
                        false
                    };

                    if !is_valid {
                        let fallback_file = official_objects_dir.join(prefix).join(hash);
                        if let Ok(f_meta) = fs::metadata(&fallback_file) {
                            if f_meta.len() > 0 && (expected_size == 0 || f_meta.len() == expected_size) {
                                if let Some(p) = target_file.parent() {
                                    let _ = fs::create_dir_all(p);
                                }
                                if fs::copy(&fallback_file, &target_file).is_ok() {
                                    continue;
                                }
                            }
                        }
                        missing_items.push((hash.to_string(), expected_size));
                    }
                }
            }
        }
    }

    if missing_items.is_empty() {
        let _ = app.emit(
            "game:log",
            format!(
                "[Assets Audit] ✅ Проверено {}/{} файлов ассетов (индекс {}). Статус: 100% ДОКАЧАНЫ!",
                total_unique, total_unique, asset_id
            ),
        );
        return Ok(());
    }

    let total = missing_items.len();
    let _ = app.emit(
        "game:log",
        format!(
            "[Mojang Assets] Найдено {} недостающих файлов из {}. Начинаем загрузку...",
            total, total_unique
        ),
    );

    let sem = Arc::new(tokio::sync::Semaphore::new(16));
    let done_counter = Arc::new(AtomicUsize::new(0));
    let failed_counter = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(total);

    for (hash, expected_size) in missing_items {
        let permit_pool = sem.clone();
        let client_clone = client.clone();
        let objects_dir_clone = objects_dir.clone();
        let app_clone = app.clone();
        let counter_clone = done_counter.clone();
        let fail_clone = failed_counter.clone();

        let handle = tokio::spawn(async move {
            let _permit = permit_pool.acquire().await;
            let prefix = &hash[0..2];
            let sub_dir = objects_dir_clone.join(prefix);
            let _ = fs::create_dir_all(&sub_dir);
            let dest = sub_dir.join(&hash);
            let url = format!("https://resources.download.minecraft.net/{}/{}", prefix, hash);

            let mut ok = false;
            for attempt in 0..5 {
                if let Ok(resp) = client_clone.get(&url).send().await {
                    if resp.status().is_success() {
                        if let Ok(bytes) = resp.bytes().await {
                            if expected_size == 0 || bytes.len() as u64 == expected_size {
                                if fs::write(&dest, &bytes).is_ok() {
                                    ok = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(150 * (attempt + 1))).await;
            }

            if !ok {
                fail_clone.fetch_add(1, Ordering::Relaxed);
            }

            let current = counter_clone.fetch_add(1, Ordering::Relaxed) + 1;
            if current % 20 == 0 || current == total {
                let _ = app_clone.emit(
                    "game:progress",
                    serde_json::json!({
                        "type": "Загрузка ресурсов Minecraft",
                        "task": current,
                        "total": total
                    }),
                );
            }
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.await;
    }

    let failed = failed_counter.load(Ordering::Relaxed);
    if failed > 0 {
        return Err(format!(
            "Не удалось докачать {} файлов ресурсов. Нажмите «Играть» еще раз для докачки!",
            failed
        ));
    }

    let _ = app.emit(
        "game:log",
        format!(
            "[Assets Audit] ✅ Все 100% игровых ресурсов ({} из {}) успешно проверены и загружены!",
            total_unique, total_unique
        ),
    );
    Ok(())
}

fn maven_to_rel_path(coord: &str) -> Option<String> {
    let clean = coord.split('@').next().unwrap_or(coord);
    let parts: Vec<&str> = clean.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let file_name = if parts.len() >= 4 {
        format!("{}-{}-{}.jar", artifact, version, parts[3])
    } else {
        format!("{}-{}.jar", artifact, version)
    };
    Some(format!("{}/{}/{}/{}", group, artifact, version, file_name))
}

fn is_lib_allowed(lib: &serde_json::Value, current_os: &str) -> bool {
    if let Some(rules) = lib["rules"].as_array() {
        let mut allowed = false;
        for rule in rules {
            let action = rule["action"].as_str().unwrap_or("allow");
            let os_match = rule["os"]["name"]
                .as_str()
                .map(|n| n == current_os)
                .unwrap_or(true);
            let arch_match = rule["os"]["arch"]
                .as_str()
                .map(|a| {
                    if cfg!(target_arch = "x86_64") {
                        a == "x64" || a == "x86_64" || a == "amd64"
                    } else if cfg!(target_arch = "aarch64") {
                        a == "arm64" || a == "aarch64"
                    } else {
                        a == "x86"
                    }
                })
                .unwrap_or(true);

            if os_match && arch_match {
                allowed = action == "allow";
            }
        }
        return allowed;
    }
    true
}

fn is_wrong_native_arch(rel_path: &str, current_os: &str) -> bool {
    let lower = rel_path.to_lowercase();
    if current_os != "osx" && (lower.contains("java-objc-bridge") || lower.contains("natives-macos") || lower.contains("natives-osx")) {
        return true;
    }
    if current_os != "linux" && lower.contains("natives-linux") {
        return true;
    }
    if current_os != "windows" && lower.contains("natives-windows") {
        return true;
    }
    if cfg!(target_arch = "x86_64") {
        if lower.contains("arm64") || lower.contains("aarch64") || lower.contains("natives-windows-x86") || lower.contains("natives-linux-arm") {
            return true;
        }
    }
    false
}

fn collect_libraries_from_json(
    ver_json: &serde_json::Value,
    libs_dir: &PathBuf,
    current_os: &str,
    classpath: &mut Vec<PathBuf>,
    seen: &mut HashSet<String>,
    missing_downloads: &mut Vec<(String, PathBuf)>,
) {
    if let Some(libs) = ver_json["libraries"].as_array() {
        for lib in libs {
            if !is_lib_allowed(lib, current_os) {
                continue;
            }
            let mut rel_opt: Option<String> = None;
            let mut url_opt: Option<String> = None;

            if let Some(artifact) = lib["downloads"]["artifact"].as_object() {
                rel_opt = artifact.get("path").and_then(|v| v.as_str()).map(|s| s.to_string());
                url_opt = artifact.get("url").and_then(|v| v.as_str()).map(|s| s.to_string());
            }
            if rel_opt.is_none() {
                if let Some(name) = lib["name"].as_str() {
                    rel_opt = maven_to_rel_path(name);
                    if let (Some(base), Some(rel)) = (lib["url"].as_str(), &rel_opt) {
                        url_opt = Some(format!("{}{}", base, rel));
                    }
                }
            }

            if let Some(rel) = rel_opt {
                if is_wrong_native_arch(&rel, current_os) {
                    continue;
                }
                let full_path = libs_dir.join(&rel);
                let key = full_path.to_string_lossy().to_lowercase();
                if seen.insert(key) {
                    classpath.push(full_path.clone());
                    if !full_path.exists() {
                        if let Some(u) = url_opt {
                            if !u.is_empty() {
                                missing_downloads.push((u, full_path));
                            }
                        }
                    }
                }
            }

            let classifier_key = if current_os == "windows" {
                "natives-windows"
            } else if current_os == "osx" {
                "natives-macos"
            } else {
                "natives-linux"
            };
            if let Some(nat_obj) = lib["downloads"]["classifiers"][classifier_key].as_object() {
                if let (Some(path_str), Some(url_str)) = (
                    nat_obj.get("path").and_then(|v| v.as_str()),
                    nat_obj.get("url").and_then(|v| v.as_str()),
                ) {
                    if !is_wrong_native_arch(path_str, current_os) {
                        let full_path = libs_dir.join(path_str);
                        let key = full_path.to_string_lossy().to_lowercase();
                        if seen.insert(key) {
                            classpath.push(full_path.clone());
                            if !full_path.exists() {
                                missing_downloads.push((url_str.to_string(), full_path));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn find_installed_neoforge_json(versions_dir: &PathBuf) -> Option<(String, PathBuf)> {
    let priority = versions_dir.join("neoforge-21.1.252").join("neoforge-21.1.252.json");
    if priority.exists() {
        return Some(("neoforge-21.1.252".to_string(), priority));
    }
    if let Ok(entries) = fs::read_dir(versions_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.to_lowercase().contains("neoforge") && entry.path().is_dir() {
                let candidate = entry.path().join(format!("{}.json", name));
                if candidate.exists() {
                    return Some((name, candidate));
                }
            }
        }
    }
    None
}

async fn launch_installed_neoforge(
    app: &AppHandle,
    config: &LauncherConfig,
    java_bin: &str,
    neo_id: &str,
    neo_json_path: &PathBuf,
) -> Result<(), String> {
    let root_dir = get_game_dir();
    let versions_dir = root_dir.join("versions");
    let libs_dir = root_dir.join("libraries");
    let assets_dir = root_dir.join("assets");

    let neo_str = fs::read_to_string(neo_json_path).map_err(|e| e.to_string())?;
    let neo_json: serde_json::Value = serde_json::from_str(&neo_str).map_err(|e| e.to_string())?;

    let inherits_from = neo_json["inheritsFrom"].as_str().unwrap_or("1.21.1");
    let natives_dir = root_dir.join("natives").join(inherits_from);
    let _ = fs::create_dir_all(&natives_dir);

    let vanilla_json_path = versions_dir.join(inherits_from).join(format!("{}.json", inherits_from));

    if !vanilla_json_path.exists() {
        let _ = download_vanilla_to_root(app, inherits_from).await?;
    }

    let van_str = fs::read_to_string(&vanilla_json_path).map_err(|e| e.to_string())?;
    let van_json: serde_json::Value = serde_json::from_str(&van_str).map_err(|e| e.to_string())?;

    let current_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };

    let mut classpath_jars: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut missing_downloads: Vec<(String, PathBuf)> = Vec::new();

    collect_libraries_from_json(&van_json, &libs_dir, current_os, &mut classpath_jars, &mut seen, &mut missing_downloads);
    collect_libraries_from_json(&neo_json, &libs_dir, current_os, &mut classpath_jars, &mut seen, &mut missing_downloads);

    let mc_jar = versions_dir.join(inherits_from).join(format!("{}.jar", inherits_from));
    classpath_jars.push(mc_jar);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    if !missing_downloads.is_empty() {
        let total = missing_downloads.len();
        for (idx, (url, dest)) in missing_downloads.iter().enumerate() {
            let _ = app.emit(
                "game:progress",
                serde_json::json!({
                    "type": "Докачка библиотек NeoForge",
                    "task": idx + 1,
                    "total": total
                }),
            );
            if let Some(p) = dest.parent() {
                let _ = fs::create_dir_all(p);
            }
            if let Ok(resp) = client.get(url).send().await {
                if let Ok(bytes) = resp.bytes().await {
                    let _ = fs::write(dest, &bytes);
                }
            }
        }
    }

    if config.selected_version.contains("iris") {
        let _ = ensure_iris_and_sodium_mods(app, &root_dir, inherits_from, "neoforge").await;
    }

    let asset_index = van_json["assetIndex"]["id"]
        .as_str()
        .or_else(|| van_json["assets"].as_str())
        .unwrap_or("17")
        .to_string();
    let asset_url = van_json["assetIndex"]["url"].as_str();

    ensure_all_assets_downloaded(app, &client, &root_dir, &asset_index, asset_url).await?;

    let _ = fs::remove_dir_all(&natives_dir);
    let _ = fs::create_dir_all(&natives_dir);
    for jar in &classpath_jars {
        let name = jar.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        if name.contains("natives") && !name.contains("arm64") && !name.contains("x86") && jar.exists() {
            let _ = extract_zip_archive(jar, &natives_dir, true);
        }
    }

    let cp_sep = if cfg!(windows) { ";" } else { ":" };
    let cp_string = classpath_jars
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(cp_sep);

    let offline_uuid = uuid::Uuid::new_v3(
        &uuid::Uuid::NAMESPACE_OID,
        format!("OfflinePlayer:{}", config.username).as_bytes(),
    )
    .to_string();

    let natives_str = natives_dir.to_string_lossy().to_string();
    let libs_str = libs_dir.to_string_lossy().to_string();

    let mut args: Vec<String> = Vec::new();
    args.push("-Xms2G".to_string());
    args.push(format!("-Xmx{}M", config.ram));
    args.push("-Xss1M".to_string());
    args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".to_string());
    args.push("-Dfile.encoding=UTF-8".to_string());
    args.push("-Dsun.stdout.encoding=UTF-8".to_string());
    args.push("-Dsun.stderr.encoding=UTF-8".to_string());
    args.push(format!("-Djava.library.path={}", natives_str));
    args.push(format!("-Djna.tmpdir={}", natives_str));
    args.push(format!("-Dio.netty.native.workdir={}", natives_str));
    args.push("-Dminecraft.launcher.brand=FenixCraft".to_string());
    args.push("-Dminecraft.launcher.version=1.0".to_string());
    args.push("-cp".to_string());
    args.push(cp_string);

    if let Some(neo_jvm) = neo_json["arguments"]["jvm"].as_array() {
        for item in neo_jvm {
            if let Some(s) = item.as_str() {
                let replaced = s
                    .replace("${library_directory}", &libs_str)
                    .replace("${classpath_separator}", cp_sep)
                    .replace("${version_name}", inherits_from);
                args.push(replaced);
            }
        }
    }

    for custom_arg in config.jvm_args.split_whitespace() {
        if !custom_arg.starts_with("-Xmx") && !custom_arg.starts_with("-Xms") {
            args.push(custom_arg.to_string());
        }
    }

    let main_class = neo_json["mainClass"]
        .as_str()
        .unwrap_or("cpw.mods.bootstraplauncher.BootstrapLauncher")
        .to_string();
    args.push(main_class);

    args.extend_from_slice(&[
        "--username".to_string(),
        config.username.clone(),
        "--version".to_string(),
        inherits_from.to_string(),
        "--gameDir".to_string(),
        root_dir.to_string_lossy().to_string(),
        "--assetsDir".to_string(),
        assets_dir.to_string_lossy().to_string(),
        "--assetIndex".to_string(),
        asset_index,
        "--uuid".to_string(),
        offline_uuid,
        "--accessToken".to_string(),
        "0".to_string(),
        "--clientId".to_string(),
        "0".to_string(),
        "--xuid".to_string(),
        "0".to_string(),
        "--userType".to_string(),
        "mojang".to_string(),
        "--versionType".to_string(),
        "release".to_string(),
        "--width".to_string(),
        config.width.to_string(),
        "--height".to_string(),
        config.height.to_string(),
    ]);

    if let Some(neo_game) = neo_json["arguments"]["game"].as_array() {
        for item in neo_game {
            if let Some(s) = item.as_str() {
                args.push(s.to_string());
            }
        }
    }

    let _ = app.emit(
        "game:log",
        format!("[FenixCraft v1.0] Старт NeoForge ({}) -> Java: {}", neo_id, java_bin),
    );

    spawn_and_stream_jvm(app, java_bin, &args, &root_dir);
    Ok(())
}

async fn download_vanilla_to_root(
    app: &AppHandle,
    requested_version: &str,
) -> Result<(PathBuf, Vec<PathBuf>, String, String, String, PathBuf, u32), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let _ = app.emit("game:log", "[Mojang Direct] Получение списка версий с piston-meta.mojang.com...");

    let meta_url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    let manifest: serde_json::Value = client
        .get(meta_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
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

    let versions = manifest["versions"].as_array().ok_or("Ошибка формата манифеста Mojang")?;
    let ver_entry = versions
        .iter()
        .find(|v| v["id"].as_str() == Some(&target_ver))
        .or_else(|| versions.iter().find(|v| v["id"].as_str() == Some(&latest_release)))
        .ok_or("Версия не найдена")?;

    let actual_ver = ver_entry["id"].as_str().unwrap_or(&latest_release).to_string();
    let ver_url = ver_entry["url"].as_str().ok_or("Нет URL версии")?;

    let root_dir = get_game_dir();
    let ver_dir = root_dir.join("versions").join(&actual_ver);
    let libs_dir = root_dir.join("libraries");
    let natives_dir = root_dir.join("natives").join(&actual_ver);
    fs::create_dir_all(&ver_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&libs_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&natives_dir).map_err(|e| e.to_string())?;

    let ver_json_path = ver_dir.join(format!("{}.json", actual_ver));
    let ver_json: serde_json::Value = if ver_json_path.exists() {
        let s = fs::read_to_string(&ver_json_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| e.to_string())?
    } else {
        let j: serde_json::Value = client
            .get(ver_url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let _ = fs::write(&ver_json_path, serde_json::to_string_pretty(&j).unwrap_or_default());
        j
    };

    let required_java = detect_required_java_version(&actual_ver, Some(&ver_json));

    let main_class = ver_json["mainClass"]
        .as_str()
        .unwrap_or("net.minecraft.client.main.Main")
        .to_string();

    let asset_id = ver_json["assetIndex"]["id"]
        .as_str()
        .or_else(|| ver_json["assets"].as_str())
        .unwrap_or("17")
        .to_string();
    let asset_url = ver_json["assetIndex"]["url"].as_str();

    let current_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };

    let mut classpath_jars: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut download_queue: Vec<(String, PathBuf)> = Vec::new();

    collect_libraries_from_json(&ver_json, &libs_dir, current_os, &mut classpath_jars, &mut seen, &mut download_queue);

    let client_jar_path = ver_dir.join(format!("{}.jar", actual_ver));
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
                "type": format!("Библиотека: {}", file_name),
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

    ensure_all_assets_downloaded(app, &client, &root_dir, &asset_id, asset_url).await?;

    let _ = fs::remove_dir_all(&natives_dir);
    let _ = fs::create_dir_all(&natives_dir);
    for jar in &classpath_jars {
        let name = jar.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        if name.contains("natives") && !name.contains("arm64") && !name.contains("x86") && jar.exists() {
            let _ = extract_zip_archive(jar, &natives_dir, true);
        }
    }

    let _ = app.emit(
        "game:progress",
        serde_json::json!({
            "type": "Готово к запуску",
            "task": 100,
            "total": 100
        }),
    );

    Ok((root_dir, classpath_jars, actual_ver, asset_id, main_class, natives_dir, required_java))
}

fn spawn_and_stream_jvm(app: &AppHandle, java_bin: &str, args: &[String], work_dir: &PathBuf) {
    let mut cmd = Command::new(java_bin);
    cmd.args(args)
        .current_dir(work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    match cmd.spawn() {
        Ok(mut proc) => {
            if let Some(stdout) = proc.stdout.take() {
                let app_out = app.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        let formatted = format!("[MC] {}", line);
                        append_to_debug_file(&formatted);
                        let _ = app_out.emit("game:log", formatted);
                    }
                });
            }
            if let Some(stderr) = proc.stderr.take() {
                let app_err = app.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines().map_while(Result::ok) {
                        let formatted = format!("[JVM] {}", line);
                        append_to_debug_file(&formatted);
                        let _ = app_err.emit("game:log", formatted);
                    }
                });
            }

            let status = proc.wait();
            let _ = app.emit("game:log", format!("[FenixCraft v1.0] Игра закрыта ({:?})", status));
            if let Ok(entries) = fs::read_dir(work_dir) {
                for entry in entries.flatten() {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if fname.starts_with("hs_err_pid") && fname.ends_with(".log") {
                        if let Ok(crash_txt) = fs::read_to_string(entry.path()) {
                            for line in crash_txt.lines().take(35) {
                                if line.contains("Problematic frame") || line.contains("EXCEPTION_") || line.contains("Command Line:") {
                                    let msg = format!("[CRASH-DIAG] {}", line);
                                    append_to_debug_file(&msg);
                                    let _ = app.emit("game:log", msg);
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(err) => {
            let _ = app.emit("game:log", format!("[ERROR] Ошибка старта JVM: {}", err));
        }
    }
}

const CURRENT_LAUNCHER_VERSION: &str = "1.0";

#[tauri::command]
async fn check_launcher_update() -> serde_json::Value {
    let client = reqwest::Client::builder()
        .user_agent("FenixCraftLauncher/1.0")
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_default();

    let cfg = get_config();
    let backend_ver_url = format!("{}/version", cfg.backend_url.trim_end_matches('/'));
    if let Ok(resp) = client.get(&backend_ver_url).send().await {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let (Some(latest), Some(dl_url)) = (json["version"].as_str(), json["url"].as_str()) {
                    let clean_latest = latest.trim_start_matches('v');
                    if clean_latest != CURRENT_LAUNCHER_VERSION {
                        return serde_json::json!({
                            "updateAvailable": true,
                            "latestVersion": clean_latest,
                            "downloadUrl": dl_url
                        });
                    }
                }
            }
        }
    }

    let gh_url = "https://api.github.com/repos/BloodRayT/launcher/releases/latest";
    if let Ok(resp) = client.get(gh_url).send().await {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(tag) = json["tag_name"].as_str() {
                    let clean_tag = tag.trim_start_matches('v');
                    if clean_tag != CURRENT_LAUNCHER_VERSION && clean_tag != "1.0.0" {
                        let mut exe_url = json["html_url"].as_str().unwrap_or("https://github.com/BloodRayT/launcher/releases").to_string();
                        if let Some(assets) = json["assets"].as_array() {
                            for asset in assets {
                                if let Some(name) = asset["name"].as_str() {
                                    if cfg!(windows) && (name.ends_with("-setup.exe") || name.ends_with(".msi")) {
                                        if let Some(u) = asset["browser_download_url"].as_str() {
                                            exe_url = u.to_string();
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        return serde_json::json!({
                            "updateAvailable": true,
                            "latestVersion": clean_tag,
                            "downloadUrl": exe_url
                        });
                    }
                }
            }
        }
    }

    serde_json::json!({
        "updateAvailable": false,
        "latestVersion": CURRENT_LAUNCHER_VERSION
    })
}

#[tauri::command]
async fn install_launcher_update(app: AppHandle, download_url: String) -> serde_json::Value {
    let client = reqwest::Client::builder()
        .user_agent("FenixCraftLauncher/1.0")
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap_or_default();

    let update_file = get_game_dir().join("FenixCraft_Update_Setup.exe");
    let _ = app.emit("game:log", format!("[Auto-Updater] Скачивание новой версии лаунчера из {}...", download_url));

    if let Ok(resp) = client.get(&download_url).send().await {
        if resp.status().is_success() {
            if let Ok(bytes) = resp.bytes().await {
                if fs::write(&update_file, &bytes).is_ok() {
                    #[cfg(target_os = "windows")]
                    {
                        let _ = Command::new(&update_file).spawn();
                        std::process::exit(0);
                    }
                }
            }
        }
    }

    open_external_url(download_url);
    serde_json::json!({ "status": "opened_in_browser" })
}

#[tauri::command]
async fn launch_game(app: AppHandle) -> serde_json::Value {
    let config = get_config();
    let app_clone = app.clone();

    tokio::spawn(async move {
        let _ = app_clone.emit(
            "game:log",
            format!("[FenixCraft v1.0] Подготовка к запуску профиля: {}...", config.selected_version),
        );

        let root_dir = get_game_dir();
        let versions_dir = root_dir.join("versions");

        sync_with_custom_backend(&app_clone, &root_dir, &config.selected_version, &config.backend_url).await;
        ensure_default_game_options(&root_dir);

        if config.selected_version.contains("neoforge") {
            let req_java = detect_required_java_version(&config.selected_version, None);
            let java_bin = match ensure_java_runtime(&app_clone, &config.java_path, req_java, &config.selected_version).await {
                Ok(bin) => bin,
                Err(e) => {
                    let _ = app_clone.emit("game:log", format!("[ERROR] {}", e));
                    let _ = app_clone.emit("game:closed", ());
                    return;
                }
            };

            if let Some((neo_id, neo_json_path)) = find_installed_neoforge_json(&versions_dir) {
                if let Err(e) = launch_installed_neoforge(&app_clone, &config, &java_bin, &neo_id, &neo_json_path).await {
                    let _ = app_clone.emit("game:log", format!("[ERROR] Ошибка запуска NeoForge: {}", e));
                }
                let _ = app_clone.emit("game:closed", ());
                return;
            }
        }

        let (work_dir, mut classpath_jars, mc_ver, asset_id, mut main_class, natives_dir, required_java) =
            match download_vanilla_to_root(&app_clone, &config.selected_version).await {
                Ok(res) => res,
                Err(e) => {
                    let _ = app_clone.emit("game:log", format!("[ERROR] Ошибка подготовки клиента: {}", e));
                    let _ = app_clone.emit("game:closed", ());
                    return;
                }
            };

        let java_bin = match ensure_java_runtime(&app_clone, &config.java_path, required_java, &mc_ver).await {
            Ok(bin) => bin,
            Err(e) => {
                let _ = app_clone.emit("game:log", format!("[ERROR] {}", e));
                let _ = app_clone.emit("game:closed", ());
                return;
            }
        };

        if config.selected_version.contains("fabric") || config.selected_version.ends_with("-iris") {
            let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().unwrap_or_default();
            let loader_list_url = format!("https://meta.fabricmc.net/v2/versions/loader/{}", mc_ver);
            if let Ok(resp) = client.get(&loader_list_url).send().await {
                if let Ok(loaders_json) = resp.json::<serde_json::Value>().await {
                    let loader_ver = loaders_json
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|item| item["loader"]["version"].as_str())
                        .unwrap_or("0.16.10")
                        .to_string();

                    let profile_url = format!(
                        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
                        mc_ver, loader_ver
                    );
                    if let Ok(p_resp) = client.get(&profile_url).send().await {
                        if let Ok(fabric_json) = p_resp.json::<serde_json::Value>().await {
                            let libs_dir = work_dir.join("libraries");
                            let mut seen: HashSet<String> = classpath_jars
                                .iter()
                                .map(|p| p.to_string_lossy().to_lowercase())
                                .collect();
                            let mut fabric_downloads: Vec<(String, PathBuf)> = Vec::new();

                            collect_libraries_from_json(
                                &fabric_json,
                                &libs_dir,
                                "windows",
                                &mut classpath_jars,
                                &mut seen,
                                &mut fabric_downloads,
                            );

                            for (url, dest) in fabric_downloads {
                                if let Some(p) = dest.parent() {
                                    let _ = fs::create_dir_all(p);
                                }
                                if let Ok(dl) = client.get(&url).send().await {
                                    if let Ok(bytes) = dl.bytes().await {
                                        let _ = fs::write(&dest, &bytes);
                                    }
                                }
                            }

                            if let Some(mc_cls) = fabric_json["mainClass"].as_str() {
                                main_class = mc_cls.to_string();
                            }
                        }
                    }
                }
            }

            if config.selected_version.contains("iris") {
                let _ = ensure_iris_and_sodium_mods(&app_clone, &work_dir, &mc_ver, "fabric").await;
            }
        }

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

        let natives_str = natives_dir.to_string_lossy().to_string();

        let mut args: Vec<String> = Vec::new();
        args.push("-Xms2G".to_string());
        args.push(format!("-Xmx{}M", config.ram));
        args.push("-Xss1M".to_string());
        args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".to_string());
        args.push("-Dfile.encoding=UTF-8".to_string());
        args.push("-Dsun.stdout.encoding=UTF-8".to_string());
        args.push("-Dsun.stderr.encoding=UTF-8".to_string());
        args.push("-Dauthlib.userApiService.enabled=false".to_string());
        args.push("-Dminecraft.api.env=custom".to_string());
        if required_java >= 25 {
            args.push("--enable-native-access=ALL-UNNAMED".to_string());
            args.push("--sun-misc-unsafe-memory-access=allow".to_string());
        }
        args.push(format!("-Djava.library.path={}", natives_str));
        args.push(format!("-Djna.tmpdir={}", natives_str));
        args.push(format!("-Dio.netty.native.workdir={}", natives_str));
        args.push("-Dminecraft.launcher.brand=FenixCraft".to_string());
        args.push("-Dminecraft.launcher.version=1.0".to_string());

        for custom_arg in config.jvm_args.split_whitespace() {
            if !custom_arg.starts_with("-Xmx") && !custom_arg.starts_with("-Xms") {
                args.push(custom_arg.to_string());
            }
        }

        args.push("-cp".to_string());
        args.push(cp_string);
        args.push(main_class);

        let dummy_jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJCbG9vZFJheVQiLCJuYW1lIjoiQmxvb2RSYXlUIn0.signature";

        args.extend_from_slice(&[
            "--username".to_string(),
            config.username.clone(),
            "--version".to_string(),
            mc_ver.clone(),
            "--gameDir".to_string(),
            work_dir.to_string_lossy().to_string(),
            "--assetsDir".to_string(),
            work_dir.join("assets").to_string_lossy().to_string(),
            "--assetIndex".to_string(),
            asset_id,
            "--uuid".to_string(),
            offline_uuid,
            "--accessToken".to_string(),
            dummy_jwt.to_string(),
            "--clientId".to_string(),
            "0".to_string(),
            "--xuid".to_string(),
            "0".to_string(),
            "--userType".to_string(),
            "mojang".to_string(),
            "--versionType".to_string(),
            "release".to_string(),
            "--width".to_string(),
            config.width.to_string(),
            "--height".to_string(),
            config.height.to_string(),
        ]);

        let _ = app_clone.emit(
            "game:log",
            format!(
                "[FenixCraft v1.0] Старт Minecraft {} (Рекомендованная Java {}: {})",
                mc_ver, required_java, java_bin
            ),
        );

        spawn_and_stream_jvm(&app_clone, &java_bin, &args, &work_dir);
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
            check_twitch_live,
            verify_assets_command,
            save_debug_log,
            check_launcher_update,
            install_launcher_update,
            mojang_authenticate,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("Ошибка при запуске Tauri v2 лаунчера FenixCraft");
}
