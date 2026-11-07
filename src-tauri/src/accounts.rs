//! Аккаунты: вход через Ely.by и Microsoft и выбор личности игрока перед запуском.
//! Refresh-токен лежит только в системном хранилище, токен Minecraft живёт только в памяти.

use fenix_core::config::{config_path, game_dir, load_config, save_config, LauncherConfig};
use fenix_core::download::probe_client;
use fenix_core::ely::{self, ElySession};
use fenix_core::errors::{LauncherError, UserError};
use fenix_core::launch::PlayerIdentity;
use fenix_core::microsoft::{
    embedded_client_id, poll_device_login, refresh_microsoft_tokens, request_device_code,
    sign_in_to_minecraft, MinecraftAccount, MsEndpoints,
};
use fenix_core::reporter::Reporter;
use fenix_core::skin_loader::new_token;
use fenix_core::Endpoints;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter};

const KEYRING_SERVICE: &str = "ru.fenixcraft.launcher";
const KEYRING_USER: &str = "microsoft-refresh-token";
const ELY_KEYRING_USER: &str = "ely-session";

/// Новый вход или отмена делают ожидание предыдущего входа неактуальным.
static LOGIN_GENERATION: AtomicU64 = AtomicU64::new(0);

fn storage_error(error: keyring::Error) -> LauncherError {
    LauncherError::Microsoft(format!(
        "Не удалось использовать хранилище системы: {error}"
    ))
}

fn keyring_entry() -> Result<keyring::Entry, LauncherError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(storage_error)
}

fn store_refresh_token(token: &str) -> Result<(), LauncherError> {
    keyring_entry()?
        .set_secret(token.as_bytes())
        .map_err(storage_error)
}

fn load_refresh_token() -> Result<String, LauncherError> {
    let secret = keyring_entry()?.get_secret().map_err(|error| match error {
        keyring::Error::NoEntry => LauncherError::MicrosoftSessionExpired,
        other => storage_error(other),
    })?;
    String::from_utf8(secret).map_err(|_| LauncherError::MicrosoftSessionExpired)
}

fn delete_refresh_token() -> Result<(), LauncherError> {
    match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(storage_error(error)),
    }
}

