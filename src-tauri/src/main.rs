#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
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
    #[serde(default)]
    vk_token: String,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            username: "BloodRayT".to_string(),
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
    let mut dir = dirs::config_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    dir.push(".fenixcraft");
    let _ = fs::create_dir_all(&dir);
    dir
}

// Проверяем стандартную папку .minecraft, чтобы мгновенно брать кешированные ассеты оттуда, если они есть
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


// --- ПРОВЕРКА ОНЛАЙНА СТРИМА TWITCH (HTTPS://WWW.TWITCH.TV/BLOODRAYT) ---
#[tauri::command]
async fn check_twitch_live() -> serde_json::Value {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_default();

    // 1. Быстрая проверка через публичный GraphQL Twitch (Client-ID веб-плеера)
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

    // 2. Резервная проверка через страницу канала
    if let Ok(resp) = client.get("https://www.twitch.tv/bloodrayt").send().await {
        if let Ok(text) = resp.text().await {
            let is_live = text.contains("\"isLiveBroadcast\":true");
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

fn is_system_java_21_or_newer() -> bool {
    if let Ok(out) = Command::new("java").arg("-version").output() {
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
        for ver in 21..=30 {
            if combined.contains(&format!("version \"{}", ver))
                || combined.contains(&format!("openjdk {}", ver))
                || combined.contains(&format!("java {}", ver))
            {
                return true;
            }
        }
    }
    false
}

async fn ensure_java_21(app: &AppHandle, custom_path: &str) -> Result<String, String> {
    if !custom_path.trim().is_empty() {
        let p = PathBuf::from(custom_path.trim());
        if p.exists() {
            return Ok(custom_path.trim().to_string());
        }
    }

    let bin_name = if cfg!(windows) { "java.exe" } else { "java" };
    let root_dir = get_game_dir();

    let existing_jdk21 = root_dir.join("java").join("jdk-21").join("bin").join(bin_name);
    if existing_jdk21.exists() {
        return Ok(existing_jdk21.to_string_lossy().to_string());
    }
    if let Some(found) = find_java_binary_in_dir(&root_dir.join("java"), bin_name) {
        return Ok(found.to_string_lossy().to_string());
    }

    let runtime_dir = root_dir.join("runtime").join("java-21");
    if let Some(found) = find_java_binary_in_dir(&runtime_dir, bin_name) {
        return Ok(found.to_string_lossy().to_string());
    }

    if is_system_java_21_or_newer() {
        return Ok("java".to_string());
    }

    let _ = app.emit(
        "game:log",
        "[Java 21 Auto-Installer] Скачиваем портативную Java 21 (Adoptium)...",
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
    let archive_path = runtime_dir.join(format!("adoptium-jre21.{}", ext));

    let adoptium_url = format!(
        "https://api.adoptium.net/v3/binary/latest/21/ga/{}/{}/jre/hotspot/normal/eclipse",
        os_name, arch_name
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&adoptium_url).send().await.map_err(|e| e.to_string())?;
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    fs::write(&archive_path, &bytes).map_err(|e| e.to_string())?;

    let _ = Command::new("tar")
        .arg("-xf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&runtime_dir)
        .status();
    let _ = fs::remove_file(&archive_path);

    if let Some(found) = find_java_binary_in_dir(&runtime_dir, bin_name) {
        return Ok(found.to_string_lossy().to_string());
    }

    Err("Не удалось найти или распаковать Java 21".to_string())
}

// --- МНОГОПОТОЧНОЕ СКАЧИВАНИЕ ВСЕХ АССЕТОВ (ИКОНОК, ШРИФТОВ, ЗВУКОВ) С CDN MOJANG ---
async fn ensure_all_assets_downloaded(
    app: &AppHandle,
    client: &reqwest::Client,
    root_dir: &PathBuf,
    asset_id: &str,
    asset_url_opt: Option<&str>,
) -> Result<(), String> {
    let indexes_dir = root_dir.join("assets").join("indexes");
    let objects_dir = root_dir.join("assets").join("objects");
    fs::create_dir_all(&indexes_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&objects_dir).map_err(|e| e.to_string())?;

    let idx_path = indexes_dir.join(format!("{}.json", asset_id));
    if !idx_path.exists() {
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
    let mut missing_hashes: Vec<String> = Vec::new();

    if let Some(objects) = idx_json["objects"].as_object() {
        for (_name, obj) in objects {
            if let Some(hash) = obj["hash"].as_str() {
                if hash.len() >= 2 {
                    let prefix = &hash[0..2];
                    let target_file = objects_dir.join(prefix).join(hash);
                    if !target_file.exists() {
                        // Если этот файл уже есть в стандартном .minecraft — копируем его мгновенно без скачивания
                        let fallback_file = official_objects_dir.join(prefix).join(hash);
                        if fallback_file.exists() {
                            if let Some(p) = target_file.parent() {
                                let _ = fs::create_dir_all(p);
                            }
                            if fs::copy(&fallback_file, &target_file).is_ok() {
                                continue;
                            }
                        }
                        missing_hashes.push(hash.to_string());
                    }
                }
            }
        }
    }

    if missing_hashes.is_empty() {
        return Ok(());
    }

    let total = missing_hashes.len();
    let _ = app.emit(
        "game:log",
        format!(
            "[Mojang Assets] Докачиваем {} игровых ресурсов (шрифты, иконки, звуки) в 32 потока...",
            total
        ),
    );

    let sem = Arc::new(tokio::sync::Semaphore::new(32));
    let done_counter = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(total);

    for hash in missing_hashes {
        let permit_pool = sem.clone();
        let client_clone = client.clone();
        let objects_dir_clone = objects_dir.clone();
        let app_clone = app.clone();
        let counter_clone = done_counter.clone();

        let handle = tokio::spawn(async move {
            let _permit = permit_pool.acquire().await;
            let prefix = &hash[0..2];
            let sub_dir = objects_dir_clone.join(prefix);
            let _ = fs::create_dir_all(&sub_dir);
            let dest = sub_dir.join(&hash);
            let url = format!("https://resources.download.minecraft.net/{}/{}", prefix, hash);

            for _attempt in 0..3 {
                if let Ok(resp) = client_clone.get(&url).send().await {
                    if resp.status().is_success() {
                        if let Ok(bytes) = resp.bytes().await {
                            let _ = fs::write(&dest, &bytes);
                            break;
                        }
                    }
                }
            }

            let current = counter_clone.fetch_add(1, Ordering::Relaxed) + 1;
            if current % 25 == 0 || current == total {
                let _ = app_clone.emit(
                    "game:progress",
                    serde_json::json!({
                        "type": "Загрузка игровых ассетов (Mojang)",
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

    let _ = app.emit("game:log", "[Mojang Assets] Все ресурсы успешно загружены!");
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
            if os_match {
                allowed = action == "allow";
            }
        }
        return allowed;
    }
    true
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
                if current_os != "osx" && rel.contains("java-objc-bridge") {
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
    let natives_dir = root_dir.join("natives");
    let assets_dir = root_dir.join("assets");
    let _ = fs::create_dir_all(&natives_dir);

    let neo_str = fs::read_to_string(neo_json_path).map_err(|e| e.to_string())?;
    let neo_json: serde_json::Value = serde_json::from_str(&neo_str).map_err(|e| e.to_string())?;

    let inherits_from = neo_json["inheritsFrom"].as_str().unwrap_or("1.21.1");
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

    let asset_index = van_json["assetIndex"]["id"]
        .as_str()
        .or_else(|| van_json["assets"].as_str())
        .unwrap_or("17")
        .to_string();
    let asset_url = van_json["assetIndex"]["url"].as_str();

    // Гарантируем, что все объекты в assets/objects скачаны перед стартом!
    ensure_all_assets_downloaded(app, &client, &root_dir, &asset_index, asset_url).await?;

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
    args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".to_string());
    args.push(format!("-Djava.library.path={}", natives_str));
    args.push(format!("-Djna.tmpdir={}", natives_str));
    args.push(format!("-Dorg.lwjgl.system.SharedLibraryExtractPath={}", natives_str));
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
) -> Result<(PathBuf, Vec<PathBuf>, String, String, String), String> {
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

    let target_ver = if requested_version.starts_with("latest") || requested_version.starts_with("26.") {
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
    let natives_dir = root_dir.join("natives");
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

    // Скачиваем все недостающие иконки, шрифты, панораму и звуки в assets/objects!
    ensure_all_assets_downloaded(app, &client, &root_dir, &asset_id, asset_url).await?;

    for jar in &classpath_jars {
        let name = jar.file_name().unwrap_or_default().to_string_lossy();
        if name.contains("natives") && jar.exists() {
            let _ = Command::new("tar")
                .arg("-xf")
                .arg(jar)
                .arg("-C")
                .arg(&natives_dir)
                .status();
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

    Ok((root_dir, classpath_jars, actual_ver, asset_id, main_class))
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
                        let _ = app_out.emit("game:log", format!("[MC] {}", line));
                    }
                });
            }
            if let Some(stderr) = proc.stderr.take() {
                let app_err = app.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines().map_while(Result::ok) {
                        let _ = app_err.emit("game:log", format!("[JVM] {}", line));
                    }
                });
            }

            let status = proc.wait();
            let _ = app.emit("game:log", format!("[FenixCraft v1.0] Игра закрыта ({:?})", status));
        }
        Err(err) => {
            let _ = app.emit("game:log", format!("[ERROR] Ошибка старта JVM: {}", err));
        }
    }
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

        let java_bin = match ensure_java_21(&app_clone, &config.java_path).await {
            Ok(bin) => bin,
            Err(e) => {
                let _ = app_clone.emit("game:log", format!("[ERROR] Ошибка подготовки Java 21: {}", e));
                let _ = app_clone.emit("game:closed", ());
                return;
            }
        };

        let root_dir = get_game_dir();
        let versions_dir = root_dir.join("versions");

        let is_modded_selected = config.selected_version.contains("neoforge");

        if is_modded_selected {
            if let Some((neo_id, neo_json_path)) = find_installed_neoforge_json(&versions_dir) {
                if let Err(e) = launch_installed_neoforge(&app_clone, &config, &java_bin, &neo_id, &neo_json_path).await {
                    let _ = app_clone.emit("game:log", format!("[ERROR] Ошибка запуска NeoForge: {}", e));
                }
                let _ = app_clone.emit("game:closed", ());
                return;
            }
        }

        let (work_dir, classpath_jars, mc_ver, asset_id, main_class) =
            match download_vanilla_to_root(&app_clone, &config.selected_version).await {
                Ok(res) => res,
                Err(e) => {
                    let _ = app_clone.emit("game:log", format!("[ERROR] Ошибка подготовки Vanilla: {}", e));
                    let _ = app_clone.emit("game:closed", ());
                    return;
                }
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

        let natives_str = work_dir.join("natives").to_string_lossy().to_string();

        let mut args: Vec<String> = Vec::new();
        args.push("-Xms2G".to_string());
        args.push(format!("-Xmx{}M", config.ram));
        args.push(format!("-Djava.library.path={}", natives_str));
        args.push(format!("-Djna.tmpdir={}", natives_str));
        args.push(format!("-Dorg.lwjgl.system.SharedLibraryExtractPath={}", natives_str));
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

        args.extend_from_slice(&[
            "--username".to_string(),
            config.username.clone(),
            "--version".to_string(),
            mc_ver,
            "--gameDir".to_string(),
            work_dir.to_string_lossy().to_string(),
            "--assetsDir".to_string(),
            work_dir.join("assets").to_string_lossy().to_string(),
            "--assetIndex".to_string(),
            asset_id,
            "--uuid".to_string(),
            offline_uuid,
            "--accessToken".to_string(),
            "0".to_string(),
            "--versionType".to_string(),
            "release".to_string(),
            "--width".to_string(),
            config.width.to_string(),
            "--height".to_string(),
            config.height.to_string(),
        ]);

        let _ = app_clone.emit(
            "game:log",
            format!("[FenixCraft v1.0] Старт Vanilla Minecraft (Java: {})", java_bin),
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
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("Ошибка при запуске Tauri v2 лаунчера FenixCraft");
}
