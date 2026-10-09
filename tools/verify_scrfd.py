"""Local paired YuNet/SCRFD execution and optional raw-tensor geometry verification.

Uses only explicitly acquired local research weights. Never downloads weights,
uploads photographs, generates truth labels, or claims comparative accuracy.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import json
import math
from pathlib import Path
import sqlite3
import subprocess
import sys

from setup_scrfd import MODEL_SHA256, OUTPUT_NAMES
from verify_sample import digest


def predictions(path: Path) -> dict:
    with closing(sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True)) as db:
        return {name: json.loads(data) for name, data in db.execute(
            "SELECT p.path,a.data FROM photos p JOIN analyses a ON a.photo_id=p.id")}


def verify_geometry(report: dict) -> dict:
    """Independently reconstruct anchor coordinates from actual raw ORT outputs.

    This tests KPS output order, anchor duplication, stride/scale restoration and
    clipping. It does not call the production decoder or certify detection truth.
    NMS/8px filtering is intentionally the application's policy, not an exact
    reproduction of upstream pixel-inclusive NMS.
    """
    if report["model_sha256"] != MODEL_SHA256 or report["output_names"] != OUTPUT_NAMES:
        raise ValueError("Geometry capture does not identify the pinned original model")
    width, height, scale = report["width"], report["height"], report["scale"]
    if width <= 0 or height <= 0 or not math.isfinite(scale) or scale <= 0:
        raise ValueError("Invalid captured geometry")
    tensors = report["tensors"]
    if len(tensors) != 9:
        raise ValueError("Nine output heads required")
    candidates = []
    for level, stride in enumerate((8, 16, 32)):
        count = 2 * (640 // stride) ** 2
        scores, boxes, points = (tensors[level + offset] for offset in (0, 3, 6))
        for tensor, channels in ((scores, 1), (boxes, 4), (points, 10)):
            if tensor["shape"] not in ([count, channels], [1, count, channels]):
                raise ValueError("Unexpected output head shape")
            if len(tensor["values"]) != count * channels or not all(map(math.isfinite, tensor["values"])):
                raise ValueError("Malformed output values")
        for index, confidence in enumerate(scores["values"]):
            if confidence < report["threshold"]:
                continue
            # Two anchor predictions at each grid point, with no half-cell offset.
            cell = index // 2
            center = (cell % (640 // stride) * stride, cell // (640 // stride) * stride)
            l, t, r, b = (v * stride for v in boxes["values"][index * 4:index * 4 + 4])
            x0, y0 = max(0., min(width, (center[0] - l) / scale)), max(0., min(height, (center[1] - t) / scale))
            x1, y1 = max(0., min(width, (center[0] + r) / scale)), max(0., min(height, (center[1] + b) / scale))
            keypoints = [(center[j % 2] + value * stride) / scale
                         for j, value in enumerate(points["values"][index * 10:index * 10 + 10])]
            candidates.append([confidence, x0, y0, x1 - x0, y1 - y0, *keypoints])
    detections = report["detections"]
    if not detections:
        raise ValueError("Geometry fixture needs at least one real detection")
    maximum_error = 0.
    for detection in detections:
        values = [detection["confidence"], *detection["bbox"],
                  *(v for point in detection["keypoints"] for v in point)]
        error, index = min((max(abs(a - b) for a, b in zip(values, candidate)), index)
                           for index, candidate in enumerate(candidates))
        if error > 0.002:
            raise ValueError(f"Rust geometry disagrees with raw stride heads: {error}")
        maximum_error = max(maximum_error, error)
        candidates.pop(index)
    return {"passed": True, "matched_detections": len(detections),
            "maximum_coordinate_or_score_error": maximum_error, "absolute_tolerance": 0.002,
            "scope": "Raw ONNX head coordinate reconstruction; not face detection accuracy or exact OpenCV preprocessing parity",
            "implementation_policy": "RGB normalized (x-127.5)/128; black bottom/right padding; Triangle resize; clipped >=8px boxes; continuous-coordinate IoU NMS at 0.4"}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, default=Path("test-photos"))
    parser.add_argument("--daemon", type=Path, default=Path("target/release/iris-daemon.exe"))
    parser.add_argument("--models", type=Path, default=Path("models"))
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--geometry", type=Path, help="Optional raw capture from the ignored Rust SCRFD test")
    parser.add_argument("--geometry-only", action="store_true", help="Verify an existing capture without running the paired workflow")
    args = parser.parse_args()
    if args.output_dir.exists():
        parser.error("Output directory already exists; preserve earlier evidence")
    if args.geometry_only and args.geometry is None:
        parser.error("--geometry-only requires --geometry")
    geometry = verify_geometry(json.loads(args.geometry.read_text("utf-8"))) if args.geometry else None
    args.output_dir.mkdir(parents=True)
    out = args.output_dir.resolve()
    if args.geometry_only:
        (out / "geometry-verification.json").write_text(json.dumps(geometry, indent=2) + "\n", "utf-8")
        print(json.dumps(geometry, indent=2))
        return
    if digest(args.models / "optional/scrfd_500m.onnx") != MODEL_SHA256:
        raise ValueError("Selected optional model does not match the official research pin")
    settings = {"face_detector": "scrfd_500m", "scrfd_model_sha256": MODEL_SHA256}
    settings_file = out / "scrfd-settings.json"
    settings_file.write_text(json.dumps(settings, indent=2), "utf-8")
    for variant in ("yunet", "scrfd"):
        command = [sys.executable, str(Path(__file__).with_name("verify_sample.py")),
                   "--fixtures", str(args.fixtures.resolve()), "--daemon", str(args.daemon.resolve()),
                   "--models", str(args.models.resolve()), "--output", str(out / f"{variant}.json"),
                   "--snapshot", str(out / f"{variant}.sqlite3")]
        if variant == "scrfd":
            command += ["--settings-file", str(settings_file)]
        with (out / f"{variant}.log").open("w", encoding="utf-8") as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
        print(f"{variant}: 100-JPG local workflow completed", flush=True)
    reports = {name: json.loads((out / f"{name}.json").read_text("utf-8")) for name in ("yunet", "scrfd")}
    before, after = (predictions(out / f"{name}.sqlite3") for name in ("yunet", "scrfd"))
    if len(before) != 100 or before.keys() != after.keys():
        raise ValueError("Paired snapshots must contain the same 100 photos")
    counts = {"same": 0, "scrfd_more": 0, "scrfd_fewer": 0}
    for path, baseline in before.items():
        selected = after[path]
        for field in ("width", "height", "original_width", "original_height", "orientation", "sharpness_lap", "sharpness_fft", "niqe", "exposure", "phash", "structure", "version"):
            if baseline[field] != selected[field]:
                raise ValueError(f"Non-detector observation changed: {field}")
        difference = len(selected["faces"]) - len(baseline["faces"])
        counts["same" if difference == 0 else "scrfd_more" if difference > 0 else "scrfd_fewer"] += 1
    report = {
        "passed": True, "photos": 100, "daemon_sha256": digest(args.daemon),
        "model_sha256": MODEL_SHA256, "license_scope": "noncommercial-research",
        "commercial_authorization": False, "default_remains_yunet": reports["yunet"]["effective_settings"]["face_detector"] == "yunet",
        "source_hashes_verified_before_after": all(r["hashes_verified_before_and_after"] for r in reports.values()),
        "face_counts": {k: r["faces"] for k, r in reports.items()}, "per_photo_face_count_comparison": counts,
        "analysis_seconds": {k: r["analysis"]["seconds"] for k, r in reports.items()},
        "cached_seconds": {k: r["incremental"]["seconds"] for k, r in reports.items()},
        "photo_latency": {k: r["photo_latency"] for k, r in reports.items()},
        "eye_states": {k: r["eye_states"] for k, r in reports.items()},
        "geometry": geometry, "accuracy": "Not evaluated: no independent face/eye/preference labels. More detections do not establish better accuracy.",
    }
    (out / "verification.json").write_text(json.dumps(report, indent=2) + "\n", "utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
