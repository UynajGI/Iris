"""Task orchestration must fail early and keep read-only checks read-only."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("dev_tasks", ROOT / "tools/dev.py")
dev = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dev)


class TaskRunnerTests(unittest.TestCase):
    def test_test_group_stops_at_first_failure(self):
        calls = []
        def fail_first(args, **kwargs):
            calls.append(args)
            raise subprocess.CalledProcessError(2, args)
        runner = dev.Runner()
        with patch.object(runner, "run", side_effect=fail_first):
            with self.assertRaises(subprocess.CalledProcessError):
                runner.execute("test")
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][:3], ["cargo", "test", "--workspace"])

    def test_source_requires_destination_before_starting_child(self):
        runner = dev.Runner()
        with patch.object(runner, "run") as child:
            with self.assertRaisesRegex(ValueError, "IRIS_OUTPUT"):
                runner.execute("source")
            child.assert_not_called()

    def test_spaces_in_output_remain_one_argument(self):
        runner = dev.Runner(output="dist/source export/archive.zip")
        with patch.object(runner, "run") as child:
            runner.execute("source")
        args = child.call_args.args[0]
        self.assertEqual(args[-2], "--output")
        self.assertEqual(args[-1], str(Path("dist/source export/archive.zip").resolve()))

    def test_release_build_includes_every_runtime_and_raw_component(self):
        runner = dev.Runner(profile="release")
        with patch.object(runner, "run") as child:
            runner.execute("build-core")
        commands = [c.args[0] for c in child.call_args_list]
        self.assertEqual(len(commands), 2)
        self.assertTrue(all("--release" in c and "--locked" in c for c in commands))
        self.assertIn("--workspace", commands[0])
        self.assertIn("components/raw-decoder/Cargo.toml", commands[1])

    def test_api_check_detects_drift_without_overwriting_source(self):
        with tempfile.TemporaryDirectory(prefix="iris-task-api-") as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            source = root / "docs/openapi.json"
            original = '{"paths": {"/api/v1/bootstrap": {}}, "info": {"version": "old"}}\n'
            source.write_text(original, encoding="utf-8")
            generated = {"paths": {"/api/v1/bootstrap": {}}, "info": {"version": "new"}}
            runner = dev.Runner()
            with patch.object(dev, "ROOT", root), patch.object(runner, "run", return_value=json.dumps(generated)) as child:
                with self.assertRaisesRegex(ValueError, "OpenAPI is stale"):
                    runner.execute("api-check")
                self.assertEqual(child.call_count, 1)
            self.assertEqual(source.read_text(encoding="utf-8"), original)

    def test_api_check_requests_no_write_generator_mode(self):
        with tempfile.TemporaryDirectory(prefix="iris-task-api-") as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            schema = {"paths": {"/api/v1/bootstrap": {}}}
            source = root / "docs/openapi.json"
            source.write_text(json.dumps(schema), encoding="utf-8")
            runner = dev.Runner()
            with patch.object(dev, "ROOT", root), patch.object(runner, "run", side_effect=[json.dumps(schema), ""]) as child:
                runner.execute("api-check")
                self.assertEqual(child.call_args_list[-1].args[0][-1], "--check")
            self.assertEqual(source.read_text(encoding="utf-8"), json.dumps(schema))


if __name__ == "__main__":
    unittest.main()
