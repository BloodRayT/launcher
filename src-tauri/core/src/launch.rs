use crate::assets::ensure_assets;
use crate::backend_sync::{sync_profile, SyncOutcome};
use crate::config::{LauncherConfig, DEFAULT_SERVER_PORT};
use crate::download::{
    download_client, ensure_files, probe_client, write_atomic, DownloadItem, Expected,
};
use crate::errors::LauncherError;
use crate::instances::prepare_instance;
use crate::java::{ensure_java, hide_console, required_java_major};
use crate::libraries::{argument_allowed, current_os, extract_natives, LibrarySet};
use crate::microsoft::MinecraftAccount;
use crate::mods::ensure_shader_mods;
use crate::neoforge::{find_installed_loader, install_loader, ModLoader};
use crate::reporter::Reporter;
use crate::safety::is_valid_username;
use crate::skin_loader::{ensure_skin_mod, write_loader_config};
use crate::versions::{base_version, load_version_json};
use crate::Endpoints;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

const LIBRARY_CONCURRENCY: usize = 8;
const INITIAL_HEAP_MB: u32 = 2048;
const LAUNCHER_BRAND: &str = "FenixCraft";
const FABRIC_FETCH_ATTEMPTS: u32 = 3;
const FABRIC_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(300);
const OFFLINE_ACCESS_TOKEN: &str =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJQbGF5ZXIiLCJuYW1lIjoiUGxheWVyIn0.signature";
const HEAP_DUMP_FLAG: &str =
    "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump";
const CRASH_LINES_TO_SCAN: usize = 35;

/// Всё, что нужно для подготовки запуска. Адреса и папки можно подменить в тестах.
#[derive(Clone)]
pub struct LaunchContext {
    pub root: PathBuf,
    pub official_mc_dir: PathBuf,
    pub endpoints: Endpoints,
    pub launcher_version: String,
    pub client: reqwest::Client,
    pub probe_client: reqwest::Client,
    pub reporter: Reporter,
}

impl LaunchContext {
    pub fn new(
        root: PathBuf,
        official_mc_dir: PathBuf,
        endpoints: Endpoints,
        launcher_version: impl Into<String>,
        reporter: Reporter,
    ) -> Result<Self, LauncherError> {
        Ok(Self {
            root,
            official_mc_dir,
            endpoints,
            launcher_version: launcher_version.into(),
            client: download_client()?,
            probe_client: probe_client()?,
            reporter,
        })
    }
}

/// От чьего имени запускается игра.
#[derive(Clone, PartialEq, Eq)]
pub struct PlayerIdentity {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: String,
    pub licensed: bool,
    /// `msa` для Microsoft, `mojang` для пиратки и Ely.by.
    pub user_type: String,
    /// authlib-injector для Ely.by: аргумент JVM `-javaagent:…=ely.by`.
    pub java_agent: Option<String>,
}

impl PlayerIdentity {
    /// Пиратка: только ник, без сетевой проверки.
    pub fn offline(username: &str) -> Self {
        Self::unverified(username, &offline_uuid(username))
    }

    /// Имя и UUID известны, но подтвердить их сейчас нельзя (например, нет сети).
    pub fn unverified(username: &str, uuid: &str) -> Self {
        Self {
            username: username.to_string(),
            uuid: uuid.to_string(),
            access_token: OFFLINE_ACCESS_TOKEN.to_string(),
            xuid: "0".to_string(),
            licensed: false,
            user_type: "mojang".to_string(),
            java_agent: None,
        }
    }

    /// Настоящая сессия Microsoft: серверы с проверкой лицензии пускают без пароля.
    pub fn licensed(account: MinecraftAccount) -> Self {
        Self {
            username: account.username,
            uuid: account.uuid,
            access_token: account.access_token,
            xuid: account.xuid,
            licensed: true,
            user_type: "msa".to_string(),
            java_agent: None,
        }
    }

    /// Аккаунт Ely.by. authlib-injector подключается перед запуском: `java_agent` заполняет лаунчер.
    pub fn ely(username: &str, uuid: &str, access_token: &str) -> Self {
        Self {
            username: username.to_string(),
            uuid: uuid.to_string(),
            access_token: access_token.to_string(),
            xuid: "0".to_string(),
            licensed: true,
            user_type: "mojang".to_string(),
            java_agent: None,
        }
    }
}

/// Готовая к выполнению команда запуска игры.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub java: String,
    pub args: Vec<String>,
    pub work_dir: PathBuf,
}

/// Загрузчик определяется по идентификатору сборки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loader {
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
}

pub fn loader_of(profile: &str) -> Loader {
    if profile.contains("neoforge") {
        Loader::NeoForge
    } else if profile.contains("forge") {
        Loader::Forge
    } else if profile.contains("fabric") || profile.ends_with("-iris") {
        Loader::Fabric
    } else {
        Loader::Vanilla
    }
}

fn wants_shaders(profile: &str) -> bool {
    profile.contains("iris")
}

struct VanillaReady {
    version: String,
    json: Value,
    classpath: Vec<PathBuf>,
    asset_id: String,
    natives_dir: PathBuf,
}

