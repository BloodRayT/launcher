//! Окно «Выбрать скин»: каталог с сервера скинов и сохранение выбора игрока.
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use fenix_core::config::{config_path, game_dir, load_config, save_config, LauncherConfig};
use fenix_core::download::{probe_client, read_limited};
use fenix_core::skin_loader::new_token;
use serde::Serialize;
use serde_json::{json, Value};

const MAX_CATALOG_BYTES: usize = 512 * 1024;
const MAX_TEXTURE_BYTES: usize = 256 * 1024;
const MAX_CATALOG_ITEMS: usize = 200;
const ELY_SKINSYSTEM: &str = "http://skinsystem.ely.by";
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const UNREACHABLE: &str = "Сервер скинов недоступен. Проверьте интернет и попробуйте позже.";

fn current_config() -> LauncherConfig {
    load_config(&config_path(&game_dir()))
}

fn skins_url(config: &LauncherConfig, endpoint: &str) -> String {
    format!(
        "{}/skins/{endpoint}",
        config.backend_url.trim_end_matches('/')
    )
}

/// Ник, под которым игрок зайдёт в игру: у Ely.by и лицензии он из профиля аккаунта.
fn player_name(config: &LauncherConfig) -> &str {
    match config.account_type.as_str() {
        "ely" => &config.ely_username,
        "microsoft" => &config.ms_username,
        _ => &config.username,
    }
}

/// Скин и плащ игрока на Ely.by. Картинки отдаются как data URL: 3D-модели не нужен доступ к чужому сайту.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElyTextures {
    username: String,
    skin: Option<String>,
    cape: Option<String>,
}

async fn texture_data_url(client: &reqwest::Client, url: &str) -> Option<String> {
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = read_limited(response, MAX_TEXTURE_BYTES).await.ok()?;
    bytes
        .starts_with(PNG_SIGNATURE)
        .then(|| format!("data:image/png;base64,{}", BASE64.encode(&bytes)))
}

#[tauri::command]
pub async fn get_ely_textures() -> Result<ElyTextures, String> {
    let config = current_config();
    let username = config.ely_username.clone();
    let valid = config.account_type == "ely"
        && !username.is_empty()
        && username
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
    if !valid {
        return Err("Войдите через Ely.by в блоке профиля, чтобы видеть скин с сайта.".to_string());
    }
    let client = probe_client().map_err(|error| error.to_string())?;
    // Адреса /skins/{ник}.png у Ely.by отвечают устаревшим перенаправлением и отстают от профиля,
    // поэтому текстуры берутся из /textures/{ник} — тех же данных, что видит игра.
    let response = client
        .get(format!("{ELY_SKINSYSTEM}/textures/{username}"))
        .send()
        .await
        .map_err(|_| "Ely.by недоступен. Проверьте интернет и попробуйте позже.".to_string())?;
    let textures: Value = match response.status().as_u16() {
        200 => response.json().await.unwrap_or(Value::Null),
        // 204: у аккаунта нет ни скина, ни плаща.
        _ => Value::Null,
    };
    let (skin_url, cape_url) = ely_texture_urls(&textures);
    let fetch = |url: Option<String>| {
        let client = client.clone();
        async move {
            match url {
                Some(url) => texture_data_url(&client, &url).await,
                None => None,
            }
        }
    };
    let (skin, cape) = tokio::join!(fetch(skin_url), fetch(cape_url));
    Ok(ElyTextures {
        username,
        skin,
        cape,
    })
}

