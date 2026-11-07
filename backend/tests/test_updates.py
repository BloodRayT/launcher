import hashlib

import httpx
import pytest
from fastapi.testclient import TestClient

import app as backend
import updates

PAYLOAD = b"MZ" + b"\x00" * 1024
SHA = hashlib.sha256(PAYLOAD).hexdigest()
NAME = "FenixCraft_1.0.20_x64-setup.exe"
URL = f"{updates.DOWNLOAD_PREFIX}v1.0.20/{NAME}"


def release(digest: str = f"sha256:{SHA}", name: str = NAME, url: str = URL) -> dict:
    return {
        "tag_name": "v1.0.20",
        "assets": [
            {"name": "FenixCraft_1.0.20_x64_en-US.msi", "browser_download_url": URL.replace("-setup.exe", ".msi")},
            {"name": name, "browser_download_url": url, "digest": digest},
        ],
    }


def github(release_body: dict, payload: bytes = PAYLOAD) -> httpx.Client:
    def handler(request: httpx.Request) -> httpx.Response:
        if str(request.url) == updates.LATEST_RELEASE_API:
            return httpx.Response(200, json=release_body)
        if str(request.url) == URL:
            return httpx.Response(200, content=payload)
        return httpx.Response(404)

    return httpx.Client(transport=httpx.MockTransport(handler))


@pytest.fixture
def updates_dir(tmp_path, monkeypatch: pytest.MonkeyPatch):
    monkeypatch.setattr(updates, "UPDATES_DIR", str(tmp_path))
    return tmp_path


def test_installer_is_picked_only_with_a_github_checksum() -> None:
    assert updates.pick_installer(release()) == (NAME, URL, SHA)
    assert updates.pick_installer(release(digest="")) is None
    assert updates.pick_installer(release(url="https://evil.example/" + NAME)) is None
    assert updates.pick_installer(release(name="../" + NAME)) is None


def test_sync_downloads_the_latest_installer_and_drops_old_ones(updates_dir) -> None:
    for old in ("1.0.18", "1.0.19"):
        (updates_dir / f"FenixCraft_{old}_x64-setup.exe").write_bytes(b"old")

    assert updates.sync_latest(github(release())) == NAME

    assert (updates_dir / NAME).read_bytes() == PAYLOAD
    remaining = sorted(path.name for path in updates_dir.iterdir())
    assert remaining == ["FenixCraft_1.0.19_x64-setup.exe", NAME]


def test_sync_refuses_a_file_with_a_different_checksum(updates_dir) -> None:
    with pytest.raises(ValueError):
        updates.sync_latest(github(release(), payload=b"tampered"))

    assert list(updates_dir.iterdir()) == []


def test_installer_is_served_by_name_only(updates_dir) -> None:
    (updates_dir / NAME).write_bytes(PAYLOAD)
    client = TestClient(backend.app)

    response = client.get(f"/api/launcher/updates/{NAME}")

    assert response.status_code == 200
    assert response.content == PAYLOAD
    assert client.get("/api/launcher/updates/FenixCraft_9.9.9_x64-setup.exe").status_code == 404
    assert client.get("/api/launcher/updates/secret.txt").status_code == 404
