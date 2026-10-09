"""Copy a random, auditable sample of JPEGs without modifying the source library."""
from __future__ import annotations

import argparse
import hashlib
import json
import random
import shutil
from pathlib import Path


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--count", type=int, default=100)
    parser.add_argument("--seed", type=int)
    args = parser.parse_args()
    source, destination = args.source.resolve(strict=True), args.destination.resolve()
    if destination == source or source in destination.parents:
        raise SystemExit("Destination must be outside the source library")
    if destination.exists() and any(destination.iterdir()):
        raise SystemExit("Destination must be empty; existing samples are never overwritten")
    candidates = sorted(p for p in source.rglob("*") if p.is_file() and p.suffix.lower() in {".jpg", ".jpeg"})
    if not 0 < args.count <= len(candidates):
        raise SystemExit(f"Requested {args.count} files from {len(candidates)} JPEGs")
    seed = args.seed if args.seed is not None else random.SystemRandom().getrandbits(64)
    sample = random.Random(seed).sample(candidates, args.count)
    destination.mkdir(parents=True, exist_ok=True)
    entries = []
    for photo in sample:
        relative = photo.relative_to(source)
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        before = digest(photo)
        shutil.copy2(photo, target)
        after = digest(target)
        if before != after:
            raise RuntimeError(f"Copy verification failed: {relative}")
        entries.append({"path": relative.as_posix(), "size_bytes": target.stat().st_size, "sha256": after})
    manifest = {"source": str(source), "seed": seed, "candidate_count": len(candidates), "count": len(entries), "files": entries}
    (destination / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"destination": str(destination), "count": len(entries), "total_bytes": sum(e["size_bytes"] for e in entries), "hash_verified": True}, ensure_ascii=False))


if __name__ == "__main__":
    main()
