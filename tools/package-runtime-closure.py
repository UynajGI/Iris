"""Copy a manifest-verified local runtime without links or unlisted payloads."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil


def copy_closure(source, destination, kind):
    source, destination = Path(source), Path(destination)
    if destination.exists() or source.is_symlink() or source.is_junction():
        raise ValueError("destination must be new and source must not be linked")
    manifest = json.loads((source / "manifest.json").read_text("utf-8"))
    plan = {}
    for entry in manifest["files"]:
        name = entry["path"]
        parts = name.split("/")
        if not name or any(p in {"", ".", ".."} for p in parts) or "\\" in name or ":" in name or name in plan:
            raise ValueError("invalid or duplicate runtime path")
        path = source
        for part in parts:
            path = path / part
            if path.is_symlink() or path.is_junction():
                raise ValueError("linked runtime entry")
        if not path.is_file() or path.stat().st_size != entry["size"] or hashlib.sha256(path.read_bytes()).hexdigest() != entry["sha256"]:
            raise ValueError(f"runtime checksum mismatch: {name}")
        if path.suffix.lower() == ".onnx":
            raise ValueError("model weights are not a runtime payload")
        plan[name] = path
    actual = set()
    for path in source.rglob("*"):
        if path.is_symlink() or path.is_junction():
            raise ValueError("linked runtime entry")
        if path.is_file():
            actual.add(path.relative_to(source).as_posix())
    if actual != set(plan) | {"manifest.json"}:
        raise ValueError("unlisted runtime payload")
    required = {"bin/exiftool.exe", "bin/exiftool_files/exiftool.pl"} if kind == "raw" else {"DirectML.dll", "onnxruntime.dll", "onnxruntime_providers_shared.dll"}
    if not required <= plan.keys() or not any("license" in p.lower() or "copying" in p.lower() for p in plan):
        raise ValueError("incomplete runtime closure")
    destination.mkdir(parents=True)
    for name, path in {**plan, "manifest.json": source / "manifest.json"}.items():
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
        if hashlib.sha256(target.read_bytes()).digest() != hashlib.sha256(path.read_bytes()).digest():
            raise ValueError("copied runtime checksum mismatch")
    return len(plan)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--kind", choices=["raw", "directml"], required=True)
    args = parser.parse_args()
    print(f"Copied {copy_closure(args.source, args.destination, args.kind)} verified {args.kind} files")
