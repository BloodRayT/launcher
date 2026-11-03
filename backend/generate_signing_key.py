"""Создаёт пару ключей подписи манифестов. Запускается один раз, ключи никуда не отправляются."""
import base64

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey


def main() -> None:
    key = Ed25519PrivateKey.generate()
    private = key.private_bytes(
        serialization.Encoding.Raw,
        serialization.PrivateFormat.Raw,
        serialization.NoEncryption(),
    )
    public = key.public_key().public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw
    )
    print("Секретный ключ: только в .env на сервере, никому не показывать и не коммитить")
    print(f"MANIFEST_SIGNING_KEY={base64.b64encode(private).decode()}")
    print()
    print("Публичный ключ: переменная репозитория FENIX_MANIFEST_PUBLIC_KEY на GitHub")
    print(f"FENIX_MANIFEST_PUBLIC_KEY={base64.b64encode(public).decode()}")


if __name__ == "__main__":
    main()
