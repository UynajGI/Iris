"""Exercise the real daemon, models and domain operations using private JPEG fixtures."""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import json
import math
import shutil
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_score_breakdown(analysis: dict) -> str | None:
    breakdown = analysis.get("score_breakdown")
    if breakdown is None:
        assert not analysis["version"].startswith(("iris-vision-v5-", "iris-vision-v6-")), "v5+ must explain its score"
        return None
    components = breakdown["components"]
    assert len(components) == 5 and {c["id"] for c in components} == {"eyes", "sharpness", "face", "exposure", "smile"}
    total = sum(c["configured_weight"] for c in components if c["score"] is not None)
    assert math.isclose(total, breakdown["effective_weight_total"], abs_tol=1e-7)
    for component in components:
        score = component["score"]
        assert 0 <= component["observed_count"] <= component["total_count"]
        weight = component["configured_weight"] / total if score is not None and total else 0
        assert math.isclose(weight, component["effective_weight"], abs_tol=1e-7)
        assert math.isclose((score or 0) * weight, component["contribution"], abs_tol=1e-7)
        if score is None:
            assert component["missing_reason"]
        else:
            assert math.isfinite(score) and 0 <= score <= 100 + 1e-7
            assert math.isclose(sum(t["contribution"] for t in component["terms"]), score, abs_tol=1e-7)
        for term in component["terms"]:
            assert all(math.isfinite(raw["value"]) for raw in term["raw"])
            assert 0 <= term["weight"] <= 1 + 1e-7
            if term["score"] is None:
                assert term["missing_reason"] and term["weight"] == 0
            else:
                assert math.isfinite(term["score"]) and 0 <= term["score"] <= 100
            assert math.isclose((term["score"] or 0) * term["weight"], term["contribution"], abs_tol=1e-7)
    assert math.isclose(sum(c["contribution"] for c in components), analysis["composite_score"], abs_tol=1e-7)
    return breakdown["method"]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, default=Path("test-photos"))
    parser.add_argument("--daemon", type=Path, default=Path("target/debug/iris-daemon.exe"))
    parser.add_argument("--models", type=Path, default=Path("models"))
    parser.add_argument("--output", type=Path, default=Path("artifacts/sample-verification.json"))
    parser.add_argument("--snapshot", type=Path, help="Preserve a new SQLite backup for independent annotation/evaluation")
    parser.add_argument("--settings-file", type=Path, help="Explicit analysis settings overrides (JSON); server validates the merged settings")
    parser.add_argument("--worker-limit", type=int, choices=range(1, 17), help="Daemon process concurrency policy; omitted preserves its default")
    args = parser.parse_args()
    manifest = json.loads((args.fixtures / "manifest.json").read_text(encoding="utf-8"))
    assert manifest["count"] == 100
    for entry in manifest["files"]:
        assert digest(args.fixtures / entry["path"]) == entry["sha256"]
    with tempfile.TemporaryDirectory(prefix="iris-verify-") as temporary:
        work = Path(temporary)
        command = [str(args.daemon.resolve()), "--data-dir", str(work / "data"), "--model-dir", str(args.models.resolve())]
        if args.worker_limit is not None:
            command += ["--worker-limit", str(args.worker_limit)]
        process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            handshake = json.loads(process.stdout.readline())
            base, token = handshake["base_url"], handshake["token"]

            def request(method: str, path: str, body=None, authenticated=True):
                headers = {"Content-Type": "application/json"}
                if authenticated:
                    headers["Authorization"] = "Bearer " + token
                data = json.dumps(body).encode() if body is not None else None
                req = urllib.request.Request(base + "/api/v1" + path, data=data, headers=headers, method=method)
                with urllib.request.urlopen(req, timeout=180) as response:
                    raw = response.read()
                    return json.loads(raw) if "json" in response.headers.get("content-type", "") else raw

            def job(project: int, kind: str):
                started = time.monotonic()
                request("POST", f"/projects/{project}/{kind}")
                while time.monotonic() - started < 1800:
                    progress = request("GET", f"/projects/{project}/progress")
                    if progress["state"] in {"completed", "cancelled", "failed"}:
                        assert progress["state"] == "completed", progress
                        assert not progress["errors"], progress
                        return {"seconds": round(time.monotonic() - started, 3), **progress}
                    time.sleep(0.2)
                raise TimeoutError("analysis did not finish")

            try:
                request("GET", "/bootstrap", authenticated=False)
                raise AssertionError("anonymous bootstrap accepted")
            except urllib.error.HTTPError as error:
                assert error.code == 401
            project = request("POST", "/projects", {"root": str(args.fixtures.resolve())})["id"]
            if args.settings_file is not None:
                overrides = json.loads(args.settings_file.read_text(encoding="utf-8"))
                assert isinstance(overrides, dict), "settings file must contain an object"
                effective_settings = request("GET", f"/settings?project_id={project}")
                effective_settings.update(overrides)
                request("PUT", f"/settings?project_id={project}", effective_settings)
            effective_settings = request("GET", f"/settings?project_id={project}")
            model_status = request("GET", f"/models?project_id={project}")
            scan = job(project, "scan")
            photos = request("GET", f"/projects/{project}/photos")
            assert len(photos) == 100
            assert all(p["analysis_status"] == "missing" for p in photos)
            analysis = job(project, "analyze")
            repeat = job(project, "analyze")
            assert repeat["result"]["reused"] == 100, repeat
            project_state = request("GET", f"/projects/{project}")
            assert project_state["groups_dirty"] is False
            assert project_state["pending_analysis"] == 0
            photos = request("GET", f"/projects/{project}/photos?sort=score")
            assert all(p["analysis"] for p in photos)
            assert all(p["analysis_status"] == "current" for p in photos)
            assert all(max(p["analysis"]["width"], p["analysis"]["height"]) <= 1280 for p in photos)
            if args.snapshot is not None:
                args.snapshot.parent.mkdir(parents=True, exist_ok=True)
                # Reserve a new output; never replace a previous evaluation snapshot.
                with args.snapshot.open("xb"):
                    pass
                source_uri = (work / "data" / "library.sqlite3").resolve().as_uri() + "?mode=ro"
                with closing(sqlite3.connect(source_uri, uri=True)) as source, closing(sqlite3.connect(args.snapshot)) as destination:
                    source.backup(destination)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            intermediate = {"fixture_count": 100, "scan": scan, "analysis": analysis, "incremental": repeat, "domain_workflow": "pending"}
            args.output.write_text(json.dumps(intermediate, ensure_ascii=False, indent=2), encoding="utf-8")
            thumbnail = request("GET", f"/photos/{photos[0]['id']}/thumb")
            preview = request("GET", f"/photos/{photos[0]['id']}/preview")
            assert thumbnail[:2] == preview[:2] == b"\xff\xd8"
            original = request("GET", f"/photos/{photos[0]['id']}/original")
            expected = next(e["sha256"] for e in manifest["files"] if e["path"] == photos[0]["path"])
            assert hashlib.sha256(original).hexdigest() == expected
            settings = request("GET", f"/settings?project_id={project}")
            request("POST", "/profiles", {"name": "baseline", "settings": settings})
            estimate = request("POST", f"/projects/{project}/profiles/baseline/estimate")

            # Potentially mutating domain operations run on disposable copies only.
            small = work / "disposable-photos"
            small.mkdir()
            for entry in manifest["files"][:2]:
                shutil.copy2(args.fixtures / entry["path"], small / Path(entry["path"]).name)
            p2 = request("POST", "/projects", {"root": str(small)})["id"]
            job(p2, "scan")
            ids = [p["id"] for p in request("GET", f"/projects/{p2}/photos")]
            request("POST", f"/projects/{p2}/decisions", {"photo_ids": [ids[0]], "action": "keep"})
            request("POST", f"/projects/{p2}/decisions", {"photo_ids": [ids[1]], "action": "reject"})
            copied = request("POST", f"/projects/{p2}/export/copy", {"destination": str(work / "selected"), "scope": "keep"})
            assert copied["written"] == 1
            xmp = request("POST", f"/projects/{p2}/export/xmp", {"scope": "all", "overwrite": False})
            assert xmp["written"] == 2
            csv = work / "decisions.csv"
            request("POST", f"/projects/{p2}/export/csv", {"destination": str(csv)})
            request("POST", f"/projects/{p2}/undo")
            request("POST", f"/projects/{p2}/import/csv", {"source": str(csv)})
            plan = request("POST", f"/projects/{p2}/quarantine/preview")
            assert len(plan["items"]) >= 1
            request("POST", f"/projects/{p2}/quarantine/commit", {"manifest_id": plan["id"]})
            request("POST", f"/projects/{p2}/quarantine/restore", {"manifest_id": plan["id"]})
            assert all(not p["quarantined"] for p in request("GET", f"/projects/{p2}/photos"))
            request("GET", f"/projects/{project}/cache")
            request("POST", f"/projects/{project}/cache/migrate", {"destination": str(work / "migrated-cache")})
            request("POST", f"/projects/{project}/cache/cleanup")
            for entry in manifest["files"]:
                assert digest(args.fixtures / entry["path"]) == entry["sha256"]
            states = {}
            face_count = 0
            eye_states = {}
            analysis_versions = {}
            unreliable_reasons = {}
            eye_quality_reasons = {}
            composition_scores = []
            latencies = []
            niqe_scores = []
            visibility_fractions = []
            visibility_probabilities = []
            visibility_methods = {}
            measured_hidden_eyes = 0
            score_methods = {}
            face_quality_scores = []
            for photo in photos:
                a = photo["analysis"]
                score_method = verify_score_breakdown(a)
                if score_method:
                    score_methods[score_method] = score_methods.get(score_method, 0) + 1
                version = a["version"]
                analysis_versions[version] = analysis_versions.get(version, 0) + 1
                composition = a.get("composition")
                if composition is not None:
                    score = composition["score"]
                    assert math.isfinite(score) and 0 <= score <= 100
                    composition_scores.append(score)
                states[a["verdict"]] = states.get(a["verdict"], 0) + 1
                face_count += len(a["faces"])
                if isinstance(a.get("elapsed_ms"), (int, float)):
                    latencies.append(a["elapsed_ms"])
                if isinstance(a.get("niqe"), (int, float)):
                    assert math.isfinite(a["niqe"])
                    niqe_scores.append(a["niqe"])
                for face in a["faces"]:
                    quality = face.get("quality")
                    if quality is not None:
                        score = quality["score"]
                        assert math.isfinite(score) and 0 <= score <= 100
                        assert quality["crop_width"] >= 3 and quality["crop_height"] >= 3
                        assert math.isclose(score, quality["sharpness_score"] * .5 + quality["exposure_score"] * .3 + quality["resolution_score"] * .2, abs_tol=1e-7)
                        face_quality_scores.append(score)
                    if face.get("unreliable_reason"):
                        reason = face["unreliable_reason"]
                        unreliable_reasons[reason] = unreliable_reasons.get(reason, 0) + 1
                    for side in ("left_eye", "right_eye"):
                        visibility = face.get(side + "_visibility")
                        if visibility is not None:
                            fraction = visibility["visible_fraction"]
                            probability = visibility["mean_probability"]
                            assert math.isfinite(fraction) and 0 <= fraction <= 1, visibility
                            assert math.isfinite(probability) and 0 <= probability <= 1, visibility
                            assert isinstance(visibility["sampled_pixels"], int) and visibility["sampled_pixels"] > 0, visibility
                            visibility_fractions.append(fraction)
                            visibility_probabilities.append(probability)
                            method = visibility["method"]
                            visibility_methods[method] = visibility_methods.get(method, 0) + 1
                            measured_hidden_eyes += face.get(side) is None
                        if face.get(side + "_unreliable_reason"):
                            reason = face[side + "_unreliable_reason"]
                            eye_quality_reasons[reason] = eye_quality_reasons.get(reason, 0) + 1
                        state = (face.get(side) or {}).get("state", "hidden_unreliable")
                        eye_states[state] = eye_states.get(state, 0) + 1
            latencies.sort()
            timing = {"p50_ms": latencies[len(latencies) // 2], "p95_ms": latencies[min(len(latencies) - 1, int(len(latencies) * .95))]} if latencies else None
            report = {"fixture_count": 100, "hashes_verified_before_and_after": True, "scan": scan, "analysis": analysis, "incremental": repeat, "analysis_versions": analysis_versions, "verdict_counts": states, "faces": face_count, "eye_states": eye_states, "unreliable_reasons": unreliable_reasons, "photo_latency": timing, "niqe": {"count": len(niqe_scores), "min": min(niqe_scores, default=None), "max": max(niqe_scores, default=None)}, "profile_estimate": estimate, "domain_workflow": "passed", "accuracy": "not measured: no human ground-truth annotations"}
            report["composition"] = {"count": len(composition_scores), "min": min(composition_scores, default=None), "max": max(composition_scores, default=None)}
            report["eye_quality_reasons"] = eye_quality_reasons
            report["effective_settings"] = effective_settings
            report["score_breakdowns_verified"] = score_methods
            report["face_quality"] = {"count": len(face_quality_scores), "min": min(face_quality_scores, default=None), "max": max(face_quality_scores, default=None)}
            report["model_status"] = model_status
            report["eye_visibility"] = {
                "measurements": len(visibility_fractions),
                "visible_fraction_min": min(visibility_fractions, default=None),
                "visible_fraction_max": max(visibility_fractions, default=None),
                "mean_probability_min": min(visibility_probabilities, default=None),
                "mean_probability_max": max(visibility_probabilities, default=None),
                "measured_hidden_eyes": measured_hidden_eyes,
                "methods": visibility_methods,
                "interpretation": "Hidden includes prior quality gates; suppression caused by masking requires paired baseline comparison. Threshold is experimental, not calibrated.",
            }
            report["project_ready"] = {"groups_dirty": project_state["groups_dirty"], "pending_analysis": project_state["pending_analysis"]}
            # Compare complete deterministic predictions across implementations;
            # timing is observational and is deliberately excluded from this digest.
            predictions = {p["path"]: {k: v for k, v in p["analysis"].items() if k != "elapsed_ms"} for p in photos}
            report["predictions_sha256"] = hashlib.sha256(json.dumps(predictions, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")).hexdigest()
            args.output.parent.mkdir(parents=True, exist_ok=True)
            report["requested_worker_limit"] = args.worker_limit
            args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
            print(json.dumps(report, ensure_ascii=False))
        finally:
            if process.poll() is None:
                process.stdin.write("shutdown\n")
                process.stdin.flush()
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


if __name__ == "__main__":
    main()
