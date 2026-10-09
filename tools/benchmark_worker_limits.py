"""Screen bounded worker counts with identical inputs, predictions and resource sampling.

Requires psutil for this development diagnostic only. Install into an isolated
directory and pass --dependency-dir; it is not a runtime/package dependency.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time

from verify_faceocc import predictions
from verify_sample import digest


def monitored_run(command: list[str], log_path: Path, psutil) -> dict:
    samples = []
    sample_times = []
    vanished = denied = 0
    with log_path.open("w", encoding="utf-8") as log:
        child = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
        root = psutil.Process(child.pid)
        try:
            while child.poll() is None:
                sweep_started = time.monotonic()
                rss = private = threads = count = 0
                private_available = True
                try:
                    descendants = root.children(recursive=True)
                except psutil.NoSuchProcess:
                    descendants = []
                    vanished += 1
                except psutil.AccessDenied:
                    descendants = []
                    denied += 1
                for process in descendants:
                    try:
                        # Own descendants only; exclude the Python test harness.
                        if process.name().lower() not in {"iris-daemon", "iris-daemon.exe"}:
                            continue
                        memory = process.memory_info()
                        rss += memory.rss
                        committed = getattr(memory, "private", None)
                        if committed is None:
                            private_available = False
                        else:
                            private += committed
                        threads += process.num_threads()
                        count += 1
                    except psutil.NoSuchProcess:
                        vanished += 1
                    except psutil.AccessDenied:
                        denied += 1
                if count:
                    sample_times.append(sweep_started)
                    samples.append({"rss": rss, "private": private if private_available else None,
                                    "threads": threads, "processes": count})
                time.sleep(0.2)
            if child.returncode:
                raise subprocess.CalledProcessError(child.returncode, command)
        finally:
            if child.poll() is None:
                # An interrupted monitor must not orphan the daemon owned by
                # the verification harness. Never enumerate unrelated roots.
                try:
                    descendants = root.children(recursive=True)
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    descendants = []
                for process in reversed(descendants):
                    try:
                        process.terminate()
                    except (psutil.NoSuchProcess, psutil.AccessDenied):
                        pass
                child.terminate()
                child.wait(timeout=30)
    assert samples, "no owned daemon process resource samples collected"
    gaps = [b - a for a, b in zip(sample_times, sample_times[1:])]
    return {"psutil_version": psutil.__version__, "sampling_sleep_seconds": 0.2,
            "observed_gap_median_seconds": statistics.median(gaps) if gaps else None,
            "observed_gap_max_seconds": max(gaps, default=None),
            "samples": len(samples), "sampled_peak_sum_rss_bytes": max(s["rss"] for s in samples),
            "sampled_peak_sum_private_commit_bytes": max((s["private"] for s in samples if s["private"] is not None), default=None),
            "sampled_peak_sum_threads": max(s["threads"] for s in samples),
            "sampled_peak_daemon_processes_including_parent": max(s["processes"] for s in samples),
            "vanished_process_observations": vanished, "access_denied_observations": denied,
            "interpretation": "Owned daemon plus workers, per-sweep sums (not atomic snapshots). Each sweep is followed by a 0.2s sleep; observed gaps include enumeration/query cost. RSS counts shared pages repeatedly; Windows private is commit, not physical USS. Peaks are sampled lower bounds. Polling overhead is included in every run; vanishing process observations can be normal shutdown races."}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--daemon", type=Path, default=Path("target/release/iris-daemon.exe"))
    parser.add_argument("--fixtures", type=Path, default=Path("test-photos"))
    parser.add_argument("--models", type=Path, default=Path("models"))
    parser.add_argument("--dependency-dir", type=Path)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--limits", type=int, nargs="+", default=[4, 8, 12, 16])
    parser.add_argument("--scale-repeat", type=int, choices=range(1, 101), help="Use the sustained hard-link benchmark instead of the 100-photo domain workflow")
    args = parser.parse_args()
    if len(set(args.limits)) != len(args.limits) or any(not 1 <= n <= 16 for n in args.limits):
        parser.error("limits must be distinct integers in 1..16")
    if args.output_dir.exists():
        parser.error("output directory exists; choose a new path")
    if args.dependency_dir:
        sys.path.insert(0, str(args.dependency_dir.resolve(strict=True)))
    import psutil
    args.output_dir.mkdir(parents=True)
    checksum = digest(args.daemon)
    order = args.limits + list(reversed(args.limits))
    baseline = None
    rows = []
    for index, limit in enumerate(order):
        name = f"{index + 1}-workers-{limit}"
        report_path = args.output_dir / f"{name}.json"
        snapshot_path = args.output_dir / f"{name}.sqlite3"
        command = [
            sys.executable, str(Path(__file__).with_name("benchmark_scale.py" if args.scale_repeat else "verify_sample.py")),
            "--daemon", str(args.daemon.resolve()), "--fixtures", str(args.fixtures.resolve()),
            "--models", str(args.models.resolve()), "--worker-limit", str(limit),
            "--output", str(report_path),
        ]
        command += ["--repeat", str(args.scale_repeat)] if args.scale_repeat else ["--snapshot", str(snapshot_path)]
        resources = monitored_run(command, args.output_dir / f"{name}.log", psutil)
        assert digest(args.daemon) == checksum, "binary changed during experiment"
        r = json.loads(report_path.read_text("utf-8"))
        current = r["prediction_consistency"]["per_source_predictions_sha256"] if args.scale_repeat else predictions(snapshot_path)
        if baseline is None:
            baseline = current
        else:
            assert current == baseline, "complete non-timing predictions differ"
        analysis = r["phases"]["first_analysis"] if args.scale_repeat else r["analysis"]
        assert 1 <= analysis["result"]["workers"] <= limit
        row = {"run": name, "worker_limit": limit, "workers": analysis["result"]["workers"],
               "seconds": analysis["seconds"], "photo_latency": r.get("photo_latency"),
               "timing_ms": analysis["result"]["timing_ms"], "resources": resources}
        if args.scale_repeat:
            row.update({k:r[k] for k in ["first_scan_and_analysis_seconds", "incremental_scan_and_analysis_seconds", "analysis_photos_per_second", "source_hashes_preserved"]})
        rows.append(row)
        print(json.dumps(row), flush=True)
    report = {"passed": True, "daemon_sha256": checksum, "order": order,
              "unique_sources": len(baseline), "photos_per_run": len(baseline) * (args.scale_repeat or 1),
              "full_non_timing_predictions_unchanged": True,
              "cpu_logical_count": os.cpu_count(), "rayon_num_threads": os.environ.get("RAYON_NUM_THREADS"),
              "runs": rows, "median_analysis_seconds": {
                  str(limit): statistics.median(r["seconds"] for r in rows if r["worker_limit"] == limit)
                  for limit in args.limits},
              "limitations": "Two runs per limit, balanced order, same 100 source photos. Repeated hardlinks add no scene diversity. No accuracy claim; resource peaks are sampled. Test flags do not change default policy."}
    (args.output_dir / "verification.json").write_text(json.dumps(report, indent=2), "utf-8")
    print(json.dumps(report["median_analysis_seconds"]), flush=True)


if __name__ == "__main__":
    main()
