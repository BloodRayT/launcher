#!/usr/bin/env python3
"""Зеркало сборок лаунчера на сайте: /var/www/html/download.

Берёт последний релиз BloodRayT/launcher с GitHub, скачивает все его файлы,
проверяет каждый по SHA-256, которую публикует GitHub, и делает постоянные имена
вида FenixCraft_latest_x64-setup.exe — на них ведут кнопки сайта. Рядом пишет
latest.json с номером версии и настоящими именами файлов.

Запуск вручную:          sudo python3 sync_downloads.py
Автообновление (cron):   sudo python3 sync_downloads.py --install-cron
Только стандартная библиотека Python 3.8+, работает на сервере без зависимостей.
"""
import argparse
import hashlib
import json
import os
import re
import sys
import tempfile
import urllib.request

LATEST_RELEASE_API = "https://api.github.com/repos/BloodRayT/launcher/releases/latest"
DOWNLOAD_PREFIX = "https://github.com/BloodRayT/launcher/releases/download/"
DEFAULT_DIR = "/var/www/html/download"
CRON_FILE = "/etc/cron.d/fenixcraft-downloads"
CRON_SCHEDULE = "0 * * * *"
USER_AGENT = "FenixCraftSite"
TIMEOUT_SECONDS = 120
CHUNK_BYTES = 1024 * 1024
MAX_ASSET_BYTES = 500 * 1024 * 1024
SAFE_NAME = re.compile(r"^FenixCraft[A-Za-z0-9._-]*$")
SHA256_DIGEST = re.compile(r"^sha256:([0-9a-fA-F]{64})$")
# Постоянное имя для сайта по окончанию настоящего имени файла.
LATEST_ALIASES = (
    ("exe", re.compile(r"_x64-setup\.exe$"), "FenixCraft_latest_x64-setup.exe"),
    ("msi", re.compile(r"_x64_en-US\.msi$"), "FenixCraft_latest_x64_en-US.msi"),
    ("dmg", re.compile(r"_aarch64\.dmg$"), "FenixCraft_latest_aarch64.dmg"),
    ("app", re.compile(r"_aarch64\.app\.tar\.gz$"), "FenixCraft_latest_aarch64.app.tar.gz"),
    ("appimage", re.compile(r"_amd64\.AppImage$"), "FenixCraft_latest_amd64.AppImage"),
    ("deb", re.compile(r"_amd64\.deb$"), "FenixCraft_latest_amd64.deb"),
    ("rpm", re.compile(r"\.x86_64\.rpm$"), "FenixCraft_latest_x86_64.rpm"),
)


def log(message: str) -> None:
    print(message, flush=True)


def open_url(url: str):
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    return urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS)


def release_assets(release: dict) -> list:
    """Файлы релиза, которым можно доверять: (имя, адрес, sha256)."""
    assets = []
    for asset in release.get("assets") or []:
        name = asset.get("name") or ""
        url = asset.get("browser_download_url") or ""
        digest = SHA256_DIGEST.fullmatch(asset.get("digest") or "")
        if SAFE_NAME.fullmatch(name) and url.startswith(DOWNLOAD_PREFIX) and digest:
            assets.append((name, url, digest.group(1).lower()))
    return assets


def sha256_of(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(CHUNK_BYTES), b""):
            h.update(chunk)
    return h.hexdigest()


def download(url: str, dest: str, sha256: str) -> None:
    """Качает во временный файл и подменяет итоговый только после проверки SHA-256."""
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(dest), suffix=".part")
    try:
        h = hashlib.sha256()
        size = 0
        with os.fdopen(fd, "wb") as out, open_url(url) as response:
            for chunk in iter(lambda: response.read(CHUNK_BYTES), b""):
                size += len(chunk)
                if size > MAX_ASSET_BYTES:
                    raise ValueError(f"{os.path.basename(dest)}: файл слишком большой")
                h.update(chunk)
                out.write(chunk)
        if h.hexdigest() != sha256:
            raise ValueError(f"{os.path.basename(dest)}: SHA-256 не совпала с GitHub")
        os.chmod(tmp, 0o644)
        os.replace(tmp, dest)
    finally:
        if os.path.exists(tmp):
            os.remove(tmp)


def link_latest(directory: str, names: list) -> dict:
    """Делает постоянные имена жёсткими ссылками на файлы текущей версии."""
    files = {}
    for key, pattern, alias in LATEST_ALIASES:
        name = next((n for n in names if pattern.search(n)), None)
        if name is None:
            continue
        alias_path = os.path.join(directory, alias)
        tmp = alias_path + ".tmp"
        if os.path.exists(tmp):
            os.remove(tmp)
        os.link(os.path.join(directory, name), tmp)
        os.replace(tmp, alias_path)
        files[key] = name
    return files


def remove_stale(directory: str, keep: set) -> None:
    for name in os.listdir(directory):
        if SAFE_NAME.fullmatch(name) and name not in keep:
            os.remove(os.path.join(directory, name))
            log(f"Удалён старый файл {name}")


def write_json(path: str, data: dict) -> None:
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=1)
    os.chmod(tmp, 0o644)
    os.replace(tmp, path)


def sync(directory: str) -> str:
    with open_url(LATEST_RELEASE_API) as response:
        release = json.load(response)
    version = (release.get("tag_name") or "").lstrip("v")
    assets = release_assets(release)
    if not version or not assets:
        raise ValueError("в последнем релизе нет файлов с контрольными суммами")
    os.makedirs(directory, exist_ok=True)
    for name, url, sha256 in assets:
        dest = os.path.join(directory, name)
        if os.path.isfile(dest) and sha256_of(dest) == sha256:
            continue
        log(f"Скачиваю {name}")
        download(url, dest, sha256)
    names = [name for name, _, _ in assets]
    files = link_latest(directory, names)
    aliases = {alias for _, _, alias in LATEST_ALIASES}
    remove_stale(directory, set(names) | aliases)
    write_json(os.path.join(directory, "latest.json"), {"version": version, "files": files})
    return version


def install_cron(directory: str) -> None:
    script = os.path.abspath(__file__)
    line = (
        f"{CRON_SCHEDULE} root /usr/bin/python3 {script} --dir {directory} "
        f">> /var/log/fenixcraft-downloads.log 2>&1\n"
    )
    with open(CRON_FILE, "w", encoding="utf-8") as f:
        f.write("# Зеркало сборок лаунчера FenixCraft на сайте, обновляется с GitHub.\n")
        f.write(line)
    os.chmod(CRON_FILE, 0o644)
    log(f"Автообновление включено: {CRON_FILE} (каждый час)")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--dir", default=DEFAULT_DIR, help="папка download на сайте")
    parser.add_argument("--install-cron", action="store_true", help="проверять релизы каждый час")
    args = parser.parse_args()
    directory = os.path.abspath(args.dir)
    if args.install_cron:
        install_cron(directory)
    os.makedirs(directory, exist_ok=True)
    import fcntl  # есть только на Linux, где скрипт и работает

    with open(os.path.join(directory, ".sync.lock"), "w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            log("Синхронизация уже идёт")
            return 0
        try:
            version = sync(directory)
        except (OSError, ValueError) as error:
            log(f"Зеркало не обновлено: {error}")
            return 1
    log(f"На сайте версия {version}: {directory}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
