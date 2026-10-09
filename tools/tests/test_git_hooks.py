"""Real Git commits in disposable repositories exercise the installed Lefthook gates."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
LEFTHOOK = ROOT / "node_modules/.bin" / ("lefthook.cmd" if os.name == "nt" else "lefthook")


@unittest.skipUnless(LEFTHOOK.exists() and shutil.which("node"), "install root Git tooling with npm ci")
class GitHookIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="iris-git-hooks-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.env = {k: v for k, v in os.environ.items() if not k.startswith(("GIT_", "LEFTHOOK"))}
        self.env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull, NO_COLOR="1")
        self.git("init", "--initial-branch=main")
        self.git("config", "user.name", "Hook fixture")
        self.git("config", "user.email", "hook-fixture@example.invalid")
        (self.root / "tools").mkdir()
        shutil.copy2(ROOT / "tools/check-staged.py", self.root / "tools/check-staged.py")
        shutil.copy2(ROOT / ".gitignore", self.root / ".gitignore")
        config = (ROOT / "lefthook.yml").read_text("utf-8")
        entry = (ROOT / "node_modules/lefthook/bin/index.js").as_posix()
        config = config.replace("node node_modules/lefthook/bin/index.js", f'node "{entry}"')
        cli = (ROOT / "node_modules/@commitlint/cli/cli.js").as_posix()
        rules = (ROOT / "commitlint.config.cjs").as_posix()
        config = config.replace("node node_modules/@commitlint/cli/cli.js", f'node "{cli}" --config "{rules}"')
        (self.root / "lefthook.yml").write_text(config, encoding="utf-8")
        subprocess.run([str(LEFTHOOK), "install"], cwd=self.root, env=self.env,
                       capture_output=True, check=True, timeout=30)

    def git(self, *args, check=True):
        return subprocess.run(["git", *args], cwd=self.root, env=self.env, check=check,
                              text=True, encoding="utf-8", errors="replace", capture_output=True, timeout=60)

    def stage(self, name, content, force=False):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        self.git("add", *( ["-f"] if force else []), "--", name)
        return path

    def commit(self, message="test(hooks): verify staged input"):
        return self.git("commit", "-m", message, check=False)

    def assert_rejected(self, result, reason, previous_head=None):
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(reason, result.stdout + result.stderr)
        head = self.git("rev-parse", "--verify", "HEAD", check=False)
        if previous_head is None:
            self.assertNotEqual(head.returncode, 0)
        else:
            self.assertEqual(head.stdout.strip(), previous_head)

    def seed_commit(self):
        # Lefthook needs HEAD to temporarily hide partially staged changes.
        self.stage("seed.json", b'{}\n')
        result = self.commit("test(hooks): initialize partial staging fixture")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return self.git("rev-parse", "HEAD").stdout.strip()

    def test_valid_commit_runs_both_installed_hooks(self):
        self.stage("good.json", b'{"ok":true}\n')
        result = self.commit("feat(hooks): 验证正常提交")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("pre-commit", result.stdout + result.stderr)
        self.assertIn("commit-msg", result.stdout + result.stderr)

    def test_invalid_message_is_rejected_by_commitlint(self):
        self.stage("good.py", b"print('ok')\n")
        self.assert_rejected(self.commit("WIP"), "type-empty")

    def test_overlong_subject_is_rejected(self):
        self.stage("good.py", b"print('ok')\n")
        self.assert_rejected(self.commit("feat: " + "x" * 100), "header-max-length")

    def test_private_artifact_cannot_be_force_added(self):
        self.stage("artifacts/private.json", b'{"fixture":true}\n', force=True)
        self.assert_rejected(self.commit(), "generated/runtime/private directory")

    def test_staged_syntax_is_checked_even_after_worktree_fix(self):
        previous_head = self.seed_commit()
        path = self.stage("broken.py", b"def broken(:\n")
        path.write_text("print('fixed but not staged')\n", encoding="utf-8")
        self.assert_rejected(self.commit(), "invalid staged syntax", previous_head)
        self.assertEqual(path.read_text("utf-8"), "print('fixed but not staged')\n")
        self.assertEqual(self.git("show", ":broken.py").stdout, "def broken(:\n")

    def test_unstaged_syntax_does_not_change_a_valid_snapshot(self):
        self.seed_commit()
        path = self.stage("good.py", b"print('staged')\n")
        path.write_text("def broken(:\n", encoding="utf-8")
        result = self.commit()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.git("show", "HEAD:good.py").stdout, "print('staged')\n")
        self.assertIn("good.py", self.git("status", "--short").stdout)
        self.assertEqual(path.read_text("utf-8"), "def broken(:\n")

    def test_private_key_marker_is_rejected_without_printing_contents(self):
        fake = b"-----BEGIN " + b"PRIVATE KEY-----\nnot-a-real-key\n"
        self.stage("credentials.txt", fake)
        result = self.commit()
        self.assert_rejected(result, "possible private key")
        self.assertNotIn("not-a-real-key", result.stdout + result.stderr)

    def test_large_blob_is_rejected(self):
        self.stage("large.txt", b"x" * (5 * 1024 * 1024 + 1))
        self.assert_rejected(self.commit(), "5 MiB source-file limit")


if __name__ == "__main__":
    unittest.main()
