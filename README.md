<div align="center">
  <img src="logo.png" alt="FenixCraft Logo" width="120" />
  <h1>🦅 FenixCraft Launcher</h1>
  <p><b>Официальный кроссплатформенный лаунчер игрового проекта FenixCraft</b></p>

  ![Tauri](https://img.shields.io/badge/Tauri_v2-24C8D5?style=for-the-badge&logo=tauri&logoColor=white)
  ![Rust](https://img.shields.io/badge/Rust-black?style=for-the-badge&logo=rust&logoColor=white)
  ![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-blue?style=for-the-badge)
</div>

---

## 📌 О проекте

**FenixCraft Launcher** — лаунчер Minecraft на **Tauri v2** и **Rust**. Он скачивает клиент и Java, синхронизирует файлы сборок с сервером проекта и запускает установленные сборки без интернета.

## ✨ Возможности

* 🎮 **Сборки:** Vanilla, Fabric, Fabric + Iris и Sodium, NeoForge с автоматической установкой. Если основной сервер NeoForge недоступен, используется зеркало.
* 🗂️ **Отдельные папки:** у каждой сборки свои моды, настройки и миры в `instances/<сборка>`, поэтому моды разных загрузчиков не конфликтуют. Версии, библиотеки, ресурсы и Java общие и скачиваются один раз. При первом запуске новой версии миры, скриншоты, ресурс-паки и шейдеры из общей папки переносятся в выбранную сборку.
* 📴 **Работа без сети:** после первой полной загрузки сборка запускается без интернета. Если файлов не хватает, лаунчер объясняет, что нужно докачать. При недоступности сервера сборок используется последний сохранённый манифест, и файлы не удаляются.
* 🔒 **Проверка файлов:** каждый скачанный файл сверяется по SHA-1 или SHA-256 и заменяет старый только после проверки. Пути из внешних данных не могут выйти за папку игры.
* ☕ **Java:** подходящая Java находится на компьютере или скачивается с Adoptium с проверкой контрольной суммы.
* 👤 **Профиль:** вход без пароля с ником или проверка аккаунта Ely.by. Запуск игры с сессией Ely.by пока не поддерживается: игра идёт в офлайн-режиме.
* 🔄 **Обновления:** проверка через GitHub Releases; установщик скачивается только из релизов этого репозитория.
* 🌐 **Сервисы проекта:** статус серверов, новости ВКонтакте с сохранением на случай отсутствия сети, индикатор стрима на Twitch, магазин.
* 🛠️ **Диагностика:** консоль с выводом игры, полная проверка ресурсов, сохранение журнала, понятные сообщения об ошибках.

## 🏗️ Стек

* **Приложение:** Rust, Tauri v2
* **Логика лаунчера:** крейт `fenix-core` без зависимости от Tauri, что позволяет тестировать её отдельно
* **Интерфейс:** HTML, JavaScript (ES-модули), Tailwind CSS, собранный локально
* **Бекенд:** Python, FastAPI

## 🚀 Разработка

Нужны [Rust](https://rustup.rs) и Node.js 20+. На Linux дополнительно:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libasound2-dev libssl-dev
```

```bash
git clone https://github.com/BloodRayT/launcher.git
cd launcher
npm ci
```

```bash
npm run tauri dev
```

Сборка установщиков под текущую ОС:

```bash
npm run tauri build
```

### Проверки

Те же проверки выполняет CI на каждый pull request.

```bash
npm test
npm run check:versions
```

```bash
cd src-tauri && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p fenix-core
```

```bash
cd backend && pip install -r requirements-dev.txt && python -m pytest tests
```

Проверка на настоящих серверах Mojang с запуском игры. Она откроет окно Minecraft:

```bash
cd src-tauri && cargo run -p fenix-core --example smoke_launch -- /путь/к/пустой/папке latest-vanilla 45
```

### Выпуск версии

Версия указывается в `package.json`, `src-tauri/Cargo.toml` и `src-tauri/tauri.conf.json` и должна совпадать с git-тегом `vX.Y.Z`. Иначе сборка релиза остановится. Релиз создаётся черновиком, чтобы перед публикацией можно было дописать описание изменений.

## 🖥️ Бекенд

Настройки передаются через переменные окружения, образец находится в `backend/.env.example`.

Подпись манифестов защищает игроков от подменённых файлов сборок. Включать её нужно в таком порядке:

1. Создайте ключи:

   ```bash
   cd backend && pip install -r requirements.txt && python generate_signing_key.py
   ```

2. Запишите `MANIFEST_SIGNING_KEY` в `.env` на сервере и перезапустите бекенд. Проверьте, что `/api/launcher/health` возвращает `"signed_manifests": true`.
3. Добавьте `FENIX_MANIFEST_PUBLIC_KEY` в переменные репозитория на GitHub (Settings → Secrets and variables → Actions → Variables) и выпустите новую версию лаунчера.

Порядок важен: лаунчер с ключом не примет манифест без подписи, а релиз без этой переменной не соберётся. Бекенд по умолчанию не запустится без секретного ключа (`REQUIRE_MANIFEST_SIGNING`). Секретный ключ нельзя коммитить; если он утёк, создайте новую пару и выпустите новую версию.

Новости и статус стрима идут через бекенд, поэтому токены VK и Twitch хранятся только на сервере.

## 📂 Структура

```text
launcher/
├── .github/workflows/      # CI и сборка релизов
├── backend/                # Сервер сборок (FastAPI), файлы сборок в storage/
├── scripts/                # Проверка совпадения версий
├── src/                    # Интерфейс
├── src-tauri/
│   ├── core/               # fenix-core: скачивание, проверка, запуск игры и тесты
│   ├── src/                # Команды Tauri, новости, статус серверов
│   └── tauri.conf.json     # Окно, CSP, сборка
└── tests/                  # Тесты интерфейса
```

## 🤝 Сообщество и контакты

* Официальный сайт: [fenixcraft.ru](https://fenixcraft.ru)
* Сообщество ВКонтакте: [vk.ru/fenixcraft](https://vk.ru/fenixcraft)
* Twitch: [twitch.tv/bloodrayt](https://www.twitch.tv/bloodrayt)
* IP сервера: `play.fenixcraft.ru:25565`
