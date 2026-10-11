"""Offline regression checks for public fixture tool orchestration."""

import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]


def load_tool(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "tools" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


expand = load_tool("expand-public-fixtures")
verify = load_tool("verify-public-fixtures")


class PublicFixtureTests(unittest.TestCase):
    def test_scan_failure_still_checks_valid_photos_and_keeps_failure_status(self):
        client = Mock()
        photo = {
            "id": 1,
            "filename": "good.dng",
            "format": "dng",
            "analysis_status": "current",
            "analysis": {"selection_evidence": {"observations": []}, "faces": []},
        }
        client.request.side_effect = [{"id": 1}, [photo], [photo], []]
        client.job.side_effect = [
            {"state": "failed", "errors": ["bad.cr2"], "failed_scan_paths": ["bad.cr2"]},
            {"state": "completed", "errors": []},
            {"state": "completed", "errors": [], "result": {"reused": 1}},
        ]
        with (
            patch.object(verify, "preview", return_value={"preview_bytes": 5}),
            patch("builtins.print"),
        ):
            result = verify.exercise(client, Path("public-copies"), 2)
        self.assertFalse(result["ok"])
        self.assertEqual(result["scanned"], 1)
        self.assertEqual(result["analyzed"], 1)
        self.assertEqual(result["previews_succeeded"], 1)
        self.assertEqual(client.job.call_count, 3)

    def test_preview_failure_is_recorded(self):
        client = Mock()
        client.request.side_effect = OSError("decoder unavailable")
        result = verify.preview(client, {"id": 1, "filename": "bad.dng", "format": "dng"})
        self.assertEqual(result["error"], "decoder unavailable")
        self.assertNotIn("preview_bytes", result)

    def test_prepare_preserves_catalog_and_raw_is_explicit(self):
        prepare = expand.PREPARE
        for raw_enabled in (False, True):
            with (
                self.subTest(raw=raw_enabled),
                tempfile.TemporaryDirectory(prefix="iris-fixture-unit-") as directory,
            ):
                root = Path(directory)
                base = root / "reference/test-data"
                base.mkdir(parents=True)
                old = {"path": "retained", "bytes": 2}
                piq = {"path": "piq", "bytes": 3}
                raw = {"path": "raw", "bytes": 5}
                manifest = base / "manifest.json"
                manifest.write_text(json.dumps([old]), encoding="utf-8")
                with (
                    patch.object(prepare, "ROOT", root),
                    patch.object(prepare, "piq_items", side_effect=lambda _: [piq]),
                    patch.object(prepare, "raw_items", return_value=[raw]) as raw_items,
                    patch.object(prepare, "persist", side_effect=lambda _, item: item),
                    patch.object(sys, "argv", ["prepare", *(["--raw"] if raw_enabled else [])]),
                    patch("builtins.print"),
                ):
                    prepare.main()
                expected = [old, piq, *([raw] if raw_enabled else [])]
                self.assertEqual(json.loads(manifest.read_text()), expected)
                self.assertEqual(raw_items.call_count, int(raw_enabled))
                self.assertFalse(manifest.with_suffix(".tmp").exists())

    def test_downloads_merge_existing_records_and_persist_verified_results(self):
        with tempfile.TemporaryDirectory(prefix="iris-fixture-unit-") as directory:
            manifest = Path(directory) / "manifest.json"
            records = {"old": {"path": "old", "sha256": "old"}}
            items = [{"path": str(index), "sha256": str(index)} for index in range(12)]
            with (
                patch.object(expand, "download", side_effect=lambda item: item),
                patch("builtins.print"),
            ):
                expand.download_items(items, records, manifest)
            self.assertEqual(len(records), 13)
            self.assertEqual(
                json.loads(manifest.read_text(encoding="utf-8")), list(records.values())
            )
            self.assertFalse(manifest.with_suffix(".tmp").exists())

    def test_download_failure_preserves_successful_results_for_retry(self):
        def download(item):
            if item["path"] == "bad":
                raise ValueError("fixture failure")
            return item

        with tempfile.TemporaryDirectory(prefix="iris-fixture-unit-") as directory:
            manifest = Path(directory) / "manifest.json"
            records = {}
            with patch.object(expand, "download", side_effect=download), patch("builtins.print"):
                with self.assertRaisesRegex(
                    RuntimeError, "1 downloads failed; verified files retained"
                ):
                    expand.download_items([{"path": "good"}, {"path": "bad"}], records, manifest)
            self.assertEqual(records, {"good": {"path": "good"}})
            self.assertEqual(json.loads(manifest.read_text()), [{"path": "good"}])

    def parse(self, arguments, records):
        with tempfile.TemporaryDirectory(prefix="iris-fixture-unit-") as directory:
            root = Path(directory)
            (root / "manifest.json").write_text(json.dumps(records), encoding="utf-8")
            with (
                patch.object(verify, "FIXTURES", root),
                patch.object(sys, "argv", ["verify", *arguments]),
            ):
                return verify.parse_inputs()

    def test_dataset_filter_is_case_insensitive_and_limit_preserves_order(self):
        records = [{"dataset": "other"}, {"dataset": "PIQ23 one"}, {"dataset": "piq23 two"}]
        args, selected = self.parse(["--dataset", "piQ23", "--limit", "1"], records)
        self.assertEqual(args.limit, 1)
        self.assertEqual(selected, [records[1]])

    def test_invalid_limit_and_empty_selection_keep_cli_errors(self):
        for arguments in (["--limit", "0"], ["--limit", "-1"], ["--dataset", "missing"]):
            with self.subTest(arguments=arguments), patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit) as error:
                    self.parse(arguments, [{"dataset": "PIQ23"}])
                self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