async fn prepare_vanilla(
    ctx: &LaunchContext,
    requested: &str,
) -> Result<VanillaReady, LauncherError> {
    let (version, json) = load_version_json(ctx, requested).await?;
    let libs_dir = ctx.root.join("libraries");
    let client_jar = ctx
        .root
        .join("versions")
        .join(&version)
        .join(format!("{version}.jar"));

    let mut libraries = LibrarySet::default();
    libraries.collect(&json, &libs_dir, current_os());
    libraries.ensure_resolved()?;

    let client_info = &json["downloads"]["client"];
    let mut downloads = std::mem::take(&mut libraries.downloads);
    match client_info["url"].as_str() {
        Some(url) => downloads.push(DownloadItem {
            url: url.to_string(),
            dest: client_jar.clone(),
            expected: Expected {
                sha1: client_info["sha1"].as_str().map(str::to_string),
                size: client_info["size"].as_u64(),
                ..Expected::default()
            },
        }),
        None if !client_jar.exists() => {
            return Err(LauncherError::BadData(format!(
                "у версии {version} нет файла клиента"
            )))
        }
        None => {}
    }
    ensure_files(
        &ctx.client,
        downloads,
        &ctx.reporter,
        "Файлы игры",
        LIBRARY_CONCURRENCY,
    )
    .await?;

    let asset_id = ensure_assets(ctx, &json).await?;

    let mut classpath = libraries.classpath;
    classpath.push(client_jar);
    let natives_dir = ctx.root.join("natives").join(&version);
    extract_natives(&classpath, &native_library_dir(&json, &natives_dir))?;

    Ok(VanillaReady {
        version,
        json,
        classpath,
        asset_id,
        natives_dir,
    })
}

enum FabricFetchError {
    Unreachable,
    Server(String),
}

async fn fetch_fabric_json_once(ctx: &LaunchContext, url: &str) -> Result<Value, FabricFetchError> {
    let response = ctx.client.get(url).send().await.map_err(|error| {
        if error.is_connect() || error.is_timeout() {
            FabricFetchError::Unreachable
        } else {
            FabricFetchError::Server(error.to_string())
        }
    })?;
    if !response.status().is_success() {
        return Err(FabricFetchError::Server(format!(
            "сервер ответил {}",
            response.status().as_u16()
        )));
    }
    response
        .json()
        .await
        .map_err(|error| FabricFetchError::Server(error.to_string()))
}

async fn fetch_fabric_json(ctx: &LaunchContext, url: &str) -> Result<Value, FabricFetchError> {
    let mut attempt = 1;
    loop {
        match fetch_fabric_json_once(ctx, url).await {
            Ok(json) => return Ok(json),
            Err(error) if attempt >= FABRIC_FETCH_ATTEMPTS => return Err(error),
            Err(_) => {
                tokio::time::sleep(FABRIC_RETRY_DELAY).await;
                attempt += 1;
            }
        }
    }
}

async fn fetch_fabric_profile(
    ctx: &LaunchContext,
    minecraft: &str,
) -> Result<Value, FabricFetchError> {
    let base = ctx.endpoints.fabric_meta.trim_end_matches('/');
    let loaders =
        fetch_fabric_json(ctx, &format!("{base}/versions/loader/{minecraft}?limit=1")).await?;
    let loader = loaders
        .as_array()
        .and_then(|list| list.first())
        .and_then(|entry| entry["loader"]["version"].as_str())
        .ok_or_else(|| {
            FabricFetchError::Server(format!("нет загрузчика для Minecraft {minecraft}"))
        })?;
    fetch_fabric_json(
        ctx,
        &format!("{base}/versions/loader/{minecraft}/{loader}/profile/json"),
    )
    .await
}

async fn load_fabric_profile(ctx: &LaunchContext, minecraft: &str) -> Result<Value, LauncherError> {
    let cache = ctx
        .root
        .join("versions")
        .join("fabric-cache")
        .join(format!("{minecraft}.json"));
    let failure = match fetch_fabric_profile(ctx, minecraft).await {
        Ok(profile) => {
            if let Ok(json) = serde_json::to_vec_pretty(&profile) {
                let _ = write_atomic(&cache, &json);
            }
            return Ok(profile);
        }
        Err(failure) => failure,
    };
    if let Some(profile) = std::fs::read(&cache)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    {
        return Ok(profile);
    }
    Err(match failure {
        FabricFetchError::Unreachable => LauncherError::NeedsInternet {
            what: format!("загрузчик Fabric для {minecraft}"),
        },
        FabricFetchError::Server(reason) => LauncherError::Download {
            file: format!("загрузчик Fabric для {minecraft}"),
            reason,
        },
    })
}

struct FabricReady {
    main_class: String,
    jvm_args: Vec<String>,
}

/// Добавляет Fabric к ванильному класспасу и возвращает его главный класс и аргументы JVM.
async fn prepare_fabric(
    ctx: &LaunchContext,
    minecraft: &str,
    classpath: &mut Vec<PathBuf>,
) -> Result<FabricReady, LauncherError> {
    let profile = load_fabric_profile(ctx, minecraft).await?;
    let mut libraries = LibrarySet::seeded(classpath);
    libraries.collect(&profile, &ctx.root.join("libraries"), current_os());
    libraries.ensure_resolved()?;
    ensure_files(
        &ctx.client,
        std::mem::take(&mut libraries.downloads),
        &ctx.reporter,
        "Файлы Fabric",
        LIBRARY_CONCURRENCY,
    )
    .await?;
    classpath.append(&mut libraries.classpath);
    let main_class = profile["mainClass"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| {
            LauncherError::BadData("у профиля Fabric нет главного класса".to_string())
        })?;
    let jvm_args = profile["arguments"]["jvm"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    Ok(FabricReady {
        main_class,
        jvm_args,
    })
}

fn offline_uuid(username: &str) -> String {
    uuid::Uuid::new_v3(
        &uuid::Uuid::NAMESPACE_OID,
        format!("OfflinePlayer:{username}").as_bytes(),
    )
    .to_string()
}

fn classpath_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

fn join_classpath(classpath: &[PathBuf]) -> String {
    classpath
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(classpath_separator())
}

fn memory_and_encoding_args(config: &LauncherConfig) -> Vec<String> {
    vec![
        format!("-Xms{}M", INITIAL_HEAP_MB.min(config.ram)),
        format!("-Xmx{}M", config.ram),
        "-Dfile.encoding=UTF-8".to_string(),
        "-Dsun.stdout.encoding=UTF-8".to_string(),
        "-Dsun.stderr.encoding=UTF-8".to_string(),
    ]
}

/// Значения для подстановки в аргументы из JSON версии.
struct JvmPlaceholders<'a> {
    natives_root: &'a Path,
    libraries: &'a Path,
    classpath: &'a str,
    version: &'a str,
    launcher_version: &'a str,
}

