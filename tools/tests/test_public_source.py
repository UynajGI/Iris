"""Public source export must not accidentally include local/private material."""
import importlib.util
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("source_package", ROOT / "tools/package-source.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PublicSourceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="iris-public-source-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "repo"
        self.root.mkdir()
        self.git("init", "-q")
        self.git("config", "user.name", "Source export test")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "core.hooksPath", str(self.root / "no-hooks"))
        for file in ("LICENSE", "THIRD_PARTY_NOTICES.md", "README.md", "docs/licensing.md"):
            self.write(file, "Test fixture\n")
        self.write(".gitignore", "artifacts/\n")
        self.commit()

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, stderr=subprocess.DEVNULL)

    def write(self, file, text):
        target = self.root / file
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")

    def commit(self):
        self.git("add", "-A")
        self.git("-c", "commit.gpgsign=false", "commit", "-qm", "test fixture")

    def test_export_contains_only_committed_public_tree_and_rejects_dirty_sources(self):
        self.write("artifacts/private.txt", "local-only")
        output = Path(self.temp.name) / "source.zip"
        revision = module.package(self.root, output)
        self.assertEqual(revision, self.git("rev-parse", "HEAD").decode().strip())
        with zipfile.ZipFile(output) as archive:
            self.assertEqual(archive.read("Iris/README.md"), self.git("show", "HEAD:README.md"))
            self.assertFalse(any("private" in x or ".git/" in x for x in archive.namelist()))
        self.write("README.md", "changed\n")
        with self.assertRaisesRegex(ValueError, "clean working tree"):
            module.package(self.root, Path(self.temp.name) / "dirty.zip")

    def test_private_tracked_file_and_missing_public_link_block_export(self):
        self.write("reference/notes.md", "private reference")
        self.write("README.md", "[private evidence](artifacts/private.txt)\n")
        self.commit()
        with self.assertRaisesRegex(ValueError, "Public source check failed"):
            module.package(self.root, Path(self.temp.name) / "rejected.zip")
        self.assertFalse((Path(self.temp.name) / "rejected.zip").exists())

    def test_normalized_line_endings_export_but_staged_and_untracked_changes_do_not(self):
        self.git("config", "core.autocrlf", "true")
        self.write(".gitattributes", "* text=auto\n")
        self.write("notice.txt", "License fixture\n")
        self.commit()
        (self.root / "notice.txt").unlink()
        self.git("checkout", "--", "notice.txt")
        (self.root / "notice.txt").write_bytes(b"License fixture\n")
        module.package(self.root, Path(self.temp.name) / "normalized.zip")
        self.write("notice.txt", "Actual change\n")
        self.git("add", "notice.txt")
        with self.assertRaisesRegex(ValueError, "clean working tree"):
            module.package(self.root, Path(self.temp.name) / "staged.zip")
        self.git("reset", "--hard", "HEAD")
        self.write("untracked.txt", "Not approved for export\n")
        with self.assertRaisesRegex(ValueError, "clean working tree"):
            module.package(self.root, Path(self.temp.name) / "untracked.zip")

    def test_gitlink_blocks_export(self):
        revision = self.git("rev-parse", "HEAD").decode().strip()
        self.git("update-index", "--add", "--cacheinfo", "160000," + revision + ",external")
        self.git("-c", "commit.gpgsign=false", "commit", "-qm", "gitlink fixture")
        # Git may report the absent checkout as dirty; either check must refuse.
        with self.assertRaises(ValueError):
            module.package(self.root, Path(self.temp.name) / "gitlink.zip")

    def test_public_check_logs_location_but_never_credential_or_dynamic_rule_label(self):
        fixture_value = "ghp_" + "A" * 36
        self.write("fixture.txt", fixture_value + "\n")
        self.git("add", "fixture.txt")
        tools = self.root / "tools"
        tools.mkdir()
        for name in ("check-public-tree.py", "check-staged.py"):
            shutil.copy2(ROOT / "tools" / name, tools / name)
        result = subprocess.run([sys.executable, str(tools / "check-public-tree.py")],
                                cwd=self.root, capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 1)
        self.assertIn("fixture.txt: possible credential material (value omitted)", result.stderr)
        self.assertNotIn(fixture_value, result.stdout + result.stderr)
        self.assertNotIn("GitHub token", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
