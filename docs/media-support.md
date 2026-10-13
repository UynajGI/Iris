# Local image decoding

The scanner and preview/analysis decoder support JPEG (`jpg`, `jpeg`), PNG,
WebP, TIFF (`tif`, `tiff`), HEVC-in-HEIF (`heic`, `heif`), and RAW (`dng`, `cr2`, `cr3`, `nef`, `nrw`,
`arw`, `srw`, `orf`, `rw2`, `pef`, `raf`, `rwl`, `raw`). A scanned extension does
not establish support for every camera variant. PNG/WebP/TIFF use the
Rust `image` codecs; HEIC uses bundled libheif 1.23.6 and libde265 1.1.3 through
their C API. No cloud service, Windows Store codec or Python runtime participates
in product decoding. RAW extraction uses bundled standalone ExifTool. The separate
`iris-raw-decoder.exe` utility links rawler 0.8.0; the application core does not.

JPEG retains its reduced-IDCT path and existing 120-million-pixel safety check;
it does not decode a full RGB frame and then resize. PNG/WebP/TIFF decode the
primary still image locally and resize without cropping to the requested edge.
HEIC first selects the smallest native thumbnail whose edge meets the request,
is smaller than the primary and matches its aspect ratio within 1%; absent or
failed thumbnail decoding falls back to the primary. Primary dimensions remain
in metadata, native transforms apply once, and cache filenames use `v2`.
These raster/HEIC selection paths have a 60-million-pixel cap; PNG/WebP/TIFF also reject decoded
buffers above 256 MiB. HEIC compressed input is limited to 256 MiB and its native
context has an approximately 60-million-pixel area limit. Native/codec working
memory is additional to output buffers; these checks are not a whole-process
memory guarantee. Animated WebP is represented by its default still frame;
multi-image HEIF is represented by its primary image.

PNG/WebP/TIFF EXIF orientation is applied once. HEIF crop/mirror/rotation is applied
by libheif; the result is not rotated again from EXIF. Transparent pixels are
composited onto white before scoring and JPEG cache generation. HEIC files with
other codecs such as AV1 are outside this HEVC-only runtime's support.

RAW first reads metadata and extracts `JpgFromRaw`, `PreviewImage`, or `OtherImage`
using ExifTool (15-second command deadline, 64 KiB metadata / 32 MiB preview cap).
The embedded JPEG goes through reduced-IDCT and must reach at least half the
requested edge; smaller previews trigger rawler development. There is no upscaling
guarantee. RAW inputs are capped at 512 MiB, metadata at 120 MP, and full rawler
development at 24 MP. A large RAW without an adequate embedded JPEG may therefore
fail explicitly. The converter returns ordinary JSON metadata or RGB8 P6 PPM;
its stdout is bounded, metadata/decode operations have a 120-second deadline,
and the host validates output pixel/allocation limits. Decode errors are isolated
per file. Windows job objects own worker/ExifTool/converter descendants so
cancellation and timeout also terminate children.
RAW orientation applies when the JPEG does not already carry its own transform.
This RGB8 path is for selection previews and scoring. It is separate from the
high-precision editing path described below.
The explicit original-detail action additionally requests a versioned native RAW
region from the converter. Sensor decoding/scaling still reads the full mosaic;
only the padded ROI is demosaiced and calibrated to RGB. It supports conventional
CFA layouts (including supported Bayer/X-Trans), up to a 60 MP sensor and a 4 MP
requested crop, using the RAW default crop and EXIF orientation. Rotated Fuji
layouts and unsupported CFA arrangements fail explicitly. This does not establish
color agreement with an embedded JPEG or a photographic RAW editor.
The result includes a JPEG region for inspection (maximum displayed edge 2048)
and scale-dependent measurements (maximum edge 1024). The UI reports any display
reduction and allows pixel-size viewing. Cancellation kills the owned converter;
successful results are cached by source/region/method/converter revision. Missing
or unsupported conversion is not cached as a successful native pass. Broad regions
may require choosing a smaller area or a face; HEIC native detail remains deferred.
The paired-operation fixture uses a real camera RAW with a JPEG rendered from
its preview. It validates pairing and linked operations, not a separately
downloaded camera-card RAW+JPEG pair or color agreement between those captures.

