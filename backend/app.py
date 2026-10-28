from fastapi import FastAPI, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.cors import CORSMiddleware
import os
import json
import hashlib
from urllib.parse import quote

app = FastAPI(title="FenixCraft Launcher Backend v1.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

STORAGE_DIR = os.path.join(os.path.dirname(__file__), "storage")
SERVERS_FILE = os.path.join(os.path.dirname(__file__), "servers.json")
BASE_PUBLIC_URL = os.getenv("BASE_PUBLIC_URL", "https://fenixcraft.ru/api/launcher")

os.makedirs(STORAGE_DIR, exist_ok=True)
app.mount("/files", StaticFiles(directory=STORAGE_DIR), name="files")

def sha256_of_file(filepath: str) -> str:
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()

@app.get("/api/launcher/servers")
def get_servers():
    if os.path.exists(SERVERS_FILE):
        with open(SERVERS_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    return [
        {"id": "fenix-main", "name": "FenixCraft", "desc": "Выживание • Vanilla/Mods", "ip": "play.fenixcraft.ru", "port": 25565, "icon": "🦅", "maxDefault": 1000},
        {"id": "fenix-mods", "name": "FenixTech", "desc": "NeoForge • 1.21.1", "ip": "tech.fenixcraft.ru", "port": 25565, "icon": "⚙️", "maxDefault": 500},
        {"id": "fenix-rpg",  "name": "FenixRPG",  "desc": "Fabric + Iris • 1.21.11", "ip": "rpg.fenixcraft.ru", "port": 25565, "icon": "⚔️", "maxDefault": 500},
        {"id": "fenix-pvp",  "name": "FenixArena", "desc": "Анархия • 1.21.11", "ip": "pvp.fenixcraft.ru", "port": 25565, "icon": "⭐", "maxDefault": 300}
    ]

@app.get("/api/launcher/manifest/{profile}")
def get_profile_manifest(profile: str):
    """
    Динамически сканирует папку storage/{profile}/ (mods, resourcepacks, shaderpacks, config)
    и отдает лаунчеру актуальный список файлов с SHA-256 контрольными суммами.
    """
    profile_dir = os.path.join(STORAGE_DIR, profile)
    if not os.path.exists(profile_dir) or not os.path.isdir(profile_dir):
        raise HTTPException(status_code=404, detail=f"Profile '{profile}' not found on backend")

    files_list = []
    for root, _, filenames in os.walk(profile_dir):
        for fname in filenames:
            if fname.startswith(".") or fname == "Thumbs.db":
                continue
            full_path = os.path.join(root, fname)
            rel_path = os.path.relpath(full_path, profile_dir).replace("\\", "/")
            file_hash = sha256_of_file(full_path)
            file_size = os.path.getsize(full_path)
            download_url = f"{BASE_PUBLIC_URL}/files/{quote(profile)}/{quote(rel_path)}"

            files_list.append({
                "path": rel_path,
                "sha256": file_hash,
                "size": file_size,
                "url": download_url
            })

    return {
        "profile": profile,
        "enforce_mods": True,
        "files": files_list
    }


@app.get("/api/launcher/version")
def get_launcher_version():
    """Укажите здесь новую версию (например 1.1), чтобы у всех игроков появилась кнопка автообновления."""
    return {
        "version": "1.0",
        "url": "https://github.com/BloodRayT/launcher/releases/latest/download/FenixCraft_1.1.0_x64-setup.exe"
    }
