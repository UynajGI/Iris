"""Synthetic refactor regressions; no model, GPU, network or photo accuracy claims."""
import importlib.util
import io
from pathlib import Path
import sys
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))


def load_tool(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), TOOLS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    with patch.dict(sys.modules, {"validation_support": SimpleNamespace(Daemon=object)}):
        spec.loader.exec_module(module)
    return module


sample = load_tool("verify_sample")
local = load_tool("verify-local-expansion")
masked = load_tool("verify_faceocc")
monitor = load_tool("benchmark_worker_limits")
expanded = load_tool("benchmark-expanded")
raw = load_tool("setup-raw-runtime")


class RefactorRegressionTests(unittest.TestCase):
    @staticmethod
    def analysis():
        components = [{"id": name, "score": None, "configured_weight": 0.2,
                       "effective_weight": 0, "contribution": 0, "observed_count": 0,
                       "total_count": 1, "missing_reason": "synthetic unavailable", "terms": []}
                      for name in ("eyes", "sharpness", "face", "exposure", "smile")]
        return {"version": "iris-vision-v6-synthetic", "composite_score": 0, "faces": [],
                "verdict": "review", "elapsed_ms": 12, "niqe": 4,
                "score_breakdown": {"method": "synthetic", "components": components,
                                    "effective_weight_total": 0}}

    def test_sample_score_checks_preserve_missing_and_invalid_term_rules(self):
        analysis = self.analysis()
        self.assertEqual(sample.verify_score_breakdown(analysis), "synthetic")
        term = {"raw": [{"value": float("nan")}], "weight": 0, "score": None,
                "missing_reason": "unavailable", "contribution": 0}
        analysis["score_breakdown"]["components"][0]["terms"] = [term]
        with self.assertRaises(AssertionError):
            sample.verify_score_breakdown(analysis)
        term["raw"] = []
        term["weight"] = 1
        with self.assertRaises(AssertionError):
            sample.verify_score_breakdown(analysis)

    def test_sample_summary_retains_deterministic_digest_and_statistics(self):
        analysis = self.analysis()
        photos = [{"path": "synthetic.jpg", "analysis": analysis}]
        state = {"groups_dirty": False, "pending_analysis": 0}
        report = sample.analysis_report(photos, {}, {}, {}, {}, {}, {}, state)
        self.assertEqual(report["photo_latency"], {"p50_ms": 12, "p95_ms": 12})
        self.assertEqual(report["score_breakdowns_verified"], {"synthetic": 1})
        self.assertEqual(report["niqe"], {"count": 1, "min": 4, "max": 4})
        analysis["elapsed_ms"] = 900
        changed = sample.analysis_report(photos, {}, {}, {}, {}, {}, {}, state)
        self.assertEqual(report["predictions_sha256"], changed["predictions_sha256"])
        analysis["verdict"] = "pending"
        changed = sample.analysis_report(photos, {}, {}, {}, {}, {}, {}, state)
        self.assertNotEqual(report["predictions_sha256"], changed["predictions_sha256"])

    def test_mask_comparison_counts_gates_without_resurrecting_or_changing_eyes(self):
        eye = {"state": "open", "ear": 0.3}
        before = {"x": {"faces": [{"left_eye": eye, "right_eye": None}]}}
        after = {"x": {"faces": [{"left_eye": None, "right_eye": None,
                                  "left_eye_visibility": {"visible_fraction": 0.1}}]}}
        self.assertEqual(masked.compare_mask_predictions(before, after),
                         ({"open -> hidden": 1, "hidden -> hidden": 1}, 1, 1))
        after["x"]["faces"][0]["right_eye"] = eye
        with self.assertRaisesRegex(AssertionError, "resurrected"):
            masked.compare_mask_predictions(before, after)
        after["x"]["faces"][0].update(right_eye=None, left_eye={"state": "closed"})
        with self.assertRaisesRegex(AssertionError, "changed an eye measurement"):
            masked.compare_mask_predictions(before, after)

    def test_embedding_checks_keep_shape_hash_normalization_and_quality_invariance(self):
        analysis = {"verdict": "review", "elapsed_ms": 1, "settings": {},
                    "embedding": {"model_sha256": local.SHA, "vector": [1.] + [0.] * 383}}
        listing = [{"id": 1, "filename": "synthetic.jpg", "analysis_status": "current",
                    "analysis": analysis}]
        local.verify_embeddings(listing)
        baseline = {1: {"verdict": "review", "elapsed_ms": 99, "settings": {"old": True}}}
        local.verify_default_quality(listing, baseline)
        analysis["embedding"]["vector"][0] = 2
        with self.assertRaises(AssertionError):
            local.verify_embeddings(listing)
        analysis["verdict"] = "reject"
        with self.assertRaises(AssertionError):
            local.verify_default_quality(listing, baseline)

    def test_measurement_targets_include_conditional_phases_and_actual_sampling(self):
        args = SimpleNamespace(first_budget_seconds=10, cache_budget_seconds=4,
                               memory_budget_mib=1, dinov3=True, failure_isolation=True)
        phases = {key: {"seconds": 1} for key in (
            "first_scan", "first_analysis", "cached_scan", "cached_analysis",
            "threshold_regroup", "failure_scan", "failure_analysis")}
        report = {"phases": phases, "memory": {"peak_tree_rss_bytes": 0,
                   "peak_tree_private_bytes": 0, "samples": 1}}
        expanded.record_targets(args, report)
        self.assertTrue(report["all_targets_passed"])
        report["memory"]["samples"] = 0
        phases["threshold_regroup"]["seconds"] = 5
        expanded.record_targets(args, report)
        self.assertFalse(report["all_targets_passed"])
        self.assertFalse(report["targets_passed"]["threshold_time"])
        self.assertFalse(report["targets_passed"]["sampling"])

    @staticmethod
    def archive(entries):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
            for name, content in entries:
                entry = tarfile.TarInfo(name)
                entry.size = len(content)
                archive.addfile(entry, io.BytesIO(content))
        return buffer.getvalue()

    def test_raw_payload_keeps_archive_and_existing_output_confinement(self):
        data = self.archive([("package/bin/exiftool.exe", b"synthetic")])
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            self.assertEqual(raw.runtime_payload(data, output), {"bin/exiftool.exe": b"synthetic"})
            for entries in [[("package/../escape", b"x")],
                            [("package/a", b"x"), ("package/a", b"x")]]:
                with self.assertRaises(ValueError):
                    raw.runtime_payload(self.archive(entries), output)
            (output / "unlisted.txt").write_text("keep", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "unlisted"):
                raw.runtime_payload(data, output)
            self.assertEqual((output / "unlisted.txt").read_text(), "keep")

    def test_resource_sampling_counts_owned_daemons_and_query_failures(self):
        class Missing(Exception):
            pass

        class Denied(Exception):
            pass

        psutil = SimpleNamespace(NoSuchProcess=Missing, AccessDenied=Denied)
        daemon = Mock()
        daemon.name.return_value = "iris-daemon.exe"
        daemon.memory_info.return_value = SimpleNamespace(rss=10, private=20)
        daemon.num_threads.return_value = 3
        private_unknown = Mock()
        private_unknown.name.return_value = "iris-daemon.exe"
        private_unknown.num_threads.return_value = 3
        private_unknown.memory_info.return_value = SimpleNamespace(rss=5)
        skipped, vanished, denied = Mock(), Mock(), Mock()
        skipped.name.return_value = "unrelated.exe"
        vanished.name.side_effect = Missing
        denied.name.side_effect = Denied
        root = Mock()
        root.children.return_value = [daemon, private_unknown, skipped, vanished, denied]
        self.assertEqual(monitor.resource_sample(root, psutil),
                         ({"rss": 15, "private": None, "threads": 6, "processes": 2}, 1, 1))
        skipped.memory_info.assert_not_called()

    def test_monitor_cleanup_terminates_only_observed_descendants_in_reverse_order(self):
        root, child = Mock(), Mock()
        first, second = Mock(), Mock()
        order = []
        first.terminate.side_effect = lambda: order.append("first")
        second.terminate.side_effect = lambda: order.append("second")
        child.terminate.side_effect = lambda: order.append("child")
        child.poll.return_value = None
        root.children.return_value = [first, second]
        psutil = SimpleNamespace(NoSuchProcess=ProcessLookupError, AccessDenied=PermissionError)
        monitor.terminate_monitor(child, root, psutil)
        self.assertEqual(order, ["second", "first", "child"])
        child.wait.assert_called_once_with(timeout=30)


if __name__ == "__main__":
    unittest.main()
