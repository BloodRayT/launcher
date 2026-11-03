from typing import Callable, Iterator

import httpx
import pytest
from fastapi.testclient import TestClient

import app as backend
import feeds

MANIFEST_URL = "/api/launcher/manifest/1.21.1-neoforge"
SECRET_TOKEN = "super-secret-token"


@pytest.fixture
def client() -> TestClient:
    return TestClient(backend.app)


@pytest.fixture(autouse=True)
def clean_caches() -> Iterator[None]:
    feeds.news_cache.clear()
    feeds.twitch_cache.clear()
    feeds.twitch_token.clear()
    yield


def use_transport(
    monkeypatch: pytest.MonkeyPatch, handler: Callable[[httpx.Request], httpx.Response]
) -> None:
    monkeypatch.setattr(
        feeds, "http_client", lambda: httpx.Client(transport=httpx.MockTransport(handler))
    )


def test_file_hash_is_not_recomputed_for_unchanged_files(
    client: TestClient, monkeypatch: pytest.MonkeyPatch
) -> None:
    backend.HASH_CACHE.clear()
    calls = []
    original = backend.sha256_of_file
    monkeypatch.setattr(
        backend, "sha256_of_file", lambda path: calls.append(path) or original(path)
    )

    client.get(MANIFEST_URL)
    first_pass = len(calls)
    client.get(MANIFEST_URL)

    assert first_pass > 0
    assert len(calls) == first_pass


def test_health_is_available(client: TestClient) -> None:
    response = client.get("/api/launcher/health")

    assert response.status_code == 200
    assert response.json()["status"] == "ok"


def test_news_without_a_token_reports_no_data(
    client: TestClient, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("VK_TOKEN", raising=False)

    assert client.get("/api/launcher/news").json() == {"success": False, "posts": []}


def test_news_become_cards_and_the_token_stays_private(
    client: TestClient, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("VK_TOKEN", SECRET_TOKEN)
    seen = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request)
        return httpx.Response(200, json={"response": {"items": [{
            "id": 5,
            "owner_id": -100,
            "date": 1700000000,
            "text": "Заголовок\nТекст",
            "attachments": [{"type": "photo", "photo": {"sizes": [
                {"url": "https://a/s.jpg"}, {"url": "https://a/l.jpg"}
            ]}}],
        }]}})

    use_transport(monkeypatch, handler)

    response = client.get("/api/launcher/news")

    body = response.json()
    assert body["success"] is True
    assert body["posts"][0] == {
        "tag": "VK.RU/FENIXCRAFT",
        "title": "Заголовок",
        "text": "Заголовок\nТекст",
        "date": "14.11.2023",
        "image": "https://a/l.jpg",
        "url": "https://vk.ru/fenixcraft?w=wall-100_5",
    }
    assert SECRET_TOKEN not in response.text
    assert SECRET_TOKEN not in str(seen[0].url), "token must not appear in the URL"

    # Повторный запрос берётся из кеша и не обращается к VK.
    client.get("/api/launcher/news")
    assert len(seen) == 1


def test_stale_news_are_served_when_vk_fails(
    client: TestClient, monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    monkeypatch.setenv("VK_TOKEN", SECRET_TOKEN)
    use_transport(monkeypatch, lambda request: httpx.Response(200, json={"response": {"items": [
        {"id": 1, "owner_id": -1, "date": 1700000000, "text": "Старая новость"}
    ]}}))
    client.get("/api/launcher/news")
    feeds.news_cache._expires_at = 0.0
    use_transport(monkeypatch, lambda request: httpx.Response(500))

    body = client.get("/api/launcher/news").json()

    assert body["posts"][0]["title"] == "Старая новость"
    assert SECRET_TOKEN not in caplog.text


def test_vk_error_response_gives_no_news(
    client: TestClient, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("VK_TOKEN", SECRET_TOKEN)
    use_transport(monkeypatch, lambda request: httpx.Response(200, json={"error": {"error_code": 5}}))

    assert client.get("/api/launcher/news").json() == {"success": False, "posts": []}


def test_twitch_without_credentials_is_offline(
    client: TestClient, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("TWITCH_CLIENT_ID", raising=False)
    monkeypatch.delenv("TWITCH_CLIENT_SECRET", raising=False)

    assert client.get("/api/launcher/twitch").json() == {"live": False}


@pytest.mark.parametrize("streams, live", [([{"id": "1"}], True), ([], False)])
def test_twitch_status_comes_from_helix(
    client: TestClient, monkeypatch: pytest.MonkeyPatch, streams: list, live: bool
) -> None:
    monkeypatch.setenv("TWITCH_CLIENT_ID", "id")
    monkeypatch.setenv("TWITCH_CLIENT_SECRET", SECRET_TOKEN)

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.host == "id.twitch.tv":
            return httpx.Response(200, json={"access_token": "t", "expires_in": 3600})
        assert request.headers["Authorization"] == "Bearer t"
        assert request.url.params["user_login"] == "bloodrayt"
        return httpx.Response(200, json={"data": streams})

    use_transport(monkeypatch, handler)

    response = client.get("/api/launcher/twitch")

    assert response.json() == {"live": live}
    assert SECRET_TOKEN not in response.text


def test_twitch_failure_does_not_break_the_endpoint(
    client: TestClient, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("TWITCH_CLIENT_ID", "id")
    monkeypatch.setenv("TWITCH_CLIENT_SECRET", SECRET_TOKEN)
    use_transport(monkeypatch, lambda request: httpx.Response(503))

    response = client.get("/api/launcher/twitch")

    assert response.status_code == 200
    assert response.json() == {"live": False}
