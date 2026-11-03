use fenix_core::config::{config_path, game_dir, load_config};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const PING_TIMEOUT: Duration = Duration::from_millis(2000);
const VK_POST_COUNT: usize = 6;

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

#[tauri::command]
pub async fn check_twitch_live() -> serde_json::Value {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_default();
    let body = serde_json::json!({
        "query": "query { user(login: \"bloodrayt\") { stream { id type } } }"
    });
    let response = client
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", "kimne78kx3ncx6brgo4mv6wki5h1ko")
        .json(&body)
        .send()
        .await;
    if let Ok(response) = response {
        if let Ok(json) = response.json::<serde_json::Value>().await {
            let live = json["data"]["user"]["stream"]["type"].as_str() == Some("live");
            return serde_json::json!({ "live": live });
        }
    }
    serde_json::json!({ "live": false })
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

#[tauri::command]
pub async fn get_vk_news() -> serde_json::Value {
    let config = load_config(&config_path(&game_dir()));
    let token = if config.vk_token.trim().is_empty() {
        std::env::var("VK_TOKEN").unwrap_or_default()
    } else {
        config.vk_token.trim().to_string()
    };
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    if !token.is_empty() {
        let url = format!(
            "https://api.vk.ru/method/wall.get?domain=fenixcraft&count={VK_POST_COUNT}&access_token={token}&v=5.199"
        );
        if let Ok(response) = client.get(&url).send().await {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                if json["response"]["items"]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
                {
                    return serde_json::json!({
                        "success": true,
                        "source": "api",
                        "data": json["response"]["items"]
                    });
                }
            }
        }
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