impl JvmPlaceholders<'_> {
    fn apply(&self, text: &str) -> String {
        text.replace("${natives_directory}", &self.natives_root.to_string_lossy())
            .replace("${library_directory}", &self.libraries.to_string_lossy())
            .replace("${classpath_separator}", classpath_separator())
            .replace("${classpath}", self.classpath)
            .replace("${version_name}", self.version)
            .replace("${launcher_name}", LAUNCHER_BRAND)
            .replace("${launcher_version}", self.launcher_version)
    }
}

/// Аргументы JVM для этой системы из JSON версии. Без них новые версии игры падают:
/// например, 26.3 требует `-XX:StackShadowPages=32`.
fn official_jvm_args(
    version: &Value,
    os: &str,
    values: &JvmPlaceholders<'_>,
) -> Option<Vec<String>> {
    let declared = version["arguments"]["jvm"].as_array()?;
    let mut args = Vec::new();
    for item in declared {
        if let Some(text) = item.as_str() {
            args.push(values.apply(text));
        } else if argument_allowed(item, os) {
            match &item["value"] {
                Value::String(text) => args.push(values.apply(text)),
                Value::Array(list) => {
                    args.extend(
                        list.iter()
                            .filter_map(Value::as_str)
                            .map(|text| values.apply(text)),
                    );
                }
                _ => {}
            }
        }
    }
    Some(args)
}

/// Для старых версий, у которых в JSON нет секции `arguments`.
fn legacy_jvm_args(values: &JvmPlaceholders<'_>) -> Vec<String> {
    let natives = values.natives_root.to_string_lossy();
    vec![
        HEAP_DUMP_FLAG.to_string(),
        format!("-Djava.library.path={natives}"),
        format!("-Djna.tmpdir={natives}"),
        format!("-Dio.netty.native.workdir={natives}"),
        format!("-Dminecraft.launcher.brand={LAUNCHER_BRAND}"),
        format!("-Dminecraft.launcher.version={}", values.launcher_version),
        "-cp".to_string(),
        values.classpath.to_string(),
    ]
}

fn standard_jvm_args(
    config: &LauncherConfig,
    version: &Value,
    values: &JvmPlaceholders<'_>,
) -> Vec<String> {
    let mut args = memory_and_encoding_args(config);
    args.extend(
        official_jvm_args(version, current_os(), values).unwrap_or_else(|| legacy_jvm_args(values)),
    );
    args
}

/// Папка для нативных библиотек зависит от того, куда на неё указывает JSON версии.
fn native_library_dir(version: &Value, natives_root: &Path) -> PathBuf {
    let in_subfolder = version["arguments"]["jvm"].as_array().is_some_and(|args| {
        args.iter()
            .filter_map(Value::as_str)
            .any(|arg| arg.contains("${natives_directory}/java"))
    });
    if in_subfolder {
        natives_root.join("java")
    } else {
        natives_root.to_path_buf()
    }
}

/// Аргументы, которыми лаунчер передаёт игроку сессию и папки: свои значения для них не принимаются.
/// Повтор любого из них в своих аргументах приводит к ошибке Minecraft «аргумент указан дважды».
const RESERVED_GAME_ARGS: [&str; 20] = [
    "--username",
    "--uuid",
    "--accessToken",
    "--xuid",
    "--userType",
    "--version",
    "--versionType",
    "--gameDir",
    "--assetsDir",
    "--assetIndex",
    "--clientId",
    "--width",
    "--height",
    "--userProperties",
    "--quickPlayPath",
    "--quickPlaySingleplayer",
    "--quickPlayMultiplayer",
    "--quickPlayRealms",
    "--server",
    "--port",
];

/// Аргумент лаунчера, в том числе в форме `--имя=значение` и в сокращении (`--user` для `--username`).
fn is_reserved_game_arg(arg: &str) -> bool {
    let name = arg.split('=').next().unwrap_or(arg).to_ascii_lowercase();
    name.len() > 2
        && RESERVED_GAME_ARGS
            .iter()
            .any(|reserved| reserved.to_ascii_lowercase().starts_with(&name))
}

/// Выключенные в настройках аргументы сохраняются, но игре не передаются.
fn custom_jvm_args(config: &LauncherConfig) -> impl Iterator<Item = String> + '_ {
    let enabled = config.custom_args_enabled;
    config
        .jvm_args
        .split_whitespace()
        .filter(move |_| enabled)
        .filter(|arg| !arg.starts_with("-Xmx") && !arg.starts_with("-Xms"))
        .map(str::to_string)
}

struct GameArgs<'a> {
    assets_dir: &'a Path,
    version: &'a str,
    asset_id: &'a str,
    identity: &'a PlayerIdentity,
}

