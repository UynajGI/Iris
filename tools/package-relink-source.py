"""Package only the standalone RAW converter and its LGPL dependency sources."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
COMPONENT = ROOT / "components/raw-decoder"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output must be new")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="iris-relink-") as temporary:
        stage = Path(temporary)
        for name in ["Cargo.toml", "Cargo.lock", "LICENSE", "README.md"]:
            shutil.copy2(COMPONENT / name, stage / name)
        shutil.copytree(COMPONENT / "src", stage / "src")
        vendor = subprocess.run(["cargo", "vendor", "--locked", "--respect-source-config", str(stage / "vendor")], cwd=COMPONENT, capture_output=True, text=True)
        if vendor.returncode:
            raise RuntimeError("cargo vendor failed: " + vendor.stderr[-6000:])
        config = vendor.stdout.replace(str(stage / "vendor").replace("\\", "\\\\"), "vendor").replace((stage / "vendor").as_posix(), "vendor")
        (stage / ".cargo").mkdir()
        (stage / ".cargo/config.toml").write_text(config, "utf-8")
        (stage / "RELINK.txt").write_text('''Standalone iris-raw-decoder source and offline dependency sources.
Includes rawler 0.8.0 under LGPL-2.1; see vendor/rawler/LICENSE*.
You may modify the LGPL library and rebuild this converter for your own use.
Reverse engineering for debugging your modifications to the LGPL library is permitted.
The converter wrapper is MIT licensed; see LICENSE. The GPL-3.0-or-later Iris
application does not link rawler. Its source is provided in a separate archive.

Install Rust stable with x86_64-pc-windows-gnu and its MinGW linker.
From this extracted directory: cargo +stable-x86_64-pc-windows-gnu build --offline --locked --release
The resulting target/release/iris-raw-decoder.exe replaces only that packaged file.
Alternatively set IRIS_RAW_DECODER_PATH to the absolute path of a compatible build.
The converter can also be run independently; see README.md. No photos/weights are included.
To modify rawler, copy vendor/rawler to modified-rawler, add to root Cargo.toml:
[patch.crates-io]
rawler = { path = "modified-rawler" }
Then run cargo build --offline --release (allow Cargo.lock update).
Retain license notices when distributing your modified version.
Only the standalone converter links rawler; core, daemon, CLI and shell do not.
''', "utf-8")
        # Prove the archive's relative vendor configuration resolves offline before shipping.
        subprocess.run(["cargo", "metadata", "--offline", "--locked", "--format-version", "1"], cwd=stage, check=True, stdout=subprocess.DEVNULL)
        with zipfile.ZipFile(args.output, "w", zipfile.ZIP_DEFLATED, compresslevel=6, strict_timestamps=False) as archive:
            for path in sorted(stage.rglob("*")):
                if path.is_file():
                    archive.write(path, path.relative_to(stage).as_posix())
    print(json.dumps({"source_archive": str(args.output), "sha256": hashlib.sha256(args.output.read_bytes()).hexdigest(), "bytes": args.output.stat().st_size}))


if __name__ == "__main__":
    main()
