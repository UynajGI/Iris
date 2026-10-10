"""Fast local Git gates. Inspect staged blobs, never rewrite working files."""
from __future__ import annotations

import ast
import json
from pathlib import PurePosixPath
import re
import subprocess
import sys

MAX_BYTES = 5 * 1024 * 1024
DENIED_ROOTS = (
    "test-photos/", "artifacts/", ".iris/", ".iris-mcp/", ".impeccable/",
    ".agents/", ".codegraph/", ".tools/", "reference/", "archive/", "archives/",
    "models/optional/", "models/directml/", "models/media/", "models/raw/",
)
DENIED_PARTS = {"node_modules", "target", "dist", "out", "__pycache__", ".venv"}
DENIED_SUFFIXES = {".exe", ".dll", ".onnx", ".npz", ".sqlite", ".sqlite3", ".db", ".pfx", ".p12", ".pem", ".key", ".pyc", ".log", ".sqlite-wal", ".sqlite-shm", ".sqlite3-wal", ".sqlite3-shm", ".db-wal", ".db-shm"}
SECRET_PATTERNS = {
    "private key": re.compile(rb"(?m)^-----BEGIN (?:(?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY|ENCRYPTED PRIVATE KEY)-----\s*$"),
    "GitHub token": re.compile(rb"\b(?:gh[pousr]_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{60,255})\b"),
    "AWS access key": re.compile(rb"\bAKIA[A-Z0-9]{16}\b"),
    "service API key": re.compile(rb"\bsk-(?:proj-|live-)?[A-Za-z0-9_-]{32,255}\b"),
}


def git(*args: str, input: bytes | None = None) -> bytes:
    return subprocess.check_output(["git", *args], input=input)


def path_problem(path: str) -> str | None:
    normalized = path.replace("\\", "/").lower()
    p = PurePosixPath(normalized)
    if normalized.startswith(DENIED_ROOTS) or DENIED_PARTS.intersection(p.parts):
        return "generated/runtime/private directory"
    if p.name == ".env" or p.name.startswith(".env."):
        return "local environment file"
    if p.suffix in DENIED_SUFFIXES:
        return "binary/runtime/database/signing artifact"
    return None


def staged_blobs():
    changed = {p for p in git("diff", "--cached", "--name-only", "--diff-filter=ACMR", "-z").split(b"\0") if p}
    entries = []
    for entry in git("ls-files", "--stage", "-z").split(b"\0"):
        if not entry:
            continue
        header, name = entry.split(b"\t", 1)
        mode, oid, stage = header.split()
        if stage != b"0":
            raise ValueError("unresolved index conflict")
        if name in changed:
            entries.append((name.decode("utf-8"), mode, oid))
    # Gitlinks are references, not blobs belonging to this repository.
    blobs = [(name, oid) for name, mode, oid in entries if mode != b"160000"]
    if not blobs:
        return
    metadata = git("cat-file", "--batch-check=%(objectname) %(objecttype) %(objectsize)",
                   input=b"\n".join(oid for _, oid in blobs) + b"\n").splitlines()
    for (name, oid), row in zip(blobs, metadata, strict=True):
        _, kind, size = row.split()
        if kind != b"blob":
            raise ValueError(f"{name}: expected a staged blob")
        count = int(size)
        # Reject large files before reading their bytes into the hook process.
        yield name, count, None if count > MAX_BYTES else git("cat-file", "blob", oid.decode("ascii"))


def pre_commit() -> None:
    subprocess.run(["git", "diff", "--cached", "--check"], check=True)
    errors = []
    count = 0
    for name, size, data in staged_blobs():
        count += 1
        problem = path_problem(name)
        if problem:
            errors.append(f"{name}: {problem}")
        if size > MAX_BYTES:
            errors.append(f"{name}: exceeds the 5 MiB source-file limit")
            continue
        assert data is not None
        for label, pattern in SECRET_PATTERNS.items():
            if pattern.search(data):
                errors.append(f"{name}: possible {label} (value omitted)")
        try:
            if name.endswith(".py"):
                ast.parse(data, filename=name)
            elif name.endswith(".json"):
                json.loads(data.decode("utf-8-sig"))
        except (SyntaxError, ValueError, UnicodeError) as error:
            errors.append(f"{name}: invalid staged syntax ({type(error).__name__})")
    if errors:
        raise ValueError("\n".join(errors))
    print(f"pre-commit: checked {count} staged source files")


def main() -> int:
    try:
        pre_commit()
    except (ValueError, subprocess.CalledProcessError) as error:
        print(f"Git hook rejected the commit: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
