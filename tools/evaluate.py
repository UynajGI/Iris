#!/usr/bin/env python3
"""Prepare independent human labels and evaluate stored predictions, without inference.

Only Python's standard library is required. Preparation never promotes predictions
to ground truth. All template records require explicit human completion.
"""
from __future__ import annotations

import argparse
import json
import math
import sqlite3
import sys
from pathlib import Path
from typing import Any

SCHEMA_VERSION = 1
COORDINATES = "oriented_normalized_xywh"
QUALITY_METHOD = "observed_technical_quality_v1_lap75_fft25_face50_30_20"
PREFERENCE_TIE_EPSILON = 1e-9


class ValidationError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def number(value: Any, name: str) -> float:
    require(isinstance(value, (int, float)) and not isinstance(value, bool), f"{name} must be numeric")
    require(math.isfinite(value), f"{name} must be finite")
    return float(value)


def load_project(database: Path, project_id: int) -> tuple[list[dict], list[dict]]:
    """Read one coherent SQLite snapshot. Never create or mutate a database."""
    require(database.is_file(), f"Database does not exist: {database}")
    connection = sqlite3.connect(database.resolve().as_uri() + "?mode=ro", uri=True)
    connection.row_factory = sqlite3.Row
    try:
        connection.execute("BEGIN")
        require(connection.execute("SELECT id FROM projects WHERE id=?", (project_id,)).fetchone() is not None,
                f"Unknown project {project_id}")
        photos = []
        for row in connection.execute(
            "SELECT p.id,p.path,p.mtime,p.size_bytes,a.data FROM photos p "
            "LEFT JOIN analyses a ON a.photo_id=p.id "
            "WHERE p.project_id=? AND p.missing=0 AND p.quarantined=0 ORDER BY p.id", (project_id,)
        ):
            photos.append({"photo_id": row["id"], "path": row["path"], "mtime": row["mtime"],
                           "size_bytes": row["size_bytes"],
                           "analysis": json.loads(row["data"]) if row["data"] is not None else None})
        groups = [{"group_id": row["id"], "kind": row["kind"], "member_photo_ids": json.loads(row["members"])}
                  for row in connection.execute("SELECT id,kind,members FROM burst_groups WHERE project_id=? ORDER BY id", (project_id,))]
        return photos, groups
    finally:
        connection.close()


def normalized_predictions(photo: dict) -> list[dict]:
    analysis = photo.get("analysis")
    require(isinstance(analysis, dict), f"Photo {photo['photo_id']} has no stored analysis")
    width, height = number(analysis.get("width"), "analysis width"), number(analysis.get("height"), "analysis height")
    require(width > 0 and height > 0, "Analysis dimensions must be positive")
    faces = analysis.get("faces")
    require(isinstance(faces, list), "Stored analysis must contain a face list")
    result = []
    for face in faces:
        require(isinstance(face, dict), "Stored face must be an object")
        box = face.get("bbox")
        require(isinstance(box, list) and len(box) == 4, "Stored face needs [x,y,width,height] bbox")
        x, y, w, h = [number(v, "predicted bbox") for v in box]
        require(w > 0 and h > 0, "Predicted box dimensions must be positive")
        result.append({**face, "bbox": [x / width, y / height, w / width, h / height]})
    return result


