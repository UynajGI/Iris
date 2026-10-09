"""Install Lefthook and a message template using repository-local configuration."""
from pathlib import Path
import os
import shutil
import subprocess


def main():
    root = Path(__file__).resolve().parents[1]
    git_root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], cwd=root, text=True).strip())
    if git_root.resolve() != root:
        raise SystemExit("Run from an Iris checkout with its own Git repository")
    for name in ["lefthook.yml", "commitlint.config.cjs", ".gitmessage", "tools/check-staged.py"]:
        if not (root / name).is_file():
            raise SystemExit(f"Missing hook prerequisite: {name}")
    npm = shutil.which("npm.cmd" if os.name == "nt" else "npm")
    if npm is None or not (root / "node_modules/lefthook/package.json").is_file():
        raise SystemExit("Install the root development dependencies with npm ci first")
    subprocess.run([npm, "exec", "--no", "--", "lefthook", "install"], cwd=root, check=True)
    subprocess.run(["git", "config", "--local", "commit.template", ".gitmessage"], cwd=root, check=True)
    print("Installed Lefthook pre-commit/commit-msg hooks and the local commit template.")


if __name__ == "__main__":
    main()
