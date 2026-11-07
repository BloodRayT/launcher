//! Вход через Ely.by (протокол Yggdrasil) и authlib-injector, который подключает игру к Ely.by.
//! Пароль отправляется только на сервер Ely.by и нигде не сохраняется.
use crate::download::{ensure_files, DownloadItem, Expected};
use crate::errors::LauncherError;
use crate::launch::LaunchContext;
use crate::safety::is_sha256_hex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const INJECTOR_DIR: &str = "authlib-injector";
const TWO_FACTOR_MARKER: &str = "two factor";

/// Сессия Ely.by: токен доступа привязан к ключу этой установки лаунчера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElySession {
    pub access_token: String,
    pub client_token: String,
    pub uuid: String,
    pub username: String,
}

fn parse_session(body: &Value, client_token: &str) -> Option<ElySession> {
    let profile = &body["selectedProfile"];
    Some(ElySession {
        access_token: body["accessToken"].as_str()?.to_string(),
        client_token: body["clientToken"]
            .as_str()
            .unwrap_or(client_token)
            .to_string(),
        uuid: profile["id"].as_str()?.to_string(),
        username: profile["name"].as_str()?.to_string(),
    })
}

/// Ошибка Ely.by в понятном игроку виде. Двухфакторную защиту лаунчер обрабатывает отдельно.
pub fn login_error(status: u16, body: &Value) -> LauncherError {
    let message = body["errorMessage"].as_str().unwrap_or_default();
    if message.to_lowercase().contains(TWO_FACTOR_MARKER) {
        return LauncherError::ElyTwoFactor;
    }
    match status {
        401 | 403 => LauncherError::ElyLogin("неверный логин, пароль или код".to_string()),
        _ if message.is_empty() => LauncherError::ElyLogin(format!("сервер ответил {status}")),
        _ => LauncherError::ElyLogin(message.to_string()),
    }
}

async fn post(
    client: &reqwest::Client,
    url: &str,
    body: &Value,
) -> Result<(u16, Value), LauncherError> {
    let response =
        client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|_| LauncherError::NeedsInternet {
                what: "Ely.by".to_string(),
            })?;
    let status = response.status().as_u16();
    let json = response.json::<Value>().await.unwrap_or(Value::Null);
    Ok((status, json))
}

/// Вход по нику или почте и паролю. Код двухфакторной защиты дописывается к паролю: `пароль:код`.
pub async fn authenticate(
    client: &reqwest::Client,
    authserver: &str,
    login: &str,
    password: &str,
    totp: Option<&str>,
    client_token: &str,
) -> Result<ElySession, LauncherError> {
    let password = match totp.map(str::trim).filter(|code| !code.is_empty()) {
        Some(code) => format!("{password}:{code}"),
        None => password.to_string(),
    };
    let body = json!({
        "username": login.trim(),
        "password": password,
        "clientToken": client_token,
        "requestUser": false,
    });
    let url = format!("{}/auth/authenticate", authserver.trim_end_matches('/'));
    let (status, json) = post(client, &url, &body).await?;
    if status != 200 {
        return Err(login_error(status, &json));
    }
    parse_session(&json, client_token)
        .ok_or_else(|| LauncherError::ElyLogin("у аккаунта нет игрового профиля".to_string()))
}

/// Свежий токен для запуска. Истёкшая сессия требует войти заново.
pub async fn refresh(
    client: &reqwest::Client,
    authserver: &str,
    session: &ElySession,
) -> Result<ElySession, LauncherError> {
    let body = json!({
        "accessToken": session.access_token,
        "clientToken": session.client_token,
        "requestUser": false,
    });
    let url = format!("{}/auth/refresh", authserver.trim_end_matches('/'));
    let (status, json) = post(client, &url, &body).await?;
    match status {
        200 => parse_session(&json, &session.client_token).ok_or(LauncherError::ElySessionExpired),
        // Сбой или перегрузка Ely.by — не повод входить заново: игра запустится без проверки аккаунта.
        429 | 500..=599 => Err(LauncherError::NeedsInternet {
            what: "Ely.by".to_string(),
        }),
        _ => Err(LauncherError::ElySessionExpired),
    }
}

/// Отзывает токен на сервере. Ошибки не важны: локально сессия удаляется в любом случае.
pub async fn invalidate(client: &reqwest::Client, authserver: &str, session: &ElySession) {
    let body = json!({ "accessToken": session.access_token, "clientToken": session.client_token });
    let url = format!("{}/auth/invalidate", authserver.trim_end_matches('/'));
    let _ = post(client, &url, &body).await;
}

/// Файл последнего выпуска authlib-injector из ответа GitHub API.
pub fn injector_asset(release: &Value, dir: &Path) -> Option<DownloadItem> {
    let asset = release["assets"].as_array()?.iter().find(|asset| {
        asset["name"]
            .as_str()
            .is_some_and(|name| name.starts_with("authlib-injector-") && name.ends_with(".jar"))
    })?;
    let name = asset["name"].as_str()?;
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return None;
    }
    // Без контрольной суммы javaagent с полными правами в игре не скачивается.
    let sha256 = asset["digest"]
        .as_str()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|hash| is_sha256_hex(hash))?
        .to_string();
    Some(DownloadItem {
        url: asset["browser_download_url"].as_str()?.to_string(),
        dest: dir.join(name),
        expected: Expected {
            sha256: Some(sha256),
            size: asset["size"].as_u64(),
            ..Expected::default()
        },
    })
}

