from fastapi import FastAPI, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.cors import CORSMiddleware
import os
import re
import json
import hashlib
from urllib.parse import quote

import feeds
import skins

app = FastAPI(title="FenixCraft Launcher Backend v1.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

STORAGE_DIR = os.path.join(os.path.dirname(__file__), "storage")
SERVERS_FILE = os.path.join(os.path.dirname(__file__), "servers.json")
BASE_PUBLIC_URL = os.getenv("BASE_PUBLIC_URL", "http://fenixcraft.ru/api/launcher")
PROFILE_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-][A-Za-z0-9._-]{0,63}$")
# Хеш файла пересчитывается только после его изменения: путь -> (mtime_ns, размер, sha256).
HASH_CACHE: dict = {}

os.makedirs(STORAGE_DIR, exist_ok=True)
# Манифест отдаёт ссылки вида {BASE_PUBLIC_URL}/files/..., поэтому файлы доступны и по этому пути.
app.mount("/api/launcher/files", StaticFiles(directory=STORAGE_DIR), name="launcher-files")
app.mount("/files", StaticFiles(directory=STORAGE_DIR), name="files")
app.include_router(skins.router)

def sha256_of_file(filepath: str) -> str:
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()

def cached_file_info(filepath: str) -> tuple:
    stat = os.stat(filepath)
    cached = HASH_CACHE.get(filepath)
    if cached and cached[0] == stat.st_mtime_ns and cached[1] == stat.st_size:
        return cached[2], stat.st_size
    file_hash = sha256_of_file(filepath)
    HASH_CACHE[filepath] = (stat.st_mtime_ns, stat.st_size, file_hash)
    return file_hash, stat.st_size

@app.get("/api/launcher/health")
def get_health():
    return {"status": "ok"}

@app.get("/api/launcher/news")
def get_news():
    return feeds.get_news()

@app.get("/api/launcher/twitch")
def get_twitch():
    return feeds.get_twitch_status()

@app.get("/api/launcher/servers")
def get_servers():
    if os.path.isfile(SERVERS_FILE):
        with open(SERVERS_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    return [
        {"id": "fenix-main", "name": "FenixCraft", "desc": "Vanilla • 26.3", "ip": "play.fenixcraft.ru", "port": 25565, "icon": "🦅", "maxDefault": 1000},
        {"id": "fenix-mods", "name": "FenixTech", "desc": "NeoForge • 1.21.1", "ip": "tech.fenixcraft.ru", "port": 25565, "icon": "⚙️", "maxDefault": 500}
    ]

@app.get("/api/launcher/manifest/{profile}")
def get_profile_manifest(profile: str):
    """
    Динамически сканирует папку storage/{profile}/ (mods, resourcepacks, shaderpacks, config)
    и отдает лаунчеру актуальный список файлов с SHA-256 контрольными суммами.
    """
    profile_dir = os.path.join(STORAGE_DIR, profile)
    if not PROFILE_ID_PATTERN.fullmatch(profile) or not os.path.isdir(profile_dir):
        raise HTTPException(status_code=404, detail=f"Profile '{profile}' not found on backend")

    files_list = []
    for root, _, filenames in os.walk(profile_dir):
        for fname in filenames:
            if fname.startswith(".") or fname == "Thumbs.db":
                continue
            full_path = os.path.join(root, fname)
            rel_path = os.path.relpath(full_path, profile_dir).replace("\\", "/")
            file_hash, file_size = cached_file_info(full_path)
            download_url = f"{BASE_PUBLIC_URL}/files/{quote(profile)}/{quote(rel_path)}"

            files_list.append({
                "path": rel_path,
                "sha256": file_hash,
                "size": file_size,
                "url": download_url
            })

    files_list.sort(key=lambda item: item["path"])
    return {
        "profile": profile,
        "enforce_mods": True,
        "files": files_list
    }


@app.get("/api/launcher/version")
def get_launcher_version():
    """Сейчас лаунчер получает обновления из GitHub Releases, этот адрес оставлен для старых версий."""
    return {
        "version": "1.0",
        "url": "https://github.com/BloodRayT/launcher/releases/latest"
    }
