"""Include locked Rust and installed frontend dependency sources with a beta."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Output must be new")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="iris-dependency-source-") as temporary:
        stage = Path(temporary)
        result = subprocess.run(["cargo", "vendor", "--locked", "--respect-source-config", "--sync", "apps/shell/src-tauri/Cargo.toml", "--sync", "components/raw-decoder/Cargo.toml", str(stage / "vendor")], cwd=ROOT, capture_output=True, text=True, check=True)
        config = result.stdout.replace(str(stage / "vendor").replace("\\", "\\\\"), "vendor").replace((stage / "vendor").as_posix(), "vendor")
        (stage / "cargo-config.toml").write_text(config, encoding="utf-8")
        shutil.copytree(ROOT / "apps/shell/node_modules", stage / "frontend-node_modules", ignore=shutil.ignore_patterns(".bin", ".cache"))
        shutil.copy2(ROOT / "apps/shell/package-lock.json", stage / "frontend-package-lock.json")
        (stage / "BUILD.txt").write_text("Extract the matching Iris application source archive first. Copy vendor/ to its root and replace .cargo/config.toml with cargo-config.toml from this archive. Copy frontend-node_modules/ to apps/shell/node_modules/. Build from the application source root with cargo build --offline --locked --release, then npm --prefix apps/shell run build and cargo build --offline --locked --release --manifest-path apps/shell/src-tauri/Cargo.toml --features desktop. Set RUSTUP_TOOLCHAIN=stable on macOS/Linux. System toolchains and platform SDKs remain prerequisites. Dependency directories retain their upstream license/source files. The independent RAW converter also has a separate relink archive. See the application README and licensing documentation.\n", encoding="utf-8")
        with zipfile.ZipFile(args.output, "x", zipfile.ZIP_DEFLATED, compresslevel=6, strict_timestamps=False) as archive:
            for path in sorted(stage.rglob("*")):
                if path.is_file():
                    archive.write(path, path.relative_to(stage).as_posix())
    print(args.output)


if __name__ == "__main__":
    main()
