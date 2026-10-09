import importlib.util
from pathlib import Path
import unittest

PATH = Path(__file__).resolve().parents[1] / "setup-dinov3.py"
spec = importlib.util.spec_from_file_location("setup_dinov3", PATH)
setup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(setup)


class DinoV3ArtifactChecks(unittest.TestCase):
    def test_source_hashes_are_complete_and_revision_is_fixed(self):
        self.assertEqual(len(setup.REVISION), 40)
        self.assertEqual(len(setup.SHA256), 64)
        self.assertIn("onnx/model.onnx_data", setup.SOURCES)
        self.assertIn("LICENSE.md", setup.SOURCES)
        for size, digest in setup.SOURCES.values():
            self.assertGreater(size, 0)
            self.assertEqual(len(bytes.fromhex(digest)), 32)

    def test_truncation_and_same_size_corruption_are_rejected(self):
        import hashlib
        data = b"verified model bytes"
        digest = hashlib.sha256(data).hexdigest()
        self.assertEqual(setup.checked(data, len(data), digest, "fixture"), data)
        for invalid in [data[:-1], b"x" + data[1:], data + b"x"]:
            with self.assertRaisesRegex(ValueError, "size/SHA-256 mismatch"):
                setup.checked(invalid, len(data), digest, "fixture")


if __name__ == "__main__":
    unittest.main()
