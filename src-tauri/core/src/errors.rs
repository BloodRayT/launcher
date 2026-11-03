use serde::Serialize;

/// Ошибки запуска. Текст каждой из них показывается игроку, поэтому он написан для человека.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LauncherError {
    #[error("Ник должен содержать от 3 до 16 символов: латинские буквы, цифры или «_».")]
    InvalidUsername,
    #[error("Нужно докачать файлы ({what}), но интернет недоступен.")]
    NeedsInternet { what: String },
    #[error("Версия {0} не найдена в списке версий Mojang.")]
    VersionNotFound(String),
    #[error("Не удалось скачать {file}: {reason}.")]
    Download { file: String, reason: String },
    #[error("Файл {file} скачан с ошибкой: контрольная сумма не совпала.")]
    Checksum { file: String },
    #[error("Не удалось установить NeoForge: {0}.")]
    NeoForgeInstall(String),
    #[error("Не удалось подготовить Java {major}: {reason}.")]
    Java { major: u32, reason: String },
    #[error("Сервер сборки прислал некорректный манифест: {0}.")]
    BadManifest(String),
    #[error("Сервер сборок сейчас недоступен: идут технические работы.")]
    BackendMaintenance,
    #[error("Файлы игры содержат некорректные данные: {0}.")]
    BadData(String),
    #[error("Ошибка работы с файлами: {0}.")]
    Io(String),
    #[error("Не удалось запустить Java: {0}.")]
    JvmSpawn(String),
    #[error("Игра завершилась с ошибкой (код {code}).")]
    GameCrashed { code: i32 },
    #[error("Игра уже запущена.")]
    AlreadyRunning,
}

impl LauncherError {
    pub fn payload(&self) -> UserError {
        let (code, title, hint) = match self {
            Self::InvalidUsername => (
                "invalid_username",
                "Проверьте ник",
                "Исправьте ник в блоке профиля и нажмите «Играть» ещё раз.",
            ),
            Self::NeedsInternet { .. } => (
                "needs_internet",
                "Нужен интернет",
                "Подключитесь к сети один раз: после полной загрузки эта сборка запускается без интернета.",
            ),
            Self::VersionNotFound(_) => (
                "version_not_found",
                "Версия недоступна",
                "Выберите другую сборку в списке рядом с кнопкой «Играть».",
            ),
            Self::Download { .. } | Self::Checksum { .. } => (
                "download_failed",
                "Ошибка загрузки",
                "Нажмите «Играть» ещё раз. Уже скачанные файлы повторно загружаться не будут.",
            ),
            Self::NeoForgeInstall(_) => (
                "neoforge_install_failed",
                "NeoForge не установился",
                "Нажмите «Играть» ещё раз. Если ошибка повторится, сохраните журнал из консоли.",
            ),
            Self::Java { .. } => (
                "java_failed",
                "Проблема с Java",
                "Проверьте интернет или укажите путь к Java в настройках.",
            ),
            Self::BadManifest(_) => (
                "bad_manifest",
                "Ошибка сервера сборки",
                "Файлы не были изменены. Попробуйте позже или сообщите администрации.",
            ),
            Self::BackendMaintenance => (
                "backend_maintenance",
                "Технические работы",
                "Сборки с модами временно недоступны, ванильные версии запускаются как обычно. Если отключён интернет, проверьте подключение.",
            ),
            Self::BadData(_) => (
                "bad_data",
                "Файлы игры повреждены",
                "Откройте консоль и нажмите «Проверить ассеты» или запустите другую сборку.",
            ),
            Self::Io(_) => (
                "io_failed",
                "Ошибка записи на диск",
                "Проверьте свободное место и права доступа к папке игры.",
            ),
            Self::JvmSpawn(_) => (
                "jvm_spawn_failed",
                "Java не запустилась",
                "Очистите путь к Java в настройках, и лаунчер подберёт её сам.",
            ),
            Self::GameCrashed { .. } => (
                "game_crashed",
                "Игра вылетела",
                "Причина записана в журнале. Откройте консоль и сохраните Debug-лог.",
            ),
            Self::AlreadyRunning => (
                "already_running",
                "Игра уже запущена",
                "Закройте текущую игру, чтобы запустить новую.",
            ),
        };
        UserError {
            code: code.to_string(),
            title: title.to_string(),
            message: self.to_string(),
            hint: hint.to_string(),
        }
    }
}

impl From<std::io::Error> for LauncherError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

/// То, что получает интерфейс в событии `game:error`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserError {
    pub code: String,
    pub title: String,
    pub message: String,
    pub hint: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_keeps_a_stable_code_and_readable_text() {
        let payload = LauncherError::NeedsInternet {
            what: "файлы игры: 3 шт.".to_string(),
        }
        .payload();

        assert_eq!(payload.code, "needs_internet");
        assert!(payload.message.contains("3 шт."));
        assert!(!payload.hint.is_empty());
    }
}
