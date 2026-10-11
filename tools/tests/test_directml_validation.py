"""DirectML evidence checks on synthetic records; not real GPU validation."""

import importlib.util
import json
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("directml_validation", TOOLS / "verify-directml.py")
validation = importlib.util.module_from_spec(SPEC)
with patch.dict(sys.modules, {"validation_support": SimpleNamespace(Daemon=object)}):
    SPEC.loader.exec_module(validation)


class DirectMLValidationTests(unittest.TestCase):
    def test_run_order_keeps_cpu_baseline_and_conditional_fallbacks(self):
        args = SimpleNamespace(fallback_only=False, dinov3=False)
        self.assertEqual(
            validation.execution_runs(args),
            [
                ("cpu", None),
                ("directml_0", 0),
                ("directml_1", 1),
                ("invalid_device_fallback", 2147483647),
                ("missing_runtime_fallback", 0),
                ("corrupt_runtime_rejected", 0),
            ],
        )
        args.fallback_only = True
        self.assertEqual(
            [name for name, _ in validation.execution_runs(args)],
            [
                "cpu",
                "invalid_device_fallback",
                "missing_runtime_fallback",
                "corrupt_runtime_rejected",
            ],
        )
        args.dinov3 = True
        self.assertEqual(
            validation.execution_runs(args),
            [("cpu", None), ("invalid_device_fallback", 2147483647)],
        )

    def test_profile_statistics_count_only_provider_events(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = Path(temporary)
            profiles = run / "profiles"
            profiles.mkdir()
            (profiles / "events.json").write_text(
                json.dumps(
                    [
                        {"args": {"provider": "DmlExecutionProvider"}, "dur": 11},
                        {"args": {"provider": "CPUExecutionProvider"}, "dur": 30},
                        {"args": {"provider": "DmlExecutionProvider"}},
                        {"dur": 90},
                    ]
                ),
                encoding="utf-8",
            )
            report = {}
            files, providers, duration = validation.operator_profiles(run, report)
            self.assertEqual(len(files), 1)
            self.assertEqual(providers, Counter(DmlExecutionProvider=2, CPUExecutionProvider=1))
            self.assertEqual(duration, 11)
            self.assertEqual(report["operator_profiles"]["directml_duration_us"], 11)

    def test_cpu_comparison_keeps_strict_score_face_and_verdict_checks(self):
        baseline = {"a.jpg": {"faces": [{}], "composite_score": 80.0, "verdict": "review"}}
        report = {}
        validation.compare_cpu_predictions(baseline, baseline, report)
        self.assertEqual(
            report["cpu_comparison"],
            {
                "face_counts_equal": True,
                "max_score_absolute_error": 0.0,
                "verdict_changes": 0,
            },
        )
        for change in [{"faces": []}, {"composite_score": 80.5}, {"verdict": "recommend"}]:
            with self.subTest(change=change), self.assertRaises(AssertionError):
                validation.compare_cpu_predictions(
                    {"a.jpg": {**baseline["a.jpg"], **change}}, baseline, {}
                )

    def test_embedding_comparison_requires_evidence_and_strict_cosine(self):
        baseline = {"a.jpg": {"embedding": {"vector": [1.0, 0.0]}}}
        report = {}
        validation.compare_embedding_predictions(baseline, baseline, report)
        self.assertEqual(report["embedding_min_cosine_to_cpu"], 1.0)
        with self.assertRaises(AssertionError):
            validation.compare_embedding_predictions(
                {"a.jpg": {"embedding": {"vector": [0.9999, 0.0]}}}, baseline, {}
            )
        with self.assertRaises(AssertionError):
            validation.compare_embedding_predictions({"a.jpg": {}}, {"a.jpg": {}}, {})

    def test_cpu_fallback_requires_warning_and_zero_gpu_events(self):
        args = SimpleNamespace(dinov3=False)
        for label in ["invalid_device_fallback", "missing_runtime_fallback"]:
            validation.verify_execution_provider(
                args, label, ["DirectML CPU fallback"], [], Counter(), {}
            )
            with self.subTest(label=label), self.assertRaises(AssertionError):
                validation.verify_execution_provider(args, label, [], [], Counter(), {})
            with self.subTest(label=label), self.assertRaises(AssertionError):
                validation.verify_execution_provider(
                    args, label, ["DirectML CPU fallback"], [], Counter(DmlExecutionProvider=1), {}
                )
        with self.assertRaises(AssertionError):
            validation.verify_execution_provider(args, "directml_0", [], [], Counter(), {})

    def test_project_settings_are_written_after_create_and_before_scan(self):
        class Daemon:
            def __init__(self):
                self.calls = []

            def request(self, method, path, body=None):
                self.calls.append((method, path, body))
                return {"id": 7} if path == "/projects" else {}

        daemon = Daemon()
        args = SimpleNamespace(dinov3=True)
        self.assertEqual(validation.configure_project(daemon, args, 1, TOOLS), 7)
        self.assertEqual(
            [(method, path) for method, path, _ in daemon.calls],
            [
                ("POST", "/projects"),
                ("GET", "/settings?project_id=7"),
                ("PUT", "/settings?project_id=7"),
            ],
        )
        settings = daemon.calls[-1][2]
        self.assertEqual(settings["execution_provider"], "directml")
        self.assertEqual(settings["directml_device_id"], 1)
        self.assertEqual(settings["embedding_provider"], "dinov3_vits16")
        self.assertEqual(settings["semantic_similarity_threshold"], 0.9)


if __name__ == "__main__":
    unittest.main()
