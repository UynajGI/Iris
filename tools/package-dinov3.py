"""Create a separate offline DINOv3 payload; never modify/install/enable an app."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import zipfile

SPEC = importlib.util.spec_from_file_location("setup_dinov3", Path(__file__).with_name("setup-dinov3.py"))
SETUP = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SETUP)
GPU_SPEC = importlib.util.spec_from_file_location("setup_dinov3_directml", Path(__file__).with_name("setup-dinov3-directml.py"))
GPU = importlib.util.module_from_spec(GPU_SPEC)
GPU_SPEC.loader.exec_module(GPU)

README = """# IrisVision optional DINOv3 offline model

This separate package contains Meta DINOv3 ViT-S/16 LVD-1689M weights converted
by onnx-community, plus the upstream license and fixed-source provenance.
It contains no SCRFD, photographs, credentials, executable or ONNX runtime.
The IrisVision general portable/installer packages exclude optional models.

## Offline deployment

1. Run `pwsh -NoProfile -File ./Verify.ps1` in this package to verify every file.
2. Stop IrisVision and its owned daemon normally. Choose the model root actually
   used by that installation (the `models` folder beside the portable programs,
   or the explicit `--model-dir` / IRIS_MODEL_DIR override). This package does
   not choose an installation or modify the registry.
3. Create `<model-root>/optional` if absent, then copy the payload files inside
   this package's `models/optional` into it. Preserve the DINOv3 license and
   metadata beside the model. Review existing same-named files before replacing
   them; leave other optional files unchanged.
4. Start IrisVision. Select `embedding_provider: dinov3_vits16`, set
   `embedding_model_sha256` to the hash below, and explicitly choose
   `semantic_similarity_threshold` in the supported range [0,1]. No threshold
   is recommended as calibrated. Merely copying this model does not enable it.
5. Reanalyze the desired project explicitly after changing these settings.
   Set `embedding_provider: none` to return to default grouping.

SHA-256: {sha256}
Model size: {size} bytes; one self-contained ONNX graph, no external tensor file.
Input: <=1280px RGB analysis preview resized to 224x224, ImageNet normalized.
Output: 384-dimensional CLS vector, L2-normalized locally.
DirectML fixed-shape graph included: {directml}. When included, choose
execution_provider=directml explicitly; CPU remains the default. The original
CPU graph is retained. All original learned initializer bytes are preserved in
the separate GPU graph; only input/Reshape dimensions are specialized.
Requires an IrisVision build with DINOv3 support and its existing ONNX Runtime.
No Python, Node, downloads or network inference are needed at deployment time.

## License and limits

