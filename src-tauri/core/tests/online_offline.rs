//! Сквозные сценарии подготовки запуска с локальным сервером вместо Mojang и бекенда.

mod common;

use common::{bind_port, Routes, TestServer};
use fenix_core::backend_sync::{sync_profile, SyncOutcome};
use fenix_core::config::LauncherConfig;
use fenix_core::errors::LauncherError;
use fenix_core::launch::{prepare_launch, run_game, LaunchContext, LaunchPlan};
use fenix_core::reporter::{GameEvent, Reporter};
use fenix_core::Endpoints;
use serde_json::json;
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

/// Адрес, на котором никто не слушает: так выглядит отсутствие сети.
const DEAD_URL: &str = "http://127.0.0.1:1";
const VERSION: &str = "1.0.0";
const CLIENT_BYTES: &[u8] = b"client jar contents";
const LIB_BYTES: &[u8] = b"library jar contents";
const ASSET_BYTES: &[u8] = b"asset contents";
const MAIN_CLASS: &str = "net.minecraft.client.main.Main";
const LIB_REL_PATH: &str = "com/test/lib/1/lib-1.jar";

fn sha1_hex(data: &[u8]) -> String {
    hex::encode(Sha1::digest(data))
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn route(routes: &mut Routes, path: &str, body: impl Into<Vec<u8>>) {
    routes.insert(path.to_string(), (200, body.into()));
}

/// Маршруты «Mojang». Адреса внутри JSON указывают на этот же сервер, поэтому порт выбирается заранее.
fn mojang_routes(base: &str) -> Routes {
    let asset_hash = sha1_hex(ASSET_BYTES);
    let asset_index = json!({ "objects": {
        "minecraft/lang/ru_ru.json": { "hash": asset_hash, "size": ASSET_BYTES.len() }
    } })
    .to_string();
    let version = json!({
        "id": VERSION,
        "mainClass": MAIN_CLASS,
        "javaVersion": { "majorVersion": 21 },
        "assetIndex": {
            "id": "test-index",
            "url": format!("{base}/asset-index.json"),
            "sha1": sha1_hex(asset_index.as_bytes())
        },
        "downloads": { "client": {
            "url": format!("{base}/client.jar"),
            "sha1": sha1_hex(CLIENT_BYTES),
            "size": CLIENT_BYTES.len()
        } },
        "libraries": [{ "name": "com.test:lib:1", "downloads": { "artifact": {
            "path": LIB_REL_PATH,
            "url": format!("{base}/lib.jar"),
            "sha1": sha1_hex(LIB_BYTES),
            "size": LIB_BYTES.len()
        } } }]
    })
    .to_string();
    let manifest = json!({
        "latest": { "release": VERSION },
        "versions": [{
            "id": VERSION,
            "url": format!("{base}/version.json"),
            "sha1": sha1_hex(version.as_bytes())
        }]
    })
    .to_string();

    let mut routes = Routes::new();
    route(&mut routes, "/manifest.json", manifest);
    route(&mut routes, "/version.json", version);
    route(&mut routes, "/client.jar", CLIENT_BYTES);
    route(&mut routes, "/lib.jar", LIB_BYTES);
    route(&mut routes, "/asset-index.json", asset_index);
    route(
        &mut routes,
        &format!("/assets/{}/{asset_hash}", &asset_hash[..2]),
        ASSET_BYTES,
    );
    routes
}

async fn start_mojang(extra: impl FnOnce(&mut Routes)) -> (TestServer, String) {
    let port = bind_port().await;
    let base = port.base.clone();
    let mut routes = mojang_routes(&base);
    extra(&mut routes);
    (port.serve(routes), base)
}

fn context(root: &Path, base: &str) -> LaunchContext {
    LaunchContext::new(
        root.to_path_buf(),
        root.join("no-official-launcher"),
        Endpoints {
            version_manifest: format!("{base}/manifest.json"),
            assets_base: format!("{base}/assets"),
            fabric_meta: format!("{base}/fabric"),
            modrinth: format!("{base}/modrinth"),
            adoptium: format!("{base}/adoptium"),
            neoforge_mavens: vec![DEAD_URL.to_string(), format!("{base}/neoforge")],
        },
        "9.9.9",
        Reporter::silent(),
    )
    .unwrap()
}

/// Конфиг с готовой «Java», чтобы тесты не скачивали настоящую.
fn config(root: &TempDir, profile: &str, backend: &str) -> LauncherConfig {
    let java = root.path().join("fake-java.bin");
    std::fs::write(&java, "").unwrap();
    LauncherConfig {
        username: "Fenix".to_string(),
        selected_version: profile.to_string(),
        backend_url: backend.to_string(),
        java_path: java.to_string_lossy().to_string(),
        ..LauncherConfig::default()
    }
}

fn arg_after(plan: &LaunchPlan, name: &str) -> String {
    let at = plan.args.iter().position(|arg| arg == name).unwrap();
    plan.args[at + 1].clone()
}

#[tokio::test]
async fn installed_game_starts_without_network_and_missing_files_are_explained() {
    let dir = TempDir::new().unwrap();
    let (server, base) = start_mojang(|_| {}).await;
    let cfg = config(&dir, "latest-vanilla", &base);

    // Первая подготовка с сетью скачивает всё нужное.
    let online = prepare_launch(&context(dir.path(), &base), &cfg)
        .await
        .unwrap();
    assert!(online.args.contains(&MAIN_CLASS.to_string()));
    assert_eq!(arg_after(&online, "--username"), "Fenix");
    assert_eq!(arg_after(&online, "--version"), VERSION);
    let client_jar = dir
        .path()
        .join("versions")
        .join(VERSION)
        .join(format!("{VERSION}.jar"));
    assert_eq!(std::fs::read(&client_jar).unwrap(), CLIENT_BYTES);

    // Сеть пропала: «latest» разрешается из кеша, игра готова к запуску.
    server.stop().await;
    let offline = prepare_launch(&context(dir.path(), &base), &cfg)
        .await
        .unwrap();
    assert_eq!(offline.args, online.args);

    // Файл пропал, а сети нет: игрок получает понятную причину, а не сбой при запуске.
    std::fs::remove_file(dir.path().join("libraries").join(LIB_REL_PATH)).unwrap();
    let missing = prepare_launch(&context(dir.path(), &base), &cfg).await;
    assert!(
        matches!(missing, Err(LauncherError::NeedsInternet { .. })),
        "{missing:?}"
    );
}

fn fake_game_plan(dir: &TempDir, args: &[&str]) -> LaunchPlan {
    LaunchPlan {
        java: std::env::var("CARGO").unwrap(),
        args: args.iter().map(|arg| arg.to_string()).collect(),
        work_dir: dir.path().to_path_buf(),
    }
}

#[test]
fn finished_game_reports_running_and_writes_its_log() {
    let dir = TempDir::new().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let reporter = Reporter::new(move |event| sink.lock().unwrap().push(event));

    run_game(&fake_game_plan(&dir, &["--version"]), &reporter).unwrap();

    assert!(events.lock().unwrap().contains(&GameEvent::Running));
    let log = std::fs::read_to_string(dir.path().join("logs/fenixcraft-debug.log")).unwrap();
    assert!(log.contains("cargo"), "{log}");
}

#[test]
fn crashed_game_is_reported_as_an_error() {
    let dir = TempDir::new().unwrap();

    let result = run_game(
        &fake_game_plan(&dir, &["--no-such-flag"]),
        &Reporter::silent(),
    );

    assert!(
        matches!(result, Err(LauncherError::GameCrashed { .. })),
        "{result:?}"
    );
}

#[test]
fn missing_java_is_reported_as_an_error() {
    let dir = TempDir::new().unwrap();
    let mut plan = fake_game_plan(&dir, &[]);
    plan.java = dir.path().join("no-java").to_string_lossy().to_string();

    let result = run_game(&plan, &Reporter::silent());

    assert!(
        matches!(result, Err(LauncherError::JvmSpawn(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn first_launch_without_network_asks_for_internet() {
    let dir = TempDir::new().unwrap();
    let cfg = config(&dir, "latest-vanilla", DEAD_URL);

    let result = prepare_launch(&context(dir.path(), DEAD_URL), &cfg).await;

    assert!(
        matches!(result, Err(LauncherError::NeedsInternet { .. })),
        "{result:?}"
    );
}

#[tokio::test]
async fn corrupted_download_is_rejected_and_not_kept() {
    let dir = TempDir::new().unwrap();
    let (server, base) = start_mojang(|routes| {
        route(routes, "/client.jar", b"tampered jar".to_vec());
    })
    .await;
    let cfg = config(&dir, "latest-vanilla", &base);

    let result = prepare_launch(&context(dir.path(), &base), &cfg).await;

    assert!(
        matches!(result, Err(LauncherError::Checksum { .. })),
        "{result:?}"
    );
    let version_dir = dir.path().join("versions").join(VERSION);
    assert!(!version_dir.join(format!("{VERSION}.jar")).exists());
    assert!(!version_dir.join(format!("{VERSION}.jar.part")).exists());
    server.stop().await;
}

#[tokio::test]
async fn http_error_page_is_not_saved_as_a_library() {
    let dir = TempDir::new().unwrap();
    let (server, base) = start_mojang(|routes| {
        routes.remove("/lib.jar");
    })
    .await;
    let cfg = config(&dir, "latest-vanilla", &base);

    let result = prepare_launch(&context(dir.path(), &base), &cfg).await;

    assert!(
        matches!(result, Err(LauncherError::Download { .. })),
        "{result:?}"
    );
    assert!(!dir.path().join("libraries").join(LIB_REL_PATH).exists());
    server.stop().await;
}

#[tokio::test]
async fn invalid_username_stops_before_any_download() {
    let dir = TempDir::new().unwrap();
    let mut cfg = config(&dir, "latest-vanilla", DEAD_URL);
    cfg.username = "Игрок".to_string();

    let result = prepare_launch(&context(dir.path(), DEAD_URL), &cfg).await;

    assert_eq!(result, Err(LauncherError::InvalidUsername));
}

#[tokio::test]
async fn failed_neoforge_install_is_reported_instead_of_starting_vanilla() {
    let dir = TempDir::new().unwrap();
    let (server, base) = start_mojang(|_| {}).await;
    let cfg = config(&dir, "1.0.0-neoforge", &base);

    let result = prepare_launch(&context(dir.path(), &base), &cfg).await;

    assert!(
        matches!(result, Err(LauncherError::NeoForgeInstall(_))),
        "{result:?}"
    );
    server.stop().await;
}

#[tokio::test]
async fn fabric_is_required_and_works_offline_once_cached() {
    let dir = TempDir::new().unwrap();
    let fabric_main = "net.fabricmc.loader.impl.launch.knot.KnotClient";
    let (server, base) = start_mojang(|routes| {
        route(
            routes,
            "/fabric/versions/loader/1.0.0",
            json!([{ "loader": { "version": "0.1" } }]).to_string(),
        );
        route(
            routes,
            "/fabric/versions/loader/1.0.0/0.1/profile/json",
            json!({ "mainClass": fabric_main, "libraries": [] }).to_string(),
        );
    })
    .await;
    let cfg = config(&dir, "1.0.0-fabric", &base);

    let online = prepare_launch(&context(dir.path(), &base), &cfg)
        .await
        .unwrap();
    assert!(online.args.contains(&fabric_main.to_string()));

    server.stop().await;
    let offline = prepare_launch(&context(dir.path(), &base), &cfg)
        .await
        .unwrap();
    assert!(offline.args.contains(&fabric_main.to_string()));

    // Без кеша Fabric не подменяется ванилью.
    std::fs::remove_file(dir.path().join("versions/fabric-cache/1.0.0.json")).unwrap();
    let result = prepare_launch(&context(dir.path(), &base), &cfg).await;
    assert!(
        matches!(result, Err(LauncherError::NeedsInternet { .. })),
        "{result:?}"
    );
}

fn backend_routes(base: &str, mod_bytes: &[u8]) -> Routes {
    let mut routes = Routes::new();
    route(
        &mut routes,
        "/manifest/test-profile",
        json!({
            "profile": "test-profile",
            "enforce_mods": true,
            "files": [{
                "path": "mods/server-mod.jar",
                "sha256": sha256_hex(mod_bytes),
                "size": mod_bytes.len(),
                "url": format!("{base}/files/server-mod.jar")
            }]
        })
        .to_string(),
    );
    route(&mut routes, "/files/server-mod.jar", mod_bytes.to_vec());
    routes
}

#[tokio::test]
async fn backend_outage_keeps_files_and_uses_the_saved_manifest() {
    let dir = TempDir::new().unwrap();
    let mod_bytes = b"server mod";
    let port = bind_port().await;
    let base = port.base.clone();
    let server = port.serve(backend_routes(&base, mod_bytes));
    let ctx = context(dir.path(), &base);
    let instance = dir.path().join("instances").join("test-profile");
    let foreign = instance.join("mods").join("player-mod.jar");
    std::fs::create_dir_all(foreign.parent().unwrap()).unwrap();

    // Свежий манифест применяется, посторонний мод удаляется.
    std::fs::write(&foreign, "player mod").unwrap();
    assert_eq!(
        sync_profile(&ctx, "test-profile", &base).await,
        Ok(SyncOutcome::Fresh)
    );
    assert!(instance.join("mods/server-mod.jar").exists());
    assert!(!foreign.exists());

    // Бекенд упал: файлы сверяются по сохранённому манифесту и ничего не удаляется.
    server.stop().await;
    std::fs::write(&foreign, "player mod").unwrap();
    assert_eq!(
        sync_profile(&ctx, "test-profile", &base).await,
        Ok(SyncOutcome::Cached)
    );
    assert!(foreign.exists());

    // Сборку без сохранённого манифеста запускать всё равно можно.
    assert_eq!(
        sync_profile(&ctx, "other-profile", &base).await,
        Ok(SyncOutcome::Unavailable)
    );
}

#[tokio::test]
async fn malicious_manifest_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).unwrap();
    let port = bind_port().await;
    let base = port.base.clone();
    let mut routes = Routes::new();
    route(
        &mut routes,
        "/manifest/test-profile",
        json!({
            "profile": "test-profile",
            "files": [
                { "path": "mods/ok.jar", "sha256": sha256_hex(b"ok"), "size": 2,
                  "url": format!("{base}/files/ok.jar") },
                { "path": "../evil.exe", "sha256": sha256_hex(b"evil"), "size": 4,
                  "url": format!("{base}/files/evil.exe") }
            ]
        })
        .to_string(),
    );
    route(&mut routes, "/files/ok.jar", b"ok".to_vec());
    route(&mut routes, "/files/evil.exe", b"evil".to_vec());
    let server = port.serve(routes);

    let result = sync_profile(&context(&game_dir, &base), "test-profile", &base).await;

    assert!(
        matches!(result, Err(LauncherError::BadManifest(_))),
        "{result:?}"
    );
    assert!(!game_dir.join("instances/evil.exe").exists());
    assert!(!game_dir.join("instances/test-profile/mods/ok.jar").exists());
    server.stop().await;
}

#[tokio::test]
async fn profiles_keep_their_game_files_apart() {
    let dir = TempDir::new().unwrap();
    let (server, base) = start_mojang(|routes| {
        route(
            routes,
            "/fabric/versions/loader/1.0.0",
            json!([{ "loader": { "version": "0.1" } }]).to_string(),
        );
        route(
            routes,
            "/fabric/versions/loader/1.0.0/0.1/profile/json",
            json!({ "mainClass": "Knot", "libraries": [] }).to_string(),
        );
    })
    .await;

    let vanilla = prepare_launch(
        &context(dir.path(), &base),
        &config(&dir, "1.0.0-vanilla", &base),
    )
    .await
    .unwrap();
    let fabric = prepare_launch(
        &context(dir.path(), &base),
        &config(&dir, "1.0.0-fabric", &base),
    )
    .await
    .unwrap();

    let instances = dir.path().join("instances");
    assert_eq!(vanilla.work_dir, instances.join("1.0.0-vanilla"));
    assert_eq!(fabric.work_dir, instances.join("1.0.0-fabric"));
    assert_eq!(
        arg_after(&vanilla, "--gameDir"),
        vanilla.work_dir.to_string_lossy()
    );
    assert_eq!(
        arg_after(&fabric, "--gameDir"),
        fabric.work_dir.to_string_lossy()
    );
    // Ресурсы и клиент общие: второй сборке не нужно скачивать их заново.
    assert_eq!(
        arg_after(&vanilla, "--assetsDir"),
        arg_after(&fabric, "--assetsDir")
    );
    assert!(dir.path().join("versions").join(VERSION).exists());
    server.stop().await;
}
