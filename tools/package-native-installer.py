"""Build native macOS PKG / Debian DEB from a verified Unix portable package.

No signing, installation, publication, optional downloads or third-party Python
dependencies. All payload paths remain adjacent to the daemon as in the portable.
"""
# SPDX-License-Identifier: GPL-3.0-or-later
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import platform
import plistlib
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = {"iris-shell", "iris-daemon", "iris-cli", "iris-mcp", "iris-raw-decoder", "release.json", "README.txt",
            "models/manifest.json", "models/niqe_params.json", "LICENSE",
            "THIRD_PARTY_NOTICES.md", "sources/iris-application-source.zip",
            "sources/raw-decoder-source.zip", "sources/dependency-sources.zip"}


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def validate(portable, mac):
    portable = portable.resolve()
    entries = json.loads((portable / "checksums.json").read_text(encoding="utf-8"))
    seen = set()
    prefix = "Iris.app/Contents/MacOS/" if mac else ""
    model_names = {m["file"] for m in json.loads((ROOT / "models/manifest.json").read_text(encoding="utf-8"))["models"]}
    model_names |= {"manifest.json", "niqe_params.json", "README.md", "libonnxruntime.dylib", "libonnxruntime.so"}
    for entry in entries:
        name = entry["path"]
        path = PurePosixPath(name)
        if (not name or "\\" in name or ":" in name or path.is_absolute()
                or ".." in path.parts or name != path.as_posix() or name in seen):
            raise ValueError(f"Invalid or duplicate manifest path: {name}")
        if "/optional/" in "/" + name.lower() or name.lower().endswith((".sqlite3", ".sqlite", ".db")):
            raise ValueError(f"Private or optional payload is not permitted: {name}")
        relative = name.removeprefix(prefix)
        allowed = name in {"LICENSE", "THIRD_PARTY_NOTICES.md", "README.txt", "release.json",
                           "Launch.command" if mac else "Launch.sh", "Iris.app/Contents/Info.plist"}
        allowed |= name in {"sources/iris-application-source.zip", "sources/raw-decoder-source.zip", "sources/dependency-sources.zip"}
        allowed |= relative in {"iris-shell", "iris-daemon", "iris-cli", "iris-mcp", "iris-raw-decoder"}
        allowed |= relative.startswith("frontend/")
        if relative.startswith("models/") and len(PurePosixPath(relative).parts) == 2:
            model = PurePosixPath(relative).name
            allowed |= model in model_names or (model.endswith(".txt") and ("LICENSE" in model or model.startswith("ThirdPartyNotices-")))
        if not allowed:
            raise ValueError(f"Unapproved installer payload: {name}")
        seen.add(name)
        file = portable / name
        if (not file.resolve().is_relative_to(portable) or file.is_symlink()
                or not file.is_file() or digest(file) != entry["sha256"]):
            raise ValueError(f"Portable checksum mismatch: {name}")
    required = {prefix + name if name.startswith(("iris-", "models/")) else name for name in REQUIRED}
    required.add(prefix + ("models/libonnxruntime.dylib" if mac else "models/libonnxruntime.so"))
    if mac:
        required.add("Iris.app/Contents/Info.plist")
    if not required <= seen:
        raise ValueError("Missing required package files: " + ", ".join(sorted(required - seen)))
    # Only manifested files are staged; no recursive copy of unlisted payload.
    return entries


