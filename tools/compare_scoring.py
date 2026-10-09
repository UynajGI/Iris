"""Audit a scoring-version change without treating unlabelled predictions as truth."""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import json
import math
from pathlib import Path
import sqlite3
import statistics


def read(path: Path) -> dict:
    with closing(sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True)) as db:
        return {name: json.loads(data) for name, data in db.execute(
            "SELECT p.path,a.data FROM photos p JOIN analyses a ON a.photo_id=p.id")}


def unchanged_observations(analysis: dict) -> dict:
    result = {key: value for key, value in analysis.items()
              if key not in {"elapsed_ms", "version", "composite_score", "verdict", "warnings", "score_breakdown"}}
    result["faces"] = [{key: value for key, value in face.items()
                        if key not in {"quality", "quality_unavailable_reason"}} for face in result["faces"]]
    settings = dict(result.get("settings", {}))
    settings.setdefault("occlusion_provider", "none")
    settings.setdefault("occlusion_model_sha256", None)
    settings.setdefault("occlusion_min_visible_fraction", None)
    result["settings"] = settings
    return result


def summary(values: list) -> dict:
    return {"min": min(values), "median": statistics.median(values), "max": max(values)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, default=Path("artifacts/faceocc-integration-v2/default.sqlite3"))
    parser.add_argument("--current", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("report already exists; choose a new output")
    baseline, current = read(args.baseline), read(args.current)
    assert baseline and baseline.keys() == current.keys(), "snapshot photo sets differ"
    transitions = {}
    deltas = []
    quality_count = 0
    face_count = 0
    changed_scores = 0
    for path, before in baseline.items():
        after = current[path]
        assert unchanged_observations(before) == unchanged_observations(after), f"non-scoring observations changed: {path}"
        assert after.get("score_breakdown") is not None, f"missing new score breakdown: {path}"
        assert math.isfinite(after["composite_score"]) and 0 <= after["composite_score"] <= 100
        key = before["verdict"] + " -> " + after["verdict"]
        transitions[key] = transitions.get(key, 0) + 1
        delta = after["composite_score"] - before["composite_score"]
        deltas.append(delta)
        changed_scores += abs(delta) > 1e-9
        for face in after["faces"]:
            face_count += 1
            quality = face.get("quality")
            if quality is not None:
                assert math.isfinite(quality["score"]) and 0 <= quality["score"] <= 100
                quality_count += 1
            else:
                assert face.get("quality_unavailable_reason"), "new analysis must explain missing face quality"
    observations = {path: unchanged_observations(value) for path, value in current.items()}
    report = {
        "passed": True, "photos": len(current), "faces": face_count,
        "face_quality_measurements": quality_count,
        "versions_before": sorted({a["version"] for a in baseline.values()}),
        "versions_after": sorted({a["version"] for a in current.values()}),
        "non_scoring_observations_unchanged": True,
        "observation_sha256": hashlib.sha256(json.dumps(observations, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
        "changed_scores": changed_scores, "score_delta": summary(deltas),
        "score_before": summary([a["composite_score"] for a in baseline.values()]),
        "score_after": summary([a["composite_score"] for a in current.values()]),
        "verdict_transitions": transitions,
        "accuracy": "Not measured. These are changes to explicit technical heuristics, not evidence of improved selections.",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2), "utf-8")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
