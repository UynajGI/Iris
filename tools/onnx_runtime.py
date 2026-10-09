"""Pinned native CPU runtimes for the macOS/Linux beta packages."""
import hashlib
import io
import platform
import tarfile
import urllib.request

RUNTIMES = {
    ("Linux", "x86_64"): ("linux-x64", "8344d55f93d5bc5021ce342db50f62079daf39aaafb5d311a451846228be49b3", "libonnxruntime.so.1.22.0", "libonnxruntime.so", "3da6146e14e7b8aaec625dde11d6114c7457c87a5f93d744897da8781e35c673"),
    ("Darwin", "arm64"): ("osx-arm64", "cab6dcbd77e7ec775390e7b73a8939d45fec3379b017c7cb74f5b204c1a1cc07", "libonnxruntime.1.22.0.dylib", "libonnxruntime.dylib", "2b885992d3d6fa4130d39ec84a80d7504ff52750027c547bb22c86165f19406a"),
    ("Darwin", "x86_64"): ("osx-x86_64", "e4ec94a7696de74fb1b12846569aa94e499958af6ffa186022cfde16c9d617f0", "libonnxruntime.1.22.0.dylib", "libonnxruntime.dylib", "283e595e61cf65df7a6b1d59a1616cbd35c8b6399dd90d799d99b71a3ff83160"),
}


def install(root):
    key = (platform.system(), platform.machine())
    if key not in RUNTIMES:
        raise ValueError(f"No pinned runtime for {key}")
    asset, digest, library, destination, library_hash = RUNTIMES[key]
    target = root / destination
    notices = ["LICENSE", "ThirdPartyNotices.txt"]
    if target.exists() and hashlib.sha256(target.read_bytes()).hexdigest() == library_hash and all((root / ("LICENSE-ONNXRuntime.txt" if n == "LICENSE" else "ThirdPartyNotices-ONNXRuntime.txt")).exists() for n in notices):
        return
    prefix = f"onnxruntime-{asset}-1.22.0"
    url = f"https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/{prefix}.tgz"
    data = urllib.request.urlopen(url, timeout=120).read()
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError("ONNX runtime archive SHA-256 mismatch")
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        contents = archive.extractfile(f"{prefix}/lib/{library}").read()
        if hashlib.sha256(contents).hexdigest() != library_hash:
            raise ValueError("ONNX runtime library SHA-256 mismatch")
        target.write_bytes(contents)
        for name in notices:
            output = "LICENSE-ONNXRuntime.txt" if name == "LICENSE" else "ThirdPartyNotices-ONNXRuntime.txt"
            (root / output).write_bytes(archive.extractfile(f"{prefix}/{name}").read())
    print(f"Verified ONNX Runtime 1.22.0 {asset}")
