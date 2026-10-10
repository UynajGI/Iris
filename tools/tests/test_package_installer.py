"""Installer preflight and payload tests; no compiler or installer is run."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PWSH = shutil.which("pwsh")


@unittest.skipUnless(os.name == "nt" and PWSH, "Windows PowerShell 7 required")
class InstallerPreflightTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="iris-installer-preflight-")
        self.root = Path(self.temporary.name)
        self.portable = self.root / "portable"
        self.portable.mkdir()
        self.output = self.root / "output"
        self.files = []
        for name in ("iris-shell.exe", "iris-daemon.exe", "iris-cli.exe",
                     "iris-mcp.exe", "iris-raw-decoder.exe", "sources/raw-decoder-source.zip",
                     "sources/iris-application-source.zip", "sources/dependency-sources.zip", "LICENSE", "THIRD_PARTY_NOTICES.md",
                     "WebView2Loader.dll", "models/manifest.json", "models/onnxruntime.dll"):
            self.add_file(name, b"non-executable test fixture")
        self.write_manifest()

    def tearDown(self):
        self.temporary.cleanup()

    def add_file(self, name, data):
        path = self.portable / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        self.files.append({"path": name, "bytes": len(data),
                           "sha256": hashlib.sha256(data).hexdigest()})

    def write_manifest(self):
        version = json.loads((ROOT / "apps/shell/package.json").read_text())["version"]
        (self.portable / "checksums.json").write_text(json.dumps({
            "configuration": "Release", "version": version, "files": self.files}))

    def run_preflight(self, *extra):
        result = subprocess.run([PWSH, "-NoProfile", "-File",
                                 str(ROOT / "tools/package-installer.ps1"),
                                 "-PortableDirectory", str(self.portable),
                                 "-OutputDirectory", str(self.output), "-PrepareOnly", *extra],
                                text=True, encoding="utf-8", capture_output=True, timeout=30)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertFalse(self.output.exists(), "Rejected input must not publish output")
        return result.stdout + result.stderr

    def test_changed_binary_is_rejected_before_staging(self):
        (self.portable / "iris-daemon.exe").write_bytes(b"changed executable")
        self.assertIn("checksum mismatch", self.run_preflight())

    def test_raw_component_and_source_must_both_be_manifested(self):
        original = self.files.copy()
        for name in ("iris-mcp.exe", "iris-raw-decoder.exe", "sources/raw-decoder-source.zip",
                     "sources/iris-application-source.zip", "sources/dependency-sources.zip", "LICENSE", "THIRD_PARTY_NOTICES.md"):
            with self.subTest(name=name):
                # The file still exists, but unmanifested files cannot be shipped.
                self.files = [entry for entry in original if entry["path"] != name]
                self.write_manifest()
                self.assertIn(f"Missing required package file: {name}", self.run_preflight())

    def test_raw_component_and_source_tampering_are_rejected(self):
        for name in ("iris-raw-decoder.exe", "sources/raw-decoder-source.zip"):
            with self.subTest(name=name):
                path = self.portable / name
                original = path.read_bytes()
                try:
                    # Same length: rejection must also cover the SHA-256 check.
                    path.write_bytes(b"x" * len(original))
                    self.assertIn(f"Portable checksum mismatch: {name}", self.run_preflight())
                finally:
                    path.write_bytes(original)

    def test_staging_preserves_approved_sources_and_notices_without_unlisted_sources(self):
        self.add_file("sources/main-application-source.zip", b"private source fixture")
        self.add_file("apps/shell/src/private.ts", b"private TypeScript fixture")
        self.write_manifest()
        result = subprocess.run([PWSH, "-NoProfile", "-File",
                                 str(ROOT / "tools/package-installer.ps1"),
                                 "-PortableDirectory", str(self.portable),
                                 "-OutputDirectory", str(self.output), "-PrepareOnly"],
                                text=True, encoding="utf-8", capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        preparation = json.loads((self.output / "preparation.json").read_text("utf-8-sig"))
        stage = Path(preparation["stage"]).resolve()
        # The script stages outside TemporaryDirectory; constrain cleanup to its
        # exact generated directory under this checkout's artifacts folder.
        self.assertEqual(stage.parent, (ROOT / "artifacts").resolve())
        self.assertRegex(stage.name, r"^installer-[0-9a-f]{32}$")
        self.addCleanup(shutil.rmtree, stage)
        config = json.loads(Path(preparation["config"]).read_text("utf-8-sig"))
        resources = config["bundle"]["resources"]
        targets = set(resources.values())
        self.assertIn("iris-raw-decoder.exe", targets)
        self.assertIn("sources/raw-decoder-source.zip", targets)
        self.assertIn("sources/iris-application-source.zip", targets)
        self.assertIn("sources/dependency-sources.zip", targets)
        self.assertIn("iris-mcp.exe", targets)
        self.assertIn("LICENSE", targets)
        self.assertIn("THIRD_PARTY_NOTICES.md", targets)
        self.assertNotIn("sources/main-application-source.zip", targets)
        self.assertNotIn("apps/shell/src/private.ts", targets)
        for staged, relative in resources.items():
            if relative == "INSTALLATION.txt":
                continue
            self.assertEqual(Path(staged).read_bytes(), (self.portable / relative).read_bytes())
        for entry in self.files:
            self.assertEqual(hashlib.sha256((self.portable / entry["path"]).read_bytes()).hexdigest(),
                             entry["sha256"], "Staging must preserve the source portable")

    def test_parent_traversal_and_absolute_manifest_paths_are_rejected(self):
        for path in ("../outside.exe", str(self.root / "outside.exe")):
            with self.subTest(path=path):
                self.files[0]["path"] = path
                self.write_manifest()
                self.assertIn("Invalid manifest path", self.run_preflight())

    def test_runtime_update_endpoint_cannot_be_added_to_offline_package(self):
        self.assertIn("require -EnableUpdater", self.run_preflight(
            "-UpdateEndpoint", "https://updates.invalid/release.json"))

    def test_unsigned_update_build_is_rejected_before_any_output(self):
        public_key = self.root / "public.key"
        public_key.write_text("test-public-key")
        previous = os.environ.pop("TAURI_SIGNING_PRIVATE_KEY", None)
        try:
            self.assertIn("TAURI_SIGNING_PRIVATE_KEY", self.run_preflight(
                "-EnableUpdater", "-UpdateEndpoint", "https://updates.invalid/release.json",
                "-UpdatePublicKeyFile", str(public_key)))
        finally:
            if previous is not None:
                os.environ["TAURI_SIGNING_PRIVATE_KEY"] = previous


if __name__ == "__main__":
    unittest.main()