def prepare(project_id: int, photos: list[dict], groups: list[dict], include_suggestions: bool = False) -> dict:
    require(bool(photos), "Project has no available photos")
    records = []
    for photo in photos:
        record = {key: photo[key] for key in ("photo_id", "path", "mtime", "size_bytes")}
        record.update(annotation_complete=False, ground_truth={"faces": []})
        if include_suggestions:
            record["suggested_face_boxes"] = [face["bbox"] for face in normalized_predictions(photo)] if photo.get("analysis") else []
            record["suggestions_are_ground_truth"] = False
        records.append(record)
    return {
        "schema_version": SCHEMA_VERSION, "project_id": project_id, "coordinate_space": COORDINATES,
        "instructions": {
            "independence": "Human labels must come from inspecting photographs. Suggestions are predictions, never ground truth.",
            "boxes": "Use [x,y,width,height] in 0..1 coordinates of the EXIF-orientation-corrected image.",
            "eyes": "Each truth face needs left_eye and right_eye: open, closed, or ungradable. Left/right refer to the person's anatomical sides.",
            "complete": "Set annotation_complete to true only after independent review. An empty face list then explicitly means no faces.",
            "groups": "Mark every photo you would accept as the group recommendation; an explicitly reviewed empty list means none are acceptable.",
            "quality_preferences": "Optional independent pair labels: dimension sharpness or face_quality; endpoints a/b reference photo_id and, for faces, zero-based human face_index. Complete explicitly with preference a, b, tie, or ungradable. No pairs or labels are inferred from predictions.",
        },
        "photos": records,
        "groups": [{**group, "group_key": group_key(group), "annotation_complete": False, "ground_truth": {"acceptable_photo_ids": []}} for group in groups],
        "quality_preferences": [],
    }


def group_key(group: dict) -> str:
    kind = group.get("kind")
    require(kind in ("burst", "duplicate"), "Group kind must be burst or duplicate")
    members = group.get("member_photo_ids")
    require(isinstance(members, list) and len(members) >= 2, "Group must have at least two photo members")
    require(all(isinstance(v, int) and not isinstance(v, bool) for v in members), "Group member ids must be integers")
    require(len(set(members)) == len(members), "Duplicate group member")
    return kind + ":" + ",".join(map(str, sorted(members)))


def design_dataset_gate(photos: list[dict], groups: list[dict], required: bool) -> dict:
    keys = {group_key(group) for group in groups}
    bursts = sum(key.startswith("burst:") for key in keys)
    duplicates = sum(key.startswith("duplicate:") for key in keys)
    return {"required": required, "satisfied": len(photos) >= 300 and bursts >= 60,
            "minimum_photos": 300, "minimum_groups": 60, "minimum_burst_groups": 60,
            "unique_burst_groups": bursts, "unique_duplicate_groups": duplicates,
            "counting": "kind=burst only, deduplicated by unordered member set; minimum_groups is the legacy alias for minimum_burst_groups",
            "limitations": "Counts do not prove real consecutive captures, independent bursts, or independent human annotation."}


def bounded_score(value: Any, name: str) -> float:
    value = number(value, name)
    require(0 <= value <= 100, f"{name} must be in 0..100")
    return value


def quality_scores(analysis: dict, dimension: str, face: dict | None) -> dict[str, float | None]:
    """Read v5 measurements only; no inference, synthetic truth or fitted weights."""
    ids = ("laplacian", "fft", "niqe") if dimension == "sharpness" else ("sharpness", "exposure", "resolution")
    missing = {"full": None, **{f"without_{name}": None for name in ids}}
    breakdown = analysis.get("score_breakdown")
    if not str(analysis.get("version", "")).startswith(("iris-vision-v5-", "iris-vision-v6-")) or not isinstance(breakdown, dict) or breakdown.get("method") != QUALITY_METHOD:
        return missing
    if dimension == "sharpness":
        components = breakdown.get("components")
        require(isinstance(components, list), "Missing score breakdown components")
        selected = [c for c in components if isinstance(c, dict) and c.get("id") == "sharpness"]
        require(len(selected) == 1, "Expected one sharpness score component")
        component = selected[0]
        if component.get("score") is None:
            return missing
        full = bounded_score(component["score"], "sharpness score")
        terms = component.get("terms")
        require(isinstance(terms, list), "Missing sharpness terms")
        require(all(isinstance(t, dict) for t in terms), "Invalid sharpness term")
        require(len(terms) == 3 and {t.get("id") for t in terms} == set(ids), "Unsupported sharpness terms")
        values = {}
        for term in terms:
            weight = number(term.get("weight"), "sharpness term weight")
            require(0 <= weight <= 1, "Sharpness term weight must be in 0..1")
            score = term.get("score")
            require(score is not None or weight == 0, "Missing sharpness term must have zero weight")
            values[term["id"]] = (None if score is None else bounded_score(score, "sharpness term score"), weight)
    else:
        quality = face.get("quality") if face is not None else None
        if not isinstance(quality, dict):
            return missing
        if quality.get("method") != "preview_bbox_clipped_no_upscale_v1_lap50_exposure30_min_side128_20":
            return missing
        full = bounded_score(quality.get("score"), "face quality score")
        values = {name: (bounded_score(quality.get(name + "_score"), "local " + name), weight)
                  for name, weight in zip(ids, (0.5, 0.3, 0.2))}
    require(abs(sum(w for _, w in values.values()) - 1.) <= 1e-9, "Quality term weights must sum to one")
    require(abs(sum((score or 0.) * w for score, w in values.values()) - full) <= 1e-7,
            "Quality score disagrees with stored weighted terms")
    result = {"full": full}
    for omitted in ids:
        retained = [(score, w) for name, (score, w) in values.items() if name != omitted and score is not None and w > 0]
        total = sum(w for _, w in retained)
        result[f"without_{omitted}"] = sum(score * w for score, w in retained) / total if total else None
    return result


