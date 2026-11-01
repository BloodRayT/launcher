<div align="center">
  <img src="logo.png" alt="FenixCraft Logo" width="120" />
  <h1>🦅 FenixCraft Launcher</h1>
  <p><b>Официальный кроссплатформенный лаунчер игрового проекта FenixCraft</b></p>

  ![Version](https://img.shields.io/badge/version-1.0.1-emerald?style=for-the-badge)
  ![Tauri](https://img.shields.io/badge/Tauri_v2-24C8D5?style=for-the-badge&logo=tauri&logoColor=white)
  ![Rust](https://img.shields.io/badge/Rust-black?style=for-the-badge&logo=rust&logoColor=white)
  ![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-blue?style=for-the-badge)
</div>

---

## 📌 О проекте

**FenixCraft Launcher** — легковесный, производительный и безопасный лаунчер для Minecraft, разработанный на базе **Tauri v2** и **Rust**. Лаунчер обеспечивает быструю загрузку игровых клиентов, автоматическую установку среды выполнения Java, синхронизацию модификаций и прямое подключение к игровым серверам комплекса FenixCraft.

---

## ✨ Ключевые возможности

* 🎮 **Поддержка актуальных сборок:**
  * Vanilla (актуальные релизы Mojang с автоматической загрузкой ассетов).
  * NeoForge 1.21.1 (FenixTech).
  * Fabric + Iris & Sodium (FenixRPG с поддержкой шейдеров).
  * Анархия / PvP (FenixArena).
* ☕ **Встроенный Smart Java Manager:**
  * Автоматическое сканирование установленных версий Java в системе.
  * Фоновая загрузка и распаковка портативной среды **Adoptium Temurin OpenJDK (Java 21 / Java 25)** при её отсутствии.
* ⚡ **Мгновенная синхронизация профиля:**
  * Поддержка пиратского режима (Offline) со сменой никнейма в интерфейсе без перезапуска.
  * Лицензионная авторизация через защищённый шлюз.
  * Установка пользовательской аватарки прямо из лаунчера.
* 🔄 **OTA Автообновление (Auto-Updater):**
  * Проверка новых версий через GitHub Releases API при каждом запуске.
  * Встроенное модальное окно со списком изменений (Changelog) и установкой в один клик.
* 🌐 **Интеграция с сервисами проекта:**
  * Мониторинг онлайна серверов FenixCraft в реальном времени через TCP-пинг.
  * Живая лента новостей из официального сообщества ВКонтакте.
  * Индикатор прямого эфира Twitch с динамической плашкой LIVE.
  * Встроенный магазин привилегий и доната.
* 🛠️ **Диагностика и логирование:**
  * Встроенная консоль реального времени с перехватом потоков `stdout`/`stderr` JVM.
  * Кнопка сохранения полного отчёта `fenixcraft-debug.log` и мгновенное открытие папки игры.

---

## 🏗️ Стек технологий

* **Ядро:** Rust 2021, Tauri v2
* **Фронтенд:** HTML5, Modern JavaScript, Tailwind CSS (Glassmorphism UI)
* **Сетевой стек:** `reqwest`, `tokio` (асинхронная многопоточная загрузка библиотек и ассетов)
* **Криптография:** `sha2`, `uuid` (валидация файлов по контрольным суммам SHA-256)
* **Архивация:** `zip`, `tar`

---

## 🚀 Сборка из исходного кода

### Предварительные требования

1. **Rust:** Установленный компилятор Rust (`rustup`):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh
