"""Independently compare persisted scale predictions, vector reuse and v5 baseline."""
import argparse
from contextlib import closing
import hashlib
import json
from pathlib import Path
import sqlite3

ROOT = Path(__file__).resolve().parents[1]


def records(database):
    with closing(sqlite3.connect(database.resolve().as_uri() + "?mode=ro", uri=True)) as db:
        return {path.replace("\\", "/"): json.loads(data) for path, data in db.execute("select p.path,a.data from photos p join analyses a on a.photo_id=p.id")}


def clean(value, keys):
    return {k: v for k, v in value.items() if k not in keys}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--default", type=Path, default=ROOT / "artifacts/scale-final-default")
    parser.add_argument("--dino", type=Path, default=ROOT / "artifacts/scale-final-dino")
    parser.add_argument("--baseline", type=Path, default=ROOT / "artifacts/scrfd-paired-final/yunet.sqlite3")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-entries", type=int, default=3000)
    args = parser.parse_args()
    default = records(args.default / "run/data/library.sqlite3")
    dino = records(args.dino / "run/data/library.sqlite3")
    baseline = {Path(p).name: a for p, a in records(args.baseline).items()}
    assert default.keys() == dino.keys() and len(default) == args.expected_entries and len(baseline) == 100
    quality_differences = [p for p in default if clean(default[p], {"elapsed_ms", "settings", "embedding"}) != clean(dino[p], {"elapsed_ms", "settings", "embedding"})]
    baseline_differences = []
    for path, value in default.items():
        source = Path(path).name.split("-", 1)[1]
        expected = baseline[source]
        if clean(value, {"elapsed_ms", "version"}) != clean(expected, {"elapsed_ms", "version"}):
            baseline_differences.append({"path": path, "keys": [k for k in value.keys() | expected.keys() if k not in {"elapsed_ms", "version"} and value.get(k) != expected.get(k)]})
    report = {"ok": not quality_differences and not baseline_differences,
        "entries": len(default), "independent_sources": len(baseline), "repeats": len(default) // len(baseline),
        "v5_versions": sorted({a["version"] for a in baseline.values()}), "current_versions": sorted({a["version"] for a in default.values()}),
        "baseline_comparison_excludes": ["elapsed_ms", "version"],
        "dino_quality_comparison_excludes": ["elapsed_ms", "settings", "embedding"],
        "baseline_differences": baseline_differences, "dino_quality_differences": quality_differences,
        "source_files": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in [args.default / "report.json", args.dino / "report.json", args.baseline]}}
    args.output.write_text(json.dumps(report, indent=2), "utf-8")
    print(json.dumps({"ok": report["ok"], "baseline_differences": len(baseline_differences), "dino_quality_differences": len(quality_differences)}))
    assert report["ok"], report


if __name__ == "__main__":
    main()