def evaluate_quality_preferences(records: Any, photos: dict[int, dict], truths: dict[int, list[dict]],
                                 matched_faces: dict[int, dict[int, dict]]) -> dict:
    require(isinstance(records, list), "quality_preferences must be a list")
    dimensions, details, seen_ids, seen_pairs = {}, [], set(), set()
    for record in records:
        require(isinstance(record, dict), "Quality preference must be an object")
        identifier = record.get("id")
        require(isinstance(identifier, str) and bool(identifier) and identifier not in seen_ids, "Quality preference requires unique nonempty id")
        seen_ids.add(identifier)
        require(record.get("annotation_complete") is True, f"Quality preference {identifier} is incomplete")
        dimension = record.get("dimension")
        require(dimension in ("sharpness", "face_quality"), "Quality dimension must be sharpness or face_quality")
        truth = record.get("ground_truth")
        require(isinstance(truth, dict) and truth.get("preference") in ("a", "b", "tie", "ungradable"), "Quality preference requires independent a/b/tie/ungradable truth")
        preference = truth["preference"]
        endpoints, scores = [], []
        for side in ("a", "b"):
            endpoint = record.get(side)
            require(isinstance(endpoint, dict), "Quality endpoint must be an object")
            photo_id = endpoint.get("photo_id")
            require(isinstance(photo_id, int) and not isinstance(photo_id, bool) and photo_id in photos, "Quality endpoint references unknown photo")
            face_index = endpoint.get("face_index")
            if dimension == "face_quality":
                require(isinstance(face_index, int) and not isinstance(face_index, bool) and 0 <= face_index < len(truths[photo_id]), "Quality face_index must reference a human truth face")
                face = matched_faces[photo_id].get(face_index)
            else:
                require("face_index" not in endpoint, "Sharpness preference compares whole photos, not faces")
                face = None
            endpoints.append((photo_id, face_index if dimension == "face_quality" else -1))
            scores.append(quality_scores(photos[photo_id]["analysis"], dimension, face))
        require(endpoints[0] != endpoints[1], "Quality preference cannot compare an endpoint to itself")
        pair = (dimension, tuple(sorted(endpoints)))
        require(pair not in seen_pairs, "Duplicate quality preference pair, including reversed endpoints")
        seen_pairs.add(pair)
        bucket = dimensions.setdefault(dimension, {"total": 0, "gradable": 0, "ungradable": 0, "variants": {}})
        bucket["total"] += 1
        bucket["ungradable" if preference == "ungradable" else "gradable"] += 1
        predictions, correct = {}, {}
        for variant in scores[0]:
            a, b = scores[0][variant], scores[1][variant]
            prediction = None if a is None or b is None else "tie" if abs(a - b) <= PREFERENCE_TIE_EPSILON else "a" if a > b else "b"
            correct[variant] = prediction == preference
            predictions[variant] = {"a_score": a, "b_score": b, "preference": prediction,
                                    "missing_reason": "Missing matched face, supported v5 measurement, or retained ablation term" if prediction is None else None}
            stats = bucket["variants"].setdefault(variant, {"answered": 0, "correct": 0, "abstained": 0,
                                                          "paired_with_full": 0, "full_correct_on_paired": 0, "variant_correct_on_paired": 0})
            if preference != "ungradable":
                stats["answered" if prediction is not None else "abstained"] += 1
                stats["correct"] += int(correct[variant])
        if preference != "ungradable":
            for variant, stats in bucket["variants"].items():
                if predictions["full"]["preference"] is not None and predictions[variant]["preference"] is not None:
                    stats["paired_with_full"] += 1
                    stats["full_correct_on_paired"] += int(correct["full"])
                    stats["variant_correct_on_paired"] += int(correct[variant])
        details.append({"id": identifier, "dimension": dimension, "human_preference": preference, "variants": predictions})
    for bucket in dimensions.values():
        for stats in bucket["variants"].values():
            stats.update(coverage=ratio(stats["answered"], bucket["gradable"]),
                         agreement_when_answered=ratio(stats["correct"], stats["answered"]),
                         end_to_end_agreement=ratio(stats["correct"], bucket["gradable"]),
                         agreement_delta_vs_full_on_paired=ratio(stats["variant_correct_on_paired"] - stats["full_correct_on_paired"], stats["paired_with_full"]))
    return {"status": "evaluated_against_explicit_human_preferences" if records else "not_requested",
            "tie_epsilon": PREFERENCE_TIE_EPSILON, "dimensions": dimensions, "pairs": details,
            "ablation": "Leave one term out and renormalize the remaining stored internal weights; no refit, inference, verdict or grouping recomputation.",
            "limitations": "Pair selection and label independence require human review. Agreement is sample-specific, not calibration or a causal quality claim. Ungradable pairs are excluded; missing observations abstain."}


