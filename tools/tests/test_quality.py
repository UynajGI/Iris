"""Regression tests for fail-closed checks, verified tools and complexity debt."""

import hashlib
import importlib.util
import io
import tempfile
import unittest
import urllib.error
import zipfile
from pathlib import Path
from unittest.mock import MagicMock, patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("quality", ROOT / "tools/quality.py")
quality = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(quality)


class QualityTests(unittest.TestCase):
    def test_transient_download_failures_retry_then_return_complete_payload(self):
        for failure in [
            urllib.error.HTTPError("https://example.test/tool", code, "transient", {}, None)
            for code in [408, 429, 500, 502, 503, 504]
        ] + [urllib.error.URLError("connection reset"), TimeoutError("read timed out")]:
            with self.subTest(failure=failure):
                response = MagicMock()
                response.__enter__.return_value.read.return_value = b"complete archive"
                with (
                    patch.object(
                        quality.urllib.request, "urlopen", side_effect=[failure, response]
                    ) as fetch,
                    patch.object(quality.time, "sleep") as sleep,
                ):
                    self.assertEqual(
                        quality.download_tool("https://example.test/tool"), b"complete archive"
                    )
                    self.assertEqual(fetch.call_count, 2)
                    sleep.assert_called_once_with(2)

    def test_download_retries_are_bounded_and_fail_closed(self):
        failure = urllib.error.HTTPError("https://example.test/tool", 504, "timeout", {}, None)
        with (
            patch.object(quality.urllib.request, "urlopen", side_effect=failure) as fetch,
            patch.object(quality.time, "sleep") as sleep,
            self.assertRaises(urllib.error.HTTPError),
        ):
            quality.download_tool("https://example.test/tool")
        self.assertEqual(fetch.call_count, 3)
        self.assertEqual([call.args[0] for call in sleep.call_args_list], [2, 4])

    def test_permanent_http_errors_are_not_retried(self):
        for code in [401, 403, 404]:
            with self.subTest(code=code):
                failure = urllib.error.HTTPError(
                    "https://example.test/tool", code, "permanent", {}, None
                )
                with (
                    patch.object(quality.urllib.request, "urlopen", side_effect=failure) as fetch,
                    patch.object(quality.time, "sleep") as sleep,
                    self.assertRaises(urllib.error.HTTPError),
                ):
                    quality.download_tool("https://example.test/tool")
                fetch.assert_called_once()
                sleep.assert_not_called()

    def test_invalid_download_never_enters_tool_cache(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            with (
                patch.object(quality, "CACHE", cache),
                patch.object(
                    quality,
                    "tool_spec",
                    return_value=("test.zip", "0" * 64, "https://example.test/tool"),
                ),
                patch.object(quality, "download_tool", return_value=b"tampered") as fetch,
                self.assertRaisesRegex(ValueError, "SHA256 mismatch"),
            ):
                quality.tool("actionlint", install=True)
            fetch.assert_called_once()
            self.assertEqual(list(cache.iterdir()), [])

    def test_archive_is_verified_before_executable_is_read(self):
        data = io.BytesIO()
        with zipfile.ZipFile(data, "w") as archive:
            archive.writestr("actionlint.exe", b"test executable")
            archive.writestr("../../escape", b"must never be extracted")
        payload = data.getvalue()
        name, binary = quality.verified_payload(
            payload, hashlib.sha256(payload).hexdigest(), "actionlint", "test.zip"
        )
        self.assertEqual((name, binary), ("actionlint.exe", b"test executable"))
        with self.assertRaisesRegex(ValueError, "SHA256 mismatch"):
            quality.verified_payload(
                payload + b"tampered", hashlib.sha256(payload).hexdigest(), "actionlint", "test.zip"
            )

    def test_complexity_limits_are_per_function_and_per_metric(self):
        item = {"path": "tools/legacy.py", "name": "old", "ccn": 20, "nloc": 130}
        limits = {"tools/legacy.py::old": {"ccn": 20, "nloc": 130}}
        self.assertEqual(quality.complexity_errors({"warnings": [item]}, limits), [])
        for change in [{"ccn": 21}, {"nloc": 131}, {"name": "new"}, {"path": "tools/other.py"}]:
            with self.subTest(change=change):
                self.assertEqual(
                    len(quality.complexity_errors({"warnings": [{**item, **change}]}, limits)), 1
                )

    def test_failed_subprocess_is_not_swallowed(self):
        import subprocess
        import sys

        with self.assertRaises(subprocess.CalledProcessError):
            quality.run([sys.executable, "-c", "raise SystemExit(23)"])


if __name__ == "__main__":
    unittest.main()
