"""Каталог скинов и плащей и выбор игроков.

Администратор кладёт PNG в catalog/skins и catalog/capes. Лаунчер показывает каталог,
игрок выбирает скин и плащ, а мод CustomSkinLoader в игре берёт их по нику:
/api/launcher/skins/player/{ник}.png и /api/launcher/skins/cape/{ник}.png.
"""
import hashlib
import json
import os
import re
import tempfile
import threading
import time
from collections import deque
from typing import Optional

from fastapi import APIRouter, HTTPException, Request
from fastapi.responses import FileResponse
from pydantic import BaseModel, Field

BASE_DIR = os.path.dirname(__file__)
CATALOG_DIR = os.getenv("CATALOG_DIR", os.path.join(BASE_DIR, "catalog"))
DATA_DIR = os.getenv("DATA_DIR", os.path.join(BASE_DIR, "data"))
BASE_PUBLIC_URL = os.getenv("BASE_PUBLIC_URL", "http://fenixcraft.ru/api/launcher")
KINDS = ("skins", "capes")
ITEM_ID = re.compile(r"^[A-Za-z0-9_-]{1,48}$")
# Дефис встречается в никах Ely.by.
USERNAME = re.compile(r"^[A-Za-z0-9_-]{3,16}$")
TOKEN = re.compile(r"^[0-9a-f]{32,64}$")
# Скины на сервере маленькие: всё, что больше, не текстура Minecraft.
MAX_TEXTURE_BYTES = 256 * 1024
NO_CACHE = {"Cache-Control": "no-cache"}
# Защита от захвата ников скриптом: сколько всего ников и как часто можно выбирать с одного адреса.
MAX_PLAYERS = 20000
SELECT_LIMIT = 10
SELECT_WINDOW_SECONDS = 60
MAX_FIELD_LENGTH = 128
MAX_ADDRESS_LENGTH = 64

router = APIRouter(prefix="/api/launcher/skins")
_players_lock = threading.Lock()
# Файл выборов перечитывается с диска, только когда он изменился.
_players_cache: dict = {"mtime": None, "players": {}}
_recent_selects: dict = {}


class Selection(BaseModel):
    username: str = Field(max_length=MAX_FIELD_LENGTH)
    token: str = Field(max_length=MAX_FIELD_LENGTH)
    skin: Optional[str] = Field(default=None, max_length=MAX_FIELD_LENGTH)
    cape: Optional[str] = Field(default=None, max_length=MAX_FIELD_LENGTH)


def _item_path(kind: str, item_id: Optional[str]) -> Optional[str]:
    if kind not in KINDS or not item_id or not ITEM_ID.fullmatch(item_id):
        return None
    path = os.path.join(CATALOG_DIR, kind, f"{item_id}.png")
    if not os.path.isfile(path) or os.path.getsize(path) > MAX_TEXTURE_BYTES:
        return None
    return path


def _title(item_id: str) -> str:
    name = item_id.removesuffix("_slim").replace("_", " ").replace("-", " ").strip()
    return name[:1].upper() + name[1:]


def list_items(kind: str) -> list:
    folder = os.path.join(CATALOG_DIR, kind)
    if not os.path.isdir(folder):
        return []
    items = []
    for name in sorted(os.listdir(folder), key=str.lower):
        item_id, ext = os.path.splitext(name)
        if ext.lower() != ".png" or _item_path(kind, item_id) is None:
            continue
        item = {"id": item_id, "name": _title(item_id), "url": f"{BASE_PUBLIC_URL}/skins/{kind}/{item_id}.png"}
        if kind == "skins":
            # Тонкие руки (модель Alex) помечаются окончанием _slim в имени файла.
            item["slim"] = item_id.endswith("_slim")
        items.append(item)
    return items


def _players_file() -> str:
    return os.path.join(DATA_DIR, "players.json")


def _load_players() -> dict:
    path = _players_file()
    try:
        mtime = os.stat(path).st_mtime_ns
    except OSError:
        return {}
    if _players_cache["mtime"] == mtime:
        return _players_cache["players"]
    try:
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except (OSError, ValueError):
        return {}
    players = data if isinstance(data, dict) else {}
    _players_cache.update(mtime=mtime, players=players)
    return players


