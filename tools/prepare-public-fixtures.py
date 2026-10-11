"""Download a small, traceable research fixture set; never read personal photos."""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
import time
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PIQ = "https://api.github.com/repos/DXOMARK-Research/PIQ2023"
RAW = "https://raw.pixls.us/json/getrepository.php?set=all"


def fetch(url: str, limit: int = 70_000_000) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "Iris-research-fixtures"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=180) as response:
                data = response.read(limit + 1)
                if len(data) > limit:
                    raise ValueError("Download exceeds fixture size budget")
                return data
        except (OSError, TimeoutError):
            if attempt == 2:
                raise
            time.sleep(2)
    raise RuntimeError("Download failed")


def persist(base: Path, item: dict) -> dict:
    path = base / item["path"]
    path.parent.mkdir(parents=True, exist_ok=True)
    data = path.read_bytes() if path.exists() else fetch(item["url"])
    digest = hashlib.sha256(data).hexdigest()
    if item.get("expected_sha256") and digest != item["expected_sha256"]:
        raise ValueError(f"SHA256 mismatch: {item['path']}")
    if item.get("git_blob_sha"):
        blob = b"blob " + str(len(data)).encode() + b"\0" + data
        if hashlib.sha1(blob).hexdigest() != item["git_blob_sha"]:
            raise ValueError(f"Git blob mismatch: {item['path']}")
    if not path.exists():
        temporary = path.with_name(path.name + ".part")
        temporary.write_bytes(data)
        temporary.replace(path)
    return {**item, "sha256": digest, "bytes": len(data)}


def piq_items(base: Path) -> list[dict]:
    commit = json.loads(fetch(PIQ + "/commits/master"))["sha"]
    tree = json.loads(fetch(PIQ + f"/git/trees/{commit}?recursive=1"))["tree"]
    prefix = "Test split/Scene Split/Scene examples/"
    items = []
    for entry in tree:
        path = entry["path"]
        if path.startswith(prefix) and Path(path).suffix.lower() in {".jpg", ".jpeg", ".png"}:
            items.append(
                {
                    "dataset": "PIQ23 official scene examples",
                    "path": "piq23/examples/" + path.removeprefix(prefix),
                    "url": f"https://raw.githubusercontent.com/DXOMARK-Research/PIQ2023/{commit}/"
                    + urllib.parse.quote(path),
                    "git_blob_sha": entry["sha"],
                    "license": "Non-commercial research only; see official README",
                    "purpose": "UI and inference smoke tests, not a scored accuracy benchmark",
                }
            )
    notices = base / "piq23"
    notices.mkdir(parents=True, exist_ok=True)
    (notices / "UPSTREAM-README.md").write_bytes(
        fetch(f"https://raw.githubusercontent.com/DXOMARK-Research/PIQ2023/{commit}/README.md")
    )
    return items


def raw_items(base: Path) -> list[dict]:
    catalog = base / "rawpixls-catalog.json"
    if not catalog.exists():
        catalog.write_bytes(fetch(RAW, 20_000_000))
    rows = json.loads(catalog.read_bytes())["data"]
    selected = {}
    for row in rows:
        if "publicdomain/zero/1.0" not in row[5]:
            continue
        link = re.search(r"href='([^']+)'", row[7])
        checksum = re.search(r"Checksum'>([a-f0-9]{64})<", row[7])
        size = re.search(r"\(([\d.]+)MB\)", row[7])
        if not link or not checksum or not size or float(size[1]) > 40:
            continue
        url = html.unescape(link[1])
        ext = Path(urllib.parse.urlparse(url).path).suffix.lower()
        if ext not in {".cr2", ".cr3", ".nef", ".arw", ".raf", ".dng"} or ext in selected:
            continue
        file_id = re.search(r"getfile.php/(\d+)/", url)[1]
        selected[ext] = {
            "dataset": "raw.pixls.us",
            "path": f"rawpixls/{file_id}{ext}",
            "url": urllib.parse.quote(url, safe=":/"),
            "expected_sha256": checksum[1],
            "license": "CC0-1.0",
            "license_url": "https://creativecommons.org/publicdomain/zero/1.0/",
            "camera": f"{row[0]} {row[1]}",
            "mode": row[2],
            "purpose": "RAW decoder compatibility only",
        }
    if len(selected) != 6:
        raise ValueError("Could not select all six CC0 RAW formats within budget")
    return list(selected.values())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--raw", action="store_true", help="Also download six CC0 RAW samples (each <=40MB)"
    )
    args = parser.parse_args()
    base = ROOT / "reference/test-data"
    base.mkdir(parents=True, exist_ok=True)
    items = piq_items(base)
    results = []
    manifest = base / "manifest.json"
    for item in items:
        results.append(persist(base, item))
        manifest.write_text(
            json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
        print(f"Verified {item['path']}", flush=True)
    if args.raw:
        for item in raw_items(base):
            results.append(persist(base, item))
            manifest.write_text(
                json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
            )
            print(f"Verified {item['path']}", flush=True)
    print(f"Complete: {len(results)} files, {sum(item['bytes'] for item in results)} bytes")


if __name__ == "__main__":
    main()
