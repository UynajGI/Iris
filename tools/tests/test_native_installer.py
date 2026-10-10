"""Native installer preflight and complete-release-set contracts (no installation)."""
import hashlib
import importlib.util
import json
import plistlib
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[1]


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, TOOLS / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


native = load("native_installer", "package-native-installer.py")
release = load("release_assets", "collect-release-assets.py")


class NativeInstallerTests(unittest.TestCase):
    def fixture(self, root, mac=False):
        prefix = "Iris.app/Contents/MacOS/" if mac else ""
        names = {prefix + p if p.startswith(("iris-", "models/")) else p for p in native.REQUIRED}
        names.add(prefix + ("models/libonnxruntime.dylib" if mac else "models/libonnxruntime.so"))
        if mac:
            names.add("Iris.app/Contents/Info.plist")
        entries = []
        for name in sorted(names):
            file = root / name
            file.parent.mkdir(parents=True, exist_ok=True)
            if name == "release.json":
                version = json.loads((native.ROOT / "apps/shell/package.json").read_text())["version"]
                file.write_text(json.dumps({"product": "Iris", "version": version,
                                           "platform": "macos-arm64" if mac else "linux-x64"}))
            else:
                file.write_bytes(name.encode())
            entries.append({"path": name, "sha256": native.digest(file)})
        (root / "checksums.json").write_text(json.dumps(entries), encoding="utf-8")
        return entries

    def test_both_platform_layouts_include_all_required_sources_and_models(self):
        for mac in (False, True):
            with self.subTest(mac=mac), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                entries = self.fixture(root, mac)
                self.assertEqual(entries, native.validate(root, mac))

    def test_tampered_payload_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.fixture(root)
            (root / "iris-daemon").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                native.validate(root, False)

    def test_manifested_private_and_optional_files_are_rejected(self):
        for name in ("private.jpg", "sources/private.zip", "models/optional/dinov3.onnx", "models/unapproved.onnx", "photos.sqlite3"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                entries = self.fixture(root)
                file = root / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(b"private fixture")
                entries.append({"path": name, "sha256": native.digest(file)})
                (root / "checksums.json").write_text(json.dumps(entries))
                with self.assertRaises(ValueError):
                    native.validate(root, False)

    def test_invalid_duplicate_and_missing_paths_are_rejected(self):
        for path in ("../escape", "/absolute", "C:/absolute", "models\\escape", "./iris-shell"):
            with self.subTest(path=path), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                entries = self.fixture(root)
                entries[0]["path"] = path
                (root / "checksums.json").write_text(json.dumps(entries))
                with self.assertRaisesRegex(ValueError, "Invalid"):
                    native.validate(root, False)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            entries = self.fixture(root)
            for changed in (entries + [entries[0]], entries[1:]):
                (root / "checksums.json").write_text(json.dumps(changed))
                with self.assertRaises(ValueError):
                    native.validate(root, False)

    def test_unlisted_private_files_never_enter_staging(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            portable = root / "portable"
            portable.mkdir()
            entries = self.fixture(portable)
            (portable / "private.jpg").write_bytes(b"private")
            native.copy_entries(portable, root / "staged", entries)
            self.assertFalse((root / "staged/private.jpg").exists())

    def test_macos_installer_ui_resources_and_architecture_are_bound(self):
        for arch in ("arm64", "x86_64"):
            dist = ET.fromstring(native.distribution("0.1.0-beta", arch))
            self.assertEqual(dist.find("options").get("hostArchitectures"), arch)
            self.assertEqual(dist.find("domains").get("enable_localSystem"), "true")
            for tag in ("welcome", "conclusion"):
                path = native.ROOT / "apps/shell/installer/macos" / dist.find(tag).get("file")
                self.assertTrue(path.is_file())
                self.assertNotIn("<script", path.read_text(encoding="utf-8").lower())
            self.assertEqual(dist.find("license").get("mime-type"), "text/plain")

    def test_debian_prerelease_and_system_dependencies(self):
        control = native.deb_control("0.1.0-beta", 123)
        self.assertIn("Version: 0.1.0~beta\n", control)
        self.assertIn("libwebkit2gtk-4.1-0", control)
        self.assertIn("Installed-Size: 123\n", control)

    def test_complete_release_set_is_required_before_checksums(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            expected = release.expected_assets("0.1.0-beta")
            self.assertEqual(len(expected), 9)
            for name in expected:
                (root / name).write_bytes(b"test artifact")
            missing = next(iter(expected))
            (root / missing).unlink()
            with self.assertRaises(ValueError):
                release.collect(root, "0.1.0-beta")
            self.assertFalse((root / "SHA256SUMS.txt").exists())
            (root / missing).write_bytes(b"test artifact")
            self.assertEqual(release.collect(root, "0.1.0-beta"), expected)
            lines = (root / "SHA256SUMS.txt").read_text().splitlines()
            self.assertEqual(len(lines), 9)
            self.assertTrue(all(line.startswith(hashlib.sha256(b"test artifact").hexdigest()) for line in lines))
            with self.assertRaises(ValueError):
                release.collect(root, "0.1.0-beta")

    def test_unknown_artifacts_fail_instead_of_being_published(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in release.expected_assets("0.1.0-beta") | {"private-report.json"}:
                (root / name).write_bytes(b"test")
            with self.assertRaisesRegex(ValueError, "unexpected"):
                release.collect(root, "0.1.0-beta")

    def test_native_build_orchestration_preserves_payload_and_embeds_ui(self):
        # Native system tools are simulated here. Real PKG/DEB extraction and
        # inference remain mandatory on their respective CI runners.
        for mac in (False, True):
            with self.subTest(mac=mac), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                portable = root / "portable"
                portable.mkdir()
                entries = self.fixture(portable, mac)
                calls = []
                package_root = None

                def fake_run(*args):
                    nonlocal package_root
                    calls.append(args)
                    if args[0] == sys.executable:
                        bundle = Path(args[args.index("--bundle") + 1])
                        for entry in json.loads((bundle / "checksums.json").read_text()):
                            self.assertEqual(native.digest(bundle / entry["path"]), entry["sha256"])
                    elif args[0] == "pkgbuild":
                        if "--analyze" in args:
                            Path(args[-1]).write_bytes(plistlib.dumps([{"RootRelativeBundlePath": "Applications/Iris.app"}]))
                        else:
                            package_root = Path(args[args.index("--root") + 1])
                            props = plistlib.loads(Path(args[args.index("--component-plist") + 1]).read_bytes())
                            self.assertFalse(props[0]["BundleIsRelocatable"])
                            Path(args[-1]).write_bytes(b"component pkg fixture")
                    elif args[0] == "productbuild":
                        dist = ET.fromstring(Path(args[args.index("--distribution") + 1]).read_text(encoding="utf-8"))
                        self.assertEqual(dist.find("options").get("hostArchitectures"), "arm64")
                        Path(args[-1]).write_bytes(b"product pkg fixture")
                    elif args[0] == "pkgutil":
                        destination = Path(args[-1])
                        shutil.copytree(package_root, destination / "iris-component.pkg/Payload")
                        shutil.copytree(Path(args[-2]).parent / "resources", destination / "Resources")
                    elif args[0] == "dpkg-deb":
                        if "--build" in args:
                            package_root = Path(args[-2])
                            Path(args[-1]).write_bytes(b"deb fixture")
                        else:
                            shutil.copytree(package_root, Path(args[-1]))
                    elif args[0] == "desktop-file-validate":
                        self.assertIn("Exec=iris", Path(args[-1]).read_text(encoding="utf-8"))
                    else:
                        self.fail(f"Unexpected native tool: {args[0]}")

                with patch.object(native.platform, "system", return_value="Darwin" if mac else "Linux"), \
                     patch.object(native.platform, "machine", return_value="arm64" if mac else "x86_64"), \
                     patch.object(native, "run", side_effect=fake_run):
                    native.build(portable, root / "output")
                report = json.loads(next((root / "output").glob("*-build-report.json")).read_text())
                self.assertTrue(report["extracted_payload_verified"])
                self.assertFalse(report["installer_executed"])
                self.assertFalse(report["signed"])
                self.assertEqual(report["sha256"], native.digest(root / "output" / report["artifact"]))
                self.assertEqual(sum("--bundle" in call for call in calls), 2)
                for entry in entries:
                    self.assertEqual(native.digest(portable / entry["path"]), entry["sha256"])


if __name__ == "__main__":
    unittest.main()
