"""Version updates preserve dependencies and validate nested application fields."""

import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import versioning


class VersioningTests(unittest.TestCase):
    def fixture(self, destination):
        root = TOOLS.parent
        for name in (
            *versioning.MANIFESTS,
            *versioning.JSON_FILES,
            *versioning.LOCKS,
            versioning.GENERATED,
            "README.md",
        ):
            path = destination / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(root / name, path)

    def test_sync_updates_all_application_fields_and_preserves_dependencies(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.fixture(root)
            original = json.loads((root / "apps/shell/package-lock.json").read_text())
            versioning.sync("1.2.3-beta.2", root)
            self.assertEqual(versioning.check("1.2.3-beta.2", root), "1.2.3-beta.2")
            changed = json.loads((root / "apps/shell/package-lock.json").read_text())
            self.assertEqual(changed["packages"][""]["version"], "1.2.3-beta.2")
            for key, value in original["packages"].items():
                if key:
                    self.assertEqual(changed["packages"][key], value)
            self.assertEqual(versioning.sync("1.2.3-beta.2", root), [])

    def test_nested_lock_version_drift_is_detected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.fixture(root)
            path = root / "apps/shell/package-lock.json"
            data = json.loads(path.read_text())
            data["packages"][""]["version"] = "9.9.9"
            path.write_text(json.dumps(data))
            with self.assertRaisesRegex(ValueError, "package-lock"):
                versioning.check(versioning.current(root), root)

    def test_invalid_versions_do_not_write_files(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.fixture(root)
            before = (root / "Cargo.toml").read_bytes()
            for value in ["main", "v1.0.0", "01.2.3", "1.0.0-01", "1.0.0\nBAD"]:
                with self.assertRaises(ValueError):
                    versioning.sync(value, root)
            self.assertEqual(before, (root / "Cargo.toml").read_bytes())
