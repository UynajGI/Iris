"""Check the complete Git index before publishing; never print secret values."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("staged_policy", ROOT / "tools/check-staged.py")
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


def inspect(root: Path) -> list[str]:
    def git(*args):
        return subprocess.check_output(["git", *args], cwd=root)

    errors = []
    entries = git("ls-files", "--stage", "-z").split(b"\0")
    paths = set()
    documents = {}
    for entry in filter(None, entries):
        header, raw_path = entry.split(b"\t", 1)
        mode, oid, stage = header.split()
        path = raw_path.decode("utf-8")
        paths.add(path)
        if stage != b"0" or mode not in (b"100644", b"100755"):
            errors.append(f"{path}: conflict, symlink or submodule is not permitted")
            continue
        problem = policy.path_problem(path)
        if problem or path == ".gitmodules":
            errors.append(f"{path}: {problem or 'submodule configuration'}")
        size = int(git("cat-file", "-s", oid.decode()))
        if size > policy.MAX_BYTES:
            errors.append(f"{path}: exceeds source size limit")
            continue
        data = git("cat-file", "blob", oid.decode())
        for pattern in policy.SECRET_PATTERNS.values():
            if pattern.search(data):
                # Do not reflect rule labels or matched data into public logs.
                errors.append(f"{path}: possible credential material (value omitted)")
        if path.endswith(".md"):
            documents[path] = data.decode("utf-8-sig")
    for path, content in documents.items():
        for target in re.findall(r"\]\(([^)]+)\)", content):
            target = target.split("#", 1)[0].split("?", 1)[0]
            if not target or re.match(r"[a-z][a-z0-9+.-]*:", target, re.I):
                continue
            resolved = (root / Path(path).parent / target).resolve()
            try:
                relative = resolved.relative_to(root.resolve()).as_posix()
            except ValueError:
                errors.append(f"{path}: link escapes repository")
                continue
            if relative not in paths and not any(p.startswith(relative.rstrip('/') + '/') for p in paths):
                errors.append(f"{path}: link is not in public index: {target}")
    for required in ("LICENSE", "THIRD_PARTY_NOTICES.md", "README.md", "docs/licensing.md"):
        if required not in paths:
            errors.append(f"missing required public file: {required}")
    return errors


if __name__ == "__main__":
    issues = inspect(ROOT)
    if issues:
        print("\n".join(issues), file=sys.stderr)
        raise SystemExit(1)
    print("Public index: regular source files, required notices, relative links and known secret patterns checked.")
