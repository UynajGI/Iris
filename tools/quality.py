"""Shared read-only quality gates. Tool installation is explicit and repository-local."""

import argparse
import hashlib
import importlib.util
import io
import json
import os
import platform
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CACHE = Path(os.environ.get("IRIS_QUALITY_CACHE", ROOT / ".tools/quality"))


def run(args, **kwargs):
    kwargs.setdefault("cwd", ROOT)
    return subprocess.run([str(a) for a in args], check=True, **kwargs)


def git(*args):
    return run(["git", *args], stdout=subprocess.PIPE).stdout


def tool_spec(name):
    manifest = json.loads((ROOT / "tools/quality-tools.json").read_text("utf-8"))
    spec = manifest[name]
    arch = platform.machine().lower()
    arch = {"amd64": "x86_64", "aarch64": "arm64"}.get(arch, arch)
    key = f"{platform.system()}-{arch}"
    if key not in spec["archives"]:
        raise ValueError(f"No reviewed {name} archive for {key}")
    filename, digest = spec["archives"][key]
    url = f"https://github.com/{spec['repository']}/releases/download/v{spec['version']}/{filename}"
    return filename, digest, url


def verified_payload(data, digest, name, filename):
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f"{name}: archive SHA256 mismatch")
    executable = name + (".exe" if filename.endswith(".zip") else "")
    # Read exactly one known member; never extract archive paths onto the filesystem.
    if filename.endswith(".zip"):
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            return executable, archive.read(executable)
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        member = archive.getmember(executable)
        if not member.isfile():
            raise ValueError(f"{name}: expected a regular executable")
        return executable, archive.extractfile(member).read()


