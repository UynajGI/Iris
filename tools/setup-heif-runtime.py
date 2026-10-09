"""Build the offline Windows HEIC decoder DLLs; no machine codec is required.

Requires Python 3.12+, CMake and x64 MinGW GCC/mingw32-make on PATH.
The output carries the exact source archives and this script for LGPL rebuilds.
For an offline rebuild, pass --source-dir <distributed models/media/sources>.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import urllib.request

SOURCES = {
    "libde265": ("1.1.3", "189baa08fd6d2dd34db099de411bd6b2e6bd5eb88e81236a3d0fa3826b9715c4"),
    "libheif": ("1.23.6", "c9a00b0452f5c16d72ef71e5e40c0e90fbe2ad37148d27a1b2f9bb21546f5f03"),
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, default=root / "artifacts/heif-build")
    parser.add_argument("--output", type=Path, default=root / "models/media")
    parser.add_argument("--source-dir", type=Path)
    parser.add_argument("--jobs", type=int, default=8)
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("this reproducible build targets Windows x64 MinGW")
    work, output = args.work_dir.resolve(), args.output.resolve()
    work.mkdir(parents=True, exist_ok=True)
    output.mkdir(parents=True, exist_ok=True)
    gcc = shutil.which("gcc")
    if not gcc or not shutil.which("cmake") or not shutil.which("mingw32-make"):
        parser.error("CMake and MinGW GCC/mingw32-make must be on PATH")
    toolchain = Path(gcc).resolve().parent
    install = work / "install"
    sources = []
    for name, (version, sha256) in SOURCES.items():
        filename = f"{name}-v{version}.tar.gz"
        archive = work / filename
        url = f"https://github.com/strukturag/{name}/archive/refs/tags/v{version}.tar.gz"
        if args.source_dir:
            supplied = args.source_dir / filename
            if supplied.resolve() != archive:
                shutil.copy2(supplied, archive)
        elif not archive.exists():
            urllib.request.urlretrieve(url, archive)
        if digest(archive) != sha256:
            raise RuntimeError(f"source SHA-256 mismatch: {archive}")
        source = work / f"{name}-{version}"
        if not source.exists():
            with tarfile.open(archive) as tar:
                tar.extractall(work, filter="data")
        build = work / ("de265-build" if name == "libde265" else "heif-build")
        configure = ["cmake", "-S", str(source), "-B", str(build), "-G", "MinGW Makefiles",
                     "-DCMAKE_BUILD_TYPE=Release", "-DBUILD_SHARED_LIBS=ON",
                     f"-DCMAKE_INSTALL_PREFIX={install}",
                     "-DCMAKE_SHARED_LINKER_FLAGS=-static-libgcc -static-libstdc++"]
        if name == "libde265":
            configure += ["-DENABLE_SDL=OFF", "-DENABLE_DECODER=OFF", "-DENABLE_ENCODER=OFF", "-DENABLE_AVX512=OFF"]
        else:
            configure += [f"-DCMAKE_PREFIX_PATH={install}", "-DENABLE_PLUGIN_LOADING=OFF",
                          "-DWITH_LIBSHARPYUV=OFF", "-DWITH_EXAMPLES=OFF", "-DWITH_GDK_PIXBUF=OFF",
                          "-DBUILD_TESTING=OFF", "-DBUILD_DOCUMENTATION=OFF", "-DWITH_HEADER_COMPRESSION=OFF"]
            codecs = re.findall(r"^plugin_option\((\w+)", (source / "CMakeLists.txt").read_text(), re.M)
            configure += [f"-DWITH_{codec}={'ON' if codec == 'LIBDE265' else 'OFF'}" for codec in codecs]
        for step, command in [("configure", configure), ("build", ["cmake", "--build", str(build), "--parallel", str(args.jobs)]),
                              ("install", ["cmake", "--install", str(build)])]:
            log = work / f"{name}-{step}.log"
            print(f"{name} {step}: {log}", flush=True)
            with log.open("w", encoding="utf-8") as stream:
                subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, check=True)
        (output / "sources").mkdir(exist_ok=True)
        (output / "licenses").mkdir(exist_ok=True)
        shutil.copy2(archive, output / "sources" / filename)
        shutil.copy2(source / "COPYING", output / "licenses" / f"{name}-COPYING.txt")
        sources.append({"name": name, "version": version, "url": url, "sha256": sha256})

    for name in ["libheif.dll", "libde265.dll"]:
        shutil.copy2(install / "bin" / name, output / name)
    shutil.copy2(toolchain / "libwinpthread-1.dll", output / "libwinpthread-1.dll")
    for component in ["libwinpthread", "winpthreads", "gcc-libs"]:
        source = toolchain.parent / "share/licenses" / component
        if not source.is_dir():
            raise RuntimeError(f"runtime license missing: {source}")
        shutil.copytree(source, output / "licenses" / component, dirs_exist_ok=True)
    shutil.copy2(Path(__file__), output / "sources/setup-heif-runtime.py")
    (output / "README.txt").write_text(
        "Offline HEIC decoder: libheif + libde265, built-in HEVC decoder only; external plugins disabled.\n"
        "No Windows Store codec or ImageMagick runtime dependency.\n"
        "libheif is LGPL-3.0-or-later; libde265 is LGPL-3.0-or-later. See licenses and exact sources.\n"
        "These DLLs are dynamically loaded and may be replaced with compatible modified builds.\n"
        "Rebuild offline using sources/setup-heif-runtime.py --source-dir sources --work-dir <build> --output <runtime>.\n"
        "MinGW GCC runtime licensing exceptions and winpthreads notices are included in licenses.\n",
        encoding="utf-8")
    dependencies = {}
    for dll in sorted(output.glob("*.dll")):
        listing = subprocess.check_output([str(toolchain / "objdump.exe"), "-p", str(dll)], text=True)
        names = re.findall(r"DLL Name:\s*(\S+)", listing)
        dependencies[dll.name] = names
        allowed = {"kernel32.dll", "msvcrt.dll", "libde265.dll", "libwinpthread-1.dll"}
        if any(name.lower() not in allowed for name in names):
            raise RuntimeError(f"unexpected dependency closure: {dll.name}: {names}")
    files = [{"path": str(path.relative_to(output)).replace("\\", "/"), "sha256": digest(path), "size": path.stat().st_size}
             for path in sorted(output.rglob("*")) if path.is_file() and path.name != "manifest.json"]
    manifest = {"schema": 1, "platform": "windows-x64", "hevc_decoder": "libde265", "plugins": False,
                "sources": sources, "dependencies": dependencies, "files": files}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"HEIC runtime ready: {output}", flush=True)


if __name__ == "__main__":
    main()