fn game_args(config: &LauncherConfig, game_dir: &Path, game: &GameArgs<'_>) -> Vec<String> {
    let pairs = [
        ("--username", game.identity.username.clone()),
        ("--version", game.version.to_string()),
        ("--gameDir", game_dir.to_string_lossy().to_string()),
        ("--assetsDir", game.assets_dir.to_string_lossy().to_string()),
        ("--assetIndex", game.asset_id.to_string()),
        ("--uuid", game.identity.uuid.clone()),
        ("--accessToken", game.identity.access_token.clone()),
        ("--clientId", "0".to_string()),
        ("--xuid", game.identity.xuid.clone()),
        ("--userType", game.identity.user_type.clone()),
        ("--versionType", "release".to_string()),
        ("--width", config.width.to_string()),
        ("--height", config.height.to_string()),
    ];
    let mut args: Vec<String> = pairs
        .into_iter()
        .chain(quick_play_target(config).map(|target| ("--quickPlayMultiplayer", target)))
        .flat_map(|(name, value)| [name.to_string(), value])
        .collect();
    args.extend(custom_game_args(config));
    args
}

/// Свои аргументы Minecraft. Аргумент из списка лаунчера пропускается вместе со своим значением.
fn custom_game_args(config: &LauncherConfig) -> Vec<String> {
    if !config.custom_args_enabled {
        return Vec::new();
    }
    let mut kept = Vec::new();
    let mut skip_value = false;
    for arg in config.game_args.split_whitespace() {
        if skip_value && !arg.starts_with("--") {
            skip_value = false;
            continue;
        }
        let reserved = arg.starts_with("--") && is_reserved_game_arg(arg);
        // У формы `--имя=значение` отдельного значения нет.
        skip_value = reserved && !arg.contains('=');
        if !reserved {
            kept.push(arg.to_string());
        }
    }
    kept
}

/// Скины из лаунчера в сборках с модами. Сборки сервера сборок получают мод оттуда же,
/// иначе синхронизация удаляла бы его при каждом запуске.
async fn prepare_skins(
    ctx: &LaunchContext,
    config: &LauncherConfig,
    game_dir: &Path,
    minecraft: &str,
    loader: &str,
    managed_by_backend: bool,
) {
    if let Err(err) = write_loader_config(game_dir, &config.backend_url) {
        ctx.reporter
            .log(format!("[Скины] Конфиг CustomSkinLoader не записан: {err}"));
    }
    if !managed_by_backend {
        ensure_skin_mod(ctx, game_dir, minecraft, loader).await;
    }
}

/// Iris и Sodium вышли не для каждой версии игры: без них сборка запускается без шейдеров.
async fn ensure_optional_shaders(
    ctx: &LaunchContext,
    game_dir: &Path,
    minecraft: &str,
    loader: &str,
) -> Result<(), LauncherError> {
    match ensure_shader_mods(ctx, game_dir, minecraft, loader).await {
        Err(LauncherError::BadData(reason)) => {
            ctx.reporter
                .log(format!("[Моды] Шейдеры пропущены: {reason}."));
            Ok(())
        }
        other => other,
    }
}

/// Адрес выбранного сервера для входа сразу после запуска (Minecraft 1.20 и новее).
/// Быстрый вход на сервер появился в Minecraft 1.20; последняя версия и номера 26.x его поддерживают.
fn supports_quick_play(profile: &str) -> bool {
    let Some(version) = base_version(profile) else {
        return true;
    };
    let mut parts = version
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0));
    match (parts.next(), parts.next()) {
        (Some(1), Some(minor)) => minor >= 20,
        (Some(major), _) => major > 1,
        _ => false,
    }
}

fn quick_play_target(config: &LauncherConfig) -> Option<String> {
    let host = config.selected_server.trim();
    if !config.auto_join || host.is_empty() || !supports_quick_play(&config.selected_version) {
        return None;
    }
    let server = config
        .custom_servers
        .iter()
        .find(|server| server.ip.eq_ignore_ascii_case(host));
    // Сервер другой версии не пустит игрока: игра откроет главное меню вместо ошибки подключения.
    let server_version = server.and_then(|server| version_in(&server.desc));
    if server_version.is_some() && server_version != base_version(&config.selected_version) {
        return None;
    }
    let port = server.map_or(DEFAULT_SERVER_PORT, |server| server.port);
    Some(format!("{host}:{port}"))
}