def download_tool(url):
    request = urllib.request.Request(url, headers={"User-Agent": "Iris-maintenance"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=120) as response:
                return response.read()
        except urllib.error.HTTPError as error:
            if error.code not in {408, 429, 500, 502, 503, 504}:
                raise
            failure = error
        except (urllib.error.URLError, TimeoutError) as error:
            failure = error
        if attempt == 2:
            raise failure
        delay = 2 ** (attempt + 1)
        print(
            f"Tool download failed: {failure}; retrying in {delay}s ({attempt + 2}/3)",
            file=sys.stderr,
        )
        time.sleep(delay)


def tool(name, install=False):
    filename, digest, url = tool_spec(name)
    directory = CACHE / digest
    archive = directory / filename
    if install and not archive.exists():
        data = download_tool(url)
        verified_payload(data, digest, name, filename)
        directory.mkdir(parents=True, exist_ok=True)
        archive.write_bytes(data)
    if not archive.exists():
        raise ValueError(f"{name} missing; run make quality-setup")
    executable, payload = verified_payload(archive.read_bytes(), digest, name, filename)
    destination = directory / executable
    if install:
        destination.write_bytes(payload)
        destination.chmod(0o755)
    if not destination.is_file() or destination.read_bytes() != payload:
        raise ValueError(
            f"{name}: executable differs from verified archive; run make quality-setup"
        )
    return destination


def load_complexity():
    spec = importlib.util.spec_from_file_location(
        "iris_complexity", ROOT / "tools/check-complexity.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    if module.version("lizard") != module.LIZARD_VERSION:
        raise ValueError("Lizard version differs from the reviewed complexity policy")
    return module


def complexity_errors(report, limits):
    errors = []
    for item in report["warnings"]:
        key = f"{item['path']}::{item['name']}"
        ceiling = limits.get(key, {"ccn": 15, "nloc": 120})
        if item["ccn"] > ceiling["ccn"] or item["nloc"] > ceiling["nloc"]:
            errors.append(f"{key}: CCN={item['ccn']} NLOC={item['nloc']} exceeds {ceiling}")
    return errors


def complexity(source_files=None, limits=None):
    module = load_complexity()
    if limits is None:
        limits = json.loads((ROOT / "tools/complexity-limits.json").read_text("utf-8"))["legacy"]
    report = module.analyze(
        module.sources() if source_files is None else source_files, 15, 120, "repo"
    )
    errors = complexity_errors(report, limits)
    if errors:
        raise ValueError("\n".join(errors))
    print(
        f"Complexity: {report['functions']} functions checked; {report['warning_count']} legacy violations within explicit limits"
    )


def workflows(directory=ROOT):
    paths = sorted((directory / ".github/workflows").glob("*.y*ml"))
    if not paths:
        raise ValueError("No workflow files found")
    # ShellCheck/Pyflakes are deliberately disabled so platforms run identical checks.
    run([tool("actionlint"), "-oneline", "-shellcheck=", "-pyflakes=", *paths], cwd=directory)


def secrets(staged=False):
    args = [tool("gitleaks"), "git", "--redact", "--no-banner"]
    args += ["--staged"] if staged else ["--log-opts=--all"]
    run([*args, ROOT])


def index_blob(name):
    return git("show", f":{name}")


def staged():
    secrets(staged=True)
    names = (
        git("diff", "--cached", "--name-only", "--diff-filter=ACMR", "-z")
        .decode("utf-8")
        .split("\0")
    )
    names = [name for name in names if name]
    module = load_complexity()
    limits = json.loads(index_blob("tools/complexity-limits.json"))["legacy"]
    source_files = [
        (name, index_blob(name).decode("utf-8-sig")) for name in names if module.source_path(name)
    ]
    complexity(source_files, limits)
    with tempfile.TemporaryDirectory(prefix="iris-staged-quality-") as temporary:
        snapshot = Path(temporary)
        config = snapshot / "pyproject.toml"
        config.write_bytes(index_blob("pyproject.toml"))
        for name in names:
            if name.endswith(".py"):
                run(
                    [
                        sys.executable,
                        "-m",
                        "ruff",
                        "check",
                        "--no-cache",
                        "--config",
                        config,
                        "--stdin-filename",
                        name,
                        "-",
                    ],
                    input=index_blob(name),
                )
        if any(name.startswith(".github/") and name.endswith((".yml", ".yaml")) for name in names):
            tracked = git("ls-files", "-z", ".github").decode("utf-8").split("\0")
            for name in filter(None, tracked):
                path = snapshot / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(index_blob(name))
            workflows(snapshot)


def audit_python():
    with tempfile.TemporaryDirectory(prefix="iris-audit-") as temporary:
        requirements = Path(temporary) / "requirements.txt"
        run(
            [
                os.environ.get("UV", "uv"),
                "export",
                "--locked",
                "--format",
                "requirements-txt",
                "--default-index",
                "https://pypi.org/simple",
                "--no-emit-project",
                "--output-file",
                requirements,
            ],
            stdout=subprocess.DEVNULL,
        )
        run(
            [
                sys.executable,
                "-m",
                "pip_audit",
                "--require-hashes",
                "--disable-pip",
                "-r",
                requirements,
            ]
        )


def audit_rust():
    scanner = (
        ROOT / ".tools/cargo-audit/bin" / ("cargo-audit.exe" if os.name == "nt" else "cargo-audit")
    )
    result = run([scanner, "--version"], capture_output=True, text=True)
    if result.stdout.strip() != "cargo-audit 0.22.2":
        raise ValueError("Install the reviewed scanner with make audit-setup")
    for lock in [
        "Cargo.lock",
        "apps/shell/src-tauri/Cargo.lock",
        "components/raw-decoder/Cargo.lock",
    ]:
        run([scanner, "audit", "--file", lock])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=[
            "setup",
            "complexity",
            "workflows",
            "secrets",
            "staged",
            "audit-python",
            "audit-rust",
        ],
    )
    args = parser.parse_args()
    if args.command == "setup":
        for name in ["actionlint", "gitleaks"]:
            print(f"Verified {name}: {tool(name, install=True)}")
    else:
        {
            "complexity": complexity,
            "workflows": workflows,
            "secrets": secrets,
            "staged": staged,
            "audit-python": audit_python,
            "audit-rust": audit_rust,
        }[args.command]()


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"Quality gate failed: {error}", file=sys.stderr)
        raise SystemExit(1)
