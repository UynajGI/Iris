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
            versioning.SCHEMA,
            versioning.API_TYPES,
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
            schema = json.loads((root / versioning.SCHEMA).read_text("utf-8"))
            self.assertEqual(schema["info"]["version"], "1.2.3-beta.2")
            original_schema = json.loads((TOOLS.parent / versioning.SCHEMA).read_text("utf-8"))
            schema["info"]["version"] = original_schema["info"]["version"]
            self.assertEqual(schema, original_schema)
            original_types = (TOOLS.parent / versioning.API_TYPES).read_text("utf-8").splitlines()
            updated_types = (root / versioning.API_TYPES).read_text("utf-8").splitlines()
            self.assertEqual(original_types[2:], updated_types[2:])
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

    def test_contract_version_and_fingerprint_drift_are_detected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.fixture(root)
            version = versioning.current(root)
            versioning.sync(version, root)
            schema_path = root / versioning.SCHEMA
            original = schema_path.read_text("utf-8")
            schema = json.loads(original)
            schema["info"]["version"] = "0.0.0"
            schema_path.write_text(json.dumps(schema), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "openapi.json"):
                versioning.check(version, root)
            schema_path.write_text(original, encoding="utf-8")
            types = root / versioning.API_TYPES
            types.write_text(
                types.read_text("utf-8").replace("// Contract SHA-256:", "// stale:"),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "contract hash"):
                versioning.check(version, root)
            before = (root / "Cargo.toml").read_bytes()
            with self.assertRaisesRegex(ValueError, "regenerate stale"):
                versioning.sync("1.2.3", root)
            self.assertEqual(before, (root / "Cargo.toml").read_bytes())
