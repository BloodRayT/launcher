"""Новости VK и статус Twitch. Токены хранятся только на сервере, лаунчер получает готовый результат."""
import logging
import os
import threading
import time
from datetime import datetime, timezone
from typing import Any, Callable, Optional

import httpx

logger = logging.getLogger("fenixcraft.feeds")

NEWS_TTL_SECONDS = 300
TWITCH_TTL_SECONDS = 60
REQUEST_TIMEOUT_SECONDS = 8
VK_API_URL = "https://api.vk.ru/method/wall.get"
VK_API_VERSION = "5.199"
VK_POST_COUNT = 6
TITLE_LENGTH = 55
TWITCH_TOKEN_URL = "https://id.twitch.tv/oauth2/token"
TWITCH_STREAMS_URL = "https://api.twitch.tv/helix/streams"
TOKEN_EXPIRY_MARGIN_SECONDS = 60


def http_client() -> httpx.Client:
    return httpx.Client(timeout=REQUEST_TIMEOUT_SECONDS)


class TimedCache:
    """Хранит последний удачный результат. При сбое источника отдаёт устаревшие данные."""

    def __init__(self, ttl_seconds: int) -> None:
        self.ttl_seconds = ttl_seconds
        self._lock = threading.Lock()
        self._value: Any = None
        self._expires_at = 0.0

    def get(self, fetch: Callable[[], Any], fallback: Any) -> Any:
        # Блокировка не даёт сотням одновременных запросов уйти к внешнему API.
        with self._lock:
            if time.monotonic() < self._expires_at:
                return self._value
            try:
                self._value = fetch()
            except (httpx.HTTPError, ValueError, KeyError) as error:
                # Текст исключения может содержать адрес запроса, поэтому пишем только тип.
                logger.warning("Feed update failed: %s", type(error).__name__)
            self._expires_at = time.monotonic() + self.ttl_seconds
            return self._value if self._value is not None else fallback

    def clear(self) -> None:
        with self._lock:
            self._value = None
            self._expires_at = 0.0


news_cache = TimedCache(NEWS_TTL_SECONDS)
twitch_cache = TimedCache(TWITCH_TTL_SECONDS)


def vk_post_to_card(post: dict) -> dict:
    text = (post.get("text") or "Новость сообщества FenixCraft").strip()
    photos = [
        attachment["photo"]["sizes"]
        for attachment in post.get("attachments", [])
        if attachment.get("type") == "photo" and attachment.get("photo", {}).get("sizes")
    ]
    published = datetime.fromtimestamp(int(post["date"]), tz=timezone.utc)
    return {
        "tag": "VK.RU/FENIXCRAFT",
        "title": text.split("\n")[0][:TITLE_LENGTH] or "FenixCraft ВК",
        "text": text,
        "date": published.strftime("%d.%m.%Y"),
        "image": photos[0][-1]["url"] if photos else "",
        "url": f"https://vk.ru/fenixcraft?w=wall{int(post['owner_id'])}_{int(post['id'])}",
    }


def fetch_vk_posts(token: str) -> list:
    with http_client() as client:
        # Токен передаётся в теле запроса, чтобы не попадать в журналы с адресами.
        response = client.post(
            VK_API_URL,
            data={
                "domain": "fenixcraft",
                "count": VK_POST_COUNT,
                "access_token": token,
                "v": VK_API_VERSION,
            },
        )
    response.raise_for_status()
    body = response.json()
    if "response" not in body:
        raise ValueError("VK returned an error")
    return [vk_post_to_card(post) for post in body["response"]["items"]]


def get_news() -> dict:
    token = os.getenv("VK_TOKEN", "").strip()
    if not token:
        return {"success": False, "posts": []}
    posts = news_cache.get(lambda: fetch_vk_posts(token), fallback=[])
    return {"success": bool(posts), "posts": posts}


class TwitchToken:
    def __init__(self) -> None:
        self._value: Optional[str] = None
        self._expires_at = 0.0

    def get(self, client: httpx.Client, client_id: str, secret: str) -> str:
        if self._value is None or time.monotonic() >= self._expires_at:
            response = client.post(
                TWITCH_TOKEN_URL,
                data={
                    "client_id": client_id,
                    "client_secret": secret,
                    "grant_type": "client_credentials",
                },
            )
            response.raise_for_status()
            body = response.json()
            self._value = body["access_token"]
            self._expires_at = (
                time.monotonic() + int(body["expires_in"]) - TOKEN_EXPIRY_MARGIN_SECONDS
            )
        return self._value

    def clear(self) -> None:
        self._value = None
        self._expires_at = 0.0


twitch_token = TwitchToken()


def fetch_twitch_live(client_id: str, secret: str, channel: str) -> dict:
    with http_client() as client:
        token = twitch_token.get(client, client_id, secret)
        response = client.get(
            TWITCH_STREAMS_URL,
            params={"user_login": channel},
            headers={"Client-Id": client_id, "Authorization": f"Bearer {token}"},
        )
    if response.status_code == 401:
        twitch_token.clear()
    response.raise_for_status()
    return {"live": bool(response.json().get("data"))}


def get_twitch_status() -> dict:
    client_id = os.getenv("TWITCH_CLIENT_ID", "").strip()
    secret = os.getenv("TWITCH_CLIENT_SECRET", "").strip()
    channel = os.getenv("TWITCH_CHANNEL", "bloodrayt").strip()
    if not client_id or not secret:
        return {"live": False}
    return twitch_cache.get(
        lambda: fetch_twitch_live(client_id, secret, channel), fallback={"live": False}
    )
