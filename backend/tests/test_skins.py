import pytest
from fastapi.testclient import TestClient

import app as backend
import skins

TOKEN = "a" * 32
OTHER_TOKEN = "b" * 32
PNG = b"\x89PNG\r\n\x1a\n" + b"\x00" * 64


@pytest.fixture
def client(tmp_path, monkeypatch: pytest.MonkeyPatch) -> TestClient:
    catalog = tmp_path / "catalog"
    for kind, names in {"skins": ["ice_boy", "alex_slim"], "capes": ["phoenix"]}.items():
        (catalog / kind).mkdir(parents=True)
        for name in names:
            (catalog / kind / f"{name}.png").write_bytes(PNG)
    (catalog / "skins" / "notes.txt").write_text("not a skin")
    (catalog / "skins" / "huge.png").write_bytes(b"x" * (skins.MAX_TEXTURE_BYTES + 1))
    monkeypatch.setattr(skins, "CATALOG_DIR", str(catalog))
    monkeypatch.setattr(skins, "DATA_DIR", str(tmp_path / "data"))
    monkeypatch.setattr(skins, "_players_cache", {"mtime": None, "players": {}})
    monkeypatch.setattr(skins, "_recent_selects", {})
    return TestClient(backend.app)


def select(client: TestClient, **body) -> int:
    payload = {"username": "BloodRayT", "token": TOKEN, "skin": None, "cape": None, **body}
    return client.post("/api/launcher/skins/select", json=payload).status_code


def test_catalog_lists_only_png_textures_with_names_and_model(client: TestClient) -> None:
    catalog = client.get("/api/launcher/skins/catalog").json()

    assert [item["id"] for item in catalog["skins"]] == ["alex_slim", "ice_boy"]
    assert catalog["skins"][0] == {
        "id": "alex_slim",
        "name": "Alex",
        "url": "http://fenixcraft.ru/api/launcher/skins/skins/alex_slim.png",
        "slim": True,
    }
    assert [item["id"] for item in catalog["capes"]] == ["phoenix"]
    assert client.get("/api/launcher/skins/skins/ice_boy.png").content == PNG


def test_player_gets_the_chosen_skin_and_cape_in_game(client: TestClient) -> None:
    assert select(client, skin="ice_boy", cape="phoenix") == 200

    # CustomSkinLoader запрашивает текстуры по нику без учёта регистра.
    assert client.get("/api/launcher/skins/player/bloodrayt.png").content == PNG
    assert client.get("/api/launcher/skins/cape/BloodRayT.png").content == PNG


def test_player_without_a_choice_falls_back_to_other_sources(client: TestClient) -> None:
    assert client.get("/api/launcher/skins/player/Steve.png").status_code == 404
    assert select(client, skin="ice_boy") == 200
    assert client.get("/api/launcher/skins/cape/BloodRayT.png").status_code == 404


def test_nickname_is_bound_to_the_first_launcher(client: TestClient) -> None:
    assert select(client, skin="ice_boy") == 200
    assert select(client, skin="alex_slim", token=OTHER_TOKEN) == 403
    assert select(client, skin="alex_slim") == 200
    assert client.get("/api/launcher/skins/player/BloodRayT.png").status_code == 200


@pytest.mark.parametrize("body", [{"skin": "missing"}, {"cape": "../skins/ice_boy"}, {"skin": "huge"}])
def test_unknown_or_unsafe_items_are_rejected(client: TestClient, body: dict) -> None:
    assert select(client, **body) == 404


def test_ely_nicknames_with_a_dash_are_accepted(client: TestClient) -> None:
    assert select(client, username="Mr-Cool", skin="ice_boy") == 200
    assert client.get("/api/launcher/skins/player/mr-cool.png").content == PNG


def test_selecting_too_often_is_throttled(client: TestClient) -> None:
    codes = [select(client, skin="ice_boy") for _ in range(skins.SELECT_LIMIT + 1)]

    assert codes[:-1] == [200] * skins.SELECT_LIMIT
    assert codes[-1] == 429


def test_player_limit_protects_the_file_from_endless_growth(
    client: TestClient, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(skins, "MAX_PLAYERS", 1)
    assert select(client, skin="ice_boy") == 200
    assert select(client, username="Another", skin="ice_boy") == 507
    # Уже привязанный ник по-прежнему меняет свой скин.
    assert select(client, skin="alex_slim") == 200


@pytest.mark.parametrize("body", [{"username": "a"}, {"username": "bad name"}, {"token": "short"}])
def test_bad_username_or_token_is_rejected(client: TestClient, body: dict) -> None:
    assert select(client, **body) == 400
