use serde::Serialize;
use serde_json::{json, Value};

pub const ELY_AUTH_URL: &str = "https://authserver.ely.by/auth/authenticate";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthResult {
    pub success: bool,
    pub username: String,
    pub uuid: String,
    pub token: String,
    pub error: String,
}

impl AuthResult {
    fn failed(error: impl Into<String>) -> Self {
        Self {
            success: false,
            username: String::new(),
            uuid: String::new(),
            token: String::new(),
            error: error.into(),
        }
    }
}

/// Успешным считается только ответ с настоящей сессией и профилем игрока.
fn parse_session(body: &Value) -> AuthResult {
    let session = (
        body["selectedProfile"]["name"].as_str(),
        body["selectedProfile"]["id"].as_str(),
        body["accessToken"].as_str(),
    );
    match session {
        (Some(username), Some(uuid), Some(token))
            if !username.is_empty() && !uuid.is_empty() && !token.is_empty() =>
        {
            AuthResult {
                success: true,
                username: username.to_string(),
                uuid: uuid.to_string(),
                token: token.to_string(),
                error: String::new(),
            }
        }
        _ => AuthResult::failed("Ely.by не вернул профиль игрока для этого аккаунта."),
    }
}

pub async fn authenticate(
    client: &reqwest::Client,
    auth_url: &str,
    username: &str,
    password: &str,
) -> AuthResult {
    let payload = json!({
        "agent": { "name": "Minecraft", "version": 1 },
        "username": username.trim(),
        "password": password,
        "requestUser": true
    });
    let response = match client.post(auth_url).json(&payload).send().await {
        Ok(response) => response,
        Err(_) => return AuthResult::failed("Нет связи с Ely.by. Проверьте интернет."),
    };
    if response.status().is_client_error() {
        return AuthResult::failed("Неверный логин или пароль Ely.by.");
    }
    if !response.status().is_success() {
        return AuthResult::failed("Сервис Ely.by сейчас недоступен. Попробуйте позже.");
    }
    match response.json::<Value>().await {
        Ok(body) => parse_session(&body),
        Err(_) => AuthResult::failed("Ely.by вернул некорректный ответ."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_session_is_accepted() {
        let body = json!({
            "accessToken": "token",
            "selectedProfile": { "name": "Fenix", "id": "abc" }
        });

        let result = parse_session(&body);

        assert!(result.success);
        assert_eq!(result.username, "Fenix");
        assert_eq!(result.token, "token");
    }

    #[test]
    fn incomplete_response_never_counts_as_success() {
        for body in [
            json!({}),
            json!({ "accessToken": "token" }),
            json!({ "selectedProfile": { "name": "Fenix", "id": "abc" } }),
            json!({ "accessToken": "", "selectedProfile": { "name": "Fenix", "id": "abc" } }),
        ] {
            let result = parse_session(&body);
            assert!(!result.success, "accepted {body}");
            assert!(result.token.is_empty());
            assert!(!result.error.is_empty());
        }
    }
}