Run `make raw` to provision the hash-pinned standalone
ExifTool 13.59.3 package into `<models>/raw`. `IRIS_EXIFTOOL_PATH` overrides its
executable explicitly; custom model roots do not fall back to developer runtimes.
Portable packaging validates the entire runtime manifest, includes licenses and
`sources/raw-decoder-source.zip` (only the standalone converter/dependency source,
offline Cargo configuration and instructions for rebuilding with modified LGPL-2.1
rawler). This archive covers only the independent converter; application source is supplied separately.
Build the converter with `cargo build --release --locked --manifest-path
components/raw-decoder/Cargo.toml`. Keep its executable beside Iris, or set
`IRIS_RAW_DECODER_PATH` to an explicit absolute path of a compatible replacement.

## High-precision editing input and output

`iris-render` and the shared core editing services decode the original source
into linear Rec.2020 D65 FP32, preserving supported raster precision instead of
reading the selection JPEG cache. TIFF import and native selection preview are
available; the editing decoder retains supported 16-bit TIFF/PNG samples and
applies RGB or grayscale ICC input transforms. HEIC high-precision editing and
CMYK raster decoding are not implied by selection support.

RAW editing uses a separate versioned float protocol from the standalone
converter, not embedded JPEG or PPM8. Its current boundary is standard RGB 2x2
Bayer, up to 64 MP full sensor and 512 MiB compressed input, explicit D65
calibration and PPG demosaicing. X-Trans and complex unsupported DNG calibration
fail explicitly on this path even when an older selection/detail path can read
them. Sensor white balance and neutral sampling prepare gains from RAW
calibration; creative rendered-RGB balance remains a separate operation.

Terminal output supports JPEG8, PNG16 and TIFF16 with explicit color settings.
The native edited-region endpoint returns 1:1 final output pixels but currently
renders the entire graph before extraction; it is not the older sensor ROI
optimization. Manual or explicitly imported Iris lens calibrations do not claim
Lensfun coverage or automatic profile downloads. Full limits and known memory,
color-fidelity and hardware-validation gaps are maintained in
[the traditional editing implementation](color-engine-implementation.md).
Current float-pipeline results are recorded in [CI evidence](ci-verification.md);
the historical selection/codec checks below are separate evidence.

## Runtime provisioning and distribution

Run `make media` with x64 MinGW GCC, mingw32-make and
CMake available. It verifies pinned upstream source SHA-256 hashes, builds only
the HEVC decoder (external plugins and encoders disabled), and creates
`models/media`. Distribution must include that **entire directory**, including
`manifest.json`, `sources`, `licenses` and the three DLLs:

- `libheif.dll`
- `libde265.dll`
- `libwinpthread-1.dll`

The DLL dependency closure is checked against these files and Windows system
libraries. Both codec libraries are LGPL-3.0-or-later and dynamically loaded.
Exact corresponding source archives and the build script are included alongside
their licenses. The script supports an offline rebuild using `--source-dir`.
Compatible modified DLLs can replace the supplied libraries.

Runtime lookup prefers `IRIS_MEDIA_RUNTIME_DIR`, then an explicit media directory
passed by the application (normally `<model-dir>/media`), then
`<executable-dir>/models/media`. Debug builds additionally have a source-tree
fallback. An explicitly configured missing directory produces an error, rather
than silently selecting another runtime. Lookup never depends on the current
working directory. Missing HEIC support is a per-file scan/decode error; the
scanner does not silently skip HEIC files as an unsupported extension.

## Local evidence

`crates/iris-core/tests/media.rs` covers PNG/WebP orientation and alpha,
pre-allocation PNG dimension rejection, format-aware scanning and cache writes,
real HEIC color pixels and 90-degree HEIF rotation, Unicode filenames, migrated
cache writes, and explicit missing-runtime/corrupt-file failures. HEIC tests are
opt-in because they require native DLL provisioning; they were explicitly run
locally. The small CC0 synthetic fixtures contain no user photos. Fixture
preparation used pillow-heif only; product decoding uses the built DLLs.

`crates/iris-daemon/tests/media.rs` additionally exercises authenticated HTTP
background scanning, correct original MIME types and unchanged bytes, JPEG
thumbnail/preview responses for all four formats, and an explicitly configured
Unicode model directory. The latter first checks that a missing custom runtime
fails, so an accidental fallback to developer DLLs cannot mask lost settings.

Expanded native-camera, public-portrait, 24 MP PNG/WebP, thumbnail, RAW+JPEG
operations and memory evidence is indexed in [the completion report](validation.md)
and [the validation overview](validation.md).
Native HDR-file decode success does not validate HDR tone mapping, ICC/color
fidelity or arbitrary HEIF codecs. All hardware evidence is from one Windows machine.