def iou(a: list[float], b: list[float]) -> float:
    width = max(0.0, min(a[0] + a[2], b[0] + b[2]) - max(a[0], b[0]))
    height = max(0.0, min(a[1] + a[3], b[1] + b[3]) - max(a[1], b[1]))
    intersection = width * height
    union = a[2] * a[3] + b[2] * b[3] - intersection
    return intersection / union if union > 0 else 0.0


def match_faces(predictions: list[dict], truth: list[dict], threshold: float) -> list[tuple[int, int, float]]:
    """Maximum-cardinality IoU-qualified matching, then maximum summed IoU.

    A padded Hungarian assignment avoids the false negatives caused by greedy
    matching when two predictions overlap one truth and only one matches another.
    """
    size = max(len(predictions), len(truth))
    if not size:
        return []
    overlap = [[iou(p["bbox"], t["bbox"]) for t in truth] for p in predictions]
    weights = [[0.0] * size for _ in range(size)]
    for p, row in enumerate(overlap):
        for t, value in enumerate(row):
            if value >= threshold:
                weights[p][t] = size + 1 + value
    u, v, assigned, previous = [0.0] * (size + 1), [0.0] * (size + 1), [0] * (size + 1), [0] * (size + 1)
    for row in range(1, size + 1):
        assigned[0] = row
        minimum, used, column = [math.inf] * (size + 1), [False] * (size + 1), 0
        while True:
            used[column] = True
            current_row, delta, next_column = assigned[column], math.inf, 0
            for candidate in range(1, size + 1):
                if not used[candidate]:
                    reduced = -weights[current_row - 1][candidate - 1] - u[current_row] - v[candidate]
                    if reduced < minimum[candidate]:
                        minimum[candidate], previous[candidate] = reduced, column
                    if minimum[candidate] < delta:
                        delta, next_column = minimum[candidate], candidate
            for candidate in range(size + 1):
                if used[candidate]:
                    u[assigned[candidate]] += delta
                    v[candidate] -= delta
                else:
                    minimum[candidate] -= delta
            column = next_column
            if assigned[column] == 0:
                break
        while column:
            prior_column = previous[column]
            assigned[column] = assigned[prior_column]
            column = prior_column
    return [(assigned[column] - 1, column - 1, overlap[assigned[column] - 1][column - 1])
            for column in range(1, size + 1)
            if 0 < assigned[column] <= len(predictions) and column <= len(truth)
            and weights[assigned[column] - 1][column - 1] > 0]


