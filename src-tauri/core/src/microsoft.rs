//! Вход через Microsoft по коду устройства и получение сессии Minecraft.
//! https://minecraft.wiki/w/Microsoft_authentication

use crate::errors::LauncherError;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const SCOPE: &str = "XboxLive.signin offline_access";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// Client ID публичного приложения Azure задаётся при сборке и не является секретом.
const EMBEDDED_CLIENT_ID: Option<&str> = option_env!("FENIX_MS_CLIENT_ID");

pub fn embedded_client_id() -> Option<String> {
    EMBEDDED_CLIENT_ID
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// Адреса сервисов входа. В тестах подменяются локальным сервером.
#[derive(Clone, Debug)]
pub struct MsEndpoints {
    pub authority: String,
    pub xbox_user: String,
    pub xsts: String,
    pub minecraft: String,
}

impl Default for MsEndpoints {
    fn default() -> Self {
        Self {
            authority: "https://login.microsoftonline.com/consumers/oauth2/v2.0".to_string(),
            xbox_user: "https://user.auth.xboxlive.com/user/authenticate".to_string(),
            xsts: "https://xsts.auth.xboxlive.com/xsts/authorize".to_string(),
            minecraft: "https://api.minecraftservices.com".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceLogin {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: Duration,
    pub expires_in: Duration,
}

#[derive(Clone, PartialEq, Eq)]
pub struct MicrosoftTokens {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct MinecraftAccount {
    pub username: String,
    pub uuid: String,
    #[serde(skip)]
    pub access_token: String,
    #[serde(skip)]
    pub xuid: String,
}

fn no_connection() -> LauncherError {
    LauncherError::NeedsInternet {
        what: "вход Microsoft".to_string(),
    }
}

fn rejected(reason: impl Into<String>) -> LauncherError {
    LauncherError::Microsoft(reason.into())
}

pub async fn request_device_code(
    client: &reqwest::Client,
    endpoints: &MsEndpoints,
    client_id: &str,
) -> Result<DeviceLogin, LauncherError> {
    let response = client
        .post(format!("{}/devicecode", endpoints.authority))
        .form(&[("client_id", client_id), ("scope", SCOPE)])
        .send()
        .await
        .map_err(|_| no_connection())?;
    if !response.status().is_success() {
        return Err(rejected("Microsoft отклонил запрос входа"));
    }
    let body: Value = response.json().await.map_err(|_| no_connection())?;
    let text = |name: &str| {
        body[name]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| rejected("Microsoft вернул неполный ответ"))
    };
    Ok(DeviceLogin {
        device_code: text("device_code")?,
        user_code: text("user_code")?,
        verification_uri: text("verification_uri")?,
        interval: Duration::from_secs(body["interval"].as_u64().unwrap_or(5)),
        expires_in: Duration::from_secs(body["expires_in"].as_u64().unwrap_or(900)),
    })
}

enum TokenPoll {
    Pending,
    SlowDown,
    Ready(MicrosoftTokens),
    Failed(LauncherError),
}

fn classify_token_response(body: &Value) -> TokenPoll {
    if let (Some(access), Some(refresh)) = (
        body["access_token"].as_str(),
        body["refresh_token"].as_str(),
    ) {
        return TokenPoll::Ready(MicrosoftTokens {
            access_token: access.to_string(),
            refresh_token: refresh.to_string(),
        });
    }
    match body["error"].as_str() {
        Some("authorization_pending") => TokenPoll::Pending,
        Some("slow_down") => TokenPoll::SlowDown,
        Some("authorization_declined") => TokenPoll::Failed(rejected("Вход был отклонён")),
        Some("expired_token") | Some("code_expired") => {
            TokenPoll::Failed(rejected("Время действия кода истекло, начните вход заново"))
        }
        Some("invalid_grant") => TokenPoll::Failed(LauncherError::MicrosoftSessionExpired),
        _ => TokenPoll::Failed(rejected("Microsoft вернул неожиданный ответ")),
    }
}

/// Ждёт, пока игрок подтвердит вход в браузере. `None` означает, что вход отменён.
pub async fn poll_device_login(
    client: &reqwest::Client,
    endpoints: &MsEndpoints,
    client_id: &str,
    device: &DeviceLogin,
    keep_going: impl Fn() -> bool,
) -> Result<Option<MicrosoftTokens>, LauncherError> {
    let deadline = Instant::now() + device.expires_in;
    let mut interval = device.interval;
    loop {
        if !keep_going() {
            return Ok(None);
        }
        if Instant::now() >= deadline {
            return Err(rejected("Время действия кода истекло, начните вход заново"));
        }
        let response = client
            .post(format!("{}/token", endpoints.authority))
            .form(&[
                ("grant_type", DEVICE_GRANT),
                ("client_id", client_id),
                ("device_code", &device.device_code),
            ])
            .send()
            .await;
        // Короткий сбой сети не должен прерывать вход, пока код действителен.
        if let Ok(response) = response {
            if let Ok(body) = response.json::<Value>().await {
                match classify_token_response(&body) {
                    TokenPoll::Ready(tokens) => return Ok(Some(tokens)),
                    TokenPoll::Failed(error) => return Err(error),
                    TokenPoll::SlowDown => interval += Duration::from_secs(5),
                    TokenPoll::Pending => {}
                }
            }
        }
        tokio::time::sleep(interval).await;
    }
}

pub async fn refresh_microsoft_tokens(
    client: &reqwest::Client,
    endpoints: &MsEndpoints,
    client_id: &str,
    refresh_token: &str,
) -> Result<MicrosoftTokens, LauncherError> {
    let response = client
        .post(format!("{}/token", endpoints.authority))
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("scope", SCOPE),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|_| no_connection())?;
    let body: Value = response.json().await.map_err(|_| no_connection())?;
    match classify_token_response(&body) {
        TokenPoll::Ready(tokens) => Ok(tokens),
        TokenPoll::Failed(error) => Err(error),
        TokenPoll::Pending | TokenPoll::SlowDown => Err(LauncherError::MicrosoftSessionExpired),
    }
}

fn xsts_error_message(code: u64) -> &'static str {
    match code {
        2148916227 => "Аккаунт Xbox заблокирован",
        2148916233 => "У этого аккаунта Microsoft нет профиля Xbox. Создайте его на xbox.com",
        2148916235 => "Xbox Live недоступен в стране этого аккаунта",
        2148916236 | 2148916237 => "Аккаунту нужно подтвердить возраст на сайте Xbox",
        2148916238 => "Детский аккаунт нужно добавить в семейную группу Microsoft",
        _ => "Xbox Live отклонил вход",
    }
}

fn xbox_token_and_hash(body: &Value) -> Option<(String, String, String)> {
    let claims = body["DisplayClaims"]["xui"].as_array()?.first()?;
    Some((
        body["Token"].as_str()?.to_string(),
        claims["uhs"].as_str()?.to_string(),
        claims["xid"].as_str().unwrap_or("0").to_string(),
    ))
}

async fn post_xbox(
    client: &reqwest::Client,
    url: &str,
    payload: &Value,
) -> Result<(String, String, String), LauncherError> {
    let response = client
        .post(url)
        .header("Accept", "application/json")
        .json(payload)
        .send()
        .await
        .map_err(|_| no_connection())?;
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if let Some(code) = body["XErr"].as_u64() {
        return Err(rejected(xsts_error_message(code)));
    }
    if !status.is_success() {
        return Err(rejected("Xbox Live отклонил вход"));
    }
    xbox_token_and_hash(&body).ok_or_else(|| rejected("Xbox Live вернул неполный ответ"))
}

/// Обменивает токен Microsoft на сессию Minecraft и проверяет, что у аккаунта есть игра.
pub async fn sign_in_to_minecraft(
    client: &reqwest::Client,
    endpoints: &MsEndpoints,
    microsoft_access_token: &str,
) -> Result<MinecraftAccount, LauncherError> {
    let (xbl_token, _, _) = post_xbox(
        client,
        &endpoints.xbox_user,
        &json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={microsoft_access_token}")
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )
    .await?;
    let (xsts_token, user_hash, xuid) = post_xbox(
        client,
        &endpoints.xsts,
        &json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )
    .await?;

    let login = client
        .post(format!(
            "{}/authentication/login_with_xbox",
            endpoints.minecraft
        ))
        .json(&json!({ "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}") }))
        .send()
        .await
        .map_err(|_| no_connection())?;
    if login.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(rejected(
            "Приложение лаунчера ещё не одобрено Mojang для входа",
        ));
    }
    if !login.status().is_success() {
        return Err(rejected("Сервис Minecraft отклонил вход"));
    }
    let login: Value = login.json().await.map_err(|_| no_connection())?;
    let access_token = login["access_token"]
        .as_str()
        .ok_or_else(|| rejected("Сервис Minecraft вернул неполный ответ"))?
        .to_string();

    let profile = client
        .get(format!("{}/minecraft/profile", endpoints.minecraft))
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|_| no_connection())?;
    if profile.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(LauncherError::MicrosoftNoGame);
    }
    if !profile.status().is_success() {
        return Err(rejected("Не удалось получить профиль Minecraft"));
    }
    let profile: Value = profile.json().await.map_err(|_| no_connection())?;
    match (profile["name"].as_str(), profile["id"].as_str()) {
        (Some(username), Some(uuid)) => Ok(MinecraftAccount {
            username: username.to_string(),
            uuid: uuid.to_string(),
            access_token,
            xuid,
        }),
        _ => Err(LauncherError::MicrosoftNoGame),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_responses_are_classified() {
        assert!(matches!(
            classify_token_response(&json!({ "error": "authorization_pending" })),
            TokenPoll::Pending
        ));
        assert!(matches!(
            classify_token_response(&json!({ "error": "slow_down" })),
            TokenPoll::SlowDown
        ));
        assert!(matches!(
            classify_token_response(&json!({ "error": "invalid_grant" })),
            TokenPoll::Failed(LauncherError::MicrosoftSessionExpired)
        ));
        assert!(matches!(
            classify_token_response(&json!({ "error": "authorization_declined" })),
            TokenPoll::Failed(LauncherError::Microsoft(_))
        ));
        assert!(matches!(
            classify_token_response(&json!({ "access_token": "a", "refresh_token": "r" })),
            TokenPoll::Ready(_)
        ));
        assert!(matches!(
            classify_token_response(&json!({ "access_token": "a" })),
            TokenPoll::Failed(_)
        ));
    }

    #[test]
    fn xbox_errors_have_readable_messages() {
        assert!(xsts_error_message(2148916233).contains("Xbox"));
        assert!(xsts_error_message(2148916238).contains("Детский"));
        assert_eq!(xsts_error_message(1), "Xbox Live отклонил вход");
    }
}