Read models/optional/LICENSE-DINOv3.md (Meta DINOv3 License, August 19, 2025).
The custom license is not Apache/MIT. Redistribution must include the agreement;
research publications must acknowledge DINO Materials. All terms continue to apply.
The public conversion source is fixed in dinov3_vits16.metadata.json; it is not
an official Meta ONNX export. The gated official checkpoint is never requested.
No accuracy labels, similarity-threshold calibration, or full-dataset performance
claim is included. The checksums establish byte integrity, not publisher signing.
"""

VERIFY = r"""$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath($PSScriptRoot)
$manifest = Get-Content -LiteralPath (Join-Path $root 'checksums.json') -Raw | ConvertFrom-Json
if ($manifest.product -ne 'IrisVision-DINOv3-optional') { throw 'Wrong optional bundle manifest' }
$seen = @{}
foreach ($entry in $manifest.files) {
    $relative = [string]$entry.path
    if (-not $relative -or $relative -match '[\:]' -or [IO.Path]::IsPathRooted($relative) -or @($relative.Split('/') | Where-Object { $_ -in @('', '.', '..') }).Count -or $seen.ContainsKey($relative)) { throw 'Invalid optional manifest path' }
    $path = $root
    foreach ($part in $relative.Split('/')) {
        $path = Join-Path $path $part
        $item = Get-Item -LiteralPath $path
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Linked bundle entry' }
    }
    if ($item.PSIsContainer -or $item.Length -ne $entry.bytes -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ine $entry.sha256) { throw "Checksum mismatch: $relative" }
    $seen[$relative] = $true
}
foreach ($required in @('models/optional/dinov3_vits16.onnx','models/optional/LICENSE-DINOv3.md','models/optional/dinov3_vits16.metadata.json','README.md','Verify.ps1')) {
    if (-not $seen.ContainsKey($required)) { throw "Missing bundle file: $required" }
}
$expected = 5
if ($manifest.directml) {
    foreach ($required in @('models/optional/dinov3_vits16_directml.onnx','models/optional/dinov3_vits16_directml.metadata.json')) {
        if (-not $seen.ContainsKey($required)) { throw "Missing GPU bundle file: $required" }
    }
    $expected = 7
}
if ($seen.Count -ne $expected) { throw 'Unexpected optional bundle contents' }
Write-Output 'Verified separate DINOv3 offline payload. No installation or settings changes performed.'
"""


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def preflight(source, include_directml=False):
    files = [source / name for name in ("dinov3_vits16.onnx", "LICENSE-DINOv3.md", "dinov3_vits16.metadata.json")]
    if source.is_symlink() or any(not path.is_file() or path.is_symlink() for path in files):
        raise ValueError("DINOv3 payload must contain three regular source files")
    model, license_path, metadata_path = files
    if model.stat().st_size != SETUP.SIZE or digest(model) != SETUP.SHA256:
        raise ValueError("Pinned DINOv3 artifact mismatch")
    license_size, license_hash = SETUP.SOURCES["LICENSE.md"]
    if license_path.stat().st_size != license_size or digest(license_path) != license_hash:
        raise ValueError("Pinned DINOv3 license missing or modified")
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    if metadata.get("sha256") != SETUP.SHA256 or metadata.get("bytes") != SETUP.SIZE or metadata.get("source_revision") != SETUP.REVISION or metadata.get("source_repository") != SETUP.REPOSITORY or metadata.get("base_model") != "facebook/dinov3-vits16-pretrain-lvd1689m":
        raise ValueError("DINOv3 provenance does not match the pinned model")
    for name, (size, sha256) in SETUP.SOURCES.items():
        entry = metadata.get("sources", {}).get(name, {})
        if entry.get("sha256") != sha256 or entry.get("bytes") != size:
            raise ValueError(f"DINOv3 provenance mismatch: {name}")
    if include_directml:
        graph = source / GPU.FILE
        provenance = graph.with_suffix(".metadata.json")
        if any(not p.is_file() or p.is_symlink() for p in [graph, provenance]):
            raise ValueError("DirectML graph and provenance must be regular files")
        if graph.stat().st_size != GPU.SIZE or digest(graph) != GPU.SHA256:
            raise ValueError("DirectML artifact mismatch")
        info = json.loads(provenance.read_text("utf-8"))
        if info.get("sha256") != GPU.SHA256 or info.get("bytes") != GPU.SIZE or info.get("original_sha256") != SETUP.SHA256:
            raise ValueError("DirectML provenance mismatch")
        files.extend([graph, provenance])
    return files


def package(source, output, make_zip=True, include_directml=False):
    source, output = source.resolve(), output.absolute()
    archive = output.with_suffix(".zip")
    if output.exists() or make_zip and archive.exists():
        raise FileExistsError("Choose a new optional bundle path; existing artifacts are never replaced")
    files = preflight(source, include_directml)
    destination = output / "models/optional"
    destination.mkdir(parents=True)
    for path in files:
        shutil.copyfile(path, destination / path.name)
    # Verify copies again so a changed source cannot silently enter the artifact.
    preflight(destination, include_directml)
    (output / "README.md").write_text(README.format(sha256=SETUP.SHA256, size=SETUP.SIZE, directml=include_directml), encoding="utf-8")
    (output / "Verify.ps1").write_text(VERIFY, encoding="utf-8")
    entries = [{"path": path.relative_to(output).as_posix(), "bytes": path.stat().st_size, "sha256": digest(path)} for path in sorted(output.rglob("*")) if path.is_file()]
    manifest = {"product": "IrisVision-DINOv3-optional", "model_sha256": SETUP.SHA256, "source_revision": SETUP.REVISION, "directml": include_directml, "files": entries}
    (output / "checksums.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    if make_zip:
        with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as bundle:
            for path in sorted(output.rglob("*")):
                if path.is_file():
                    bundle.write(path, path.relative_to(output).as_posix())
    return {"ok": True, "directory": str(output), "manifest_sha256": digest(output / "checksums.json"), "files": len(entries), "archive": str(archive) if make_zip else None, "archive_sha256": digest(archive) if make_zip else None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parents[1] / "models/optional")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--no-zip", action="store_true")
    parser.add_argument("--include-directml", action="store_true")
    args = parser.parse_args()
    print(json.dumps(package(args.source, args.output, not args.no_zip, args.include_directml)))


if __name__ == "__main__":
    main()
