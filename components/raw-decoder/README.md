# Standalone RAW converter

This small MIT-licensed utility is separate from the GPL-3.0-or-later Iris application.
It links rawler 0.8.0 (LGPL-2.1); its complete source and dependency sources are
distributed so recipients can rebuild it with a modified library. This license
does not apply to Iris core, daemon, CLI or desktop sources.

```bash
iris-raw-decoder metadata photograph.dng
iris-raw-decoder develop photograph.dng > photograph.ppm
```

`metadata` prints JSON with width, height and EXIF orientation. `develop` emits
standard binary RGB PPM (P6, maxval 255), without applying orientation or resizing.
The caller can use any ordinary PPM reader. No Iris data structures, database,
models, application code or private protocol are used. Input is limited to
512 MiB, metadata to 120 MP, full sensor development to 24 MP. Errors go to
stderr and return a nonzero exit code. The program also works independently.

Build: `cargo build --release --locked --manifest-path components/raw-decoder/Cargo.toml`.
The distributed source archive has a standalone Cargo root and offline vendored
dependencies. Replace the installed `iris-raw-decoder.exe` with a compatible
modified build, or set `IRIS_RAW_DECODER_PATH` to its absolute path. Reverse
engineering for debugging modifications to the LGPL library is permitted.
