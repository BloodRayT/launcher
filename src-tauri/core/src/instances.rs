use crate::errors::LauncherError;
use crate::reporter::Reporter;
use crate::safety::is_safe_profile_id;
use std::path::{Path, PathBuf};

const INSTANCES_DIR: &str = "instances";
const LEGACY_MARKER: &str = ".legacy-migrated";
/// Данные игрока из общей папки старых версий. Переносятся один раз, в выбранную сборку.
const LEGACY_FOLDERS: [&str; 4] = ["saves", "screenshots", "resourcepacks", "shaderpacks"];
/// Настройки, с которых удобно начинать новую сборку. Копируются, оригинал остаётся.
const SEEDED_FILES: [&str; 2] = ["options.txt", "servers.dat"];

/// Папка игры конкретной сборки. Версии, библиотеки, ресурсы и Java остаются общими.
pub fn instance_dir(root: &Path, profile: &str) -> Result<PathBuf, LauncherError> {
    if !is_safe_profile_id(profile) {
        return Err(LauncherError::BadData(format!(
            "недопустимое имя сборки «{profile}»"
        )));
    }
    Ok(root.join(INSTANCES_DIR).join(profile))
}

fn migrate_legacy_once(root: &Path, instance: &Path, reporter: &Reporter) -> std::io::Result<()> {
    let marker = root.join(INSTANCES_DIR).join(LEGACY_MARKER);
    if marker.exists() {
        return Ok(());
    }
    for folder in LEGACY_FOLDERS {
        let (from, to) = (root.join(folder), instance.join(folder));
        if from.is_dir() && !to.exists() {
            std::fs::rename(&from, &to)?;
            reporter.log(format!(
                "[Сборка] Папка {folder} перенесена в {}",
                instance.display()
            ));
        }
    }
    if root.join("mods").is_dir() {
        reporter.log(format!(
            "[Сборка] Моды из общей папки не подключаются к сборкам, файлы остались в {}",
            root.join("mods").display()
        ));
    }
    std::fs::write(marker, "")
}

fn seed_settings(root: &Path, instance: &Path) -> std::io::Result<()> {
    for file in SEEDED_FILES {
        let (from, to) = (root.join(file), instance.join(file));
        if from.is_file() && !to.exists() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Создаёт папку сборки и один раз переносит в неё данные из общей папки старых версий.
pub fn prepare_instance(
    root: &Path,
    profile: &str,
    reporter: &Reporter,
) -> Result<PathBuf, LauncherError> {
    let instance = instance_dir(root, profile)?;
    std::fs::create_dir_all(&instance)?;
    migrate_legacy_once(root, &instance, reporter)?;
    seed_settings(root, &instance)?;
    Ok(instance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_name_cannot_leave_the_instances_folder() {
        let root = Path::new("root");
        assert_eq!(
            instance_dir(root, "1.21.11-fabric").unwrap(),
            root.join("instances").join("1.21.11-fabric")
        );
        assert!(instance_dir(root, "..").is_err());
        assert!(instance_dir(root, "a/b").is_err());
    }

    #[test]
    fn player_data_moves_once_and_nothing_is_lost() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("saves/World")).unwrap();
        std::fs::write(root.join("saves/World/level.dat"), "world").unwrap();
        std::fs::create_dir_all(root.join("mods")).unwrap();
        std::fs::write(root.join("mods/player.jar"), "mod").unwrap();
        std::fs::write(root.join("options.txt"), "fov:1").unwrap();

        let first = prepare_instance(root, "latest-vanilla", &Reporter::silent()).unwrap();
        assert_eq!(
            std::fs::read_to_string(first.join("saves/World/level.dat")).unwrap(),
            "world"
        );
        assert!(!root.join("saves").exists());
        assert!(root.join("mods/player.jar").exists(), "mods stay in place");
        assert!(!first.join("mods/player.jar").exists());
        assert_eq!(
            std::fs::read_to_string(first.join("options.txt")).unwrap(),
            "fov:1"
        );

        // Вторая сборка получает настройки, но не чужие миры.
        std::fs::create_dir_all(root.join("saves/Later")).unwrap();
        let second = prepare_instance(root, "1.21.11-fabric", &Reporter::silent()).unwrap();
        assert!(!second.join("saves").exists());
        assert!(root.join("saves/Later").exists(), "moved only once");
        assert_eq!(
            std::fs::read_to_string(second.join("options.txt")).unwrap(),
            "fov:1"
        );
    }

    #[test]
    fn existing_instance_data_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let instance = instance_dir(root, "latest-vanilla").unwrap();
        std::fs::create_dir_all(instance.join("saves/Mine")).unwrap();
        std::fs::write(instance.join("options.txt"), "mine").unwrap();
        std::fs::create_dir_all(root.join("saves/Old")).unwrap();
        std::fs::write(root.join("options.txt"), "old").unwrap();

        prepare_instance(root, "latest-vanilla", &Reporter::silent()).unwrap();

        assert_eq!(
            std::fs::read_to_string(instance.join("options.txt")).unwrap(),
            "mine"
        );
        assert!(instance.join("saves/Mine").exists());
        assert!(root.join("saves/Old").exists());
    }
}
