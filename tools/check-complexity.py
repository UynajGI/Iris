"""Repeatable, first-party Lizard inventory; not a substitute for code review.

Run with: uv tool run --from lizard==1.24.1 python tools/check-complexity.py
"""

import argparse
import json
import subprocess
from collections import Counter
from importlib.metadata import version
from pathlib import Path, PurePosixPath

import lizard

ROOT = Path(__file__).resolve().parents[1]
LIZARD_VERSION = "1.24.1"
SOURCE_ROOTS = (
    "apps/shell/src/",
    "apps/shell/test/",
    "apps/shell/src-tauri/src/",
    "crates/",
    "components/raw-decoder/src/",
    "tools/",
)
EXTENSIONS = {".rs", ".ts", ".tsx", ".js", ".mjs", ".cjs", ".py"}


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=ROOT)


def source_path(path):
    """Only explicit first-party roots; never traverse ignored runtime directories."""
    return path.startswith(SOURCE_ROOTS) and PurePosixPath(path).suffix in EXTENSIONS


def area(path):
    if path.startswith("apps/shell/src/"):
        return "frontend"
    if path.endswith(".rs"):
        return "rust"
    return "tools-and-tests"


def sources(reference=None):
    if reference:
        # Resolve first so an option-like or nonexistent ref cannot change git's command.
        commit = git("rev-parse", "--verify", f"{reference}^{{commit}}").decode().strip()
        names = git("ls-tree", "-r", "--name-only", "-z", commit)
    else:
        commit = None
        names = git("ls-files", "--cached", "--others", "--exclude-standard", "-z")
    for path in sorted(set(names.decode("utf-8").split("\0"))):
        if not source_path(path):
            continue
        if commit:
            yield path, git("show", f"{commit}:{path}").decode("utf-8-sig")
        elif (ROOT / path).is_file():
            yield path, (ROOT / path).read_text(encoding="utf-8-sig")


def analyze(source_files, ccn, nloc, scope):
    functions = []
    files = 0
    for path, code in source_files:
        if scope != "repo" and area(path) != scope:
            continue
        files += 1
        for function in lizard.analyze_file.analyze_source_code(path, code).function_list:
            functions.append(
                {
                    "path": path,
                    "name": function.name,
                    "line": function.start_line,
                    "ccn": function.cyclomatic_complexity,
                    "nloc": function.nloc,
                    "area": area(path),
                }
            )
    warnings = [f for f in functions if f["ccn"] > ccn or f["nloc"] > nloc]
    warnings.sort(key=lambda f: (-f["ccn"], -f["nloc"], f["path"], f["line"]))
    return {
        "files": files,
        "functions": len(functions),
        "warning_count": len(warnings),
        "warnings_by_area": dict(Counter(f["area"] for f in warnings)),
        "warnings": warnings,
    }


def save_report(path, report):
    destination = (ROOT / path).resolve()
    artifact_root = (ROOT / "artifacts").resolve()
    if not destination.is_relative_to(artifact_root):
        raise ValueError("reports must be saved below ignored artifacts/")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--scope", choices=["repo", "frontend", "rust", "tools-and-tests"], default="repo"
    )
    parser.add_argument("--ccn", type=int, default=15)
    parser.add_argument("--nloc", type=int, default=100)
    parser.add_argument("--baseline", help="compare immutable Git source without checking it out")
    parser.add_argument("--json", help="save details under artifacts/ (never commit reports)")
    parser.add_argument("--top", type=int, default=15)
    parser.add_argument("--fail", action="store_true", help="exit 1 when this scope has warnings")
    args = parser.parse_args()
    if min(args.ccn, args.nloc) < 1 or args.top < 0:
        parser.error("thresholds must be positive and --top must be nonnegative")
    return args


def main():
    if version("lizard") != LIZARD_VERSION:
        raise SystemExit(f"use lizard=={LIZARD_VERSION} for comparable metrics")
    args = arguments()
    current = analyze(sources(), args.ccn, args.nloc, args.scope)
    report = {
        "lizard": LIZARD_VERSION,
        "scope": args.scope,
        "thresholds": {"ccn": args.ccn, "nloc": args.nloc},
        "current": current,
    }
    if args.baseline:
        report["baseline_ref"] = args.baseline
        report["baseline"] = analyze(sources(args.baseline), args.ccn, args.nloc, args.scope)
        print(
            f"Baseline: {report['baseline']['warning_count']} warnings; current: {current['warning_count']}"
        )
    print(
        f"Lizard {LIZARD_VERSION}: {current['files']} files, {current['functions']} functions; "
        f"{current['warning_count']} warnings (CCN > {int(args.ccn)} or NLOC > {int(args.nloc)})"
    )
    print(f"By area: {current['warnings_by_area']}")
    for function in current["warnings"][: args.top]:
        print(
            f"{function['path']}:{function['line']} {function['name']} "
            f"CCN={int(function['ccn'])} NLOC={int(function['nloc'])}"
        )
    if args.json:
        save_report(args.json, report)
    return int(args.fail and current["warning_count"] > 0)


if __name__ == "__main__":
    raise SystemExit(main())
