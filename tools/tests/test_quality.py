"""Regression tests for fail-closed checks, verified tools and complexity debt."""
import hashlib
import importlib.util
import io
from pathlib import Path
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("quality", ROOT / "tools/quality.py")
quality = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(quality)


class QualityTests(unittest.TestCase):
    def test_archive_is_verified_before_executable_is_read(self):
        data = io.BytesIO()
        with zipfile.ZipFile(data, "w") as archive:
            archive.writestr("actionlint.exe", b"test executable")
            archive.writestr("../../escape", b"must never be extracted")
        payload = data.getvalue()
        name, binary = quality.verified_payload(payload, hashlib.sha256(payload).hexdigest(), "actionlint", "test.zip")
        self.assertEqual((name, binary), ("actionlint.exe", b"test executable"))
        with self.assertRaisesRegex(ValueError, "SHA256 mismatch"):
            quality.verified_payload(payload + b"tampered", hashlib.sha256(payload).hexdigest(), "actionlint", "test.zip")

    def test_complexity_limits_are_per_function_and_per_metric(self):
        item = {"path": "tools/legacy.py", "name": "old", "ccn": 20, "nloc": 130}
        limits = {"tools/legacy.py::old": {"ccn": 20, "nloc": 130}}
        self.assertEqual(quality.complexity_errors({"warnings": [item]}, limits), [])
        for change in [{"ccn": 21}, {"nloc": 131}, {"name": "new"}, {"path": "tools/other.py"}]:
            with self.subTest(change=change):
                self.assertEqual(len(quality.complexity_errors({"warnings": [{**item, **change}]}, limits)), 1)

    def test_failed_subprocess_is_not_swallowed(self):
        import subprocess
        import sys
        with self.assertRaises(subprocess.CalledProcessError):
            quality.run([sys.executable, "-c", "raise SystemExit(23)"])


if __name__ == "__main__":
    unittest.main()
