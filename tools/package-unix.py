"""Assemble an experimental macOS/Linux CPU package from release binaries."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() not in ("Darwin", "Linux"):
        parser.error("Run on a native macOS or Linux builder")
    output = args.output.resolve()
    if output.exists():
        parser.error("Output must be new")
    output.mkdir(parents=True)
    mac = platform.system() == "Darwin"
    binary = output / "Iris.app/Contents/MacOS" if mac else output
    binary.mkdir(parents=True, exist_ok=True)
    for name, directory in [("iris-shell", "apps/shell/src-tauri"), ("iris-daemon", "."), ("iris-cli", "."), ("iris-mcp", "."), ("iris-raw-decoder", "components/raw-decoder")]:
        shutil.copy2(ROOT / directory / "target/release" / name, binary / name)
        (binary / name).chmod(0o755)
    models = binary / "models"
    models.mkdir()
    manifest = json.loads((ROOT / "models/manifest.json").read_text())
    names = [m["file"] for m in manifest["models"]] + ["manifest.json", "niqe_params.json", "README.md", "libonnxruntime.dylib" if mac else "libonnxruntime.so"]
    names += [p.name for p in (ROOT / "models").glob("*LICENSE*.txt")]
    names += [p.name for p in (ROOT / "models").glob("ThirdPartyNotices-*.txt")]
    for name in sorted(set(names)):
        if Path(name).name != name or (ROOT / "models" / name).is_symlink():
            raise ValueError("Unexpected model path")
        shutil.copy2(ROOT / "models" / name, models / name)
    for name in ["LICENSE", "THIRD_PARTY_NOTICES.md"]:
        shutil.copy2(ROOT / name, output / name)
    shutil.copytree(ROOT / "apps/shell/dist", binary / "frontend")
    subprocess.run([sys.executable, ROOT / "tools/package-source.py", "--output", output / "sources/iris-application-source.zip"], check=True)
    subprocess.run([sys.executable, ROOT / "tools/package-relink-source.py", "--output", output / "sources/raw-decoder-source.zip"], check=True)
    if mac:
        version = json.loads((ROOT / "apps/shell/package.json").read_text())["version"]
        info = {"CFBundleExecutable": "iris-shell", "CFBundleName": "Iris", "CFBundleIdentifier": "local.irisvision.app", "CFBundlePackageType": "APPL", "CFBundleShortVersionString": version.split("-")[0], "CFBundleVersion": version.split("-")[0], "NSHighResolutionCapable": True}
        (output / "Iris.app/Contents/Info.plist").write_bytes(plistlib.dumps(info))
    launcher = output / ("Launch.command" if mac else "Launch.sh")
    executable = 'Iris.app/Contents/MacOS/iris-shell' if mac else 'iris-shell'
    launcher.write_text('#!/bin/sh\nset -eu\ncd "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"\nexec "./' + executable + '"\n')
    launcher.chmod(0o755)
    (output / "README.txt").write_text("Iris experimental CPU beta. Start with Launch.command (macOS) or ./Launch.sh (Linux).\nLinux requires WebKitGTK 4.1 and GTK 3 (Ubuntu 22.04+). macOS is not notarized.\nJPEG/PNG/WebP and CPU analysis are included. HEIC runtime and ExifTool are not bundled on Unix yet; RAW uses the independent bounded converter. DirectML is Windows-only.\nCLI/MCP executables and models are beside iris-shell; on macOS inside Iris.app/Contents/MacOS.\nUse disposable authorized photos for testing and report OS, architecture and reproduction steps.\nSources and third-party notices are included. No user photographs or optional model weights are included.\n")
    entries = [{"path": p.relative_to(output).as_posix(), "sha256": hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(output.rglob("*")) if p.is_file()]
    (output / "checksums.json").write_text(json.dumps(entries, indent=2) + "\n")
    archive = Path(str(output) + ".tar.gz")
    if archive.exists():
        raise ValueError("Archive already exists")
    with tarfile.open(archive, "w:gz") as bundle:
        bundle.add(output, arcname=output.name)
    print(archive)


if __name__ == "__main__":
    main()
