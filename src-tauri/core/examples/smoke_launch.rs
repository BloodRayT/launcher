//! Ручная проверка на настоящих серверах Mojang и настоящей игре. В CI не запускается.
//!
//! cargo run -p fenix-core --example smoke_launch -- <пустая папка> [сборка] [секунд работы игры]
//!
//! Скачивает игру, повторяет подготовку с недоступной сетью и запускает Minecraft на указанное время.

use fenix_core::config::LauncherConfig;
use fenix_core::launch::{loader_of, prepare_launch, LaunchContext, Loader};
use fenix_core::reporter::{GameEvent, Reporter};
use fenix_core::Endpoints;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DEAD_URL: &str = "http://127.0.0.1:1";
const LOG_MARKERS: [&str; 4] = [
    "Setting user",
    "Backend library",
    "Reloading ResourceManager",
    "OpenAL initialized",
];

fn console_reporter() -> Reporter {
    Reporter::new(|event| match event {
        GameEvent::Log(line) => println!("{line}"),
        GameEvent::Progress { label, task, total } if task == total => {
            println!("[готово] {label}: {total}")
        }
        _ => {}
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().ok_or("укажите папку")?);
    let profile = args.next().unwrap_or_else(|| "latest-vanilla".to_string());
    let seconds: u64 = args.next().map_or(Ok(45), |value| value.parse())?;
    std::fs::create_dir_all(&root)?;

    let config = LauncherConfig {
        username: "Fenix_Smoke".to_string(),
        selected_version: profile,
        ram: 2048,
        // Сборкам с модами нужен рабочий бекенд, адрес можно указать для локальной проверки.
        backend_url: std::env::var("SMOKE_BACKEND_URL")
            .unwrap_or_else(|_| LauncherConfig::default().backend_url),
        ..LauncherConfig::default()
    };
    let official = root.join("no-official-launcher");

    let started = Instant::now();
    let online_ctx = LaunchContext::new(
        root.clone(),
        official.clone(),
        Endpoints::default(),
        "smoke",
        console_reporter(),
    )?;
    let online = prepare_launch(&online_ctx, &config).await?;
    println!(
        "ONLINE OK за {:?}, Java: {}",
        started.elapsed(),
        online.java
    );

    let dead = Endpoints {
        version_manifest: format!("{DEAD_URL}/manifest"),
        assets_base: format!("{DEAD_URL}/assets"),
        fabric_meta: format!("{DEAD_URL}/fabric"),
        modrinth: format!("{DEAD_URL}/modrinth"),
        adoptium: format!("{DEAD_URL}/adoptium"),
        neoforge_mavens: vec![format!("{DEAD_URL}/neoforge")],
    };
    let offline_config = LauncherConfig {
        backend_url: if loader_of(&config.selected_version) == Loader::Vanilla {
            DEAD_URL.to_string()
        } else {
            config.backend_url.clone()
        },
        ..config.clone()
    };
    let started = Instant::now();
    let offline_ctx =
        LaunchContext::new(root.clone(), official, dead, "smoke", Reporter::silent())?;
    let offline = prepare_launch(&offline_ctx, &offline_config).await?;
    if offline.args != online.args || offline.java != online.java {
        return Err("без сети получилась другая команда запуска".into());
    }
    println!(
        "OFFLINE OK за {:?}: команда запуска совпадает",
        started.elapsed()
    );

    let command_line = std::iter::once(offline.java.clone())
        .chain(offline.args.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(root.join("smoke-command.txt"), command_line)?;

    let log_path = root.join("smoke-game.log");
    let log = std::fs::File::create(&log_path)?;
    let mut game = Command::new(&offline.java)
        .args(&offline.args)
        .current_dir(&offline.work_dir)
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log))
        .spawn()?;
    std::thread::sleep(Duration::from_secs(seconds));
    let exited_early = game.try_wait()?;
    let _ = game.kill();
    let _ = game.wait();

    let text = String::from_utf8_lossy(&std::fs::read(&log_path)?).to_string();
    for marker in LOG_MARKERS {
        println!(
            "лог игры, «{marker}»: {}",
            if text.contains(marker) {
                "есть"
            } else {
                "нет"
            }
        );
    }
    match exited_early {
        Some(status) => {
            let tail: Vec<&str> = text.lines().rev().take(30).collect();
            println!("{}", tail.into_iter().rev().collect::<Vec<_>>().join("\n"));
            Err(format!("игра завершилась раньше времени: {status}").into())
        }
        None => {
            println!("GAME OK: игра проработала {seconds} с");
            Ok(())
        }
    }
}