/// Адреса скина и плаща из ответа `/textures/{ник}`. Принимаются только файлы самого Ely.by.
fn ely_texture_urls(textures: &Value) -> (Option<String>, Option<String>) {
    let url_of = |key: &str| {
        let url = textures[key]["url"].as_str()?;
        let parsed = reqwest::Url::parse(url).ok()?;
        let host = parsed.host_str()?;
        let from_ely = host == "ely.by" || host.ends_with(".ely.by");
        (from_ely && matches!(parsed.scheme(), "http" | "https")).then(|| url.to_string())
    };
    (url_of("SKIN"), url_of("CAPE"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ely_textures_come_from_the_profile_data() {
        let textures = json!({
            "SKIN": { "url": "http://ely.by/storage/skins/037f47a8.png", "metadata": { "model": "slim" } },
            "CAPE": { "url": "http://ely.by/storage/cloaks/abc.png" }
        });

        assert_eq!(
            ely_texture_urls(&textures),
            (
                Some("http://ely.by/storage/skins/037f47a8.png".to_string()),
                Some("http://ely.by/storage/cloaks/abc.png".to_string())
            )
        );
    }

    #[test]
    fn missing_or_foreign_textures_are_ignored() {
        assert_eq!(ely_texture_urls(&Value::Null), (None, None));
        let foreign = json!({ "SKIN": { "url": "http://evil.example/ely.by.png" } });
        assert_eq!(ely_texture_urls(&foreign), (None, None));
        let only_skin = json!({ "SKIN": { "url": "https://ely.by/storage/skins/a.png" } });
        assert_eq!(
            ely_texture_urls(&only_skin),
            (Some("https://ely.by/storage/skins/a.png".to_string()), None)
        );
    }
}

#[tauri::command]
pub async fn get_skin_catalog() -> Result<Value, String> {
    let config = current_config();
    let client = probe_client().map_err(|error| error.to_string())?;
    let response = client
        .get(skins_url(&config, "catalog"))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| UNREACHABLE.to_string())?;
    let body = read_limited(response, MAX_CATALOG_BYTES).await?;
    let catalog: Value = serde_json::from_slice(&body)
        .map_err(|_| "Сервер скинов ответил непонятно.".to_string())?;
    Ok(inline_textures(&client, &catalog).await)
}

/// Картинки каталога передаются в интерфейс как data URL: страница лаунчера открыта по HTTPS,
/// и WebView2 не загрузил бы картинки с HTTP-сайта. Ответ всегда вида `{skins: [], capes: []}`.
async fn inline_textures(client: &reqwest::Client, catalog: &Value) -> Value {
    let mut result = serde_json::Map::new();
    for kind in ["skins", "capes"] {
        let items: Vec<Value> = catalog[kind]
            .as_array()
            .into_iter()
            .flatten()
            .take(MAX_CATALOG_ITEMS)
            .cloned()
            .collect();
        let mut downloads = tokio::task::JoinSet::new();
        for (index, item) in items.into_iter().enumerate() {
            let client = client.clone();
            downloads.spawn(async move {
                let url = item["url"].as_str().unwrap_or_default().to_string();
                let is_web = url.starts_with("http://") || url.starts_with("https://");
                let texture = if is_web {
                    texture_data_url(&client, &url).await
                } else {
                    None
                };
                (index, item, texture)
            });
        }
        let mut loaded = Vec::new();
        while let Some(Ok(entry)) = downloads.join_next().await {
            loaded.push(entry);
        }
        loaded.sort_by_key(|(index, ..)| *index);
        // Скин без картинки игроку не нужен: в окне его не показать.
        let list = loaded
            .into_iter()
            .filter_map(|(_, mut item, texture)| {
                item["url"] = Value::String(texture?);
                Some(item)
            })
            .collect();
        result.insert(kind.to_string(), Value::Array(list));
    }
    Value::Object(result)
}

fn select_error(status: u16) -> String {
    match status {
        403 => "Этот ник уже закреплён за другим лаунчером. Выберите другой ник.".to_string(),
        404 => "Этого скина или плаща больше нет на сервере. Обновите список.".to_string(),
        400 => "Укажите ник от 3 до 16 символов: латиница, цифры, _ и -.".to_string(),
        429 => "Слишком часто. Подождите минуту и попробуйте снова.".to_string(),
        507 => "Сервер скинов переполнен. Сообщите администрации.".to_string(),
        _ => format!("Сервер скинов ответил ошибкой {status}."),
    }
}

/// Сохраняет выбор на сервере, откуда его берёт мод в игре, и в настройках лаунчера.
#[tauri::command(rename_all = "camelCase")]
pub async fn select_skin(
    skin: Option<String>,
    cape: Option<String>,
) -> Result<LauncherConfig, String> {
    let mut config = current_config();
    if config.skin_token.is_empty() {
        config.skin_token = new_token();
    }
    let body = json!({
        "username": player_name(&config),
        "token": config.skin_token,
        "skin": skin,
        "cape": cape,
    });
    let client = probe_client().map_err(|error| error.to_string())?;
    let response = client
        .post(skins_url(&config, "select"))
        .json(&body)
        .send()
        .await
        .map_err(|_| UNREACHABLE.to_string())?;
    if !response.status().is_success() {
        return Err(select_error(response.status().as_u16()));
    }
    config.skin_choice = skin.unwrap_or_default();
    config.cape_choice = cape.unwrap_or_default();
    save_config(&config_path(&game_dir()), config).map_err(|error| error.to_string())
}