def ratio(numerator: int, denominator: int) -> float | None:
    return numerator / denominator if denominator else None


def percentile(values: list[float], quantile: float) -> float | None:
    if not values:
        return None
    values = sorted(values)
    position = (len(values) - 1) * quantile
    lower, upper = math.floor(position), math.ceil(position)
    return values[lower] + (values[upper] - values[lower]) * (position - lower)


def keyed(records: Any, key: str, name: str) -> dict:
    require(isinstance(records, list), f"{name} must be a list")
    result = {}
    for record in records:
        require(isinstance(record, dict) and key in record, f"Every {name} entry needs {key}")
        identifier = record[key]
        require(isinstance(identifier, (str, int)) and not isinstance(identifier, bool), f"Invalid {key}")
        require(identifier not in result, f"Duplicate {key}: {identifier}")
        result[identifier] = record
    return result


def truth_faces(record: dict) -> list[dict]:
    require(record.get("annotation_complete") is True, f"Photo {record['photo_id']} annotation is incomplete")
    ground_truth = record.get("ground_truth")
    require(isinstance(ground_truth, dict) and isinstance(ground_truth.get("faces"), list), "Missing ground_truth.faces")
    for face in ground_truth["faces"]:
        require(isinstance(face, dict), "Ground-truth face must be an object")
        box = face.get("bbox")
        require(isinstance(box, list) and len(box) == 4, "Truth face needs [x,y,width,height] bbox")
        x, y, w, h = [number(value, "truth bbox") for value in box]
        require(x >= 0 and y >= 0 and w > 0 and h > 0 and x + w <= 1 + 1e-9 and y + h <= 1 + 1e-9,
                "Truth boxes must be within normalized 0..1 image coordinates")
        for eye in ("left_eye", "right_eye"):
            require(face.get(eye) in ("open", "closed", "ungradable"), f"Human {eye} label must be open, closed, or ungradable")
            reason = face.get(eye + "_ungradable_reason")
            if reason is not None:
                require(face[eye] == "ungradable" and reason in ("occlusion", "small_face", "pose", "lighting", "blur", "other"),
                        "Ungradable reason requires an ungradable eye and a supported reason")
    return ground_truth["faces"]


