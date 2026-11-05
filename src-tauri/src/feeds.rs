use fenix_core::config::{config_path, game_dir, load_config};
use fenix_core::download::{probe_client, read_limited};
use fenix_core::network::{is_twitch_live, twitch_preview_url};
use fenix_core::news::{fetch_community_news, COMMUNITY_WIDGET_URL};
use fenix_core::updater::{fetch_release_notes, ReleaseNote};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const PING_TIMEOUT: Duration = Duration::from_millis(2000);
const TWITCH_CHANNEL: &str = "bloodrayt";
const MAX_FEED_BYTES: usize = 1024 * 1024;

fn current_backend_url() -> String {
    load_config(&config_path(&game_dir())).backend_url
}

#[derive(Deserialize)]
pub struct ServerItem {
    id: String,
    ip: String,
    port: u16,
    #[serde(rename = "maxDefault")]
    max_default: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    online: bool,
    players: u32,
    max_players: u32,
}

fn ping_minecraft_server(ip: &str, port: u16, fallback_max: u32) -> ServerStatus {
    let offline = ServerStatus {
        online: false,
        players: 0,
        max_players: fallback_max,
    };
    let Some(address) = (ip, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut all| all.next())
    else {
        return offline;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&address, PING_TIMEOUT) else {
        return offline;
    };
    let _ = stream.set_read_timeout(Some(PING_TIMEOUT));
    let _ = stream.set_write_timeout(Some(PING_TIMEOUT));

    let mut buffer = [0u8; 512];
    if stream.write_all(&[0xFE, 0x01]).is_ok() {
        if let Ok(read) = stream.read(&mut buffer) {
            if read > 3 && buffer[0] == 0xFF {
                let (pairs, _) = buffer[3..read].as_chunks::<2>();
                let utf16: Vec<u16> = pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
                let text = String::from_utf16_lossy(&utf16);
                let parts: Vec<&str> = text.split('\0').collect();
                if parts.len() >= 6 {
                    return ServerStatus {
                        online: true,
                        players: parts[4].parse().unwrap_or(0),
                        max_players: parts[5].parse().unwrap_or(fallback_max),
                    };
                }
            }
        }
    }
    ServerStatus {
        online: true,
        players: 0,
        max_players: fallback_max,
    }
}

#[tauri::command]
pub async fn get_servers_status(servers: Vec<ServerItem>) -> HashMap<String, ServerStatus> {
    let pings: Vec<_> = servers
        .into_iter()
        .map(|server| {
            tauri::async_runtime::spawn_blocking(move || {
                let status = ping_minecraft_server(&server.ip, server.port, server.max_default);
                (server.id, status)
            })
        })
        .collect();
    let mut statuses = HashMap::new();
    for ping in pings {
        if let Ok((id, status)) = ping.await {
            statuses.insert(id, status);
        }
    }
    statuses
}

/// Статус эфира определяется без ключей, поэтому не зависит от бекенда.
#[tauri::command]
pub async fn check_twitch_live() -> serde_json::Value {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(8))
        .build();
    let live = match client {
        Ok(client) => is_twitch_live(&client, &twitch_preview_url(TWITCH_CHANNEL)).await,
        Err(_) => false,
    };
    serde_json::json!({ "live": live })
}

async fn fetch_backend_json(client: &reqwest::Client, endpoint: &str) -> Option<serde_json::Value> {
    let url = format!("{}/{endpoint}", current_backend_url().trim_end_matches('/'));
    let response = client
        .get(&url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let body = read_limited(response, MAX_FEED_BYTES).await.ok()?;
    serde_json::from_slice(&body).ok()
}

/// Основной источник новостей — публичный виджет сообщества, он работает без бекенда и ключей.
/// Если VK его не отдал, используется бекенд.
#[tauri::command]
pub async fn get_vk_news() -> serde_json::Value {
    let Ok(client) = probe_client() else {
        return serde_json::json!({ "success": false });
    };
    if let Ok(posts) = fetch_community_news(&client, COMMUNITY_WIDGET_URL).await {
        if !posts.is_empty() {
            return serde_json::json!({ "success": true, "posts": posts });
        }
    }
    match fetch_backend_json(&client, "news").await {
        Some(body) if body["success"].as_bool() == Some(true) => body,
        _ => serde_json::json!({ "success": false }),
    }
}

#[tauri::command]
pub async fn get_launcher_releases() -> Result<Vec<ReleaseNote>, String> {
    let client = probe_client().map_err(|error| error.to_string())?;
    fetch_release_notes(&client).await
}
