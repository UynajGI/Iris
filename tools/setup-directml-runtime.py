"""Install pinned Microsoft ONNX Runtime DirectML and native dependency, offline-capable."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request
import zipfile

PACKAGES = [
    ("microsoft.ml.onnxruntime.directml", "1.22.0", "29f9872d786236b79aa83f94482f3a17c14297e4833768d6d0ed4883ee732e60", {
        "runtimes/win-x64/native/onnxruntime.dll": "onnxruntime.dll",
        "runtimes/win-x64/native/onnxruntime_providers_shared.dll": "onnxruntime_providers_shared.dll",
        "LICENSE": "licenses/ONNX-Runtime-LICENSE.txt", "ThirdPartyNotices.txt": "licenses/ONNX-Runtime-ThirdPartyNotices.txt"}),
    ("microsoft.ai.directml", "1.15.4", "4e7cb7ddce8cf837a7a75dc029209b520ca0101470fcdf275c1f49736a3615b9", {
        "bin/x64-win/DirectML.dll": "DirectML.dll", "LICENSE.txt": "licenses/DirectML-LICENSE.txt",
        "LICENSE-CODE.txt": "licenses/DirectML-LICENSE-CODE.txt", "ThirdPartyNotices.txt": "licenses/DirectML-ThirdPartyNotices.txt"}),
]


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=root / "artifacts/directml-downloads")
    parser.add_argument("--output", type=Path, default=root / "models/directml")
    args = parser.parse_args()
    args.source_dir.mkdir(parents=True, exist_ok=True)
    sources = []
    payload = {}
    for name, version, checksum, entries in PACKAGES:
        filename = f"{name}.{version}.nupkg"
        path = args.source_dir / filename
        url = f"https://api.nuget.org/v3-flatcontainer/{name}/{version}/{filename}"
        if not path.exists():
            temporary = path.with_suffix(".partial")
            with urllib.request.urlopen(url, timeout=180) as response, temporary.open("wb") as output:
                total = 0
                while chunk := response.read(1024 * 1024):
                    total += len(chunk)
                    if total > 512 * 1024 * 1024:
                        raise ValueError("package exceeds safety bound")
                    output.write(chunk)
            temporary.rename(path)
        with path.open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != checksum:
                raise ValueError(f"package SHA-256 mismatch: {filename}")
        with zipfile.ZipFile(path) as archive:
            payload.update({target: archive.read(source) for source, target in entries.items()})
        sources.append({"url": url, "sha256": checksum, "version": version})
    for name, data in payload.items():
        target = args.output / name
        if target.exists() and target.read_bytes() != data:
            raise ValueError(f"refusing to overwrite modified runtime: {target}")
    for name, data in payload.items():
        target = args.output / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    files = [{"path": name, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()} for name, data in sorted(payload.items())]
    (args.output / "manifest.json").write_text(json.dumps({"platform": "windows-x64", "sources": sources, "files": files}, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"runtime": str(args.output), "verified_packages": len(sources), "files": len(files)}))


if __name__ == "__main__":
    main()
