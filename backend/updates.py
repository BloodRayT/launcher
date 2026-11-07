"""Копия установщика лаунчера на сервере FenixCraft.

У части игроков GitHub не отдаёт файлы релизов, поэтому сервер сам скачивает установщик
последнего релиза, сверяет его SHA-256 с указанной на GitHub и раздаёт по адресу
/api/launcher/updates/FenixCraft_<версия>_x64-setup.exe. Лаунчер принимает копию,
только если её SHA-256 совпала с той, что он сам получил от GitHub.
"""
import hashlib
import logging
import os
import re
import tempfile
import threading
import time
from typing import Optional

import httpx
from fastapi import APIRouter, HTTPException
from fastapi.responses import FileResponse

BASE_DIR = os.path.dirname(__file__)
UPDATES_DIR = os.getenv("UPDATES_DIR", os.path.join(BASE_DIR, "updates"))
LATEST_RELEASE_API = "https://api.github.com/repos/BloodRayT/launcher/releases/latest"
DOWNLOAD_PREFIX = "https://github.com/BloodRayT/launcher/releases/download/"
INSTALLER_NAME = re.compile(r"^FenixCraft_(\d+)\.(\d+)\.(\d+)_x64-setup\.exe$")
SHA256_DIGEST = re.compile(r"^sha256:([0-9a-fA-F]{64})$")
MAX_INSTALLER_BYTES = 300 * 1024 * 1024
SYNC_INTERVAL_SECONDS = 60 * 60
# Предыдущая версия остаётся, пока её докачивают игроки, начавшие обновление раньше.
KEEP_INSTALLERS = 2
CHUNK_BYTES = 1024 * 1024

log = logging.getLogger("fenixcraft.updates")
router = APIRouter(prefix="/api/launcher/updates")
_sync_lock = threading.Lock()
_started = threading.Event()


def pick_installer(release: dict) -> Optional[tuple]:
    """Установщик Windows из ответа GitHub: (имя, адрес, sha256) или None."""
    for asset in release.get("assets") or []:
        name = asset.get("name") or ""
        url = asset.get("browser_download_url") or ""
        digest = SHA256_DIGEST.fullmatch(asset.get("digest") or "")
        if INSTALLER_NAME.fullmatch(name) and url.startswith(DOWNLOAD_PREFIX) and digest:
            return name, url, digest.group(1).lower()
    return None


def _sha256_of(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(CHUNK_BYTES), b""):
            h.update(chunk)
    return h.hexdigest()


def _version_of(name: str) -> tuple:
    match = INSTALLER_NAME.fullmatch(name)
    return tuple(int(part) for part in match.groups()) if match else ()


def _remove_old_installers() -> None:
    names = sorted(
        (name for name in os.listdir(UPDATES_DIR) if INSTALLER_NAME.fullmatch(name)),
        key=_version_of,
        reverse=True,
    )
    for name in names[KEEP_INSTALLERS:]:
        os.remove(os.path.join(UPDATES_DIR, name))


def _download(client: httpx.Client, url: str, dest: str, sha256: str) -> None:
    """Скачивает во временный файл и подменяет итоговый только после проверки SHA-256."""
    fd, tmp = tempfile.mkstemp(dir=UPDATES_DIR, suffix=".part")
    try:
        h = hashlib.sha256()
        size = 0
        with os.fdopen(fd, "wb") as f, client.stream("GET", url, follow_redirects=True) as response:
            response.raise_for_status()
            for chunk in response.iter_bytes(CHUNK_BYTES):
                size += len(chunk)
                if size > MAX_INSTALLER_BYTES:
                    raise ValueError("установщик больше допустимого размера")
                h.update(chunk)
                f.write(chunk)
        if h.hexdigest() != sha256:
            raise ValueError("SHA-256 не совпала с указанной на GitHub")
        os.replace(tmp, dest)
    finally:
        if os.path.exists(tmp):
            os.remove(tmp)


def sync_latest(client: httpx.Client) -> Optional[str]:
    """Приводит копию к последнему релизу. Возвращает имя установщика или None, если его нет."""
    with _sync_lock:
        response = client.get(LATEST_RELEASE_API, headers={"Accept": "application/vnd.github+json"})
        response.raise_for_status()
        picked = pick_installer(response.json())
        if picked is None:
            return None
        name, url, sha256 = picked
        os.makedirs(UPDATES_DIR, exist_ok=True)
        dest = os.path.join(UPDATES_DIR, name)
        if not (os.path.isfile(dest) and _sha256_of(dest) == sha256):
            log.info("Скачиваю %s с GitHub", name)
            _download(client, url, dest, sha256)
        _remove_old_installers()
        return name


def _sync_forever() -> None:
    while True:
        try:
            with httpx.Client(timeout=60, headers={"User-Agent": "FenixCraftBackend"}) as client:
                sync_latest(client)
        except (httpx.HTTPError, OSError, ValueError) as error:
            log.warning("Копия установщика не обновлена: %s", error)
        time.sleep(SYNC_INTERVAL_SECONDS)


def start_sync() -> None:
    """Запускает фоновую проверку релизов один раз за жизнь процесса."""
    if os.getenv("UPDATES_SYNC", "1") == "0" or _started.is_set():
        return
    _started.set()
    threading.Thread(target=_sync_forever, name="updates-sync", daemon=True).start()


@router.get("/{name}")
def get_installer(name: str):
    if not INSTALLER_NAME.fullmatch(name):
        raise HTTPException(status_code=404, detail="Not found")
    path = os.path.join(UPDATES_DIR, name)
    if not os.path.isfile(path):
        raise HTTPException(status_code=404, detail="Not found")
    return FileResponse(path, media_type="application/octet-stream", filename=name)