def copy_entries(portable, target, entries):
    for entry in entries:
        source = portable / entry["path"]
        destination = target / entry["path"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        destination.chmod(0o755 if source.stat().st_mode & 0o111 else 0o644)


def manifest(root):
    entries = [{"path": file.relative_to(root).as_posix(), "sha256": digest(file)}
               for file in sorted(root.rglob("*")) if file.is_file() and file.name != "checksums.json"]
    (root / "checksums.json").write_text(json.dumps(entries, indent=2) + "\n", encoding="utf-8")


def distribution(version, architecture):
    root = ET.Element("installer-gui-script", {"minSpecVersion": "2"})
    ET.SubElement(root, "title").text = f"Iris / 伊人 {version}"
    ET.SubElement(root, "options", {"customize": "never", "require-scripts": "false",
                                    "hostArchitectures": architecture})
    ET.SubElement(root, "domains", {"enable_localSystem": "true", "enable_currentUserHome": "false",
                                    "enable_anywhere": "false"})
    ET.SubElement(root, "welcome", {"file": "welcome.html", "mime-type": "text/html"})
    ET.SubElement(root, "conclusion", {"file": "conclusion.html", "mime-type": "text/html"})
    ET.SubElement(root, "license", {"file": "LICENSE.txt", "mime-type": "text/plain"})
    outline = ET.SubElement(root, "choices-outline")
    ET.SubElement(outline, "line", {"choice": "iris"})
    choice = ET.SubElement(root, "choice", {"id": "iris", "title": "Iris", "description": "Local photo workspace", "visible": "false"})
    ET.SubElement(choice, "pkg-ref", {"id": "local.irisvision.app"})
    ET.SubElement(root, "pkg-ref", {"id": "local.irisvision.app", "version": version.split("-")[0]}).text = "iris-component.pkg"
    return ET.tostring(root, encoding="unicode", xml_declaration=True)


def deb_control(version, installed_size):
    # Debian sorts a pre-release before its corresponding stable version.
    version = version.replace("-", "~", 1)
    return (f"Package: iris\nVersion: {version}\nArchitecture: amd64\n"
            "Maintainer: Iris contributors <UynajGI@users.noreply.github.com>\n"
            "Section: graphics\nPriority: optional\n"
            f"Installed-Size: {installed_size}\n"
            "Depends: libc6 (>= 2.35), libgcc-s1, libstdc++6, libgtk-3-0, libwebkit2gtk-4.1-0\n"
            "Homepage: https://github.com/UynajGI/Iris\n"
            "Description: Local-first photo culling workspace\n"
            " Desktop, CLI and MCP with bundled default CPU models.\n"
            " HEIC runtime and ExifTool are not bundled on Linux.\n")


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def build(portable, output):
    system = platform.system()
    machine = platform.machine().lower()
    if system not in {"Darwin", "Linux"} or machine not in {"arm64", "aarch64", "x86_64", "amd64"}:
        raise ValueError("Use a supported native macOS or Linux builder")
    mac = system == "Darwin"
    if not mac and machine not in {"x86_64", "amd64"}:
        raise ValueError("Debian installer currently supports x64 only")
    portable, output = portable.resolve(), output.resolve()
    if output.is_relative_to(portable):
        raise ValueError("Installer output must be outside the source portable")
    if output.exists():
        raise ValueError("Output must be new")
    entries = validate(portable, mac)
    version = json.loads((ROOT / "apps/shell/package.json").read_text(encoding="utf-8"))["version"]
    label = "macos-" + ("arm64" if machine in {"arm64", "aarch64"} else "x64") if mac else "linux-x64"
    identity = json.loads((portable / "release.json").read_text(encoding="utf-8"))
    if identity != {"product": "Iris", "version": version, "platform": label}:
        raise ValueError("Portable version or architecture does not match this release builder")
    with tempfile.TemporaryDirectory(prefix="iris-native-installer-") as temporary:
        stage = Path(temporary)
        payload = stage / "payload"
        payload.mkdir()
        copy_entries(portable, payload, entries)
        # Verify the staged allowlisted copy, including host and synthetic inference.
        shutil.copy2(portable / "checksums.json", payload / "checksums.json")
        run(sys.executable, ROOT / "tools/verify-unix.py", "--bundle", payload)
        artifact = stage / f"Iris-{version}-{label}.{'pkg' if mac else 'deb'}"
        if mac:
            app = payload / "Iris.app"
            resources = app / "Contents/Resources"
            resources.mkdir(parents=True, exist_ok=True)
            for name in ("LICENSE", "THIRD_PARTY_NOTICES.md", "sources", "README.txt"):
                shutil.move(str(payload / name), resources / name)
            manifest(app / "Contents")
            pkg_root = stage / "root/Applications"
            pkg_root.mkdir(parents=True)
            shutil.move(str(app), pkg_root / "Iris.app")
            components = stage / "components.plist"
            run("pkgbuild", "--analyze", "--root", pkg_root.parent, components)
            properties = plistlib.loads(components.read_bytes())
            for component in properties:
                component["BundleIsRelocatable"] = False
                component["BundleOverwriteAction"] = "upgrade"
            components.write_bytes(plistlib.dumps(properties))
            run("pkgbuild", "--root", pkg_root.parent, "--install-location", "/", "--identifier",
                "local.irisvision.app", "--version", version.split("-")[0], "--component-plist", components,
                stage / "iris-component.pkg")
            ui = stage / "resources"
            shutil.copytree(ROOT / "apps/shell/installer/macos", ui)
            shutil.copy2(ROOT / "LICENSE", ui / "LICENSE.txt")
            dist = stage / "distribution.xml"
            dist.write_text(distribution(version, "arm64" if label.endswith("arm64") else "x86_64"), encoding="utf-8")
            run("productbuild", "--distribution", dist, "--resources", ui, "--package-path", stage, artifact)
            extracted = stage / "expanded"
            run("pkgutil", "--expand-full", artifact, extracted)
            apps = list(extracted.rglob("Iris.app"))
            if len(apps) != 1:
                raise ValueError("Expected exactly one extracted Iris.app")
            run(sys.executable, ROOT / "tools/verify-unix.py", "--bundle", apps[0] / "Contents",
                "--binary-directory", apps[0] / "Contents/MacOS")
            # Gate the actual installer UI, not only the component payload.
            for name in ("welcome.html", "conclusion.html", "LICENSE.txt"):
                if not any(p.read_bytes() == (ui / name).read_bytes() for p in extracted.rglob(name)):
                    raise ValueError(f"Installer resource missing: {name}")
        else:
            deb = stage / "deb"
            installed = deb / "opt/iris"
            installed.parent.mkdir(parents=True)
            shutil.move(str(payload), installed)
            for name in ("iris", "iris-cli", "iris-mcp"):
                wrapper = deb / "usr/bin" / name
                wrapper.parent.mkdir(parents=True, exist_ok=True)
                executable = "iris-shell" if name == "iris" else name
                wrapper.write_text(f'#!/bin/sh\nexec /opt/iris/{executable} "$@"\n', encoding="utf-8")
                wrapper.chmod(0o755)
            desktop = deb / "usr/share/applications/iris.desktop"
            desktop.parent.mkdir(parents=True)
            shutil.copy2(ROOT / "apps/shell/installer/linux/iris.desktop", desktop)
            size = (sum(p.stat().st_size for p in deb.rglob("*") if p.is_file()) + 1023) // 1024
            control = deb / "DEBIAN"
            control.mkdir()
            (control / "control").write_text(deb_control(version, size), encoding="utf-8")
            run("dpkg-deb", "--root-owner-group", "--build", deb, artifact)
            extracted = stage / "expanded"
            run("dpkg-deb", "--extract", artifact, extracted)
            run(sys.executable, ROOT / "tools/verify-unix.py", "--bundle", extracted / "opt/iris")
            run("desktop-file-validate", extracted / "usr/share/applications/iris.desktop")
        # Exclusive destination, created only after native verification succeeds.
        output.mkdir(parents=True)
        shutil.copy2(artifact, output / artifact.name)
        report = {"version": version, "platform": label, "artifact": artifact.name,
                  "sha256": digest(artifact), "source_manifest_sha256": digest(portable / "checksums.json"),
                  "extracted_payload_verified": True, "installer_executed": False,
                  "signed": False, "notarized": False, "clean_system_verified": False}
        (output / f"{label}-build-report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(output / artifact.name)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--portable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    build(args.portable, args.output)