fn update_config(
    change: impl FnOnce(&mut LauncherConfig),
) -> Result<LauncherConfig, LauncherError> {
    let path = config_path(&game_dir());
    let mut config = load_config(&path);
    change(&mut config);
    save_config(&path, config).map_err(LauncherError::from)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeView {
    user_code: String,
    verification_uri: String,
}

#[tauri::command]
pub fn ms_login_available() -> bool {
    embedded_client_id().is_some()
}

async fn finish_login(
    tokens_refresh: String,
    account: MinecraftAccount,
) -> Result<LauncherConfig, LauncherError> {
    store_refresh_token(&tokens_refresh)?;
    update_config(|config| {
        config.account_type = "microsoft".to_string();
        config.ms_username = account.username;
        config.ms_uuid = account.uuid;
    })
}

/// Показывает игроку код и в фоне ждёт подтверждения. Итог приходит событиями
/// `ms:login-done` с новым конфигом или `ms:login-failed` с описанием ошибки.
#[tauri::command]
pub async fn ms_login_start(app: AppHandle) -> Result<DeviceCodeView, UserError> {
    let client_id = embedded_client_id().ok_or_else(|| {
        LauncherError::Microsoft("Вход через Microsoft ещё не настроен".to_string()).payload()
    })?;
    let client = probe_client().map_err(|error| error.payload())?;
    let endpoints = MsEndpoints::default();
    let device = request_device_code(&client, &endpoints, &client_id)
        .await
        .map_err(|error| error.payload())?;
    let view = DeviceCodeView {
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
    };

    let generation = LOGIN_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn(async move {
        let is_current = || LOGIN_GENERATION.load(Ordering::SeqCst) == generation;
        let polled = poll_device_login(&client, &endpoints, &client_id, &device, is_current).await;
        let finished = match polled {
            Ok(None) => return,
            Ok(Some(tokens)) => {
                match sign_in_to_minecraft(&client, &endpoints, &tokens.access_token).await {
                    Ok(account) => finish_login(tokens.refresh_token, account).await,
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        };
        if !is_current() {
            return;
        }
        let _ = match finished {
            Ok(config) => app.emit("ms:login-done", config),
            Err(error) => app.emit("ms:login-failed", error.payload()),
        };
    });
    Ok(view)
}

#[tauri::command]
pub fn ms_login_cancel() {
    LOGIN_GENERATION.fetch_add(1, Ordering::SeqCst);
}

#[tauri::command]
pub fn ms_logout() -> Result<LauncherConfig, UserError> {
    LOGIN_GENERATION.fetch_add(1, Ordering::SeqCst);
    delete_refresh_token().map_err(|error| error.payload())?;
    update_config(|config| {
        config.account_type = "offline".to_string();
        config.ms_username.clear();
        config.ms_uuid.clear();
    })
    .map_err(|error| error.payload())
}

// --- Ely.by ---

fn ely_entry() -> Result<keyring::Entry, LauncherError> {
    keyring::Entry::new(KEYRING_SERVICE, ELY_KEYRING_USER).map_err(storage_error)
}

fn store_ely_session(session: &ElySession) -> Result<(), LauncherError> {
    let json = serde_json::to_vec(session).map_err(|error| LauncherError::Io(error.to_string()))?;
    ely_entry()?.set_secret(&json).map_err(storage_error)
}

fn load_ely_session() -> Result<ElySession, LauncherError> {
    let secret = ely_entry()?.get_secret().map_err(|error| match error {
        keyring::Error::NoEntry => LauncherError::ElySessionExpired,
        other => storage_error(other),
    })?;
    serde_json::from_slice(&secret).map_err(|_| LauncherError::ElySessionExpired)
}

fn delete_ely_session() -> Result<(), LauncherError> {
    match ely_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(storage_error(error)),
    }
}

/// Вход через Ely.by. Пароль уходит только на authserver.ely.by и не сохраняется.
/// Если аккаунт защищён двухфакторной аутентификацией, ответ просит код, и вход повторяется с ним.
#[tauri::command(rename_all = "camelCase")]
pub async fn ely_login(
    login: String,
    password: String,
    totp: Option<String>,
) -> Result<LauncherConfig, UserError> {
    let client = probe_client().map_err(|error| error.payload())?;
    let session = ely::authenticate(
        &client,
        &Endpoints::default().ely_authserver,
        &login,
        &password,
        totp.as_deref(),
        &new_token(),
    )
    .await
    .map_err(|error| error.payload())?;
    store_ely_session(&session).map_err(|error| error.payload())?;
    update_config(|config| {
        config.account_type = "ely".to_string();
        config.ely_username = session.username;
        config.ely_uuid = session.uuid;
    })
    .map_err(|error| error.payload())
}

#[tauri::command]
pub async fn ely_logout() -> Result<LauncherConfig, UserError> {
    if let (Ok(session), Ok(client)) = (load_ely_session(), probe_client()) {
        ely::invalidate(&client, &Endpoints::default().ely_authserver, &session).await;
    }
    delete_ely_session().map_err(|error| error.payload())?;
    update_config(|config| {
        config.account_type = "offline".to_string();
        config.ely_username.clear();
        config.ely_uuid.clear();
    })
    .map_err(|error| error.payload())
}

/// Свежая сессия Ely.by. Без сети игра запускается под тем же ником, а сервер Ely.by её не подтвердит.
async fn resolve_ely(
    config: &LauncherConfig,
    reporter: &Reporter,
) -> Result<PlayerIdentity, LauncherError> {
    let session = load_ely_session()?;
    let client = probe_client()?;
    match ely::refresh(&client, &Endpoints::default().ely_authserver, &session).await {
        Ok(fresh) => {
            store_ely_session(&fresh)?;
            Ok(PlayerIdentity::ely(
                &fresh.username,
                &fresh.uuid,
                &fresh.access_token,
            ))
        }
        Err(LauncherError::NeedsInternet { .. }) => {
            reporter.log("[Аккаунт] Нет связи с Ely.by: игра запустится без проверки аккаунта.");
            Ok(PlayerIdentity::unverified(
                &config.ely_username,
                &config.ely_uuid,
            ))
        }
        Err(error) => Err(error),
    }
}

/// Для лицензии получает свежую сессию. Без сети игра запускается под тем же именем,
/// но серверы с проверкой лицензии такого игрока не пустят.
pub async fn resolve_identity(
    config: &LauncherConfig,
    reporter: &Reporter,
) -> Result<PlayerIdentity, LauncherError> {
    if config.account_type == "ely" {
        return resolve_ely(config, reporter).await;
    }
    if config.account_type != "microsoft" {
        return Ok(PlayerIdentity::offline(&config.username));
    }
    let client_id = embedded_client_id().ok_or_else(|| {
        LauncherError::Microsoft("Вход через Microsoft не настроен в этой сборке".to_string())
    })?;
    let client = probe_client()?;
    let endpoints = MsEndpoints::default();
    let refresh_token = load_refresh_token()?;
    let session = async {
        let tokens =
            refresh_microsoft_tokens(&client, &endpoints, &client_id, &refresh_token).await?;
        store_refresh_token(&tokens.refresh_token)?;
        sign_in_to_minecraft(&client, &endpoints, &tokens.access_token).await
    }
    .await;
    match session {
        Ok(account) => Ok(PlayerIdentity::licensed(account)),
        Err(LauncherError::NeedsInternet { .. }) => {
            reporter.log("[Аккаунт] Нет связи с Microsoft: сетевая игра на лицензионных серверах будет недоступна.");
            Ok(PlayerIdentity::unverified(
                &config.ms_username,
                &config.ms_uuid,
            ))
        }
        Err(error) => Err(error),
    }
}