/// Версия игры из описания сервера, например «NeoForge • 1.21.1» → «1.21.1».
fn version_in(text: &str) -> Option<&str> {
    text.split(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .find(|part| part.contains('.') && part.split('.').all(|piece| !piece.is_empty()))
}

/// NeoForge и Forge ставятся одним установщиком и запускаются одинаково.
async fn plan_forge_family(
    ctx: &LaunchContext,
    config: &LauncherConfig,
    game_dir: &Path,
    identity: &PlayerIdentity,
    loader: ModLoader,
    managed_by_backend: bool,
) -> Result<LaunchPlan, LauncherError> {
    let minecraft = base_version(&config.selected_version)
        .ok_or_else(|| {
            LauncherError::BadData(format!(
                "в сборке «{}» не указана версия игры",
                config.selected_version
            ))
        })?
        .to_string();
    let mut vanilla = prepare_vanilla(ctx, &minecraft).await?;
    let major = required_java_major(&vanilla.json);
    let java = ensure_java(ctx, &config.java_path, major).await?;

    let versions_dir = ctx.root.join("versions");
    if find_installed_loader(loader, &versions_dir, &minecraft).is_none() {
        install_loader(ctx, loader, &minecraft, &java).await?;
    }
    let (neo_id, neo_path) =
        find_installed_loader(loader, &versions_dir, &minecraft).ok_or_else(|| {
            LauncherError::NeoForgeInstall("установщик не создал файлы версии".to_string())
        })?;
    let neo_json: Value = serde_json::from_str(&std::fs::read_to_string(&neo_path)?)
        .map_err(|e| LauncherError::BadData(format!("{neo_id}: {e}")))?;

    // Ванильный клиент исключается: NeoForge использует свой пропатченный,
    // а два клиента на classpath дают конфликт модулей при старте.
    vanilla.classpath.pop();
    let mut libraries = LibrarySet::seeded(&vanilla.classpath);
    let libs_dir = ctx.root.join("libraries");
    libraries.collect(&neo_json, &libs_dir, current_os());
    libraries.ensure_resolved()?;
    ensure_files(
        &ctx.client,
        std::mem::take(&mut libraries.downloads),
        &ctx.reporter,
        &format!("Файлы {}", loader.name()),
        LIBRARY_CONCURRENCY,
    )
    .await?;
    vanilla.classpath.append(&mut libraries.classpath);
    let neo_jar = versions_dir.join(&neo_id).join(format!("{neo_id}.jar"));
    if neo_jar.is_file() {
        vanilla.classpath.push(neo_jar);
    }

    let mod_loader = loader.name().to_lowercase();
    if wants_shaders(&config.selected_version) {
        ensure_optional_shaders(ctx, game_dir, &minecraft, &mod_loader).await?;
    }
    prepare_skins(
        ctx,
        config,
        game_dir,
        &minecraft,
        &mod_loader,
        managed_by_backend,
    )
    .await;

    let classpath = join_classpath(&vanilla.classpath);
    let placeholders = JvmPlaceholders {
        natives_root: &vanilla.natives_dir,
        libraries: &libs_dir,
        classpath: &classpath,
        version: &neo_id,
        launcher_version: &ctx.launcher_version,
    };
    let mut args = standard_jvm_args(config, &vanilla.json, &placeholders);
    args.extend(
        neo_json["arguments"]["jvm"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|text| placeholders.apply(text)),
    );
    // Ely.by: authlib-injector подключается до основного класса игры.
    args.extend(identity.java_agent.iter().cloned());
    args.extend(custom_jvm_args(config));
    args.push(
        neo_json["mainClass"]
            .as_str()
            .unwrap_or("cpw.mods.bootstraplauncher.BootstrapLauncher")
            .to_string(),
    );
    args.extend(game_args(
        config,
        game_dir,
        &GameArgs {
            assets_dir: &ctx.root.join("assets"),
            version: &neo_id,
            asset_id: &vanilla.asset_id,
            identity,
        },
    ));
    for arg in neo_json["arguments"]["game"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(text) = arg.as_str() {
            args.push(text.to_string());
        }
    }

    Ok(LaunchPlan {
        java,
        args,
        work_dir: game_dir.to_path_buf(),
    })
}

async fn plan_vanilla_or_fabric(
    ctx: &LaunchContext,
    config: &LauncherConfig,
    game_dir: &Path,
    identity: &PlayerIdentity,
    managed_by_backend: bool,
) -> Result<LaunchPlan, LauncherError> {
    let profile = &config.selected_version;
    let mut vanilla = prepare_vanilla(ctx, profile).await?;
    let major = required_java_major(&vanilla.json);
    let java = ensure_java(ctx, &config.java_path, major).await?;

    let mut main_class = vanilla.json["mainClass"]
        .as_str()
        .unwrap_or("net.minecraft.client.main.Main")
        .to_string();
    let mut fabric_jvm_args = Vec::new();
    if loader_of(profile) == Loader::Fabric {
        let fabric = prepare_fabric(ctx, &vanilla.version, &mut vanilla.classpath).await?;
        main_class = fabric.main_class;
        fabric_jvm_args = fabric.jvm_args;
        if wants_shaders(profile) {
            ensure_optional_shaders(ctx, game_dir, &vanilla.version, "fabric").await?;
        }
        prepare_skins(
            ctx,
            config,
            game_dir,
            &vanilla.version,
            "fabric",
            managed_by_backend,
        )
        .await;
    }

    let classpath = join_classpath(&vanilla.classpath);
    let placeholders = JvmPlaceholders {
        natives_root: &vanilla.natives_dir,
        libraries: &ctx.root.join("libraries"),
        classpath: &classpath,
        version: &vanilla.version,
        launcher_version: &ctx.launcher_version,
    };
    let mut args = standard_jvm_args(config, &vanilla.json, &placeholders);
    // Эти флаги нужны только без настоящей сессии: иначе игра сама обращается к сервисам Mojang.
    if !identity.licensed {
        args.push("-Dauthlib.userApiService.enabled=false".to_string());
        args.push("-Dminecraft.api.env=custom".to_string());
    }
    if major >= 25 {
        args.push("--sun-misc-unsafe-memory-access=allow".to_string());
    }
    args.extend(fabric_jvm_args);
    // Ely.by: authlib-injector подключается до основного класса игры.
    args.extend(identity.java_agent.iter().cloned());
    args.extend(custom_jvm_args(config));
    args.push(main_class);
    args.extend(game_args(
        config,
        game_dir,
        &GameArgs {
            assets_dir: &ctx.root.join("assets"),
            version: &vanilla.version,
            asset_id: &vanilla.asset_id,
            identity,
        },
    ));

    Ok(LaunchPlan {
        java,
        args,
        work_dir: game_dir.to_path_buf(),
    })
}

fn default_options_text(root: &Path) -> String {
    let mut packs = vec!["\"vanilla\"".to_string(), "\"mod_resources\"".to_string()];
    for entry in std::fs::read_dir(root.join("resourcepacks"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".zip") {
            packs.push(format!("\"file/{name}\""));
        }
    }
    format!(
        "lang:ru_ru\nnarrator:0\nguiScale:0\nmaxFps:260\nenableVsync:false\nrenderDistance:12\nsimulationDistance:10\ngamma:0.5\ntutorialStep:none\njoinedFirstServer:true\nresourcePacks:[{}]\n",
        packs.join(",")
    )
}

fn with_narrator_off(existing: &str) -> String {
    let mut lines: Vec<String> = existing
        .lines()
        .map(|line| {
            if line.starts_with("narrator:") {
                "narrator:0".to_string()
            } else {
                line.to_string()
            }
        })
        .collect();
    if !lines.iter().any(|line| line == "narrator:0") {
        lines.push("narrator:0".to_string());
    }
    lines.join("\n") + "\n"
}

fn ensure_default_game_options(root: &Path) -> Result<(), LauncherError> {
    let options = root.join("options.txt");
    let contents = match std::fs::read_to_string(&options) {
        Ok(existing) => with_narrator_off(&existing),
        Err(_) => default_options_text(root),
    };
    write_atomic(&options, contents.as_bytes())?;

    let iris = root.join("config").join("iris.properties");
    if !iris.exists() {
        let shader = std::fs::read_dir(root.join("shaderpacks"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .find(|name| name.ends_with(".zip"));
        let contents = match shader {
            Some(name) => format!("enableShaders=true\nshaderPack={name}\n"),
            None => "enableShaders=true\n".to_string(),
        };
        write_atomic(&iris, contents.as_bytes())?;
    }
    Ok(())
}

/// Проверяет и докачивает всё нужное и собирает команду запуска. Сама игра здесь не запускается.
pub async fn prepare_launch(
    ctx: &LaunchContext,
    config: &LauncherConfig,
) -> Result<LaunchPlan, LauncherError> {
    prepare_launch_as(ctx, config, &PlayerIdentity::offline(&config.username)).await
}

/// То же для указанного игрока: пиратки или лицензии.
pub async fn prepare_launch_as(
    ctx: &LaunchContext,
    config: &LauncherConfig,
    identity: &PlayerIdentity,
) -> Result<LaunchPlan, LauncherError> {
    // Ник с сервера входа (Microsoft, Ely.by) не проверяется правилами пиратки: например, у Ely.by бывает дефис.
    if !identity.licensed && !is_valid_username(&identity.username) {
        return Err(LauncherError::InvalidUsername);
    }
    let profile = &config.selected_version;
    ctx.reporter
        .log(format!("[Запуск] Подготовка сборки: {profile}"));
    std::fs::create_dir_all(&ctx.root)?;

    let game_dir = prepare_instance(&ctx.root, profile, &ctx.reporter)?;
    let outcome = sync_profile(ctx, profile, &config.backend_url).await?;
    if matches!(outcome, SyncOutcome::Unavailable) {
        ctx.reporter
            .log("[Сборка] Сохранённого манифеста нет, запускаем с файлами на диске.");
    }
    ensure_default_game_options(&game_dir)?;
    let managed = matches!(outcome, SyncOutcome::Fresh | SyncOutcome::Cached);

    match loader_of(profile) {
        Loader::NeoForge => {
            plan_forge_family(
                ctx,
                config,
                &game_dir,
                identity,
                ModLoader::NeoForge,
                managed,
            )
            .await
        }
        Loader::Forge => {
            plan_forge_family(ctx, config, &game_dir, identity, ModLoader::Forge, managed).await
        }
        Loader::Vanilla | Loader::Fabric => {
            plan_vanilla_or_fabric(ctx, config, &game_dir, identity, managed).await
        }
    }
}

fn pipe_lines<R: Read + Send + 'static>(
    stream: R,
    prefix: &'static str,
    reporter: Reporter,
    log_file: Arc<Mutex<Option<File>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let formatted = format!("[{prefix}] {line}");
            if let Ok(mut guard) = log_file.lock() {
                if let Some(file) = guard.as_mut() {
                    let _ = writeln!(file, "{formatted}");
                }
            }
            reporter.log(formatted);
        }
    })
}

