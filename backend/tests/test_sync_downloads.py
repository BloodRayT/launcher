import hashlib
import io
import json
import os
import sys

import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "deploy"))
import sync_downloads  # noqa: E402

PREFIX = sync_downloads.DOWNLOAD_PREFIX + "v1.0.21/"
FILES = {
    "FenixCraft_1.0.21_x64-setup.exe": b"exe",
    "FenixCraft_1.0.21_amd64.deb": b"deb",
    "FenixCraft_aarch64.app.tar.gz": b"app",
}


def release(files: dict, tamper: str = "") -> dict:
    return {
        "tag_name": "v1.0.21",
        "assets": [
            {
                "name": name,
                "browser_download_url": PREFIX + name,
                "digest": "sha256:" + hashlib.sha256(b"x" if name == tamper else body).hexdigest(),
            }
            for name, body in files.items()
        ],
    }


def fake_github(monkeypatch: pytest.MonkeyPatch, release_body: dict) -> None:
    def open_url(url: str):
        if url == sync_downloads.LATEST_RELEASE_API:
            return io.BytesIO(json.dumps(release_body).encode())
        return io.BytesIO(FILES[url.removeprefix(PREFIX)])

    monkeypatch.setattr(sync_downloads, "open_url", open_url)


def test_release_files_get_latest_names_and_old_versions_go(tmp_path, monkeypatch) -> None:
    (tmp_path / "FenixCraft_1.0.15_x64-setup.exe").write_bytes(b"old")
    (tmp_path / "index.html").write_text("site")
    fake_github(monkeypatch, release(FILES))

    assert sync_downloads.sync(str(tmp_path)) == "1.0.21"

    assert (tmp_path / "FenixCraft_latest_x64-setup.exe").read_bytes() == b"exe"
    assert (tmp_path / "FenixCraft_latest_amd64.deb").read_bytes() == b"deb"
    assert not (tmp_path / "FenixCraft_1.0.15_x64-setup.exe").exists()
    assert (tmp_path / "index.html").exists()
    latest = json.loads((tmp_path / "latest.json").read_text(encoding="utf-8"))
    assert latest == {
        "version": "1.0.21",
        "files": {
            "exe": "FenixCraft_1.0.21_x64-setup.exe",
            "app": "FenixCraft_aarch64.app.tar.gz",
            "deb": "FenixCraft_1.0.21_amd64.deb",
        },
    }


def test_file_with_a_wrong_checksum_is_not_published(tmp_path, monkeypatch) -> None:
    fake_github(monkeypatch, release(FILES, tamper="FenixCraft_1.0.21_amd64.deb"))

    with pytest.raises(ValueError):
        sync_downloads.sync(str(tmp_path))

    assert not (tmp_path / "FenixCraft_1.0.21_amd64.deb").exists()
    assert not (tmp_path / "latest.json").exists()


def test_assets_without_checksum_or_from_elsewhere_are_ignored() -> None:
    body = {"assets": [
        {"name": "FenixCraft_a.exe", "browser_download_url": PREFIX + "FenixCraft_a.exe"},
        {"name": "FenixCraft_b.exe", "browser_download_url": "https://evil.example/b", "digest": "sha256:" + "0" * 64},
        {"name": "../evil", "browser_download_url": PREFIX + "evil", "digest": "sha256:" + "0" * 64},
    ]}
    assert sync_downloads.release_assets(body) == []
