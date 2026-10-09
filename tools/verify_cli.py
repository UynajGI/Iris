"""Exercise the shipped CLI on disposable JPEG copies with real models."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
binary = ROOT / "target/release/iris-cli.exe"
fixtures = ROOT / "test-photos"
manifest = json.loads((fixtures / "manifest.json").read_text(encoding="utf-8"))
with tempfile.TemporaryDirectory(prefix="iris-cli-test-") as temporary:
    work = Path(temporary)
    source = work / "photos"
    source.mkdir()
    files = []
    for entry in manifest["files"][:2]:
        copied = source / Path(entry["path"]).name
        shutil.copy2(fixtures / entry["path"], copied)
        files.append((copied, entry["sha256"]))

    def cli(*args, expect_success=True):
        result = subprocess.run([str(binary), "--database", str(work / "db.sqlite3"), "--model-dir", str(ROOT / "models"), *map(str, args)], cwd=work, text=True, capture_output=True, timeout=120)
        if not expect_success:
            assert result.returncode != 0, "invalid command unexpectedly succeeded"
            return result.stderr
        assert result.returncode == 0, result.stderr
        return json.loads(result.stdout)

    project = cli("scan", source)["project"]["id"]
    other = work / "other-project"
    other.mkdir()
    other_project = cli("scan", other)["project"]["id"]
    assert cli("projects")[0]["id"] == other_project
    assert cli("open", project)["id"] == project
    assert cli("projects")[0]["id"] == project
    cli("hide", project)
    assert project not in [p["id"] for p in cli("projects")]
    assert len(cli("photos", project)) == 2  # Hiding history never deletes the project.
    cli("open", project)
    assert cli("projects")[0]["id"] == project
    settings = cli("settings", project)
    assert settings["face_detector"] == "yunet"
    model_status = cli("models", project)
    assert model_status["selected"] == "yunet"
    assert any(d["provider"] == "yunet" and d["state"] == "available" for d in model_status["detectors"])
    settings_file = work / "settings.json"
    settings["eyes_weight"] = 26
    settings_file.write_text(json.dumps(settings), encoding="utf-8")
    assert cli("settings", project, "--file", settings_file)["eyes_weight"] == 26
    invalid_settings = work / "invalid-settings.json"
    invalid_settings.write_text(json.dumps({"face_detector": "scrfd_500m"}), encoding="utf-8")
    cli("settings", project, "--file", invalid_settings, expect_success=False)
    assert cli("settings", project)["face_detector"] == "yunet"
    analyzed = cli("analyze", project)
    assert analyzed["analyzed"] == 2 and analyzed["failed"] == 0
    assert cli("analyze", project)["reused"] == 2
    photos = cli("photos", project)
    assert cli("accept", project, "--photo-ids")["changed"] == 0
    cli("accept", project, "--photo-ids", 999999, expect_success=False)
    cli("decide", project, "keep", photos[0]["id"])
    cli("decide", project, "reject", photos[1]["id"])
    assert cli("export", project, "copy", "--destination", work / "chosen")["written"] == 1
    assert cli("export", project, "xmp", "--scope", "all")["written"] == 2
    cli("export", project, "csv", "--destination", work / "decisions.csv")
    cli("undo", project)
    cli("import", project, work / "decisions.csv")
    plan = cli("quarantine-preview", project)
    cli("quarantine-commit", plan["id"])
    cli("quarantine-restore", plan["id"])
    old_cache = Path(cli("cache", project)["root"])
    assert old_cache.resolve().is_relative_to(work.resolve())
    old_cache.mkdir(parents=True, exist_ok=True)
    cache_file = old_cache / "probe.cache"
    cache_file.write_bytes(b"disposable verified cache fixture")
    destination = work / "migrated-cache"
    cli("cache-migrate", project, destination)
    migration = cli("cache-history", project)[0]
    assert cache_file.read_bytes() == (destination / "probe.cache").read_bytes()
    unlisted = old_cache / "unlisted.txt"
    unlisted.write_bytes(b"must remain")
    cleaned = cli("cache-cleanup-old", project, migration["id"])
    assert all(item["cleaned"] for item in cleaned["files"])
    assert not cache_file.exists() and unlisted.read_bytes() == b"must remain"
    assert (destination / "probe.cache").is_file()
    cli("cache-cleanup-old", project, migration["id"])  # Idempotent retry.
    for copied, expected in files:
        with copied.open("rb") as stream:
            assert hashlib.file_digest(stream, "sha256").hexdigest() == expected
    print(json.dumps({"cli_workflow": "passed", "real_model_photos": 2, "sources_preserved": True, "outside_repo_cwd": True, "detector_settings_and_status": "passed", "recent_project_history": "passed", "manifest_scoped_cache_cleanup": "passed"}))