/// Выводит причину из аварийных отчётов JVM, созданных во время этого запуска.
fn report_new_crash_files(work_dir: &Path, started: SystemTime, reporter: &Reporter) {
    for entry in std::fs::read_dir(work_dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_crash_file = name.starts_with("hs_err_pid") && name.ends_with(".log");
        let is_new = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .is_ok_and(|modified| modified >= started);
        if !is_crash_file || !is_new {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for line in text.lines().take(CRASH_LINES_TO_SCAN) {
            if line.contains("Problematic frame") || line.contains("EXCEPTION_") {
                reporter.log(format!("[Вылет] {line}"));
            }
        }
    }
}

/// Запускает игру и ждёт её завершения. Вызывается в отдельном потоке.
pub fn run_game(plan: &LaunchPlan, reporter: &Reporter) -> Result<(), LauncherError> {
    let log_path = plan.work_dir.join("logs").join("fenixcraft-debug.log");
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let log_file = Arc::new(Mutex::new(File::create(&log_path).ok()));

    let mut command = Command::new(&plan.java);
    command
        .args(&plan.args)
        .current_dir(&plan.work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut command);

    let started = SystemTime::now();
    let mut child = command
        .spawn()
        .map_err(|e| LauncherError::JvmSpawn(e.to_string()))?;
    reporter.running();

    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(pipe_lines(stdout, "MC", reporter.clone(), log_file.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(pipe_lines(
            stderr,
            "JVM",
            reporter.clone(),
            log_file.clone(),
        ));
    }
    let status = child.wait()?;
    for reader in readers {
        let _ = reader.join();
    }

    if status.success() {
        reporter.log("[Запуск] Игра закрыта.");
        return Ok(());
    }
    report_new_crash_files(&plan.work_dir, started, reporter);
    Err(LauncherError::GameCrashed {
        code: status.code().unwrap_or(-1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_is_detected_from_the_profile_id() {
        assert_eq!(loader_of("latest-vanilla"), Loader::Vanilla);
        assert_eq!(loader_of("1.21.11-vanilla"), Loader::Vanilla);
        assert_eq!(loader_of("1.21.11-fabric"), Loader::Fabric);
        assert_eq!(loader_of("1.21.11-iris"), Loader::Fabric);
        assert_eq!(loader_of("1.21.11-iris-fabric"), Loader::Fabric);
        assert_eq!(loader_of("1.21.1-neoforge"), Loader::NeoForge);
        assert_eq!(loader_of("1.21.1-iris-neoforge"), Loader::NeoForge);
        assert_eq!(loader_of("26.2-iris-neoforge"), Loader::NeoForge);
        assert_eq!(loader_of("1.21.11-forge"), Loader::Forge);
        assert_eq!(loader_of("1.21.11-iris-forge"), Loader::Forge);
        assert!(!wants_shaders("1.21.11-forge"));
        assert!(wants_shaders("26.2-iris-neoforge"));
        assert_eq!(base_version("26.2-iris-neoforge"), Some("26.2"));
    }

    #[test]
    fn heap_arguments_respect_the_configured_memory() {
        let config = LauncherConfig {
            ram: 1024,
            ..LauncherConfig::default()
        };
        let args = memory_and_encoding_args(&config);

        assert!(args.contains(&"-Xms1024M".to_string()));
        assert!(args.contains(&"-Xmx1024M".to_string()));
    }

    fn placeholders<'a>() -> JvmPlaceholders<'a> {
        JvmPlaceholders {
            natives_root: Path::new("natives"),
            libraries: Path::new("libs"),
            classpath: "a.jar",
            version: "26.3",
            launcher_version: "1.2.3",
        }
    }

    #[test]
    fn official_jvm_arguments_are_applied_with_rules_and_placeholders() {
        let version = serde_json::json!({ "arguments": { "jvm": [
            { "rules": [{ "action": "allow", "os": { "name": "osx" } }], "value": ["-XstartOnFirstThread"] },
            { "rules": [{ "action": "allow", "os": { "name": "linux" } }], "value": "-Dlinux.only=true" },
            { "rules": [{ "action": "allow", "features": { "is_demo_user": true } }], "value": "--demo" },
            "-XX:StackShadowPages=32",
            "--add-exports",
            "java.base/jdk.internal.misc=ALL-UNNAMED",
            "-Djava.library.path=${natives_directory}/java",
            "-Dminecraft.launcher.brand=${launcher_name}",
            "-Dminecraft.launcher.version=${launcher_version}",
            "-cp",
            "${classpath}"
        ] } });

        let args = official_jvm_args(&version, "linux", &placeholders()).unwrap();

        assert_eq!(
            args,
            vec![
                "-Dlinux.only=true",
                "-XX:StackShadowPages=32",
                "--add-exports",
                "java.base/jdk.internal.misc=ALL-UNNAMED",
                "-Djava.library.path=natives/java",
                "-Dminecraft.launcher.brand=FenixCraft",
                "-Dminecraft.launcher.version=1.2.3",
                "-cp",
                "a.jar",
            ]
        );
    }

    #[test]
    fn versions_without_jvm_arguments_use_the_legacy_set() {
        let config = LauncherConfig::default();
        let args = standard_jvm_args(&config, &serde_json::json!({}), &placeholders());

        assert!(args.contains(&"-Djava.library.path=natives".to_string()));
        assert!(args.contains(&"-cp".to_string()));
        assert!(args.contains(&"a.jar".to_string()));
    }

    #[test]
    fn natives_folder_follows_the_version_file() {
        let modern = serde_json::json!({ "arguments": { "jvm": ["-Djava.library.path=${natives_directory}/java"] } });
        let older = serde_json::json!({ "arguments": { "jvm": ["-Djava.library.path=${natives_directory}"] } });
        let root = Path::new("natives");

        assert_eq!(native_library_dir(&modern, root), root.join("java"));
        assert_eq!(native_library_dir(&older, root), root);
    }

    #[test]
    fn custom_jvm_args_cannot_override_the_memory_setting() {
        let config = LauncherConfig {
            jvm_args: "-Xmx99G -XX:+UseG1GC -Xms99G".to_string(),
            ..LauncherConfig::default()
        };
        assert_eq!(
            custom_jvm_args(&config).collect::<Vec<_>>(),
            vec!["-XX:+UseG1GC"]
        );
    }

    #[test]
    fn turned_off_custom_arguments_are_not_passed_to_the_game() {
        let config = LauncherConfig {
            jvm_args: "-XX:+UseG1GC".to_string(),
            game_args: "--fullscreen".to_string(),
            custom_args_enabled: false,
            ..LauncherConfig::default()
        };

        assert_eq!(custom_jvm_args(&config).count(), 0);
        assert!(custom_game_args(&config).is_empty());
    }

    #[test]
    fn custom_game_arguments_cannot_replace_the_session() {
        let config = LauncherConfig {
            game_args: "--fullscreen --username Hacker --demo --uuid".to_string(),
            ..LauncherConfig::default()
        };

        assert_eq!(custom_game_args(&config), vec!["--fullscreen", "--demo"]);
    }

    #[test]
    fn launcher_arguments_cannot_be_repeated_in_any_spelling() {
        let config = LauncherConfig {
            game_args: "--width 1920 --user=Hacker --quickPlayMultiplayer x --fullscreen --demo"
                .to_string(),
            ..LauncherConfig::default()
        };

        assert_eq!(custom_game_args(&config), vec!["--fullscreen", "--demo"]);
    }

    #[test]
    fn quick_join_is_only_for_versions_that_support_it() {
        assert!(supports_quick_play("latest-vanilla"));
        assert!(supports_quick_play("26.2-iris-neoforge"));
        assert!(supports_quick_play("1.20.1-forge"));
        assert!(!supports_quick_play("1.19.4-vanilla"));
        assert!(!supports_quick_play("1.12.2-forge"));
    }

    #[test]
    fn narrator_is_forced_off_without_touching_other_options() {
        assert_eq!(
            with_narrator_off("narrator:2\nfov:0.5\n"),
            "narrator:0\nfov:0.5\n"
        );
        assert_eq!(with_narrator_off("fov:0.5\n"), "fov:0.5\nnarrator:0\n");
    }

    #[test]
    fn game_arguments_contain_the_player_and_version() {
        let config = LauncherConfig {
            username: "Fenix".to_string(),
            ..LauncherConfig::default()
        };
        let args = game_args(
            &config,
            Path::new("instance"),
            &GameArgs {
                assets_dir: Path::new("root/assets"),
                version: "1.21.11",
                asset_id: "29",
                identity: &PlayerIdentity::offline("Fenix"),
            },
        );
        let value_of = |name: &str| {
            let at = args.iter().position(|arg| arg == name).unwrap();
            args[at + 1].clone()
        };

        assert_eq!(value_of("--gameDir"), "instance");
        assert_eq!(value_of("--assetsDir"), "root/assets");
        assert_eq!(value_of("--username"), "Fenix");
        assert_eq!(value_of("--version"), "1.21.11");
        assert_eq!(value_of("--assetIndex"), "29");
        assert_eq!(value_of("--uuid"), offline_uuid("Fenix"));
    }

    #[test]
    fn licensed_player_gets_the_real_session_in_game_arguments() {
        let identity = PlayerIdentity::licensed(MinecraftAccount {
            username: "RealPlayer".to_string(),
            uuid: "abc".to_string(),
            access_token: "mc-token".to_string(),
            xuid: "2535".to_string(),
        });
        let args = game_args(
            &LauncherConfig::default(),
            Path::new("instance"),
            &GameArgs {
                assets_dir: Path::new("assets"),
                version: "1.21.11",
                asset_id: "29",
                identity: &identity,
            },
        );
        let value_of = |name: &str| {
            let at = args.iter().position(|arg| arg == name).unwrap();
            args[at + 1].clone()
        };

        assert_eq!(value_of("--username"), "RealPlayer");
        assert_eq!(value_of("--uuid"), "abc");
        assert_eq!(value_of("--accessToken"), "mc-token");
        assert_eq!(value_of("--xuid"), "2535");
        assert_eq!(value_of("--userType"), "msa");
    }

    fn quick_play_of(config: &LauncherConfig) -> Option<String> {
        let args = game_args(
            config,
            Path::new("instance"),
            &GameArgs {
                assets_dir: Path::new("assets"),
                version: "26.3",
                asset_id: "29",
                identity: &PlayerIdentity::offline("Fenix"),
            },
        );
        let at = args
            .iter()
            .position(|arg| arg == "--quickPlayMultiplayer")?;
        args.get(at + 1).cloned()
    }

    #[test]
    fn game_joins_the_selected_server_right_after_start() {
        let config = LauncherConfig {
            selected_version: "26.2-iris-neoforge".to_string(),
            selected_server: "play.fenixcraft.ru".to_string(),
            ..LauncherConfig::default()
        };

        assert_eq!(
            quick_play_of(&config).as_deref(),
            Some("play.fenixcraft.ru:25565")
        );
    }

    #[test]
    fn quick_join_uses_the_port_of_a_custom_server() {
        let mut custom = crate::config::default_servers();
        custom[0].ip = "my.server.test".to_string();
        custom[0].port = 25570;
        let config = LauncherConfig {
            selected_version: "26.2-iris-neoforge".to_string(),
            selected_server: "my.server.test".to_string(),
            custom_servers: custom,
            ..LauncherConfig::default()
        };

        assert_eq!(
            quick_play_of(&config).as_deref(),
            Some("my.server.test:25570")
        );
    }

    #[test]
    fn quick_join_is_skipped_for_a_server_of_another_version() {
        let config = LauncherConfig {
            selected_version: "1.21.1-neoforge".to_string(),
            selected_server: "play.fenixcraft.ru".to_string(),
            ..LauncherConfig::default()
        };

        assert_eq!(quick_play_of(&config), None);
    }

    #[test]
    fn quick_join_is_skipped_when_turned_off_or_no_server() {
        let off = LauncherConfig {
            auto_join: false,
            ..LauncherConfig::default()
        };
        let no_server = LauncherConfig {
            selected_server: "  ".to_string(),
            ..LauncherConfig::default()
        };

        assert_eq!(quick_play_of(&off), None);
        assert_eq!(quick_play_of(&no_server), None);
    }

    #[test]
    fn stale_crash_reports_are_not_shown_for_a_new_launch() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("hs_err_pid1.log"),
            "# Problematic frame: old\n",
        )
        .unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(60);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let reporter = Reporter::new(move |event| sink.lock().unwrap().push(event));

        report_new_crash_files(dir.path(), later, &reporter);
        assert!(seen.lock().unwrap().is_empty());

        report_new_crash_files(dir.path(), SystemTime::UNIX_EPOCH, &reporter);
        assert_eq!(seen.lock().unwrap().len(), 1);
    }
}
