"""Verify optional Rust FaceOcc on private JPGs, paired with the default path.

Uses only an already acquired, pinned local research model; never downloads or
bundles weights. The explicit test threshold is not an accuracy calibration.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import json
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile

from verify_sample import digest


ONNX_SHA256 = "e61151ef3be24948a2d45ba870a434fa3ce6b4c1b2d2dab90eb80a4fd635ea86"
REVISION = "03f229dc75fa14ae480cca9810f983912c2730ad"


def predictions(snapshot: Path) -> dict:
    with closing(sqlite3.connect(snapshot.resolve().as_uri() + "?mode=ro", uri=True)) as db:
        result = {
            path: {key: value for key, value in json.loads(data).items() if key != "elapsed_ms"}
            for path, data in db.execute("SELECT p.path,a.data FROM photos p JOIN analyses a ON a.photo_id=p.id")
        }
    # Older serialized settings predate the additive, disabled provider fields.
    # Normalize only their documented defaults; retain every prediction field.
    for analysis in result.values():
        settings = analysis.get("settings", {})
        settings.setdefault("occlusion_provider", "none")
        settings.setdefault("occlusion_model_sha256", None)
        settings.setdefault("occlusion_min_visible_fraction", None)
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, default=Path("test-photos"))
    parser.add_argument("--daemon", type=Path, default=Path("target/release/iris-daemon.exe"))
    parser.add_argument("--models", type=Path, default=Path("models"))
    parser.add_argument("--onnx", type=Path, default=Path("artifacts/faceocc-research/faceocc-research.onnx"))
    parser.add_argument("--baseline", type=Path, help="Optional same-analysis-version snapshot for exact default regression")
    parser.add_argument("--threshold", type=float, required=True, help="Explicit experimental minimum visible fraction, not calibrated")
    parser.add_argument("--output-dir", type=Path, default=Path("artifacts/faceocc-integration"))
    args = parser.parse_args()
    if not 0 < args.threshold <= 1:
        parser.error("threshold must be in (0,1]")
    if args.output_dir.exists():
        parser.error("output directory already exists; choose a new path")
    assert digest(args.onnx) == ONNX_SHA256, "research ONNX hash mismatch"
    historical = predictions(args.baseline) if args.baseline is not None else None
    if historical is not None:
        assert len(historical) == 100
    args.output_dir.mkdir(parents=True)
    out = args.output_dir.resolve()
    with tempfile.TemporaryDirectory(prefix="iris-faceocc-models-") as temporary:
        model_dir = Path(temporary)
        manifest = json.loads((args.models / "manifest.json").read_text("utf-8"))
        # Explicit artifact allowlist: never inherit another optional provider.
        expected = {row["file"]: row["sha256"] for row in manifest["models"]}
        expected[manifest["niqe"]["file"]] = manifest["niqe"]["sha256"]
        expected["onnxruntime.dll"] = manifest["runtime"]["sha256"]
        for filename, checksum in expected.items():
            assert Path(filename).name == filename
            assert digest(args.models / filename) == checksum
            shutil.copy2(args.models / filename, model_dir / filename)
        shutil.copy2(args.models / "manifest.json", model_dir / "manifest.json")
        optional = model_dir / "optional"
        optional.mkdir()
        shutil.copy2(args.onnx, optional / "faceocc.onnx")
        metadata = {
            "model_id": "faceocc_visible_face_v1",
            "source_url": f"https://huggingface.co/mertakin/FaceOcc/tree/{REVISION}",
            "license_url": f"https://huggingface.co/mertakin/FaceOcc/blob/{REVISION}/LICENSE",
            "license_note": "Local technical integration test only. MIT declarations do not supersede third-party training resource terms. Not distribution clearance.",
        }
        (optional / "faceocc.metadata.json").write_text(json.dumps(metadata, indent=2), "utf-8")
        settings = {"occlusion_provider": "faceocc", "occlusion_model_sha256": ONNX_SHA256,
                    "occlusion_min_visible_fraction": args.threshold}
        settings_file = out / "settings.json"
        settings_file.write_text(json.dumps(settings, indent=2), "utf-8")
        for variant in ("default", "faceocc"):
            command = [sys.executable, str(Path(__file__).with_name("verify_sample.py")),
                       "--fixtures", str(args.fixtures.resolve()), "--daemon", str(args.daemon.resolve()),
                       "--models", str(model_dir), "--output", str(out / f"{variant}.json"),
                       "--snapshot", str(out / f"{variant}.sqlite3")]
            if variant == "faceocc":
                command += ["--settings-file", str(settings_file)]
            with (out / f"{variant}.log").open("w", encoding="utf-8") as log:
                subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
            print(f"{variant} sample workflow passed", flush=True)
        default, masked = (predictions(out / f"{name}.sqlite3") for name in ("default", "faceocc"))
        if historical is not None:
            assert default == historical, "default deterministic predictions changed"
        assert default.keys() == masked.keys()
        transitions = {}
        measurements = 0
        newly_hidden = 0
        for path, before in default.items():
            after = masked[path]
            assert len(before["faces"]) == len(after["faces"])
            for b, a in zip(before["faces"], after["faces"]):
                for side in ("left_eye", "right_eye"):
                    old, new = b.get(side), a.get(side)
                    old_state = old["state"] if old else "hidden"
                    new_state = new["state"] if new else "hidden"
                    key = old_state + " -> " + new_state
                    transitions[key] = transitions.get(key, 0) + 1
                    if old is None:
                        assert new is None, "mask resurrected an eye suppressed by existing quality gates"
                    if new is not None:
                        assert new == old, "mask changed an eye measurement rather than gating it"
                    newly_hidden += old is not None and new is None
                    measurements += a.get(side + "_visibility") is not None
        assert measurements > 0, "optional path produced no eye visibility measurements"
        reports = {name: json.loads((out / f"{name}.json").read_text("utf-8")) for name in ("default", "faceocc")}
        report = {
            "passed": True, "default_predictions_unchanged": True if historical is not None else None, "photos": len(default),
            "historical_regression_checked": historical is not None,
            "comparison_normalization": "Exclude elapsed_ms; expand only absent occlusion settings to disabled defaults",
            "onnx_sha256": ONNX_SHA256, "daemon_sha256": digest(args.daemon),
            "metadata": metadata, "settings": settings, "visibility_measurements": measurements,
            "newly_hidden_eyes": newly_hidden, "eye_state_transitions": transitions,
            "analysis_seconds": {name: r["analysis"]["seconds"] for name, r in reports.items()},
            "cached_seconds": {name: r["incremental"]["seconds"] for name, r in reports.items()},
            "accuracy": "Not evaluated. No independent labels; crop geometry and threshold remain experimental.",
        }
        (out / "verification.json").write_text(json.dumps(report, indent=2), "utf-8")
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
