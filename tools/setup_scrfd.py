"""Acquire the official optional SCRFD-500M for non-commercial local research.

Never enables the detector, modifies default model manifests, or installs the
recognition model carried by the upstream archive. No commercial grant is implied.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
REVISION = "3e6486942a1be2da0e5b475fac375ea73264bd21"
ARCHIVE_URL = "https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_sc.zip"
ARCHIVE_BYTES = 14_969_382
ARCHIVE_SHA256 = "57d31b56b6ffa911c8a73cfc1707c73cab76efe7f13b675a05223bf42de47c72"
MODEL_BYTES = 2_524_817
MODEL_SHA256 = "5e4447f50245bbd7966bd6c0fa52938c61474a04ec7def48753668a9d8b4ea3a"
LICENSE_URL = f"https://github.com/deepinsight/insightface/blob/{REVISION}/python-package/README.md#license"
OUTPUT_NAMES = ["443", "468", "493", "446", "471", "496", "449", "474", "499"]
LICENSE_NOTE = (
    "noncommercial-research only: InsightFace pretrained weights are available for "
    "non-commercial research purposes only, including manual downloads. The MIT "
    "code license does not license these weights commercially. Local optional "
    "research use only; excluded from general-purpose bundles. This metadata is "
    "a provenance declaration, not commercial authorization or accuracy evidence."
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def checked(data: bytes, size: int, checksum: str, name: str) -> bytes:
    if len(data) != size or sha256(data) != checksum:
        raise ValueError(f"{name} differs from pinned size/SHA-256")
    return data


def download(url: str, limit: int) -> bytes:
    with urllib.request.urlopen(url, timeout=60) as response:
        if not response.url.startswith("https://"):
            raise ValueError("Artifact redirect left HTTPS")
        data = response.read(limit + 1)
    if len(data) > limit:
        raise ValueError("Artifact exceeds expected size limit")
    return data


def save_exact(path: Path, data: bytes) -> None:
    if path.is_symlink():
        raise ValueError(f"Refusing symlink output: {path}")
    if path.exists():
        if path.read_bytes() != data:
            raise ValueError(f"Refusing to overwrite different local content: {path}")
        return
    with path.open("xb") as stream:
        stream.write(data)


def detector_from_archive(data: bytes) -> bytes:
    checked(data, ARCHIVE_BYTES, ARCHIVE_SHA256, "Official archive")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        # Select one literal member. Never extract paths or recognition weights.
        members = [entry for entry in archive.infolist() if entry.filename == "det_500m.onnx"]
        if len(members) != 1 or members[0].file_size != MODEL_BYTES:
            raise ValueError("Archive must contain exactly one expected detector")
        return checked(archive.read(members[0]), MODEL_BYTES, MODEL_SHA256, "Detector")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--research-only", action="store_true", required=True,
                        help="Explicitly select the non-commercial research workflow; not a commercial license")
    parser.add_argument("--archive", type=Path, help="Use an already downloaded archive, with identical hash checks")
    parser.add_argument("--output", type=Path, default=ROOT / "models/optional")
    parser.add_argument("--evidence", type=Path, default=ROOT / "artifacts/scrfd-research")
    args = parser.parse_args()
    args.evidence.mkdir(parents=True, exist_ok=True)
    archive = args.archive.read_bytes() if args.archive else download(ARCHIVE_URL, ARCHIVE_BYTES)
    model = detector_from_archive(archive)
    evidence = []
    for name, upstream in [
        ("InsightFace-MODEL-TERMS.md", "python-package/README.md"),
        ("InsightFace-CODE-LICENSE.txt", "LICENSE"),
        ("InsightFace-SCRFD-README.md", "detection/scrfd/README.md"),
    ]:
        url = f"https://raw.githubusercontent.com/deepinsight/insightface/{REVISION}/{upstream}"
        data = download(url, 512 * 1024)
        if name == "InsightFace-MODEL-TERMS.md" and b"non-commercial research purposes only" not in data:
            raise ValueError("Pinned upstream model-use restriction was not found")
        save_exact(args.evidence / name, data)
        evidence.append({"file": name, "url": url, "sha256": sha256(data), "bytes": len(data)})
    metadata = {"model_id": "scrfd_500m_kps", "source_url": ARCHIVE_URL,
                "license_url": LICENSE_URL, "license_note": LICENSE_NOTE}
    provenance = {
        "revision": REVISION, "source_url": ARCHIVE_URL,
        "archive_bytes": ARCHIVE_BYTES, "archive_sha256": ARCHIVE_SHA256,
        "archive_member": "det_500m.onnx", "model_bytes": MODEL_BYTES, "model_sha256": MODEL_SHA256,
        "model_bytes_modified": False, "output_names": OUTPUT_NAMES,
        "license_scope": "noncommercial-research", "commercial_authorization": False,
        "default_detector_changed": False, "general_distribution": False,
        "other_archive_models_installed": False, "license_evidence": evidence,
        "hash_provenance": "Locally measured pins of bytes obtained from the official HTTPS release; not a publisher signature",
    }
    args.output.mkdir(parents=True, exist_ok=True)
    for name, data in [
        ("scrfd_500m.onnx", model),
        ("scrfd_500m.metadata.json", (json.dumps(metadata, indent=2) + "\n").encode()),
        ("scrfd_500m.provenance.json", (json.dumps(provenance, indent=2) + "\n").encode()),
        ("SCRFD-MODEL-TERMS.md", (args.evidence / "InsightFace-MODEL-TERMS.md").read_bytes()),
    ]:
        save_exact(args.output / name, data)
    settings = {"face_detector": "scrfd_500m", "scrfd_model_sha256": MODEL_SHA256}
    # Enum serialization is checked against the application before use.
    save_exact(args.evidence / "settings.json", (json.dumps(settings, indent=2) + "\n").encode())
    print(json.dumps({"installed_optional": str(args.output.resolve()), "sha256": MODEL_SHA256,
                      "license_scope": "noncommercial-research", "accuracy": "not established"}, indent=2))


if __name__ == "__main__":
    main()