def evaluate(project_id: int, photos: list[dict], groups: list[dict], labels: dict,
             threshold: float = 0.5, require_design_gate: bool = False) -> dict:
    threshold = number(threshold, "IoU threshold")
    require(0 < threshold <= 1, "IoU threshold must be in (0,1]")
    require(isinstance(labels, dict) and labels.get("schema_version") == SCHEMA_VERSION, "Unsupported annotation schema")
    require(labels.get("project_id") == project_id, "Annotation project does not match")
    require(labels.get("coordinate_space") == COORDINATES, "Annotation coordinate space does not match")
    require(bool(photos), "Project has no available photos")
    dataset_gate = design_dataset_gate(photos, groups, require_design_gate)
    if require_design_gate:
        require(dataset_gate["satisfied"], "Design gate requires at least 300 photos and 60 unique burst groups")
    photo_labels, group_labels = keyed(labels.get("photos"), "photo_id", "photos"), keyed(labels.get("groups"), "group_key", "groups")
    require(set(photo_labels) == {p["photo_id"] for p in photos}, "Missing or extra photo annotations; dataset changed")
    current_group_keys = [group_key(group) for group in groups]
    require(len(set(current_group_keys)) == len(current_group_keys), "Duplicate stored group membership")
    require(set(group_labels) == set(current_group_keys), "Missing or extra group annotations; group membership changed")
    tp = fp = fn = 0
    eyes = {key: 0 for key in ("gradable", "ungradable", "answered", "correct", "incorrect", "abstained", "missing_face_eyes", "uncertain_predictions", "unreliable_or_missing_predictions")}
    ungradable = {}
    confusion = {truth: {pred: 0 for pred in ("open", "closed", "abstain")} for truth in ("open", "closed")}
    timings, details = [], []
    versions = set()
    quality_truths, quality_matches = {}, {}
    for photo in photos:
        record = photo_labels[photo["photo_id"]]
        for key in ("path", "mtime", "size_bytes"):
            require(record.get(key) == photo[key], f"Photo {photo['photo_id']} fingerprint differs; review stale annotation")
        truth = truth_faces(record)
        predictions = normalized_predictions(photo)
        matches = match_faces(predictions, truth, threshold)
        mapping = {truth_id: prediction_id for prediction_id, truth_id, _ in matches}
        quality_truths[photo["photo_id"]] = truth
        quality_matches[photo["photo_id"]] = {truth_id: predictions[prediction_id] for truth_id, prediction_id in mapping.items()}
        tp += len(matches)
        fp += len(predictions) - len(matches)
        fn += len(truth) - len(matches)
        details.append({"photo_id": photo["photo_id"], "tp": len(matches), "fp": len(predictions) - len(matches), "fn": len(truth) - len(matches)})
        for truth_id, face in enumerate(truth):
            prediction = predictions[mapping[truth_id]] if truth_id in mapping else None
            for eye in ("left_eye", "right_eye"):
                human = face[eye]
                predicted_eye = prediction.get(eye) if prediction is not None else None
                state = predicted_eye.get("state") if isinstance(predicted_eye, dict) else None
                require(state in (None, "open", "closed", "uncertain"), "Invalid stored eye prediction")
                if human == "ungradable":
                    eyes["ungradable"] += 1
                    reason = face.get(eye + "_ungradable_reason") or "unspecified"
                    bucket = ungradable.setdefault(reason, {key: 0 for key in (
                        "total", "matched_face_eyes", "missing_face_eyes", "determinate_predictions",
                        "uncertain_predictions", "hidden_predictions")})
                    bucket["total"] += 1
                    if prediction is None:
                        bucket["missing_face_eyes"] += 1
                    else:
                        bucket["matched_face_eyes"] += 1
                        bucket["determinate_predictions" if state in ("open", "closed") else
                               "uncertain_predictions" if state == "uncertain" else "hidden_predictions"] += 1
                    continue
                eyes["gradable"] += 1
                if state in ("open", "closed"):
                    eyes["answered"] += 1
                    eyes["correct" if state == human else "incorrect"] += 1
                    confusion[human][state] += 1
                else:
                    eyes["abstained"] += 1
                    confusion[human]["abstain"] += 1
                    if prediction is None:
                        eyes["missing_face_eyes"] += 1
                    elif state == "uncertain":
                        eyes["uncertain_predictions"] += 1
                    else:
                        eyes["unreliable_or_missing_predictions"] += 1
        analysis = photo["analysis"]
        if analysis.get("elapsed_ms") is not None:
            elapsed = number(analysis["elapsed_ms"], "elapsed_ms")
            require(elapsed >= 0, "elapsed_ms must not be negative")
            timings.append(elapsed)
        versions.add(str(analysis.get("version", "unknown")))
    ungradable_totals = {key: sum(bucket[key] for bucket in ungradable.values()) for key in (
        "total", "matched_face_eyes", "missing_face_eyes", "determinate_predictions", "uncertain_predictions", "hidden_predictions")}
    for bucket in [ungradable_totals, *ungradable.values()]:
        bucket["determinate_rate_when_matched"] = ratio(bucket["determinate_predictions"], bucket["matched_face_eyes"])
        bucket["state_display_rate_when_matched"] = ratio(bucket["determinate_predictions"] + bucket["uncertain_predictions"], bucket["matched_face_eyes"])
    group_successes = 0
    group_details = []
    for group in groups:
        record = group_labels[group_key(group)]
        require(record.get("annotation_complete") is True, f"Group {group['group_id']} annotation is incomplete")
        members = group["member_photo_ids"]
        require(isinstance(members, list) and bool(members) and len(set(members)) == len(members), "Invalid stored group members")
        require(set(members) <= set(photo_labels), "Stored group contains unavailable photos")
        require(group_key(record) == group_key(group), "Group membership changed; review group labels")
        ground_truth = record.get("ground_truth")
        require(isinstance(ground_truth, dict), "Missing group ground_truth")
        accepted = ground_truth.get("acceptable_photo_ids")
        require(isinstance(accepted, list), "Missing human acceptable_photo_ids")
        require(all(isinstance(v, int) and not isinstance(v, bool) for v in accepted), "Human acceptable ids must be integers")
        require(len(set(accepted)) == len(accepted) and set(accepted) <= set(members), "Human acceptable ids must be unique group members")
        success = members[0] in accepted
        group_successes += success
        group_details.append({"group_id": group["group_id"], "top_photo_id": members[0], "success": success})
    return {
        "status": "evaluated_against_explicit_human_labels", "project_id": project_id,
        "photos": len(photos), "model_versions": sorted(versions),
        "design_dataset_gate": dataset_gate,
        "face_detection": {"tp": tp, "fp": fp, "fn": fn, "precision": ratio(tp, tp + fp), "recall": ratio(tp, tp + fn),
                           "iou_threshold": threshold, "matching": "maximum cardinality, then maximum total IoU"},
        "eyes": {**eyes, "coverage": ratio(eyes["answered"], eyes["gradable"]),
                 "accuracy_when_answered": ratio(eyes["correct"], eyes["answered"]),
                 "end_to_end_correct_rate": ratio(eyes["correct"], eyes["gradable"]), "confusion": confusion},
        "ungradable_eyes": {**ungradable_totals, "by_reason": ungradable,
                            "meaning": "State display on human-ungradable eyes, not eye-state accuracy; missing faces are reported separately."},
        "groups": {"total": len(groups), "successful": group_successes, "top_recommendation_success_rate": ratio(group_successes, len(groups))},
        "timing": {"photos_with_timing": len(timings), "p50_ms": percentile(timings, 0.5), "p95_ms": percentile(timings, 0.95),
                   "method": "linear interpolation of stored per-photo elapsed_ms; not end-to-end throughput"},
        "per_photo": details, "per_group": group_details,
        "quality_preferences": evaluate_quality_preferences(labels.get("quality_preferences", []),
                                                             {p["photo_id"]: p for p in photos}, quality_truths, quality_matches),
        "limitations": "Annotation independence and correctness require human review; this tool cannot certify either or infer missing labels.",
    }


