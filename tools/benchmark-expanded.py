"""Current-engine scale evidence on manifest-verified repeated inputs, not accuracy."""
import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import platform
import sqlite3
import statistics
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "artifacts/validation-python"))
from validation_support import Daemon
from benchmark_scale import digest, verify_repeated_predictions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeat", type=int, default=30)
    parser.add_argument("--dinov3", action="store_true")
    parser.add_argument("--workers", type=int, default=6)
    parser.add_argument("--daemon", type=Path, default=ROOT / "target/release/iris-daemon.exe")
    parser.add_argument("--models", type=Path, default=ROOT / "models")
    parser.add_argument("--first-budget-seconds", type=float, default=180)
    parser.add_argument("--cache-budget-seconds", type=float, default=30)
    parser.add_argument("--memory-budget-mib", type=float, default=4096)
    parser.add_argument("--baseline-report", type=Path)
    parser.add_argument("--failure-isolation", action="store_true")
    args = parser.parse_args()
    args.output = args.output.resolve()
    if args.output.exists() or not 1 <= args.repeat <= 100 or not 1 <= args.workers <= 16:
        parser.error("output must be new; repeat must be 1..100")
    if min(args.first_budget_seconds, args.cache_budget_seconds, args.memory_budget_mib) <= 0:
        parser.error("measurement budgets must be positive")
    manifest = json.loads((args.fixtures / "manifest.json").read_text("utf-8"))
    entries = manifest["files"]
    sources = [(args.fixtures / e["path"]).resolve(strict=True) for e in entries]
    assert len(entries) == manifest["count"]
    for path, entry in zip(sources, entries):
        assert path.is_relative_to(args.fixtures.resolve()) and digest(path) == entry["sha256"]
    args.output.mkdir(parents=True)
    photos = args.output / "photos"
    expected = {}
    for repeat in range(args.repeat):
        batch = photos / f"batch-{repeat:03}"
        batch.mkdir(parents=True)
        for index, source in enumerate(sources):
            target = batch / f"{index:03}-{source.name}"
            os.link(source, target)
            expected[target.relative_to(photos).as_posix()] = index
    report = {"complete": False, "unique_source_photos": len(sources), "repeats": args.repeat,
        "entries": len(expected), "dataset": manifest.get("description", "manifest-verified authorized source photographs; repeated hard links, not independent scenes"), "accuracy_evaluated": False,
        "platform": platform.platform(), "cpu_logical_count": os.cpu_count(), "requested_workers": args.workers,
        "per_photo_latency_scope": "base quality worker time; optional embedding pass is included in total job wall time, not per-photo quality latency",
        "daemon_sha256": digest(args.daemon), "dinov3": args.dinov3, "phases": {},
        "measurement_targets": {"first_seconds": args.first_budget_seconds,
            "cache_seconds": args.cache_budget_seconds, "rss_and_private_mib": args.memory_budget_mib},
        "memory_phases": {}}
    try:
        with Daemon(args.daemon, args.models, args.output / "run", args.workers) as daemon:
            report["memory"] = daemon.memory
            report["gpu_inventory"] = daemon.request("GET", "/devices/gpu")
            def phase(label, kind, expected_state="completed"):
                start = len(daemon.memory_samples)
                result = daemon.job(project, kind, expected=None)
                samples = daemon.memory_samples[start:]
                report["phases"][label] = result
                report["memory_phases"][label] = {"samples": len(samples),
                    "peak_rss_bytes": max((s["rss_bytes"] for s in samples), default=0),
                    "peak_private_bytes": max((s["private_bytes"] for s in samples), default=0),
                    "peak_processes": max((s["processes"] for s in samples), default=0)}
                if expected_state is not None:
                    assert result["state"] == expected_state, result
                return result
            project = daemon.request("POST", "/projects", {"root": str(photos.resolve())})["id"]
            if args.dinov3:
                settings = daemon.request("GET", f"/settings?project_id={project}")
                settings.update(embedding_provider="dinov3_vits16", embedding_model_sha256="f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c", semantic_similarity_threshold=.9)
                daemon.request("PUT", f"/settings?project_id={project}", settings)
            for label, kind in [("first_scan", "scan"), ("first_analysis", "analyze"), ("cached_scan", "scan"), ("cached_analysis", "analyze")]:
                phase(label, kind)
                print(label, json.dumps(report["phases"][label]["result"]), flush=True)
            first, cached = (report["phases"][x]["result"] for x in ["first_analysis", "cached_analysis"])
            assert first["analyzed"] == len(expected) and first["failed"] == 0
            assert cached["reused"] == len(expected) and cached["workers"] == 0
            database = args.output / "run/data/library.sqlite3"
            with closing(sqlite3.connect(database.as_uri() + "?mode=ro", uri=True)) as db:
                before = [json.loads(row[0]) for row in db.execute("select data from analyses")]
                vectors_before = {row[0]: json.loads(row[1]).get("embedding") for row in db.execute("select photo_id,data from analyses")}
                times = sorted(a["elapsed_ms"] for a in before if "elapsed_ms" in a)
                report["per_photo_latency_ms"] = {"count": len(times), "median": statistics.median(times), "p95": times[int(.95 * (len(times) - 1))]} if times else {"count": 0}
            if args.dinov3:
                assert first["semantic"]["mode"] == "dinov3" and first["semantic"]["candidates"] > 0
                assert cached["semantic"]["computed"] == 0
                settings["semantic_similarity_threshold"] = .91
                daemon.request("PUT", f"/settings?project_id={project}", settings)
                phase("threshold_regroup", "analyze")
                regroup = report["phases"]["threshold_regroup"]["result"]
                assert regroup["analyzed"] == 0 and regroup["semantic"]["computed"] == 0
            database = args.output / "run/data/library.sqlite3"
            with closing(sqlite3.connect(database.as_uri() + "?mode=ro", uri=True)) as db:
                assert db.execute("pragma quick_check").fetchone() == ("ok",)
                report["prediction_consistency"] = verify_repeated_predictions(db, expected)
                if args.baseline_report:
                    baseline = json.loads(args.baseline_report.read_text("utf-8"))
                    assert report["prediction_consistency"]["per_source_predictions_sha256"] == baseline["prediction_consistency"]["per_source_predictions_sha256"]
                    report["baseline_predictions_identical"] = True
                analyses = [json.loads(row[0]) for row in db.execute("select data from analyses")]
                report["embedding_rows"] = sum(bool(a.get("embedding")) for a in analyses)
                if args.dinov3:
                    vectors_after = {row[0]: json.loads(row[1]).get("embedding") for row in db.execute("select photo_id,data from analyses")}
                    assert vectors_before == vectors_after
                    report["threshold_vectors_identical"] = True
            if args.failure_isolation:
                (photos / "intentional-corrupt.jpg").write_bytes(b"intentionally invalid JPEG")
                scan_failure = phase("failure_scan", "scan", expected_state="failed")
                assert len(scan_failure["errors"]) == 1 and "intentional-corrupt.jpg" in scan_failure["errors"][0], scan_failure
                assert scan_failure["result"]["unchanged"] == len(expected), scan_failure
                failure = phase("failure_analysis", "analyze")
                assert failure["result"]["reused"] == len(expected) and failure["result"]["failed"] == 0, failure
                assert failure["result"]["analyzed"] == 0, failure
                report["failure_isolated"] = True
            # Short idle observation, not a long-term leak proof.
            start = len(daemon.memory_samples)
            time.sleep(3)
            tail = daemon.memory_samples[start:]
            report["idle_memory"] = {"observation_seconds": 3, "samples": len(tail),
                "median_rss_bytes": statistics.median(s["rss_bytes"] for s in tail) if tail else None,
                "median_private_bytes": statistics.median(s["private_bytes"] for s in tail) if tail else None}
            report["memory"] = dict(daemon.memory)
            phases = report["phases"]
            report["targets_passed"] = {
                "first_time": sum(phases[k]["seconds"] for k in ["first_scan", "first_analysis"]) <= args.first_budget_seconds,
                "cache_time": sum(phases[k]["seconds"] for k in ["cached_scan", "cached_analysis"]) <= args.cache_budget_seconds,
                "rss": report["memory"]["peak_tree_rss_bytes"] <= args.memory_budget_mib * 1024**2,
                "private": report["memory"]["peak_tree_private_bytes"] <= args.memory_budget_mib * 1024**2,
                "sampling": report["memory"]["samples"] > 0,
            }
            if args.dinov3:
                report["targets_passed"]["threshold_time"] = phases["threshold_regroup"]["seconds"] <= args.cache_budget_seconds
            if args.failure_isolation:
                report["targets_passed"]["failure_time"] = sum(phases[k]["seconds"] for k in ["failure_scan", "failure_analysis"]) <= args.cache_budget_seconds
            report["all_targets_passed"] = all(report["targets_passed"].values())
            report["complete"] = True
    except Exception as error:
        report["error"] = repr(error)
        raise
    finally:
        report["source_hashes_preserved"] = all(digest(p) == e["sha256"] for p, e in zip(sources, entries))
        if "first_analysis" in report["phases"]:
            report["analysis_photos_per_second"] = len(expected) / report["phases"]["first_analysis"]["seconds"]
        (args.output / "report.json").write_text(json.dumps(report, indent=2), "utf-8")
    assert report["source_hashes_preserved"]
    print(json.dumps({"complete": report["complete"], "entries": len(expected), "throughput": report["analysis_photos_per_second"], "memory": report["memory"]}))
    if not report["all_targets_passed"]:
        raise SystemExit("Measurement completed, but one or more predeclared targets failed; inspect report.json")


if __name__ == "__main__":
    main()
