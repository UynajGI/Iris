import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "complexity", Path(__file__).resolve().parents[1] / "check-complexity.py"
)
complexity = importlib.util.module_from_spec(SPEC)
with patch.dict("sys.modules", {"lizard": SimpleNamespace()}):
    SPEC.loader.exec_module(complexity)


class ComplexityTests(unittest.TestCase):
    def test_explicit_first_party_roots_and_extensions(self):
        for path in ["apps/shell/src/ui/App.tsx", "crates/iris-core/src/lib.rs", "tools/build.mjs"]:
            self.assertTrue(complexity.source_path(path))
        for path in ["models/runtime.py", ".tools/check.py", "node_modules/code.ts", "apps/shell/dist/app.js", "tools/report.json"]:
            self.assertFalse(complexity.source_path(path))

    def test_scope_classification(self):
        self.assertEqual(complexity.area("apps/shell/src/ui/App.tsx"), "frontend")
        self.assertEqual(complexity.area("apps/shell/src-tauri/src/lib.rs"), "rust")
        self.assertEqual(complexity.area("tools/tests/example.py"), "tools-and-tests")

    def test_warning_thresholds_are_strict_and_combined(self):
        functions = [SimpleNamespace(name=name, start_line=i, nloc=nloc, cyclomatic_complexity=ccn)
                     for i, (name, ccn, nloc) in enumerate([
                         ("at_limit", 15, 100), ("complex", 16, 10), ("long", 1, 101)
                     ], 1)]
        reader = SimpleNamespace(analyze_source_code=lambda *_: SimpleNamespace(function_list=functions))
        with patch.object(complexity.lizard, "analyze_file", reader, create=True):
            result = complexity.analyze([("apps/shell/src/a.ts", "source")], 15, 100, "frontend")
        self.assertEqual(result["functions"], 3)
        self.assertEqual(result["warning_count"], 2)
        self.assertEqual([f["name"] for f in result["warnings"]], ["complex", "long"])

    def test_unselected_sources_are_not_analyzed(self):
        result = complexity.analyze([("crates/core/src/lib.rs", "source")], 15, 100, "frontend")
        self.assertEqual(result["files"], 0)
        self.assertEqual(result["functions"], 0)

    def test_current_scan_includes_nonignored_new_sources_and_deduplicates(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "tools").mkdir()
            (root / "tools/new.py").write_text("def new(): pass", encoding="utf-8")
            names = b"tools/new.py\0tools/new.py\0tools/deleted.py\0artifacts/report.py\0"
            with patch.object(complexity, "ROOT", root), patch.object(complexity, "git", return_value=names):
                self.assertEqual(list(complexity.sources()), [("tools/new.py", "def new(): pass")])

    def test_baseline_is_read_without_checkout(self):
        calls = []
        def git(*args):
            calls.append(args)
            return {"rev-parse": b"abcd\n", "ls-tree": b"tools/old.py\0", "show": b"old source"}[args[0]]
        with patch.object(complexity, "git", git):
            self.assertEqual(list(complexity.sources("saved")), [("tools/old.py", "old source")])
        self.assertEqual(calls, [("rev-parse", "--verify", "saved^{commit}"),
                                 ("ls-tree", "-r", "--name-only", "-z", "abcd"),
                                 ("show", "abcd:tools/old.py")])

    def test_reports_cannot_overwrite_source_or_escape_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(complexity, "ROOT", Path(temporary)):
            for destination in ["tools/source.py", "artifacts/../README.md"]:
                with self.assertRaises(ValueError):
                    complexity.save_report(destination, {})
            complexity.save_report("artifacts/complexity/result.json", {"count": 1})
            self.assertIn('"count": 1', (Path(temporary) / "artifacts/complexity/result.json").read_text())


if __name__ == "__main__":
    unittest.main()
