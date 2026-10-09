"""Acquire licensed public portraits and native RAW/HEIC compatibility samples.

Originals are read-only test inputs. No model, inference, or identity API is called.
The per-file manifest retains attribution, license, source and SHA-256.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import html
import json
from pathlib import Path
import re
import time
import urllib.parse
import urllib.request
import urllib.error

ROOT = Path(__file__).resolve().parents[1]
USER_AGENT = "IrisVision/0.1 (local image decoder compatibility research)"
REVISION = "5332a9cf0220e9c6c93f88a187daa90808051e10"


def fetch(url, limit=96 * 1024 * 1024):
    for attempt in range(3):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
            with urllib.request.urlopen(req, timeout=60) as response:
                data = response.read(limit + 1)
            if len(data) > limit:
                raise ValueError("download exceeds bound")
            return data
        except Exception as error:
            if attempt == 2:
                raise
            if isinstance(error, urllib.error.HTTPError) and error.code == 429:
                delay = max(30, min(180, int(error.headers.get("Retry-After", "60"))))
                print(f"rate limited; waiting {delay}s", flush=True)
                time.sleep(delay)
            else:
                time.sleep(2 * (attempt + 1))


def download(entry, directory):
    path = directory / entry["path"]
    if path.exists():
        data = path.read_bytes()
    else:
        data = fetch(entry["url"])
    sha = hashlib.sha256(data).hexdigest()
    if entry.get("sha256") and entry["sha256"] != sha:
        raise ValueError(f"source hash mismatch: {entry['path']}")
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        path.write_bytes(data)
    return {**entry, "sha256": sha, "bytes": len(data)}


def portraits(directory, count, width):
    manifest = directory / "manifest.json"
    if manifest.exists():
        data = json.loads(manifest.read_text("utf-8"))
        assert data["count"] == count
        for entry in data["files"]:
            download(entry, directory)
        return
    selection = directory / "selection.json"
    if selection.exists():
        selected = json.loads(selection.read_text("utf-8"))
        assert len(selected) == count
        finish_portraits(directory, selected, count)
        return
    selected = []
    for category in ["Portrait photographs of women", "Portrait photographs of men"]:
        continuation = {}
        target = count // 2 if not selected else count
        while len(selected) < target:
            params = {"action": "query", "generator": "categorymembers", "gcmtitle": "Category:" + category,
                      "gcmtype": "file", "gcmlimit": 50, "prop": "imageinfo", "iiprop": "url|extmetadata|size", "iiurlwidth": width, "format": "json", **continuation}
            result = json.loads(fetch("https://commons.wikimedia.org/w/api.php?" + urllib.parse.urlencode(params), 8 * 1024 * 1024))
            for page in sorted(result.get("query", {}).get("pages", {}).values(), key=lambda p: p["pageid"]):
                if len(selected) >= target:
                    break
                if not page["title"].lower().endswith((".jpg", ".jpeg")):
                    continue
                if not page.get("imageinfo") or any(e["path"] == f"commons-{page['pageid']}.jpg" for e in selected):
                    continue
                info = page["imageinfo"][0]
                meta = info["extmetadata"]
                license_name = meta.get("LicenseShortName", {}).get("value", "")
                if not (license_name.startswith(("CC BY", "CC0")) or license_name == "Public domain"):
                    continue
                if "NC" in license_name or "ND" in license_name or info["size"] > 6 * 1024 * 1024 or info["width"] * info["height"] > 20_000_000:
                    continue
                if max(info["width"], info["height"]) < 640:
                    continue
                selected.append({"path": f"commons-{page['pageid']}.jpg", "url": info.get("thumburl", info["url"]), "original_url": info["url"], "source_page": info["descriptionurl"],
                    "title": page["title"], "license": license_name, "license_url": meta.get("LicenseUrl", {}).get("value"),
                    "author": meta.get("Artist", {}).get("value"), "credit": meta.get("Credit", {}).get("value"),
                    "width": info.get("thumbwidth", info["width"]), "height": info.get("thumbheight", info["height"]), "category": category,
                    "modifications": f"Wikimedia standard {width}px thumbnail; original source URL retained"})
            continuation = result.get("continue")
            if not continuation:
                break
    if len(selected) != count:
        raise ValueError(f"only {len(selected)} eligible public portraits for requested {count}")
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "selection.json").write_text(json.dumps(selected, ensure_ascii=False, indent=2), "utf-8")
    finish_portraits(directory, selected, count)


def finish_portraits(directory, selected, count):
    files = []
    for item in selected:
        if not (directory / item["path"]).exists():
            time.sleep(2)
        entry = download(item, directory)
        files.append(entry)
        print(f"portrait {len(files)}/{count}: {entry['path']}", flush=True)
    (directory / "manifest.json").write_text(json.dumps({"count": count, "description": "independent Commons public portrait photographs; per-file license and attribution",
        "accuracy_labels": False, "files": files}, ensure_ascii=False, indent=2), "utf-8")


def native(directory):
    rows = json.loads(fetch("https://raw.pixls.us/json/getrepository.php", 16 * 1024 * 1024))["data"]
    entries = []
    for identifier, filename in [(771, "canon-5d3.cr2"), (958, "nikon-d7000.nef"), (846, "sony-a7r.arw"), (915, "iphone-6s.dng")]:
        row = next(r for r in rows if f"getfile.php/{identifier}/" in r[7])
        assert "zero/1.0" in row[5]
        url = html.unescape(re.search(r"href='([^']+)'", row[7]).group(1))
        url = urllib.parse.quote(url, safe=":/")
        sha = re.search(r"[a-f0-9]{64}", row[7]).group()
        entries.append({"path": filename, "url": url, "sha256": sha, "license": "CC0-1.0", "license_url": "https://creativecommons.org/publicdomain/zero/1.0/",
            "camera_make": row[0], "camera_model": row[1], "source_page": "https://raw.pixls.us/", "repository_record": row})
    for source, filename in [("IMG_5195.HEIC", "iphone-5195.heic"), ("mobile/iphone_13_pro_max.HEIC", "iphone-13-pro-max.heic"),
                             ("mobile/HMD_Nokia_8.3_5G.heif", "nokia-8-3.heif"), ("mobile/HMD_Nokia_8.3_5G_hdr.heif", "nokia-8-3-hdr.heif")]:
        entries.append({"path": filename, "url": f"https://raw.githubusercontent.com/ianare/exif-samples/{REVISION}/heic/{source}",
            "license": "CC-BY-SA-4.0", "license_url": "https://creativecommons.org/licenses/by-sa/4.0/",
            "author": "Exif Samples contributors; see source repository history", "source_page": f"https://github.com/ianare/exif-samples/tree/{REVISION}/heic/{source}",
            "license_evidence": f"https://github.com/ianare/exif-samples/blob/{REVISION}/README.rst"})
    directory.mkdir(parents=True, exist_ok=True)
    files = []
    with ThreadPoolExecutor(3) as pool:
        for entry in pool.map(lambda e: download(e, directory), entries):
            files.append(entry)
            print(f"native {len(files)}/{len(entries)}: {entry['path']}", flush=True)
    (directory / "manifest.json").write_text(json.dumps({"count": len(files), "files": files}, ensure_ascii=False, indent=2), "utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/public-corpus")
    parser.add_argument("--portraits", type=int, default=100)
    parser.add_argument("--portrait-width", type=int, choices=[960, 1280], default=960)
    parser.add_argument("--finalize-existing", action="store_true", help="Record only already downloaded licensed portraits; no network requests")
    parser.add_argument("--native-only", action="store_true")
    args = parser.parse_args()
    if args.finalize_existing:
        directory = args.output / f"portraits-{args.portrait_width}"
        selected = json.loads((directory / "selection.json").read_text("utf-8"))
        present = [e for e in selected if (directory / e["path"]).is_file()]
        assert present
        finish_portraits(directory, present, len(present))
    else:
        native(args.output / "native")
        if not args.native_only:
            portraits(args.output / f"portraits-{args.portrait_width}", args.portraits, args.portrait_width)