/// Номер версии из имени `authlib-injector-1.2.10.jar` для сравнения по числам, а не по буквам.
fn injector_version(path: &Path) -> Vec<u32> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.strip_prefix("authlib-injector-"))
        .map(|version| {
            version
                .split('.')
                .filter_map(|part| part.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn cached_injector(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jar"))
        .max_by_key(|path| injector_version(path))
}

/// Скачивает authlib-injector или берёт уже скачанный, если GitHub недоступен.
pub async fn ensure_authlib_injector(ctx: &LaunchContext) -> Result<PathBuf, LauncherError> {
    let dir = ctx.root.join(INJECTOR_DIR);
    let release = ctx
        .probe_client
        .get(&ctx.endpoints.authlib_injector_release)
        .header("User-Agent", "FenixCraft-Launcher")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status);
    let item = match release {
        Ok(response) => response
            .json::<Value>()
            .await
            .ok()
            .and_then(|json| injector_asset(&json, &dir)),
        Err(_) => None,
    };
    let Some(item) = item else {
        return cached_injector(&dir).ok_or_else(|| LauncherError::NeedsInternet {
            what: "authlib-injector для входа через Ely.by".to_string(),
        });
    };
    let dest = item.dest.clone();
    ensure_files(
        &ctx.client,
        vec![item],
        &ctx.reporter,
        "authlib-injector",
        1,
    )
    .await?;
    Ok(dest)
}

/// Аргумент JVM, который направляет вход и скины игры на Ely.by.
pub fn javaagent_arg(jar: &Path) -> String {
    format!("-javaagent:{}=ely.by", jar.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_is_read_from_the_authserver_reply() {
        let body = json!({
            "accessToken": "jwt",
            "clientToken": "client",
            "selectedProfile": { "id": "ffc8fdc95824509e8a57c99b940fb996", "name": "ErickSkrauch" }
        });

        assert_eq!(
            parse_session(&body, "client"),
            Some(ElySession {
                access_token: "jwt".to_string(),
                client_token: "client".to_string(),
                uuid: "ffc8fdc95824509e8a57c99b940fb996".to_string(),
                username: "ErickSkrauch".to_string(),
            })
        );
        assert_eq!(parse_session(&json!({ "accessToken": "jwt" }), "c"), None);
    }

    #[test]
    fn two_factor_and_bad_password_are_told_apart() {
        let two_factor = json!({
            "error": "ForbiddenOperationException",
            "errorMessage": "Account protected with two factor auth."
        });
        let wrong = json!({
            "error": "ForbiddenOperationException",
            "errorMessage": "Invalid credentials. Invalid email or password."
        });

        assert!(matches!(
            login_error(401, &two_factor),
            LauncherError::ElyTwoFactor
        ));
        assert!(matches!(
            login_error(401, &wrong),
            LauncherError::ElyLogin(_)
        ));
    }

    #[test]
    fn injector_jar_is_picked_from_the_release_with_its_hash() {
        let release = json!({ "assets": [
            { "name": "authlib-injector-1.2.6.jar.sha256", "browser_download_url": "https://x/a.sha256" },
            { "name": "authlib-injector-1.2.6.jar", "browser_download_url": "https://x/a.jar",
              "size": 300, "digest": format!("sha256:{}", "a".repeat(64)) }
        ]});
        let item = injector_asset(&release, Path::new("inj")).unwrap();

        assert_eq!(item.url, "https://x/a.jar");
        assert_eq!(
            item.dest,
            Path::new("inj").join("authlib-injector-1.2.6.jar")
        );
        assert_eq!(
            item.expected.sha256.as_deref(),
            Some("a".repeat(64).as_str())
        );
        assert!(injector_asset(&json!({ "assets": [] }), Path::new("inj")).is_none());
        let no_digest = json!({ "assets": [
            { "name": "authlib-injector-1.2.6.jar", "browser_download_url": "https://x/a.jar" }
        ]});
        assert!(injector_asset(&no_digest, Path::new("inj")).is_none());
    }

    #[test]
    fn newest_cached_injector_is_chosen_by_version_number() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["authlib-injector-1.2.9.jar", "authlib-injector-1.2.10.jar"] {
            std::fs::write(dir.path().join(name), b"jar").unwrap();
        }

        assert_eq!(
            cached_injector(dir.path()).unwrap(),
            dir.path().join("authlib-injector-1.2.10.jar")
        );
    }

    #[test]
    fn javaagent_points_the_game_at_ely_by() {
        let jar = Path::new("inj").join("authlib-injector.jar");
        assert_eq!(
            javaagent_arg(&jar),
            format!("-javaagent:{}=ely.by", jar.display())
        );
    }
}
