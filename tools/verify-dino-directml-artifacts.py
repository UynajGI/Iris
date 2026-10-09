"""Exercise missing GPU-graph fallback and corruption rejection before cache reuse."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "artifacts/validation-python"))
from validation_support import Daemon


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output must be new")
    photos = args.output / "photos"
    models = args.output / "models"
    photos.mkdir(parents=True)
    models.mkdir()
    source = sorted((ROOT / "test-photos").glob("*.jpg"))[0]
    source_hash = digest(source)
    for name in ["a.jpg", "b.jpg"]:
        shutil.copy2(source, photos / name)
    for file in (ROOT / "models").iterdir():
        if file.is_file():
            shutil.copy2(file, models / file.name)
    shutil.copytree(ROOT / "models/directml", models / "directml")
    (models / "optional").mkdir()
    shutil.copy2(ROOT / "models/optional/dinov3_vits16.onnx", models / "optional/dinov3_vits16.onnx")
    binary = ROOT / "target/release/iris-daemon.exe"
    report = {"ok": False, "daemon_sha256": digest(binary)}
    try:
        with Daemon(binary, models, args.output / "run", workers=1) as daemon:
            project = daemon.request("POST", "/projects", {"root": str(photos.resolve())})["id"]
            settings = daemon.request("GET", f"/settings?project_id={project}")
            settings.update(embedding_provider="dinov3_vits16", embedding_model_sha256="f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c", semantic_similarity_threshold=.9, execution_provider="directml", directml_device_id=0)
            daemon.request("PUT", f"/settings?project_id={project}", settings)
            daemon.job(project, "scan")
            report["missing_graph"] = daemon.job(project, "analyze")
            rows = daemon.request("GET", f"/projects/{project}/photos")
            assert report["missing_graph"]["result"]["semantic"]["computed"] == 2
            assert all(any("fixed-shape graph missing; CPU fallback" in w for w in p["analysis"]["warnings"]) for p in rows)
            vectors = [p["analysis"]["embedding"] for p in rows]
            graph = models / "optional/dinov3_vits16_directml.onnx"
            shutil.copy2(ROOT / "models/optional/dinov3_vits16_directml.onnx", graph)
            with graph.open("r+b") as target:
                value = target.read(1)
                target.seek(0)
                target.write(bytes([value[0] ^ 1]))
            report["corrupt_graph_with_cached_vectors"] = daemon.job(project, "analyze", expected="failed")
            assert "DirectML artifact SHA-256 mismatch" in json.dumps(report["corrupt_graph_with_cached_vectors"])
            after = daemon.request("GET", f"/projects/{project}/photos")
            assert vectors == [p["analysis"]["embedding"] for p in after]
        report.update(ok=True, cached_vectors_preserved=True, source_hash_preserved=digest(source) == source_hash)
        assert report["source_hash_preserved"]
    finally:
        (args.output / "report.json").write_text(json.dumps(report, indent=2), "utf-8")
    print(json.dumps({"ok": True, "missing_graph_cpu_fallback": True, "corrupt_graph_cache_bypass_rejected": True}))


if __name__ == "__main__":
    main()
