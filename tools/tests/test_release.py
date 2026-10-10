"""Release identity and native runtime integrity regressions."""
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import onnx_runtime

spec = importlib.util.spec_from_file_location("release_check", TOOLS / "check-release.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def test_current_tag_matches_and_wrong_tag_fails(self):
        version = json.loads((TOOLS.parent / "apps/shell/package.json").read_text())["version"]
        self.assertEqual(release.check(f"v{version}"), version)
        with self.assertRaises(ValueError):
            release.check("v9.0.0")
        with self.assertRaises(ValueError):
            release.check("main")

    def test_untrusted_runtime_archive_never_installs(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            with patch.object(onnx_runtime.platform, "system", return_value="Linux"), patch.object(onnx_runtime.platform, "machine", return_value="x86_64"), patch.object(onnx_runtime.urllib.request, "urlopen", return_value=io.BytesIO(b"untrusted")):
                with self.assertRaisesRegex(ValueError, "SHA-256"):
                    onnx_runtime.install(root)
            self.assertEqual(list(root.iterdir()), [])

    def test_unknown_runtime_platform_is_explicit(self):
        with patch.object(onnx_runtime.platform, "system", return_value="Other"):
            with self.assertRaisesRegex(ValueError, "No pinned runtime"):
                onnx_runtime.install(Path("unused"))


if __name__ == "__main__":
    unittest.main()
