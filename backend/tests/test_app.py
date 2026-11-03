import hashlib

import pytest
from fastapi.testclient import TestClient

import app as backend

PROFILE = "1.21.1-neoforge"
OPTIONS_PATH = f"/files/{PROFILE}/options.txt"


@pytest.fixture
def client() -> TestClient:
    return TestClient(backend.app)


def test_servers_are_served_from_the_committed_file(client: TestClient) -> None:
    response = client.get("/api/launcher/servers")

    assert response.status_code == 200
    assert {server["id"] for server in response.json()} == {
        "fenix-main",
        "fenix-mods",
        "fenix-rpg",
        "fenix-pvp",
    }


def test_servers_fall_back_when_the_file_path_is_a_directory(
    client: TestClient, tmp_path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # Docker создаёт папку на месте несуществующего файла из volume.
    monkeypatch.setattr(backend, "SERVERS_FILE", str(tmp_path))

    response = client.get("/api/launcher/servers")

    assert response.status_code == 200
    assert len(response.json()) == 4


def test_manifest_lists_files_with_correct_hashes(client: TestClient) -> None:
    response = client.get(f"/api/launcher/manifest/{PROFILE}")

    assert response.status_code == 200
    manifest = response.json()
    assert manifest["profile"] == PROFILE
    options = next(item for item in manifest["files"] if item["path"] == "options.txt")
    served = client.get(OPTIONS_PATH).content
    assert options["sha256"] == hashlib.sha256(served).hexdigest()
    assert options["size"] == len(served)


def test_every_manifest_url_is_downloadable(client: TestClient) -> None:
    manifest = client.get(f"/api/launcher/manifest/{PROFILE}").json()

    assert manifest["files"]
    for item in manifest["files"]:
        path = item["url"].removeprefix("https://fenixcraft.ru")
        assert client.get(path).status_code == 200, path


def test_unknown_profile_is_not_found(client: TestClient) -> None:
    assert client.get("/api/launcher/manifest/latest-vanilla").status_code == 404


@pytest.mark.parametrize("profile", ["%2e%2e", "..", ".hidden", "%2e%2e%5capp"])
def test_profile_cannot_escape_the_storage_folder(client: TestClient, profile: str) -> None:
    response = client.get(f"/api/launcher/manifest/{profile}")

    assert response.status_code == 404
    assert "app.py" not in response.text
