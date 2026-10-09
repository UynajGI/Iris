"""Real offline daemon integration: DINO cache/settings/artifact gates and mixed media.

Uses disposable copies, local loopback only; never edits source fixtures or models.
No human labels or accuracy claims. Run after the release build.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import urllib.request

SHA = "f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--daemon", type=Path, default=root / "target/release/iris-daemon.exe")
    parser.add_argument("--models", type=Path, default=root / "models")
    parser.add_argument("--embedding-model", type=Path, default=root / "models/optional/dinov3_vits16.onnx")
    parser.add_argument("--output", type=Path, default=root / "artifacts/local-expansion-integration.json")
    args = parser.parse_args()
    sources = sorted((root / "test-photos").glob("*.jpg"))[:2]
    assert len(sources) == 2
    source_hashes = {str(p): digest(p) for p in sources}
    report = {"quality_validated": False, "external_api_calls": False}
    with tempfile.TemporaryDirectory(prefix="iris-local-expansion-") as temp:
        work = Path(temp)
        models = work / "custom-model-root"
        shutil.copytree(args.models, models, ignore=shutil.ignore_patterns("optional"))
        (models / "optional").mkdir()
        model = models / "optional/dinov3_vits16.onnx"
        shutil.copy2(args.embedding_model, model)
        photos = work / "photos"
        photos.mkdir()
        for i, source in enumerate(sources):
            shutil.copy2(source, photos / f"real-{i}.jpg")
        shutil.copy2(sources[0], photos / "real-0-duplicate.jpg")
        media = root / "crates/iris-core/tests/fixtures/media"
        for source in media.iterdir():
            if source.suffix.lower() in {".png", ".webp", ".heic", ".heif"}:
                shutil.copy2(source, photos / source.name)
        original_hashes = {p.name: digest(p) for p in photos.iterdir()}
        with (work / "daemon.log").open("w", encoding="utf-8") as err:
            process = subprocess.Popen([str(args.daemon.resolve()), "--data-dir", str(work / "data"), "--model-dir", str(models), "--worker-limit", "2"], cwd=work, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err, text=True)
            try:
                handshake = json.loads(process.stdout.readline())

                def request(method, path, body=None, raw=False):
                    data = None if body is None else json.dumps(body).encode()
                    req = urllib.request.Request(handshake["base_url"] + "/api/v1" + path, data=data, method=method, headers={"Authorization": "Bearer " + handshake["token"], "Content-Type": "application/json"})
                    with urllib.request.urlopen(req, timeout=180) as response:
                        payload = response.read()
                        return (response.headers.get_content_type(), payload) if raw else json.loads(payload)

                project = request("POST", "/projects", {"root": str(photos)})["id"]

                def job(kind, succeeds=True):
                    started = time.monotonic()
                    request("POST", f"/projects/{project}/{kind}")
                    while time.monotonic() - started < 300:
                        progress = request("GET", f"/projects/{project}/progress")
                        if progress["state"] in {"completed", "failed", "cancelled"}:
                            assert progress["state"] == ("completed" if succeeds else "failed"), progress
                            if succeeds:
                                assert not progress["errors"], progress
                            return {**progress, "seconds": time.monotonic() - started}
                        time.sleep(.1)
                    raise TimeoutError(kind)

                report["scan"] = job("scan")
                listing = request("GET", f"/projects/{project}/photos")
                assert len(listing) == len(original_hashes), listing
                mime = {"jpeg": "image/jpeg", "png": "image/png", "webp": "image/webp", "heic": "image/heic"}
                formats = set()
                for photo in listing:
                    formats.add(photo["format"])
                    content_type, data = request("GET", f"/photos/{photo['id']}/original", raw=True)
                    assert content_type == mime[photo["format"]], (photo, content_type)
                    assert hashlib.sha256(data).hexdigest() == original_hashes[photo["filename"]]
                    for route in ("thumb", "preview"):
                        content_type, data = request("GET", f"/photos/{photo['id']}/{route}", raw=True)
                        assert content_type == "image/jpeg" and data[:2] == b"\xff\xd8"
                assert formats == {"jpeg", "png", "webp", "heic"}, formats
                report["formats"] = sorted(formats)
                report["default_analysis"] = job("analyze")
                baseline = {p["id"]: p["analysis"] for p in request("GET", f"/projects/{project}/photos")}
                settings = request("GET", f"/settings?project_id={project}")
                settings.update(embedding_provider="dinov3_vits16", embedding_model_sha256=SHA, semantic_similarity_threshold=.9)
                request("PUT", f"/settings?project_id={project}", settings)
                report["analysis"] = job("analyze")
                assert report["analysis"]["result"]["analyzed"] == 0
                listing = request("GET", f"/projects/{project}/photos")
                vectors = {p["id"]: p["analysis"].get("embedding") for p in listing}
                assert 0 < sum(v is not None for v in vectors.values()) < len(listing), vectors
                report["candidate_only_embedding_count"] = sum(v is not None for v in vectors.values())
                for photo in listing:
                    quality = dict(photo["analysis"])
                    quality.pop("embedding", None)
                    quality.pop("settings", None)
                    expected = dict(baseline[photo["id"]])
                    expected.pop("embedding", None)
                    expected.pop("settings", None)
                    quality.pop("elapsed_ms", None)
                    expected.pop("elapsed_ms", None)
                    assert quality == expected, (photo["filename"], {k: (expected.get(k), quality.get(k)) for k in quality.keys() | expected.keys() if quality.get(k) != expected.get(k)})
                report["default_quality_preserved"] = True
                for photo in listing:
                    assert photo["analysis_status"] == "current"
                    embedding = photo["analysis"].get("embedding")
                    if embedding is None:
                        continue
                    assert embedding["model_sha256"] == SHA and len(embedding["vector"]) == 384
                    assert abs(sum(x*x for x in embedding["vector"]) - 1) < .001
                    assert all(math.isfinite(x) for x in embedding["vector"])
                report["incremental"] = job("analyze")
                assert report["incremental"]["result"]["reused"] == len(listing)
                settings["semantic_similarity_threshold"] = .91
                request("PUT", f"/settings?project_id={project}", settings)
                after_settings = request("GET", f"/projects/{project}/photos")
                assert all(p["analysis_status"] == "current" for p in after_settings), [(p["id"], p["analysis_status"]) for p in after_settings]
                report["threshold_reanalysis"] = job("analyze")
                assert report["threshold_reanalysis"]["result"]["reused"] == len(listing)
                assert report["threshold_reanalysis"]["result"]["semantic"]["computed"] == 0
                assert vectors == {p["id"]: p["analysis"].get("embedding") for p in request("GET", f"/projects/{project}/photos")}
                report["threshold_vectors_identical"] = True
                # Artifact preflight must reject even when every persisted vector is current.
                model.write_bytes(b"corrupt disposable model")
                report["corrupt_model"] = job("analyze", False)
                model.unlink()
                report["missing_model"] = job("analyze")
                assert report["missing_model"]["result"]["semantic"]["mode"] == "phash_fallback"
                assert report["missing_model"]["result"]["semantic"]["warnings"]
                shutil.copy2(args.embedding_model, model)
                report["restored_model"] = job("analyze")
                assert report["restored_model"]["result"]["reused"] == len(listing)
                settings["embedding_model_sha256"] = "a" * 64
                request("PUT", f"/settings?project_id={project}", settings)
                report["wrong_hash"] = job("analyze", False)
                settings.update(embedding_provider="none", embedding_model_sha256=None, semantic_similarity_threshold=None)
                request("PUT", f"/settings?project_id={project}", settings)
                report["disabled"] = job("analyze")
                assert report["disabled"]["result"]["semantic"]["mode"] == "phash"
                assert {p.name: digest(p) for p in photos.iterdir() if p.is_file()} == original_hashes
                report["source_bytes_unchanged"] = True
                report["photo_count"] = len(listing)
                report["ok"] = True
            finally:
                process.stdin.write("shutdown\n")
                process.stdin.flush()
                process.stdin.close()
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
    assert {str(p): digest(p) for p in sources} == source_hashes
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"ok": True, "photos": report["photo_count"], "formats": report["formats"], "report": str(args.output)}))


if __name__ == "__main__":
    main()
