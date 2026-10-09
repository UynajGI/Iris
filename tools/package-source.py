"""Export the exact clean Git HEAD for an Iris application source release.

Third-party runtime/dependency source obligations are reviewed separately.
"""
from __future__ import annotations

import argparse
import importlib.util
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def package(root: Path, output: Path) -> str:
    def git(*args):
        return subprocess.check_output(["git", *args], cwd=root)

    if output.exists():
        raise ValueError("Output must be new")
    # On Windows, downloaded LF notices can appear modified in porcelain status
    # after a CRLF checkout even though Git's normalized content is identical.
    # Compare both actual content diffs and untracked files; still reject every
    # real staged/unstaged change instead of relaxing the source export gate.
    dirty = False
    for args in [("diff", "--quiet"), ("diff", "--cached", "--quiet")]:
        result = subprocess.run(["git", *args], cwd=root, check=False)
        if result.returncode not in (0, 1):
            raise RuntimeError("Git source comparison failed")
        dirty |= result.returncode == 1
    if dirty or git("ls-files", "--others", "--exclude-standard").strip():
        status = git("status", "--porcelain", "--untracked-files=normal").decode("utf-8", errors="replace").strip()
        raise ValueError("Source export requires a clean working tree and index:\n" + status)
    spec = importlib.util.spec_from_file_location("public_tree", ROOT / "tools/check-public-tree.py")
    guard = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(guard)
    issues = guard.inspect(root)
    if issues:
        raise ValueError("Public source check failed: " + "; ".join(issues))
    revision = git("rev-parse", "HEAD").decode().strip()
    output.parent.mkdir(parents=True, exist_ok=True)
    # Exclusive creation; export only committed blobs, never recurse over local files.
    with output.open("xb") as archive:
        subprocess.run(["git", "-c", "core.autocrlf=false", "-c", "core.eol=lf",
                        "archive", "--format=zip", "--prefix=Iris/", revision],
                       cwd=root, stdout=archive, check=True)
    return revision


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print("Application source commit: " + package(ROOT, args.output.resolve()))
