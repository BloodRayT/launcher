"""Подпись манифестов Ed25519: лаунчер содержит только публичный ключ и проверяет каждый ответ."""
import base64
import binascii
from typing import Optional

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

SEED_SIZE = 32


def load_signing_key(raw: Optional[str]) -> Optional[Ed25519PrivateKey]:
    """Без ключа манифесты не подписываются. Неверный ключ останавливает запуск сервера."""
    if not raw or not raw.strip():
        return None
    try:
        seed = base64.b64decode(raw.strip(), validate=True)
    except binascii.Error as error:
        raise ValueError("MANIFEST_SIGNING_KEY должен быть в формате base64") from error
    if len(seed) != SEED_SIZE:
        raise ValueError(f"MANIFEST_SIGNING_KEY должен содержать {SEED_SIZE} байта")
    return Ed25519PrivateKey.from_private_bytes(seed)


def require_signing_key(key: Optional[Ed25519PrivateKey], required: Optional[str]) -> None:
    """Если подпись обязательна, сервер без ключа не запускается, а не работает без защиты молча."""
    if key is None and (required or "").strip().lower() in {"1", "true", "yes"}:
        raise ValueError("REQUIRE_MANIFEST_SIGNING включён, но MANIFEST_SIGNING_KEY не задан")


def sign_body(key: Ed25519PrivateKey, body: bytes) -> str:
    return base64.b64encode(key.sign(body)).decode("ascii")