def _client_address(request: Request) -> str:
    """Адрес игрока за веб-сервером: иначе все игроки выглядели бы одним адресом прокси.

    Порт бекенда открыт только для веб-сервера. Настоящий адрес он дописывает в конец
    X-Forwarded-For, а всё, что левее, мог прислать сам клиент.
    """
    forwarded = request.headers.get("x-forwarded-for", "")
    address = forwarded.rsplit(",", 1)[-1].strip()[:MAX_ADDRESS_LENGTH]
    return address or (request.client.host if request.client else "unknown")


def _allow_select(address: str) -> bool:
    """Не больше SELECT_LIMIT выборов в минуту с одного адреса."""
    now = time.monotonic()
    recent = _recent_selects.setdefault(address, deque())
    while recent and now - recent[0] > SELECT_WINDOW_SECONDS:
        recent.popleft()
    if len(recent) >= SELECT_LIMIT:
        return False
    recent.append(now)
    if len(_recent_selects) > MAX_PLAYERS:
        _recent_selects.clear()
    return True


def _save_players(players: dict) -> None:
    os.makedirs(DATA_DIR, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=DATA_DIR, suffix=".tmp")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(players, f, ensure_ascii=False, indent=1)
        os.replace(tmp, _players_file())
    except BaseException:
        if os.path.exists(tmp):
            os.remove(tmp)
        raise


def _token_hash(token: str) -> str:
    return hashlib.sha256(token.encode("ascii")).hexdigest()


def _player_item(username: str, field: str, kind: str) -> str:
    if not USERNAME.fullmatch(username):
        raise HTTPException(status_code=404, detail="Not found")
    path = _item_path(kind, _load_players().get(username.lower(), {}).get(field))
    if path is None:
        # 404 — сигнал CustomSkinLoader взять скин из следующего источника, например Mojang.
        raise HTTPException(status_code=404, detail="Not found")
    return path


@router.get("/catalog")
def get_catalog():
    return {"skins": list_items("skins"), "capes": list_items("capes")}


@router.post("/select")
def select(selection: Selection, request: Request):
    """Первый выбор закрепляет ник за установкой лаунчера: дальше его меняет только тот же токен.
    Снять чужую привязку администратор может, удалив запись ника из data/players.json."""
    if not _allow_select(_client_address(request)):
        raise HTTPException(status_code=429, detail="Too many requests")
    if not USERNAME.fullmatch(selection.username) or not TOKEN.fullmatch(selection.token):
        raise HTTPException(status_code=400, detail="Bad username or token")
    for kind, item_id in (("skins", selection.skin), ("capes", selection.cape)):
        if item_id is not None and _item_path(kind, item_id) is None:
            raise HTTPException(status_code=404, detail=f"Unknown {kind[:-1]} '{item_id}'")
    key = selection.username.lower()
    token_hash = _token_hash(selection.token)
    with _players_lock:
        players = _load_players()
        current = players.get(key)
        if current and current.get("token_sha256") != token_hash:
            raise HTTPException(status_code=403, detail="Nickname is bound to another launcher")
        if not current and len(players) >= MAX_PLAYERS:
            raise HTTPException(status_code=507, detail="Too many players")
        updated = {**players, key: {"token_sha256": token_hash, "skin": selection.skin, "cape": selection.cape}}
        _save_players(updated)
    return {"success": True, "skin": selection.skin, "cape": selection.cape}


# Адреса по нику объявлены раньше общего маршрута каталога, иначе он перехватил бы их.
@router.get("/player/{username}.png")
def get_player_skin(username: str):
    return FileResponse(_player_item(username, "skin", "skins"), media_type="image/png", headers=NO_CACHE)


@router.get("/cape/{username}.png")
def get_player_cape(username: str):
    return FileResponse(_player_item(username, "cape", "capes"), media_type="image/png", headers=NO_CACHE)


@router.get("/{kind}/{item_id}.png")
def get_catalog_file(kind: str, item_id: str):
    path = _item_path(kind, item_id)
    if path is None:
        raise HTTPException(status_code=404, detail="Not found")
    return FileResponse(path, media_type="image/png")
