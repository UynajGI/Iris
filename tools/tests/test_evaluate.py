import copy
import importlib.util
import io
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout

SPEC = importlib.util.spec_from_file_location("iris_evaluate", Path(__file__).resolve().parents[1] / "evaluate.py")
evaluate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(evaluate)


def predicted(box, left="open", right="open"):
    def eye(state):
        return None if state is None else {"state": state}
    return {"bbox": [v * 100 for v in box], "left_eye": eye(left), "right_eye": eye(right)}


def human(box, left="open", right="open"):
    return {"bbox": box, "left_eye": left, "right_eye": right}


def photo(identifier=1, faces=None, elapsed=None):
    analysis = {"width": 100, "height": 100, "faces": faces or [], "version": "unit-test-fixture"}
    if elapsed is not None:
        analysis["elapsed_ms"] = elapsed
    return {"photo_id": identifier, "path": f"{identifier}.jpg", "mtime": 123, "size_bytes": 456, "analysis": analysis}


def complete(photos, truths=None, groups=None, acceptable=None):
    groups = groups or []
    labels = evaluate.prepare(1, photos, groups)
    for record in labels["photos"]:
        record["annotation_complete"] = True
        record["ground_truth"]["faces"] = (truths or {}).get(record["photo_id"], [])
    for record in labels["groups"]:
        record["annotation_complete"] = True
        record["ground_truth"]["acceptable_photo_ids"] = (acceptable or {}).get(record["group_id"], [])
    return labels


def v5_photo(identifier, scores=(90, 0, 50), weights=(0.6, 0.2, 0.2), faces=None):
    p = photo(identifier, faces=faces)
    p["analysis"].update(version="iris-vision-v5-observed-quality-2026-10-07", score_breakdown={
        "method": evaluate.QUALITY_METHOD, "components": [{"id": "sharpness",
            "score": sum((score or 0) * weight for score, weight in zip(scores, weights)),
            "terms": [{"id": name, "score": score, "weight": weight}
                      for name, score, weight in zip(("laplacian", "fft", "niqe"), scores, weights)]}]})
    return p


def preference(identifier, a, b, winner, dimension="sharpness"):
    return {"id": identifier, "dimension": dimension, "a": a, "b": b,
            "annotation_complete": True, "ground_truth": {"preference": winner}}


def quality_face(box, scores):
    f = predicted(box)
    f["quality"] = {"method": "preview_bbox_clipped_no_upscale_v1_lap50_exposure30_min_side128_20",
                    **dict(zip(("sharpness_score", "exposure_score", "resolution_score"), scores)),
                    "score": sum(v*w for v, w in zip(scores, (0.5, 0.3, 0.2)))}
    return f


