"""Compare inherited-default versus one-thread Rayon on the same release binary.

Four independent 100-JPG workflows run in ABBA order; this is not an accuracy
benchmark and does not alter the production worker configuration.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys

from verify_faceocc import predictions
from verify_sample import digest


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--daemon", type=Path, default=Path("target/release/iris-daemon.exe"))
    parser.add_argument("--fixtures", type=Path, default=Path("test-photos"))
    parser.add_argument("--models", type=Path, default=Path("models"))
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    if args.output_dir.exists():
        parser.error("output directory exists; choose a new path")
    args.output_dir.mkdir(parents=True)
    checksum = digest(args.daemon)
    baseline = None
    rows = []
    for index, threads in enumerate((None, "1", "1", None)):
        name = f"{index + 1}-rayon-{threads or 'default'}"
        env = os.environ.copy()
        env.pop("RAYON_NUM_THREADS", None)
        if threads is not None:
            env["RAYON_NUM_THREADS"] = threads
        report_path = args.output_dir / f"{name}.json"
        snapshot_path = args.output_dir / f"{name}.sqlite3"
        with (args.output_dir / f"{name}.log").open("w", encoding="utf-8") as log:
            subprocess.run([sys.executable, str(Path(__file__).with_name("verify_sample.py")),
                            "--daemon", str(args.daemon.resolve()), "--fixtures", str(args.fixtures.resolve()),
                            "--models", str(args.models.resolve()), "--output", str(report_path),
                            "--snapshot", str(snapshot_path)], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        assert digest(args.daemon) == checksum, "release binary changed during experiment"
        current = predictions(snapshot_path)
        if baseline is None:
            baseline = current
        else:
            assert current == baseline, "full deterministic predictions differ across thread modes"
        r = json.loads(report_path.read_text("utf-8"))
        row = {"run": name, "rayon_num_threads": threads, "seconds": r["analysis"]["seconds"],
               "photo_latency": r["photo_latency"], "workers": r["analysis"]["result"]["workers"],
               "timing_ms": r["analysis"]["result"]["timing_ms"]}
        rows.append(row)
        print(json.dumps(row), flush=True)
    report = {"passed": True, "daemon_sha256": checksum, "order": "ABBA",
              "photos_per_run": len(baseline), "runs": rows,
              "full_non_timing_predictions_unchanged": True,
              "median_seconds": {mode: statistics.median([r["seconds"] for r in rows if r["rayon_num_threads"] == value])
                                 for mode, value in (("default", None), ("one", "1"))},
              "median_service_ms": {mode: statistics.median([r["timing_ms"]["total"] for r in rows if r["rayon_num_threads"] == value])
                                    for mode, value in (("default", None), ("one", "1"))},
              "median_processing_ms": {mode: statistics.median([r["timing_ms"]["processing"] for r in rows if r["rayon_num_threads"] == value])
                                       for mode, value in (("default", None), ("one", "1"))},
              "limitations": "Two runs per mode, same 100 photos. No sustained-load claim, hardware/thread-count measurement or accuracy validation. No production setting changed."}
    (args.output_dir / "verification.json").write_text(json.dumps(report, indent=2), "utf-8")
    print(json.dumps(report), flush=True)


if __name__ == "__main__":
    main()
