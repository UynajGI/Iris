"""Exercise actual Iris GPU sessions, profile operator placement, and CPU fallback."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "artifacts/validation-python"))
from validation_support import Daemon


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--photos", type=Path, default=ROOT / "test-photos")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--count", type=int, default=12)
    parser.add_argument("--dinov3", action="store_true")
    parser.add_argument("--require-dino-gpu", action="store_true", help="require completed DINO GPU inference with no CPU fallback on DXGI indices 0 and 1")
    parser.add_argument("--require-inventory", action="store_true", help="record the backend DXGI inventory and match tested hardware indices")
    parser.add_argument("--fallback-only", action="store_true", help="CI without GPU: missing runtime and bad device CPU fallback, corrupt runtime refusal")
    parser.add_argument("--daemon", type=Path, default=ROOT / "target/release/iris-daemon.exe")
    parser.add_argument("--models", type=Path, default=ROOT / "models")
    args = parser.parse_args()
    if args.require_dino_gpu and (not args.dinov3 or args.fallback_only):
        parser.error("--require-dino-gpu requires --dinov3 and real adapter runs")
    if args.output.exists():
        parser.error("output must be new")
    photos = args.output / "photos"
    photos.mkdir(parents=True)
    sources = sorted(p for p in args.photos.rglob("*") if p.suffix.lower() in {".jpg", ".jpeg"})[:args.count]
    assert len(sources) == args.count
    source_hashes = {str(p): digest(p) for p in sources}
    for i, source in enumerate(sources):
        shutil.copy2(source, photos / f"{i:03}.jpg")
    if args.dinov3:
        shutil.copy2(sources[0], photos / "duplicate.jpg")
    count = len(sources) + int(args.dinov3)
    report = {"ok": False, "count": count, "dinov3": args.dinov3, "daemon_sha256": digest(args.daemon), "runs": {}}
    try:
        runs = [("cpu", None), ("directml_0", 0), ("directml_1", 1), ("invalid_device_fallback", 2147483647)]
        if args.fallback_only:
            runs = [("cpu", None), ("invalid_device_fallback", 2147483647)]
        if not args.dinov3:
            runs += [("missing_runtime_fallback", 0), ("corrupt_runtime_rejected", 0)]
        for label, device in runs:
            run = args.output / label
            models = args.models
            if label in {"missing_runtime_fallback", "corrupt_runtime_rejected"}:
                models = run / "models"
                models.mkdir(parents=True)
                for path in args.models.iterdir():
                    if path.is_file():
                        shutil.copy2(path, models / path.name)
                if label == "corrupt_runtime_rejected":
                    shutil.copytree(args.models / "directml", models / "directml")
                    (models / "directml/DirectML.dll").write_bytes(b"corrupt disposable runtime")
            with Daemon(args.daemon, models, run, workers=2,
                        env={"IRIS_ORT_PROFILE_DIR": str((run / "profiles").resolve())}) as daemon:
                adapter = None
                if args.require_inventory:
                    inventory = daemon.request("GET", "/devices/gpu")
                    report.setdefault("gpu_inventories", {})[label] = inventory
                    assert inventory["status"] == "available" and not inventory["inference_verified"]
                    if label.startswith("directml_"):
                        adapter = next(a for a in inventory["adapters"] if a["device_id"] == device)
                        assert not adapter["is_software"], adapter
                project = daemon.request("POST", "/projects", {"root": str(photos.resolve())})["id"]
                if device is not None or args.dinov3:
                    settings = daemon.request("GET", f"/settings?project_id={project}")
                    if device is not None:
                        settings.update(execution_provider="directml", directml_device_id=device)
                    if args.dinov3:
                        settings.update(embedding_provider="dinov3_vits16", embedding_model_sha256="f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c", semantic_similarity_threshold=.9)
                    daemon.request("PUT", f"/settings?project_id={project}", settings)
                daemon.job(project, "scan")
                result = daemon.job(project, "analyze", expected="failed" if label == "corrupt_runtime_rejected" else "completed")
                report["runs"][label] = {"job": result, "memory": daemon.memory}
                if label == "corrupt_runtime_rejected":
                    assert "SHA-256 mismatch" in json.dumps(result), result
                    report["runs"][label] = {"job": result, "rejected": True}
                    continue
                listing = daemon.request("GET", f"/projects/{project}/photos")
                records = {p["filename"]: p["analysis"] for p in listing}
                (run / "analyses.json").write_text(json.dumps(records, indent=2), "utf-8")
                assert result["result"]["analyzed"] == count
                if args.dinov3:
                    assert result["result"]["semantic"]["mode"] == "dinov3" and result["result"]["semantic"]["computed"] >= 2, result
                warnings = sorted({w for a in records.values() for w in a["warnings"] if "DirectML" in w})
                report["runs"][label] = {"job": result, "memory": daemon.memory, "warnings": warnings}
                if adapter is not None:
                    report["runs"][label]["adapter"] = adapter
            profiles = list((run / "profiles").glob("*.json"))
            providers = Counter()
            gpu_microseconds = 0
            for path in profiles:
                for event in json.loads(path.read_text("utf-8")):
                    provider = event.get("args", {}).get("provider")
                    if provider:
                        providers[provider] += 1
                        if provider == "DmlExecutionProvider":
                            gpu_microseconds += event.get("dur", 0)
            report["runs"][label]["operator_profiles"] = {"files": len(profiles), "provider_event_counts": dict(providers), "directml_duration_us": gpu_microseconds}
            if device is None:
                baseline = records
            else:
                counts_equal = all(len(records[k]["faces"]) == len(baseline[k]["faces"]) for k in baseline)
                score_error = max(abs(records[k]["composite_score"] - baseline[k]["composite_score"]) for k in baseline)
                verdict_changes = sum(records[k]["verdict"] != baseline[k]["verdict"] for k in baseline)
                report["runs"][label]["cpu_comparison"] = {"face_counts_equal": counts_equal, "max_score_absolute_error": score_error, "verdict_changes": verdict_changes}
                assert counts_equal and score_error < .5 and verdict_changes == 0, report["runs"][label]
                if args.dinov3:
                    cosines = []
                    for key, base in baseline.items():
                        if base.get("embedding"):
                            current = records[key]["embedding"]["vector"]
                            cosines.append(sum(x*y for x, y in zip(base["embedding"]["vector"], current)))
                    assert cosines and min(cosines) > .9999, cosines
                    report["runs"][label]["embedding_min_cosine_to_cpu"] = min(cosines)
                if label in {"invalid_device_fallback", "missing_runtime_fallback"}:
                    assert warnings and any("CPU fallback" in w for w in warnings)
                    assert providers["DmlExecutionProvider"] == 0
                else:
                    assert providers["DmlExecutionProvider"] > 0, report["runs"][label]
                    if args.dinov3:
                        dino_profiles = [p for p in profiles if "dinov3" in p.name]
                        dino_gpu = sum(e.get("args", {}).get("provider") == "DmlExecutionProvider" for p in dino_profiles for e in json.loads(p.read_text("utf-8")))
                        dino_fallback = any("dinov3" in w.lower() and "CPU fallback" in w for w in warnings)
                        report["runs"][label]["dinov3_execution"] = {"gpu_events": dino_gpu, "startup_cpu_fallback": dino_fallback}
                        assert dino_profiles and (dino_gpu > 0 or dino_fallback)
                        if args.require_dino_gpu:
                            assert dino_gpu > 0 and not any("CPU fallback" in w for w in warnings), warnings
            print(label, json.dumps(report["runs"][label]), flush=True)
        report["ok"] = True
    except Exception as error:
        report["error"] = repr(error)
        raise
    finally:
        report["source_hashes_preserved"] = all(digest(Path(p)) == sha for p, sha in source_hashes.items())
        (args.output / "report.json").write_text(json.dumps(report, indent=2), "utf-8")


if __name__ == "__main__":
    main()
