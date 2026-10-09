import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

SPEC = importlib.util.spec_from_file_location("package_dinov3", Path(__file__).resolve().parents[1] / "package-dinov3.py")
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


class OptionalBundleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.source.mkdir()
        self.model = b"fixture-model"
        self.license = b"fixture-license"
        self.sha = hashlib.sha256(self.model).hexdigest()
        self.sources = {"LICENSE.md": (len(self.license), hashlib.sha256(self.license).hexdigest())}
        self.patches = [patch.object(package.SETUP, "SIZE", len(self.model)), patch.object(package.SETUP, "SHA256", self.sha), patch.object(package.SETUP, "SOURCES", self.sources)]
        for change in self.patches:
            change.start()
        (self.source / "dinov3_vits16.onnx").write_bytes(self.model)
        (self.source / "LICENSE-DINOv3.md").write_bytes(self.license)
        metadata = {"sha256": self.sha, "bytes": len(self.model), "source_revision": package.SETUP.REVISION, "source_repository": package.SETUP.REPOSITORY, "base_model": "facebook/dinov3-vits16-pretrain-lvd1689m", "sources": {name: {"bytes": size, "sha256": digest} for name, (size, digest) in self.sources.items()}}
        (self.source / "dinov3_vits16.metadata.json").write_text(json.dumps(metadata))

    def tearDown(self):
        for change in reversed(self.patches):
            change.stop()
        self.temporary.cleanup()

    def test_bundle_has_only_explicit_payload_and_valid_hashes(self):
        (self.source / "scrfd_500m.onnx").write_bytes(b"private")
        output = self.root / "offline"
        result = package.package(self.source, output)
        manifest = json.loads((output / "checksums.json").read_text())
        self.assertEqual(len(manifest["files"]), 5)
        for entry in manifest["files"]:
            file = output / entry["path"]
            self.assertEqual(hashlib.sha256(file.read_bytes()).hexdigest(), entry["sha256"])
            self.assertEqual(file.stat().st_size, entry["bytes"])
        with zipfile.ZipFile(result["archive"]) as archive:
            self.assertEqual(set(archive.namelist()), {entry["path"] for entry in manifest["files"]} | {"checksums.json"})
        self.assertFalse((output / "models/optional/scrfd_500m.onnx").exists())
        self.assertIn("does not enable", (output / "README.md").read_text())

    def test_modified_license_rejected_before_output(self):
        (self.source / "LICENSE-DINOv3.md").write_bytes(b"changed-license")
        output = self.root / "offline"
        with self.assertRaisesRegex(ValueError, "license"):
            package.package(self.source, output)
        self.assertFalse(output.exists())

    def test_wrong_provenance_and_corrupt_model_rejected(self):
        path = self.source / "dinov3_vits16.metadata.json"
        metadata = json.loads(path.read_text())
        metadata["source_revision"] = "0" * 40
        path.write_text(json.dumps(metadata))
        with self.assertRaisesRegex(ValueError, "provenance"):
            package.package(self.source, self.root / "offline")
        (self.source / "dinov3_vits16.onnx").write_bytes(b"x" + self.model[1:])
        with self.assertRaisesRegex(ValueError, "artifact mismatch"):
            package.package(self.source, self.root / "offline")
        self.assertFalse((self.root / "offline").exists())

    def test_existing_delivery_never_replaced(self):
        output = self.root / "offline"
        output.mkdir()
        marker = output / "keep.txt"
        marker.write_text("keep")
        with self.assertRaises(FileExistsError):
            package.package(self.source, output)
        self.assertEqual(marker.read_text(), "keep")

    def test_gpu_artifact_explicit_and_corruption_rejected(self):
        data = b"fixture-gpu-graph"
        sha = hashlib.sha256(data).hexdigest()
        graph = self.source / package.GPU.FILE
        graph.write_bytes(data)
        graph.with_suffix(".metadata.json").write_text(json.dumps({"sha256": sha, "bytes": len(data), "original_sha256": self.sha}))
        with patch.object(package.GPU, "SIZE", len(data)), patch.object(package.GPU, "SHA256", sha):
            output = self.root / "gpu"
            result = package.package(self.source, output, make_zip=False, include_directml=True)
            self.assertEqual(result["files"], 7)
            self.assertEqual((output / "models/optional" / package.GPU.FILE).read_bytes(), data)
            graph.write_bytes(b"x" + data[1:])
            with self.assertRaisesRegex(ValueError, "DirectML artifact"):
                package.package(self.source, self.root / "bad-gpu", include_directml=True)
            self.assertFalse((self.root / "bad-gpu").exists())


if __name__ == "__main__":
    unittest.main()
