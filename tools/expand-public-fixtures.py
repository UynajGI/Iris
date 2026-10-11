"""Expand the local research set with complete real HDR+ bursts and CC0 RAW formats."""

from __future__ import annotations

import base64
import hashlib
import html
import importlib.util
import json
import re
import shutil
import time
import urllib.parse
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "reference/test-data"
SPEC = importlib.util.spec_from_file_location("prepare", ROOT / "tools/prepare-public-fixtures.py")
PREPARE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PREPARE)


def hdr_catalog() -> list[dict]:
    path = BASE / "hdrplus-catalog.json"
    if path.exists():
        return json.loads(path.read_bytes())
    rows, token = [], None
    while True:
        query = {"prefix": "20171106_subset/bursts/", "maxResults": "1000"}
        if token:
            query["pageToken"] = token
        url = "https://storage.googleapis.com/storage/v1/b/hdrplusdata/o?" + urllib.parse.urlencode(
            query
        )
        result = json.loads(PREPARE.fetch(url, 20_000_000))
        rows.extend(result.get("items", []))
        token = result.get("nextPageToken")
        if not token:
            break
    path.write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")
    return rows


def burst_items() -> list[dict]:
    groups = defaultdict(list)
    for row in hdr_catalog():
        if re.search(r"/payload_N\d+\.dng$", row["name"]):
            groups[Path(row["name"]).parent.name].append(row)
    names = sorted(groups)
    selected = [names[index * len(names) // 60] for index in range(60)]
    items = []
    for group in selected:
        for row in sorted(groups[group], key=lambda item: item["name"]):
            name = Path(row["name"]).name
            items.append(
                {
                    "dataset": "Google HDR+ curated real bursts",
                    "group_id": group,
                    "sequence_kind": "real camera burst",
                    "frame_index": int(re.search(r"N(\d+)", name)[1]),
                    "path": f"hdrplus/bursts/{group}/{name}",
                    "url": "https://storage.googleapis.com/hdrplusdata/"
                    + urllib.parse.quote(row["name"]),
                    "expected_bytes": int(row["size"]),
                    "expected_md5_base64": row["md5Hash"],
                    "license": "CC-BY-SA-4.0",
                    "license_url": "https://creativecommons.org/licenses/by-sa/4.0/",
                    "source_page": "https://www.hdrplusdata.org/dataset.html",
                    "purpose": "Real burst ordering, grouping and RAW preview regression; not human preference labels",
                }
            )
    return items


def camera_items() -> list[dict]:
    rows = json.loads((BASE / "rawpixls-catalog.json").read_bytes())["data"]
    formats = {"cr2", "cr3", "nef", "arw", "raf", "dng", "orf", "rw2", "pef", "srw", "3fr", "iiq"}
    selected, cameras, items = Counter(), set(), []
    for row in rows:
        link = re.search(r"href='([^']+)'", row[7])
        digest = re.search(r"Checksum'>([a-f0-9]{64})<", row[7])
        size = re.search(r"\(([\d.]+)MB\)", row[7])
        if "publicdomain/zero/1.0" not in row[5] or not link or not digest or not size:
            continue
        url = html.unescape(link[1])
        ext = Path(urllib.parse.urlparse(url).path).suffix.lower().lstrip(".")
        camera = f"{row[0]} {row[1]}"
        if (
            ext not in formats
            or selected[ext] >= 2
            or (ext, camera) in cameras
            or float(size[1]) > 40
        ):
            continue
        selected[ext] += 1
        cameras.add((ext, camera))
        file_id = re.search(r"getfile.php/(\d+)/", url)[1]
        items.append(
            {
                "dataset": "raw.pixls.us",
                "path": f"rawpixls/{file_id}.{ext}",
                "url": urllib.parse.quote(url, safe=":/"),
                "expected_sha256": digest[1],
                "license": "CC0-1.0",
                "license_url": "https://creativecommons.org/publicdomain/zero/1.0/",
                "camera": camera,
                "mode": row[2],
                "purpose": "RAW decoder compatibility only",
            }
        )
    return items


def download(item: dict) -> dict:
    record = PREPARE.persist(BASE, item)
    path = BASE / item["path"]
    if item.get("expected_bytes") and record["bytes"] != item["expected_bytes"]:
        raise ValueError(f"Size mismatch: {item['path']}")
    if item.get("expected_md5_base64"):
        digest = base64.b64encode(hashlib.md5(path.read_bytes()).digest()).decode()
        if digest != item["expected_md5_base64"]:
            raise ValueError(f"Official MD5 mismatch: {item['path']}")
    return record


def save_manifest(manifest: Path, records: dict) -> None:
    temporary = manifest.with_suffix(".tmp")
    temporary.write_text(
        json.dumps(list(records.values()), ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    for attempt in range(20):
        try:
            temporary.replace(manifest)
            return
        except PermissionError:
            if attempt == 19:
                raise
            time.sleep(0.1)


def download_items(items: list[dict], records: dict[str, dict], manifest: Path) -> None:
    errors = []
    with ThreadPoolExecutor(max_workers=16) as pool:
        futures = [pool.submit(download, item) for item in items]
        for index, future in enumerate(as_completed(futures), 1):
            try:
                item = future.result()
                records[item["path"]] = item
                save_manifest(manifest, records)
            except Exception as error:
                errors.append(str(error))
                print(f"Download failed: {error}", flush=True)
                continue
            if index % 10 == 0 or index == len(items):
                print(f"Verified {index}/{len(items)}; catalog {len(records)} images", flush=True)
    save_manifest(manifest, records)
    if errors:
        raise RuntimeError(f"{len(errors)} downloads failed; verified files retained for retry")


def main() -> None:
    manifest = BASE / "manifest.json"
    records = {item["path"]: item for item in json.loads(manifest.read_bytes())}
    items = burst_items() + camera_items()
    required = sum(
        item.get("expected_bytes", 42_000_000)
        for item in items
        if not (BASE / item["path"]).exists()
    )
    if shutil.disk_usage(BASE).free < required * 2 + 5_000_000_000:
        raise RuntimeError("Insufficient space for downloads and disposable verification copies")
    (BASE / "expansion-plan.json").write_text(json.dumps(items, indent=2) + "\n", encoding="utf-8")
    print(
        f"Plan: {len(items)} downloads, 60 complete real bursts, estimated {required / 1e9:.2f} GB",
        flush=True,
    )
    download_items(items, records, manifest)
    hashes = {item["sha256"] for item in records.values()}
    assert len(hashes) >= 500, f"Only {len(hashes)} distinct source files"
    report = {
        "files": len(records),
        "unique_sha256": len(hashes),
        "bytes": sum(item["bytes"] for item in records.values()),
        "formats": dict(Counter(Path(item["path"]).suffix.lower() for item in records.values())),
        "real_bursts": len({item["group_id"] for item in records.values() if "group_id" in item}),
    }
    (BASE / "coverage.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report), flush=True)


if __name__ == "__main__":
    main()
