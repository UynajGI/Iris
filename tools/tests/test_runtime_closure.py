import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("runtime_closure", Path(__file__).parents[1] / "package-runtime-closure.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class RuntimeClosureTests(unittest.TestCase):
    def fixture(self, root):
        source = root / "source"
        source.mkdir()
        names = ["DirectML.dll", "onnxruntime.dll", "onnxruntime_providers_shared.dll", "LICENSE.txt"]
        files = []
        for name in names:
            data = name.encode()
            (source / name).write_bytes(data)
            files.append({"path": name, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()})
        (source / "manifest.json").write_text(json.dumps({"files": files}), "utf-8")
        return source

    def test_copy_exact_closure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.fixture(root)
            self.assertEqual(module.copy_closure(source, root / "out", "directml"), 4)
            self.assertEqual((source / "DirectML.dll").read_bytes(), (root / "out/DirectML.dll").read_bytes())

    def test_corrupt_unlisted_and_traversal_are_rejected_before_copy(self):
        for kind in ["corrupt", "unlisted", "traversal"]:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                source = self.fixture(root)
                if kind == "corrupt":
                    (source / "DirectML.dll").write_bytes(b"wrong")
                elif kind == "unlisted":
                    (source / "extra.onnx").write_bytes(b"weight")
                else:
                    manifest = json.loads((source / "manifest.json").read_text())
                    manifest["files"][0]["path"] = "../DirectML.dll"
                    (source / "manifest.json").write_text(json.dumps(manifest))
                with self.assertRaises(ValueError):
                    module.copy_closure(source, root / "out", "directml")
                self.assertFalse((root / "out").exists())