class QualityPreferenceTests(unittest.TestCase):
    def test_no_quality_labels_is_not_an_accuracy_result_and_legacy_remains_valid(self):
        photos = [photo()]
        self.assertEqual(evaluate.prepare(1, photos, [], True)["quality_preferences"], [])
        labels = complete(photos)
        labels.pop("quality_preferences")
        q = evaluate.evaluate(1, photos, [], labels)["quality_preferences"]
        self.assertEqual(q["status"], "not_requested")
        self.assertEqual(q["dimensions"], {})
        self.assertEqual(q["pairs"], [])

    def test_known_ablation_ties_ungradable_and_missing_coverage(self):
        photos = [v5_photo(1), v5_photo(2, (70, 100, 50)), v5_photo(3), photo(4)]
        labels = complete(photos)
        labels["quality_preferences"] = [
            preference("better-focus", {"photo_id": 1}, {"photo_id": 2}, "a"),
            preference("same-focus", {"photo_id": 1}, {"photo_id": 3}, "tie"),
            preference("cannot-grade", {"photo_id": 2}, {"photo_id": 3}, "ungradable"),
            preference("missing-current-feature", {"photo_id": 1}, {"photo_id": 4}, "a"),
        ]
        before = copy.deepcopy(labels)
        q = evaluate.evaluate(1, photos, [], labels)["quality_preferences"]
        self.assertEqual(labels, before)
        bucket = q["dimensions"]["sharpness"]
        self.assertEqual((bucket["total"], bucket["gradable"], bucket["ungradable"]), (4, 3, 1))
        full, ablated = bucket["variants"]["full"], bucket["variants"]["without_fft"]
        self.assertEqual((full["answered"], full["correct"], full["abstained"]), (2, 1, 1))
        self.assertEqual(full["coverage"], 2/3)
        self.assertEqual(full["agreement_when_answered"], 0.5)
        self.assertEqual(full["end_to_end_agreement"], 1/3)
        self.assertEqual(ablated["agreement_delta_vs_full_on_paired"], 0.5)
        self.assertEqual(q["pairs"][0]["variants"]["without_fft"]["a_score"], 80.)
        self.assertEqual(q["pairs"][1]["variants"]["full"]["preference"], "tie")
        self.assertIsNone(q["pairs"][3]["variants"]["full"]["preference"])

    def test_local_quality_references_matched_human_faces_not_detection_order(self):
        a, b = [0, 0, .2, .2], [.5, .5, .2, .2]
        photos = [v5_photo(1, faces=[quality_face(a, (100, 0, 100))]),
                  v5_photo(2, faces=[quality_face(b, (0, 100, 100)), predicted(a)])]
        labels = complete(photos, {1: [human(a)], 2: [human(a), human(b)]})
        labels["quality_preferences"] = [preference("portrait", {"photo_id": 1, "face_index": 0},
                                                   {"photo_id": 2, "face_index": 1}, "a", "face_quality")]
        q = evaluate.evaluate(1, photos, [], labels)["quality_preferences"]
        variants = q["dimensions"]["face_quality"]["variants"]
        self.assertEqual(variants["full"]["correct"], 1)
        self.assertEqual(variants["without_sharpness"]["correct"], 0)
        self.assertEqual(q["pairs"][0]["variants"]["full"]["b_score"], 50.)
        labels["quality_preferences"][0]["a"]["face_index"] = 1
        with self.assertRaisesRegex(evaluate.ValidationError, "human truth face"):
            evaluate.evaluate(1, photos, [], labels)
        labels["quality_preferences"][0]["a"]["face_index"] = 0
        photos[1]["analysis"]["faces"] = []
        q = evaluate.evaluate(1, photos, [], labels)["quality_preferences"]["dimensions"]["face_quality"]
        self.assertEqual(q["variants"]["full"]["abstained"], 1)
        self.assertIsNone(q["variants"]["full"]["agreement_when_answered"])

    def test_duplicate_reversed_self_invalid_and_incomplete_pairs_refused(self):
        photos = [v5_photo(1), v5_photo(2)]
        good = preference("one", {"photo_id": 1}, {"photo_id": 2}, "a")
        reversed_pair = preference("two", {"photo_id": 2}, {"photo_id": 1}, "b")
        for records, expected in [
            ([good, reversed_pair], "Duplicate quality preference pair"),
            ([good, dict(good, id="two")], "Duplicate quality preference pair"),
            ([good, dict(good)], "unique nonempty id"),
            ([dict(good, b={"photo_id": 1})], "itself"),
            ([dict(good, b={"photo_id": 999})], "unknown photo"),
            ([dict(good, a={"photo_id": True})], "unknown photo"),
            ([dict(good, annotation_complete=False)], "incomplete"),
            ([dict(good, ground_truth={"preference": None})], "independent"),
            ([dict(good, dimension="aesthetic")], "dimension"),
            ([dict(good, a={"photo_id": 1, "face_index": 0})], "whole photos"),
        ]:
            with self.subTest(expected=expected):
                labels = complete(photos)
                labels["quality_preferences"] = records
                with self.assertRaisesRegex(evaluate.ValidationError, expected):
                    evaluate.evaluate(1, photos, [], labels)

    def test_missing_terms_and_zero_retained_budget_never_invent_a_score(self):
        photos = [v5_photo(1, (None, None, 50), (0, 0, 1)), v5_photo(2, (None, None, 50), (0, 0, 1))]
        labels = complete(photos)
        labels["quality_preferences"] = [preference("tie", {"photo_id": 1}, {"photo_id": 2}, "tie")]
        q = evaluate.evaluate(1, photos, [], labels)["quality_preferences"]["dimensions"]["sharpness"]["variants"]
        self.assertEqual(q["full"]["correct"], 1)
        self.assertEqual(q["without_niqe"]["abstained"], 1)
        self.assertIsNone(q["without_niqe"]["agreement_delta_vs_full_on_paired"])
        photos[0]["analysis"]["score_breakdown"]["components"][0]["score"] = 99
        with self.assertRaisesRegex(evaluate.ValidationError, "disagrees"):
            evaluate.evaluate(1, photos, [], labels)


