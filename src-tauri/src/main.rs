#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
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
    #[serde(default)]
    vk_token: String,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            username: "BloodRayT".to_string(),
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

// Ищем java.exe (а не javaw.exe), чтобы перехватывать полный лог ошибок и stdout
fn find_java_binary_in_dir(dir: &PathBuf, target_name: &str) -> Option<PathBuf> {
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

// Проверяем, что установленная в системе Java имеет версию >= 21 (иначе Minecraft 1.21+ падает с -1)
fn is_system_java_21_or_newer() -> bool {
    if let Ok(out) = Command::new("java").arg("-version").output() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let combined = format!("{}\n{}", stderr, stdout);
        for ver in 21..=30 {
            let pat1 = format!("version \"{}", ver);
            let pat2 = format!("openjdk {}", ver);
            let pat3 = format!("java {}", ver);
            if combined.contains(&pat1) || combined.contains(&pat2) || combined.contains(&pat3) {
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

    // Используем java.exe вместо javaw.exe, чтобы видеть все логи в консоли лаунчера
    let bin_name = if cfg!(windows) { "java.exe" } else { "java" };
    let runtime_dir = get_game_dir().join("runtime").join("java-21");

    if let Some(found) = find_java_binary_in_dir(&runtime_dir, bin_name) {
        return Ok(found.to_string_lossy().to_string());
    }

    if is_system_java_21_or_newer() {
        return Ok("java".to_string());
    }

    let _ = app.emit(
        "game:log",
        "[Java 21 Auto-Installer] Требуется Java 21+ (в системе не найдена или устарела). Скачиваем Adoptium Java 21...",
    );
    let _ = app.emit(
        "game:progress",
        serde_json::json!({
            "type": "Скачивание Java 21 (Adoptium)",
            "task": 15,
            "total": 100
        }),
    );

    fs::create_dir_all(&runtime_dir).map_err(|e| e.to_string())?;

    let os_name = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };

    let arch_name = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };

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

    let resp = client
        .get(&adoptium_url)
        .send()
        .await
        .map_err(|e| format!("Ошибка загрузки Java 21: {}", e))?;

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    fs::write(&archive_path, &bytes).map_err(|e| e.to_string())?;

    let _ = app.emit(
        "game:progress",
        serde_json::json!({
            "type": "Распаковка Java 21...",
            "task": 75,
            "total": 100
        }),
    );

    let status = Command::new("tar")
        .arg("-xf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&runtime_dir)
        .status()
        .map_err(|e| format!("Ошибка распаковки Java 21: {}", e))?;

    let _ = fs::remove_file(&archive_path);

    if !status.success() {
        return Err("Не удалось распаковать архив Java 21".to_string());
    }

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
        let _ = app.emit("game:log", "[Java 21 Auto-Installer] Портативная Java 21 успешно установлена!");
        return Ok(found.to_string_lossy().to_string());
    }

    Err("Исполняемый файл Java 21 не найден после распаковки".to_string())
}

async fn download_vanilla_from_mojang(
    app: &AppHandle,
    requested_version: &str,
) -> Result<(PathBuf, Vec<PathBuf>, String, String, String), String> {
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

    let ver_json: serde_json::Value = client
        .get(ver_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let main_class = ver_json["mainClass"]
        .as_str()
        .unwrap_or("net.minecraft.client.main.Main")
        .to_string();

    let game_dir = get_game_dir();
    let instance_dir = game_dir.join("instances").join(format!("vanilla-{}", actual_ver));
    let natives_dir = instance_dir.join("natives");
    let libs_dir = game_dir.join("libraries");
    let assets_indexes_dir = game_dir.join("assets").join("indexes");
    let assets_objects_dir = game_dir.join("assets").join("objects");
    fs::create_dir_all(&instance_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&natives_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&libs_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&assets_indexes_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&assets_objects_dir).map_err(|e| e.to_string())?;

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

    Ok((instance_dir, classpath_jars, actual_ver, asset_id, main_class))
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

        let (instance_dir, classpath_jars, mc_ver, asset_id, main_class) =
            match download_vanilla_from_mojang(&app_clone, &config.selected_version).await {
                Ok(res) => res,
                Err(e) => {
                    let _ = app_clone.emit("game:log", format!("[ERROR] Ошибка скачивания клиента: {}", e));
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
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect::<Vec<_>>()
            .join(cp_sep);

        let natives_dir = instance_dir.join("natives").to_string_lossy().replace('\\', "/");

        // Записываем длинный Classpath и JVM флаги в @jvm_args.txt, чтобы обойти лимит длины строки Windows (причина ошибки 4294967295)
        let mut jvm_file_lines: Vec<String> = Vec::new();
        jvm_file_lines.push(format!("-Xmx{}M", config.ram));
        jvm_file_lines.push("-Xms1024M".to_string());
        jvm_file_lines.push(format!("-Djava.library.path=\"{}\"", natives_dir));
        jvm_file_lines.push("-Dminecraft.launcher.brand=FenixCraft".to_string());
        jvm_file_lines.push("-Dminecraft.launcher.version=1.0".to_string());

        for custom_arg in config.jvm_args.split_whitespace() {
            jvm_file_lines.push(custom_arg.to_string());
        }

        jvm_file_lines.push("-cp".to_string());
        jvm_file_lines.push(format!("\"{}\"", cp_string));

        let argfile_path = instance_dir.join("jvm_args.txt");
        if let Err(e) = fs::write(&argfile_path, jvm_file_lines.join("\n")) {
            let _ = app_clone.emit("game:log", format!("[ERROR] Не удалось создать jvm_args.txt: {}", e));
            let _ = app_clone.emit("game:closed", ());
            return;
        }

        let mut cmd_args: Vec<String> = Vec::new();
        cmd_args.push(format!("@{}", argfile_path.to_string_lossy()));
        cmd_args.push(main_class);

        cmd_args.extend_from_slice(&[
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
            "--userType".to_string(),
            "legacy".to_string(),
            "--versionType".to_string(),
            "FenixCraft-1.0".to_string(),
            "--width".to_string(),
            config.width.to_string(),
            "--height".to_string(),
            config.height.to_string(),
            "--quickPlayMultiplayer".to_string(),
            format!("{}:25565", config.selected_server),
        ]);

        let _ = app_clone.emit(
            "game:log",
            format!("[FenixCraft v1.0] Запуск JVM ({}) -> Сервер: {}", java_bin, config.selected_server),
        );

        let mut cmd = Command::new(&java_bin);
        cmd.args(&cmd_args)
            .current_dir(&instance_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW — скрываем черное консольное окно Windows
        }

        match cmd.spawn() {
            Ok(mut proc) => {
                // Читаем stdout и stderr игры в реальном времени в консоль лаунчера
                if let Some(stdout) = proc.stdout.take() {
                    let app_out = app_clone.clone();
                    std::thread::spawn(move || {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().map_while(Result::ok) {
                            let _ = app_out.emit("game:log", format!("[MC] {}", line));
                        }
                    });
                }
                if let Some(stderr) = proc.stderr.take() {
                    let app_err = app_clone.clone();
                    std::thread::spawn(move || {
                        let reader = BufReader::new(stderr);
                        for line in reader.lines().map_while(Result::ok) {
                            let _ = app_err.emit("game:log", format!("[JVM-ERR] {}", line));
                        }
                    });
                }

                let status = proc.wait();
                let _ = app_clone.emit("game:log", format!("[FenixCraft v1.0] Игра закрыта ({:?})", status));
            }
            Err(err) => {
                let _ = app_clone.emit("game:log", format!("[ERROR] Ошибка старта JVM: {}", err));
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
