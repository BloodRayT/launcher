#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod feeds;

use fenix_core::assets::{verify_assets, AssetAudit};
use fenix_core::config::{
    config_path, game_dir, load_config, official_minecraft_dir, save_config, LauncherConfig,
};
use fenix_core::download::{download_client, probe_client};
use fenix_core::errors::{LauncherError, UserError};
use fenix_core::instances::instance_dir;
use fenix_core::launch::{prepare_launch_as, run_game, LaunchContext};
use fenix_core::network::{check_connectivity, Connectivity};
use fenix_core::reporter::{GameEvent, Reporter};
use fenix_core::safety::{is_trusted_update_url, parse_external_url};
use fenix_core::updater::{check_update, download_installer, UpdateInfo, RELEASES_PAGE};
use fenix_core::Endpoints;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Window};
use tauri_plugin_opener::OpenerExt;

const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTALLER_FILE: &str = "FenixCraft_Update_Setup.exe";
const CONSOLE_REPORT_TAIL_LINES: usize = 150;

static GAME_RUNNING: AtomicBool = AtomicBool::new(false);

fn current_config() -> LauncherConfig {
    load_config(&config_path(&game_dir()))
}

fn app_reporter(app: AppHandle) -> Reporter {
    Reporter::new(move |event| {
        let _ = match event {
            GameEvent::Log(message) => app.emit("game:log", message),
            GameEvent::Progress { label, task, total } => app.emit(
                "game:progress",
                serde_json::json!({ "type": label, "task": task, "total": total }),
            ),
            GameEvent::Running => app.emit("game:running", ()),
        };
    })
}

#[tauri::command]
fn get_launcher_version() -> String {
    LAUNCHER_VERSION.to_string()
}

#[tauri::command]
fn get_config() -> LauncherConfig {
    current_config()
}

#[tauri::command(rename_all = "camelCase")]
fn save_launcher_config(new_config: LauncherConfig) -> Result<LauncherConfig, String> {
    save_config(&config_path(&game_dir()), new_config)
        .map_err(|err| format!("Не удалось сохранить настройки: {err}"))
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
fn window_set_fullscreen(window: Window, fullscreen: bool) {
    let _ = window.set_fullscreen(fullscreen);
}

#[tauri::command]
fn window_close(window: Window) {
    let _ = window.close();
}

#[tauri::command]
fn open_external_url(app: AppHandle, url: String) -> Result<(), String> {
    let url = parse_external_url(&url).ok_or("Этот адрес нельзя открыть.")?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn get_connectivity() -> Connectivity {
    let backend_url = current_config().backend_url;
    let Ok(client) = probe_client() else {
        return Connectivity {
            internet: false,
            backend: false,
        };
    };
    check_connectivity(
        &client,
        &Endpoints::default().version_manifest,
        &backend_url,
    )
    .await
}

#[tauri::command]
async fn verify_assets_command(app: AppHandle) -> Result<AssetAudit, String> {
    let reporter = app_reporter(app);
    tauri::async_runtime::spawn_blocking(move || verify_assets(&game_dir(), &reporter))
        .await
        .map_err(|err| err.to_string())
}

fn last_lines(path: &PathBuf, count: usize) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    Some(lines[lines.len().saturating_sub(count)..].join("\n"))
}

#[tauri::command(rename_all = "camelCase")]
fn save_debug_log(
    app: AppHandle,
    console_text: String,
    custom_path: Option<String>,
) -> Result<String, String> {
    let root = game_dir();
    let logs_dir = instance_dir(&root, &current_config().selected_version)
        .map_err(|err| err.to_string())?
        .join("logs");
    std::fs::create_dir_all(&logs_dir).map_err(|err| err.to_string())?;
    let target = custom_path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| logs_dir.join("fenixcraft-report.log"));

    let mut report =
        format!("===== FENIXCRAFT LAUNCHER v{LAUNCHER_VERSION} =====\n{console_text}\n");
    for name in ["debug.log", "latest.log"] {
        if let Some(tail) = last_lines(&logs_dir.join(name), CONSOLE_REPORT_TAIL_LINES) {
            report.push_str(&format!(
                "\n--- logs/{name}, последние строки ---\n{tail}\n"
            ));
        }
    }
    std::fs::write(&target, report).map_err(|err| err.to_string())?;
    let _ = app.opener().reveal_item_in_dir(&target);
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
async fn check_launcher_update() -> UpdateInfo {
    match probe_client() {
        Ok(client) => check_update(&client, LAUNCHER_VERSION).await,
        Err(_) => UpdateInfo {
            update_available: false,
            latest_version: LAUNCHER_VERSION.to_string(),
            download_url: String::new(),
            changelog: String::new(),
        },
    }
}

#[tauri::command(rename_all = "camelCase")]
async fn install_launcher_update(app: AppHandle, download_url: String) -> Result<(), String> {
    if !(cfg!(windows) && is_trusted_update_url(&download_url)) {
        return app
            .opener()
            .open_url(RELEASES_PAGE, None::<&str>)
            .map_err(|err| err.to_string());
    }
    let installer = game_dir().join(INSTALLER_FILE);
    let client = download_client().map_err(|err| err.to_string())?;
    download_installer(&client, &download_url, &installer)
        .await
        .map_err(|err| err.to_string())?;
    std::process::Command::new(&installer)
        .spawn()
        .map_err(|err| format!("Не удалось запустить установщик: {err}"))?;
    app.exit(0);
    Ok(())
}

async fn prepare_and_run(reporter: Reporter) -> Result<(), LauncherError> {
    let config = current_config();
    let ctx = LaunchContext::new(
        game_dir(),
        official_minecraft_dir(),
        Endpoints::default(),
        LAUNCHER_VERSION,
        reporter.clone(),
    )?;
    let identity = accounts::resolve_identity(&config, &reporter).await?;
    let plan = prepare_launch_as(&ctx, &config, &identity).await?;
    tauri::async_runtime::spawn_blocking(move || run_game(&plan, &reporter))
        .await
        .map_err(|err| LauncherError::Io(err.to_string()))?
}

/// Запуск идёт в фоне: результат приходит событиями `game:running`, `game:error` и `game:closed`.
#[tauri::command]
fn launch_game(app: AppHandle) -> Result<(), UserError> {
    if GAME_RUNNING.swap(true, Ordering::SeqCst) {
        return Err(LauncherError::AlreadyRunning.payload());
    }
    tauri::async_runtime::spawn(async move {
        let reporter = app_reporter(app.clone());
        if let Err(err) = prepare_and_run(reporter.clone()).await {
            reporter.log(format!("[Ошибка] {err}"));
            let _ = app.emit("game:error", err.payload());
        }
        GAME_RUNNING.store(false, Ordering::SeqCst);
        let _ = app.emit("game:closed", ());
    });
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_launcher_version,
            get_config,
            save_launcher_config,
            window_minimize,
            window_maximize,
            window_close,
            window_set_fullscreen,
            open_external_url,
            get_connectivity,
            feeds::get_servers_status,
            feeds::get_vk_news,
            feeds::check_twitch_live,
            feeds::get_launcher_releases,
            verify_assets_command,
            accounts::ms_login_available,
            accounts::ms_login_start,
            accounts::ms_login_cancel,
            accounts::ms_logout,
            save_debug_log,
            check_launcher_update,
            install_launcher_update,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("Ошибка при запуске лаунчера FenixCraft");
}