class EvaluationTests(unittest.TestCase):
    def test_ungradable_occlusion_reports_display_separately_from_accuracy(self):
        a, b, c = [0, 0, 0.2, 0.2], [0.3, 0, 0.2, 0.2], [0.6, 0, 0.2, 0.2]
        photos = [photo(faces=[predicted(a, "open", "uncertain"), predicted(b, None, "closed")])]
        truth = [human(box, "ungradable", "ungradable") for box in [a, b, c]]
        for face in truth:
            face.update(left_eye_ungradable_reason="occlusion", right_eye_ungradable_reason="occlusion")
        report = evaluate.evaluate(1, photos, [], complete(photos, {1: truth}))
        stats = report["ungradable_eyes"]
        self.assertEqual((stats["total"], stats["matched_face_eyes"], stats["missing_face_eyes"]), (6, 4, 2))
        self.assertEqual((stats["determinate_predictions"], stats["uncertain_predictions"], stats["hidden_predictions"]), (2, 1, 1))
        self.assertEqual(stats["determinate_rate_when_matched"], 0.5)
        self.assertEqual(stats["state_display_rate_when_matched"], 0.75)
        self.assertEqual(stats["by_reason"]["occlusion"]["total"], 6)
        self.assertEqual(report["eyes"]["gradable"], 0)
        self.assertIsNone(report["eyes"]["accuracy_when_answered"])

    def test_ungradable_reasons_are_optional_but_must_be_consistent(self):
        box = [0, 0, 0.2, 0.2]
        photos = [photo(faces=[predicted(box, None, None)])]
        labels = complete(photos, {1: [human(box, "ungradable", "ungradable")]})
        stats = evaluate.evaluate(1, photos, [], labels)["ungradable_eyes"]
        self.assertEqual(stats["by_reason"]["unspecified"]["hidden_predictions"], 2)
        face = labels["photos"][0]["ground_truth"]["faces"][0]
        face["left_eye_ungradable_reason"] = "made_up"
        with self.assertRaisesRegex(evaluate.ValidationError, "Ungradable reason"):
            evaluate.evaluate(1, photos, [], labels)
        face.update(left_eye="open", left_eye_ungradable_reason="occlusion")
        with self.assertRaisesRegex(evaluate.ValidationError, "Ungradable reason"):
            evaluate.evaluate(1, photos, [], labels)

    def test_template_never_creates_truth_from_suggestions(self):
        photos = [photo(faces=[predicted([0.1, 0.1, 0.2, 0.2])]), photo(2)]
        groups = [{"group_id": "g", "kind": "burst", "member_photo_ids": [1, 2]}]
        template = evaluate.prepare(1, photos, groups, include_suggestions=True)
        self.assertEqual(template["photos"][0]["suggested_face_boxes"], [[0.1, 0.1, 0.2, 0.2]])
        for record in template["photos"]:
            self.assertIs(record["annotation_complete"], False)
            self.assertEqual(record["ground_truth"], {"faces": []})
        self.assertIs(template["groups"][0]["annotation_complete"], False)
        self.assertEqual(template["groups"][0]["ground_truth"], {"acceptable_photo_ids": []})
        self.assertNotIn("suggested_face_boxes", evaluate.prepare(1, photos, groups)["photos"][0])
        with self.assertRaisesRegex(evaluate.ValidationError, "incomplete"):
            evaluate.evaluate(1, photos, groups, template)

    def test_known_false_positives_false_negatives_and_eye_abstention(self):
        a, b, false_positive = [0.1, 0.1, 0.2, 0.2], [0.5, 0.1, 0.2, 0.2], [0.8, 0.8, 0.1, 0.1]
        photos = [photo(faces=[predicted(a, "open", "uncertain"), predicted(false_positive)])]
        labels = complete(photos, {1: [human(a, "open", "closed"), human(b, "closed", "ungradable")]})
        report = evaluate.evaluate(1, photos, [], labels)
        self.assertEqual({key: report["face_detection"][key] for key in ("tp", "fp", "fn")}, {"tp": 1, "fp": 1, "fn": 1})
        self.assertEqual(report["face_detection"]["precision"], 0.5)
        self.assertEqual(report["face_detection"]["recall"], 0.5)
        eyes = report["eyes"]
        self.assertEqual((eyes["gradable"], eyes["ungradable"], eyes["answered"], eyes["abstained"]), (3, 1, 1, 2))
        self.assertEqual((eyes["missing_face_eyes"], eyes["uncertain_predictions"]), (1, 1))
        self.assertEqual(eyes["coverage"], 1 / 3)
        self.assertEqual(eyes["accuracy_when_answered"], 1.0)
        self.assertEqual(eyes["confusion"]["closed"]["abstain"], 2)

    def test_eye_errors_and_null_predictions_are_not_counted_as_correct(self):
        box = [0, 0, 0.3, 0.3]
        photos = [photo(faces=[predicted(box, "closed", None)])]
        report = evaluate.evaluate(1, photos, [], complete(photos, {1: [human(box)]}))
        self.assertEqual(report["eyes"]["incorrect"], 1)
        self.assertEqual(report["eyes"]["unreliable_or_missing_predictions"], 1)
        self.assertEqual(report["eyes"]["accuracy_when_answered"], 0.0)
        self.assertEqual(report["eyes"]["end_to_end_correct_rate"], 0.0)

    def test_maximum_cardinality_matching_avoids_greedy_loss(self):
        predictions = [{"bbox": [0.05, 0, 0.5, 0.4]}, {"bbox": [0, 0, 0.3, 0.4]}]
        truth = [{"bbox": [0, 0, 0.5, 0.4]}, {"bbox": [0.2, 0, 0.5, 0.4]}]
        matches = evaluate.match_faces(predictions, truth, 0.5)
        self.assertEqual({(p, t) for p, t, _ in matches}, {(0, 1), (1, 0)})
        self.assertEqual(len({p for p, _, _ in matches}), 2)
        self.assertEqual(len({t for _, t, _ in matches}), 2)

    def test_matching_threshold_and_duplicate_detections(self):
        truth = [{"bbox": [0, 0, 0.5, 1]}]
        predictions = [{"bbox": [0, 0, 1, 1]}, {"bbox": [0, 0, 1, 1]}]
        self.assertEqual(len(evaluate.match_faces(predictions, truth, 0.5)), 1)
        self.assertEqual(evaluate.match_faces(predictions, truth, 0.5001), [])

    def test_explicit_no_face_truth_reports_false_positive_and_undefined_recall(self):
        photos = [photo(faces=[predicted([0, 0, 0.2, 0.2])])]
        report = evaluate.evaluate(1, photos, [], complete(photos))
        self.assertEqual(report["face_detection"]["precision"], 0.0)
        self.assertIsNone(report["face_detection"]["recall"])
        self.assertIsNone(report["eyes"]["coverage"])
        self.assertIsNone(report["eyes"]["accuracy_when_answered"])

    def test_group_success_is_independent_of_model_order_or_ephemeral_id(self):
        photos = [photo(1), photo(2), photo(3)]
        baseline = [{"group_id": "old-id", "kind": "burst", "member_photo_ids": [1, 2, 3]}]
        labels = complete(photos, groups=baseline, acceptable={"old-id": [2, 3]})
        first = evaluate.evaluate(1, photos, baseline, labels)
        self.assertEqual(first["groups"]["top_recommendation_success_rate"], 0.0)
        reranked = [{"group_id": "new-id", "kind": "burst", "member_photo_ids": [2, 1, 3]}]
        second = evaluate.evaluate(1, photos, reranked, labels)
        self.assertEqual(second["groups"]["top_recommendation_success_rate"], 1.0)
        self.assertEqual(labels["groups"][0]["ground_truth"]["acceptable_photo_ids"], [2, 3])

    def test_same_members_in_burst_and_duplicate_groups_keep_independent_labels(self):
        photos = [photo(1), photo(2)]
        groups = [
            {"group_id": "burst-id", "kind": "burst", "member_photo_ids": [1, 2]},
            {"group_id": "duplicate-id", "kind": "duplicate", "member_photo_ids": [1, 2]},
        ]
        labels = complete(photos, groups=groups, acceptable={"burst-id": [1], "duplicate-id": [2]})
        self.assertEqual([g["group_key"] for g in labels["groups"]], ["burst:1,2", "duplicate:1,2"])
        report = evaluate.evaluate(1, photos, groups, labels)
        self.assertEqual(report["groups"], {"total": 2, "successful": 1, "top_recommendation_success_rate": 0.5})
        changed = copy.deepcopy(groups)
        changed[1]["member_photo_ids"] = [2, 1]
        changed[1]["group_id"] = "regenerated-duplicate-id"
        self.assertEqual(evaluate.evaluate(1, photos, changed, labels)["groups"]["successful"], 2)

    def test_duplicate_membership_is_still_rejected_within_same_group_kind(self):
        photos = [photo(1), photo(2)]
        groups = [
            {"group_id": "one", "kind": "burst", "member_photo_ids": [1, 2]},
            {"group_id": "two", "kind": "burst", "member_photo_ids": [2, 1]},
        ]
        labels = complete(photos, groups=groups)
        with self.assertRaisesRegex(evaluate.ValidationError, "Duplicate group_key"):
            evaluate.evaluate(1, photos, groups, labels)

    def test_incomplete_missing_and_stale_annotations_are_refused(self):
        photos = [photo(), photo(2)]
        good = complete(photos)
        for change, error in [
            (lambda labels: labels["photos"].pop(), "Missing or extra"),
            (lambda labels: labels["photos"][0].update(annotation_complete=False), "incomplete"),
            (lambda labels: labels["photos"][0].update(mtime=999), "fingerprint differs"),
            (lambda labels: labels["photos"].append(copy.deepcopy(labels["photos"][0])), "Duplicate"),
        ]:
            labels = copy.deepcopy(good)
            change(labels)
            with self.assertRaisesRegex(evaluate.ValidationError, error):
                evaluate.evaluate(1, photos, [], labels)
        photos[0]["analysis"] = None
        with self.assertRaisesRegex(evaluate.ValidationError, "no stored analysis"):
            evaluate.evaluate(1, photos, [], good)

    def test_incomplete_groups_invalid_truth_and_unknown_members_refused(self):
        photos = [photo(), photo(2)]
        groups = [{"group_id": "g", "kind": "burst", "member_photo_ids": [1, 2]}]
        labels = complete(photos, groups=groups)
        labels["groups"][0]["annotation_complete"] = False
        with self.assertRaisesRegex(evaluate.ValidationError, "incomplete"):
            evaluate.evaluate(1, photos, groups, labels)
        labels["groups"][0]["annotation_complete"] = True
        labels["groups"][0]["ground_truth"]["acceptable_photo_ids"] = [999]
        with self.assertRaisesRegex(evaluate.ValidationError, "group members"):
            evaluate.evaluate(1, photos, groups, labels)
        for bad in [human([0, 0, 2, 1]), human([0, 0, 1, 1], left="uncertain")]:
            labels = complete(photos, {1: [bad]})
            with self.assertRaises(evaluate.ValidationError):
                evaluate.evaluate(1, photos, [], labels)

    def test_design_gate_is_opt_in_and_not_an_accuracy_claim(self):
        photos = [photo()]
        labels = complete(photos)
        report = evaluate.evaluate(1, photos, [], labels)
        self.assertFalse(report["design_dataset_gate"]["satisfied"])
        with self.assertRaisesRegex(evaluate.ValidationError, "300 photos and 60 unique burst groups"):
            evaluate.evaluate(1, photos, [], labels, require_design_gate=True)

    def test_design_gate_counts_only_unique_burst_member_sets(self):
        photos = [photo(i) for i in range(300)]
        bursts = [{"group_id": str(i), "kind": "burst", "member_photo_ids": [2*i, 2*i+1]} for i in range(60)]
        duplicates = [{**g, "group_id": "d"+g["group_id"], "kind": "duplicate"} for g in bursts]
        for groups in [duplicates, bursts[:30]+duplicates, bursts[:59]+duplicates]:
            with self.assertRaisesRegex(evaluate.ValidationError, "60 unique burst"):
                evaluate.evaluate(1, photos, groups, complete(photos, groups=groups), require_design_gate=True)
        repeated = [dict(bursts[0], group_id=str(i), member_photo_ids=[1, 0]) for i in range(60)]
        gate = evaluate.design_dataset_gate(photos, repeated + duplicates, True)
        self.assertEqual(gate["unique_burst_groups"], 1)
        self.assertFalse(gate["satisfied"])
        report = evaluate.evaluate(1, photos, bursts+duplicates, complete(photos, groups=bursts+duplicates), require_design_gate=True)
        self.assertTrue(report["design_dataset_gate"]["satisfied"])
        self.assertEqual(report["design_dataset_gate"]["unique_burst_groups"], 60)
        self.assertEqual(report["design_dataset_gate"]["unique_duplicate_groups"], 60)
        self.assertIn("do not prove", report["design_dataset_gate"]["limitations"])

    def test_singleton_groups_cannot_satisfy_burst_gate(self):
        photos = [photo(i) for i in range(300)]
        groups = [{"group_id": str(i), "kind": "burst", "member_photo_ids": [i]} for i in range(60)]
        with self.assertRaisesRegex(evaluate.ValidationError, "at least two"):
            evaluate.design_dataset_gate(photos, groups, True)
        with self.assertRaisesRegex(evaluate.ValidationError, "at least two"):
            evaluate.prepare(1, photos, groups)
        with self.assertRaisesRegex(evaluate.ValidationError, "at least two"):
            evaluate.evaluate(1, photos, groups, complete(photos), require_design_gate=True)

    def test_timing_uses_existing_measurements_with_defined_interpolation(self):
        photos = [photo(1, elapsed=10), photo(2, elapsed=20), photo(3, elapsed=30), photo(4)]
        report = evaluate.evaluate(1, photos, [], complete(photos))
        self.assertEqual(report["timing"]["photos_with_timing"], 3)
        self.assertEqual(report["timing"]["p50_ms"], 20)
        self.assertEqual(report["timing"]["p95_ms"], 29)
        self.assertIsNone(evaluate.percentile([], 0.5))

    def test_sqlite_snapshot_cli_refusal_and_no_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            database = root / "project.sqlite"
            connection = sqlite3.connect(database)
            connection.executescript("CREATE TABLE projects(id INTEGER); INSERT INTO projects VALUES(1); "
                                     "CREATE TABLE photos(id INTEGER,project_id INTEGER,path TEXT,mtime INTEGER,size_bytes INTEGER,missing INTEGER,quarantined INTEGER);"
                                     "CREATE TABLE analyses(photo_id INTEGER,data TEXT);"
                                     "CREATE TABLE burst_groups(id TEXT,project_id INTEGER,kind TEXT,members TEXT);"
                                     "INSERT INTO photos VALUES(1,1,'1.jpg',123,456,0,0);")
            connection.execute("INSERT INTO analyses VALUES(1,?)", (json.dumps(photo()["analysis"]),))
            connection.commit()
            connection.close()
            before = database.read_bytes()
            output = root / "labels.json"
            command = ["--database", str(database), "--project", "1"]
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                self.assertEqual(evaluate.main(["prepare", *command, "--output", str(output)]), 0)
                self.assertEqual(evaluate.main(["prepare", *command, "--output", str(output)]), 2)
                self.assertEqual(evaluate.main(["evaluate", *command, "--annotations", str(output)]), 2)
                labels = json.loads(output.read_text())
                labels["photos"][0]["annotation_complete"] = True
                output.write_text(json.dumps(labels))
                self.assertEqual(evaluate.main(["evaluate", *command, "--annotations", str(output)]), 0)
            self.assertEqual(database.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
