"""Portable development tasks; run `python tools/dev.py help` or `make help`."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
WEB = ROOT / "apps/shell"
TASKS = {
    "help": "List tasks and configuration (no changes)",
    "doctor": "Check local tools; does not install anything",
    "setup": "Install locked root/frontend npm dependencies and local Git hooks",
    "check": "Rust workspace check, Rust formatting and frontend type check",
    "fmt": "Format all three Rust workspaces",
    "fmt-check": "Check formatting in all three Rust workspaces",
    "test": "Workspace Rust, Python tools, frontend and native library tests",
    "test-rust": "Test the main Rust workspace; ignored media tests stay ignored",
    "test-python": "Test Python development and packaging tools",
    "test-web": "Test frontend; live API needs IRIS_TEST_DAEMON and IRIS_TEST_PHOTOS",
    "test-native": "Test the native host/updater library",
    "build": "Build core binaries, RAW converter and frontend (no desktop shell)",
    "build-core": "Build daemon, CLI, MCP and independent RAW converter",
    "build-web": "Build frontend assets and preserve their notices",
    "build-desktop": "Build frontend and native desktop shell",
    "api": "Regenerate OpenAPI and TypeScript API declarations",
    "api-check": "Compare generated API with committed files without editing them",
    "public-check": "Check complete staged public-source index and links",
    "verify": "Run check, test, API comparison and public index checks",
    "models": "Explicitly download and verify default models",
    "media": "Explicitly build the Windows HEIC runtime from pinned sources",
    "raw": "Explicitly provision Windows ExifTool and build the RAW converter",
    "directml": "Explicitly provision optional Windows DirectML runtime",
    "source": "Export clean HEAD; requires --output or IRIS_OUTPUT",
    "portable": "Build Windows portable package; requires --output or IRIS_OUTPUT",
}
MANIFESTS = ("Cargo.toml", "apps/shell/src-tauri/Cargo.toml", "components/raw-decoder/Cargo.toml")


class Runner:
    def __init__(self, profile="debug", output=None, dry_run=False):
        self.profile = profile
        self.output = output
        self.dry_run = dry_run

    @property
    def profile_flags(self):
        return ["--release"] if self.profile == "release" else []

    def run(self, args, cwd=ROOT, capture=False):
        args = [str(a) for a in args]
        print("+ " + shlex.join(args), flush=True)
        if self.dry_run:
            return ""
        executable = shutil.which(args[0])
        if not executable:
            raise ValueError(f"Missing tool: {args[0]}; run doctor for prerequisites")
        return subprocess.run([executable, *args[1:]], cwd=cwd, check=True,
                              stdout=subprocess.PIPE if capture else None,
                              text=True, encoding="utf-8").stdout

    def python(self, script, *args):
        return self.run([sys.executable, ROOT / "tools" / script, *args])

    def cargo(self, *args):
        return self.run(["cargo", *args])

    def require_output(self):
        if not self.output:
            raise ValueError("Provide --output PATH or set IRIS_OUTPUT; existing output is never overwritten")
        return str(Path(self.output).resolve())

    def api(self, check):
        command = ["cargo", "run", "--locked", *self.profile_flags, "-p", "iris-daemon", "--", "--openapi"]
        raw = self.run(command, capture=True)
        if self.dry_run:
            print("Compare schema and generated types" if check else "Write schema and generated types")
            return
        schema = json.loads(raw.lstrip("\ufeff"))
        if "/api/v1/bootstrap" not in schema.get("paths", {}):
            raise ValueError("Unexpected API schema; existing files were not changed")
        path = ROOT / "docs/openapi.json"
        if check and json.loads(path.read_text(encoding="utf-8-sig")) != schema:
            raise ValueError("OpenAPI is stale; run api and review the changes")
        with tempfile.TemporaryDirectory(prefix="iris-api-") as tmp:
            temporary = Path(tmp) / "openapi.json"
            temporary.write_text(json.dumps(schema, indent=2) + "\n", encoding="utf-8")
            self.run(["node", ROOT / "tools/generate-client.mjs", temporary, *( ["--check"] if check else [])])
            if not check:
                path.write_text(temporary.read_text(encoding="utf-8"), encoding="utf-8")

    def execute(self, task):
        groups = {"check": ["fmt-check"], "test": ["test-rust", "test-python", "test-web", "test-native"],
                  "build": ["build-core", "build-web"],
                  "verify": ["check", "test", "api-check", "public-check"]}
        if task in groups:
            for child in groups[task]:
                self.execute(child)
            if task == "check":
                self.cargo("check", "--workspace", "--locked")
                self.run(["npm", "run", "check"], cwd=WEB)
        elif task == "help":
            for name, description in TASKS.items():
                print(f"{name:16} {description}")
            print("\nPython options: --profile debug|release, --output PATH, --dry-run")
            print("Make environment: IRIS_PROFILE (default debug), IRIS_OUTPUT. Example: make check")
            print("Provisioning tasks may download files. No task deletes photos or accepts optional model licenses.")
        elif task == "doctor":
            missing = []
            for name in ("git", "cargo", "rustc", "node", "npm", "python"):
                found = shutil.which(name)
                print(f"{name}: {found or 'MISSING'}")
                if not found:
                    missing.append(name)
            print("GNU Make is optional; Windows packaging additionally needs PowerShell 7, MinGW and WebView2.")
            print("Models/media dependencies are installed only by explicit provisioning tasks.")
            if missing:
                raise ValueError("Missing required tools: " + ", ".join(missing))
        elif task == "setup":
            self.run(["npm", "ci"])
            self.run(["npm", "ci"], cwd=WEB)
        elif task in ("fmt", "fmt-check"):
            for manifest in MANIFESTS:
                self.cargo("fmt", "--manifest-path", manifest, "--all", *( ["--", "--check"] if task == "fmt-check" else []))
        elif task == "test-rust":
            self.cargo("test", "--workspace", "--locked", *self.profile_flags)
        elif task == "test-python":
            self.run([sys.executable, "-m", "unittest", "discover", "-s", "tools/tests", "-p", "test_*.py", "-v"])
        elif task == "test-web":
            self.run(["npm", "test"], cwd=WEB)
        elif task == "test-native":
            self.cargo("test", "--locked", "--manifest-path", MANIFESTS[1], "--features", "updater-client", "--lib", *self.profile_flags)
        elif task == "build-core":
            self.cargo("build", "--workspace", "--locked", *self.profile_flags)
            self.cargo("build", "--locked", "--manifest-path", MANIFESTS[2], *self.profile_flags)
        elif task == "build-web":
            self.run(["npm", "run", "build"], cwd=WEB)
        elif task == "build-desktop":
            self.execute("build-web")
            self.cargo("build", "--locked", "--manifest-path", MANIFESTS[1], "--features", "desktop", *self.profile_flags)
        elif task in ("api", "api-check"):
            self.api(task == "api-check")
        elif task == "public-check":
            self.python("check-public-tree.py")
        elif task in ("models", "media", "raw", "directml"):
            if task != "models" and os.name != "nt":
                raise ValueError(f"{task} provisioning is currently Windows-only; see docs/media-support.md")
            self.python({"models": "setup-models.py", "media": "setup-heif-runtime.py",
                         "raw": "setup-raw-runtime.py", "directml": "setup-directml-runtime.py"}[task])
            if task == "raw":
                self.cargo("build", "--locked", "--manifest-path", MANIFESTS[2], *self.profile_flags)
        elif task == "source":
            self.python("package-source.py", "--output", self.require_output())
        elif task == "portable":
            if os.name != "nt":
                raise ValueError("Portable packaging currently supports Windows only")
            self.run(["pwsh", "-NoProfile", "-File", ROOT / "tools/package-local.ps1",
                      "-Configuration", self.profile.title(), "-OutputDirectory", self.require_output()])
        else:
            raise ValueError("Unknown task: " + task)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("task", nargs="?", choices=TASKS, default="help")
    parser.add_argument("--profile", choices=("debug", "release"), default=os.environ.get("IRIS_PROFILE", "debug"))
    parser.add_argument("--output", default=os.environ.get("IRIS_OUTPUT"))
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if args.profile not in ("debug", "release"):
        parser.error("IRIS_PROFILE must be debug or release")
    try:
        Runner(args.profile, args.output, args.dry_run).execute(args.task)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"Task failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
