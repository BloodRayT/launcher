use fenix_core::config::{config_path, game_dir, load_config};
use fenix_core::download::{probe_client, read_limited};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const PING_TIMEOUT: Duration = Duration::from_millis(2000);
const VK_POST_COUNT: usize = 6;
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

/// Статус стрима приходит с бекенда, потому что данные приложения Twitch хранятся на сервере.
#[tauri::command]
pub async fn check_twitch_live() -> serde_json::Value {
    let Ok(client) = probe_client() else {
        return serde_json::json!({ "live": false });
    };
    let live = fetch_backend_json(&client, "twitch")
        .await
        .is_some_and(|body| body["live"].as_bool() == Some(true));
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

fn strip_html_tags(input: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for character in input.replace("<br>", "\n").replace("<br/>", "\n").chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            other if !in_tag => output.push(other),
            _ => {}
        }
    }
    output
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&#33;", "!")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

fn find_image_url(html: &str) -> String {
    let area = &html[..html.floor_char_boundary(html.len().min(4500))];
    for prefix in ["https://sun", "https://pp.userapi.com"] {
        if let Some(position) = area.find(prefix) {
            let rest = &area[position..];
            let end = rest.find(['"', '\'', ')', ' ', '&']).unwrap_or(rest.len());
            if end > 20 {
                return rest[..end].to_string();
            }
        }
    }
    String::new()
}

fn parse_wall_html(html: &str) -> Vec<serde_json::Value> {
    let marker = if html.contains("wall_post_text") {
        "wall_post_text"
    } else {
        "pi_text"
    };
    html.split(marker)
        .skip(1)
        .take(VK_POST_COUNT)
        .enumerate()
        .filter_map(|(index, block)| {
            let rest = &block[block.find('>')? + 1..];
            let text = strip_html_tags(&rest[..rest.find("</div>")?]);
            if text.chars().count() < 5 {
                return None;
            }
            let title: String = text
                .lines()
                .next()
                .unwrap_or("Новость FenixCraft")
                .chars()
                .take(55)
                .collect();
            Some(serde_json::json!({
                "id": index + 1,
                "tag": "VK.RU/FENIXCRAFT",
                "title": title,
                "text": text,
                "date": "Стена сообщества ВК",
                "image": find_image_url(rest),
                "url": "https://vk.ru/fenixcraft"
            }))
        })
        .collect()
}

async fn news_from_backend(client: &reqwest::Client) -> Option<serde_json::Value> {
    let body = fetch_backend_json(client, "news").await?;
    (body["success"].as_bool() == Some(true)).then_some(body)
}

/// Основной источник новостей — бекенд, где хранится токен VK.
/// Если он недоступен, лаунчер читает открытую стену сообщества без токена.
#[tauri::command]
pub async fn get_vk_news() -> serde_json::Value {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    if let Some(news) = news_from_backend(&client).await {
        return news;
    }

    for url in ["https://m.vk.ru/fenixcraft", "https://vk.ru/fenixcraft"] {
        let Ok(response) = client
            .get(url)
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .send()
            .await
        else {
            continue;
        };
        let Ok(html) = response.text().await else {
            continue;
        };
        let posts = parse_wall_html(&html);
        if !posts.is_empty() {
            return serde_json::json!({ "success": true, "source": "public_wall", "posts": posts });
        }
    }
    serde_json::json!({ "success": false })
}