def write_json(path: Path, value: dict) -> None:
    with path.open("x", encoding="utf-8") as file:
        json.dump(value, file, ensure_ascii=False, indent=2, allow_nan=False)
        file.write("\n")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("prepare", "evaluate"):
        command = commands.add_parser(name)
        command.add_argument("--database", type=Path, required=True)
        command.add_argument("--project", type=int, required=True)
        command.add_argument("--output", type=Path, required=name == "prepare")
        if name == "prepare":
            command.add_argument("--include-suggestions", action="store_true")
        else:
            command.add_argument("--annotations", type=Path, required=True)
            command.add_argument("--iou-threshold", type=float, default=0.5)
            command.add_argument("--require-design-gate", action="store_true",
                                 help="Require 300 photos and 60 unique kind=burst member sets; not proof of real bursts or independent labeling")
    args = parser.parse_args(argv)
    try:
        photos, groups = load_project(args.database, args.project)
        if args.command == "prepare":
            report = prepare(args.project, photos, groups, args.include_suggestions)
        else:
            report = evaluate(args.project, photos, groups, json.loads(args.annotations.read_text(encoding="utf-8")),
                              args.iou_threshold, args.require_design_gate)
        if args.output:
            write_json(args.output, report)
            print(str(args.output))
        else:
            print(json.dumps(report, ensure_ascii=False, indent=2, allow_nan=False))
        return 0
    except (ValidationError, OSError, sqlite3.Error, json.JSONDecodeError) as error:
        print(f"Evaluation refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
