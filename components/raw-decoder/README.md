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
models or application code are used by these modes. Input is limited to
512 MiB, metadata to 120 MP, full sensor development to 24 MP. Errors go to
stderr and return a nonzero exit code. The program also works independently.

Build: `cargo build --release --locked --manifest-path components/raw-decoder/Cargo.toml`.
The distributed source archive has a standalone Cargo root and offline vendored
dependencies. Replace the installed `iris-raw-decoder.exe` with a compatible
modified build, or set `IRIS_RAW_DECODER_PATH` to its absolute path. Reverse
engineering for debugging modifications to the LGPL library is permitted.

## Scene-linear FP32 mode

```bash
iris-raw-decoder develop-float photograph.dng > photograph.irisf32
iris-raw-decoder develop-float photograph.dng --wb=2,1,1.5 > photograph.irisf32
```

`develop-float` is a separate scene-linear input path, with no preview fallback.
It normalizes sensor samples as `(sample - black) / (white - black)`, performs
rawler's PPG RGB Bayer interpolation, applies green-normalized as-shot
camera white balance (or the explicit three camera gains), and transforms to
linear Rec.2020 / D65. Black noise below zero, over-white samples, and negative
out-of-gamut RGB are preserved as finite FP32. No display gamma, tone mapping,
highlight compression, gamut clamp or 8/16-bit RGB intermediate is applied.
PPG is edge-directed, with bilinear three-pixel borders; sensor ROIs smaller
than 6×6 use an explicitly reported bilinear fallback. This is not a completed
high-quality camera renderer or a claim of measured camera-color accuracy.

The explicit calibration is `inverse(row_normalize(XYZ_to_camera_D65 *
Rec2020_to_XYZ_D65))`, applied to white-balanced camera RGB. Row normalization
maps balanced camera neutral to working-space neutral. Only a recorded D65
matrix is accepted; there is no identity fallback, illuminant interpolation or
unverified chromatic adaptation. Camera/profile look tables, DNG calibration
chains, lens corrections and highlight reconstruction are not implemented.
The scene reference is normalized sensor white, not absolute radiometry.

The protocol is:

1. Eight magic bytes `IRISF32\0`.
2. Little-endian `u32` JSON-header byte length, at most 65,536.
3. UTF-8 JSON with `version: 1`, `width`, `height`, `channels: 4`,
   `color_space: "linear_rec2020_d65"`, `reference_kind: "scene"`,
   `source_kind: "raw"`, `orientation` (EXIF 1–8),
   `orientation_applied: false`, `alpha: "straight"`, `sample_format: "f32_le"`.
   `sensor_bit_depth` is the optional decoder-reported sensor precision (null
   when unknown), independently of the FP32 transport precision.
   `input_processing` records decoder version, level values and repeat layout,
   applied WB gains, matrix, interpolation, crop and disabled display processing.
4. Exactly `width * height * 4` little-endian IEEE-754 `f32` values, row-major
   RGBA with alpha 1. No padding or trailing payload. Orientation is left to
   the consumer, and dimensions refer to the cropped, unrotated sensor image.

Compressed input remains limited to 512 MiB. Float development accepts up to
64 MP full sensor dimensions (even when the default crop is smaller), covering
the nominal 45/61 MP range. Legacy 8-bit `develop` retains its 24 MP limit.
Maximum float pixel payload is 1,024,000,000 bytes. Consumers must enforce bounds before allocation, reject
nonfinite samples, require exact payload length and successful process exit.
All semantic validation completes before any stdout byte. An output I/O failure
may produce a truncated frame, which is not a valid result. Float-mode failures
produce a JSON error on stderr and nonzero exit; decoder panics are contained
at the executable boundary.

Serialization validates every transformed sample in a first pass, then computes
the same transform while streaming bytes. It does not retain a full RGBA copy.
The normalized sensor and decoded samples are released after demosaic. At peak,
the known worker image buffers are at most `20 * sensor_pixels` bytes (decoded
FP32 + normalized FP32 + three-channel FP32 demosaic; integer sensors use 18).
At 64 MP this is 1.28 GB, plus compressed input/file mappings, allocator overhead
and decoder-specific scratch. The header's `worker_memory_estimate` explicitly
excludes those extras and is not a hard process memory bound. Parent-side frame,
orientation, editing and output memory are additional; a 2 GiB total budget does
not imply that 64 MP full-resolution processing fits. Callers must serialize or
admit RAW workers separately, and enforce their own parent-render budgets.

The initial supported sensor set is one-plane 2×2 RGB Bayer with RGB camera
plane ordering, valid as-shot gains (unless overridden), scalar or 2×2 black
levels, scalar or four-site white levels, and a nonsingular D65 3×3 matrix.
Four-site levels follow rawler's 2×2 sensor-site convention, preserving the two
green sites' distinct black/white levels. Distinct fourth-green WB coefficients
are rejected rather than silently merged. X-Trans, four-color CFA, monochrome,
already-linear RGB RAW, Fuji rotated sensors, unsupported calibration/level
layouts and missing/invalid orientation fail explicitly. Camera decoder support
alone does not imply float-mode support or validated color accuracy.
DNG additionally requires explicitly tagged D65 calibration; nonidentity
CameraCalibration/AnalogBalance, linearization tables, black-level deltas and
sensor OpcodeList1/2 are rejected because this path cannot apply them correctly.
Identity calibration tags are accepted. This deliberately narrows supported
DNGs instead of silently omitting required sensor corrections.

Implementation evidence: the pinned rawler 0.8.0 source in `src/imgop/develop.rs`
enables `Rescale`, `Calibrate` and `SRgb` by default. `src/imgop/raw.rs` clips
negative values during rescaling and calls nonlinear `clip_euclidean_norm_avg`
during calibration; disabling `SRgb` alone therefore does not give an unclipped
linear pipeline. This mode bypasses that development pipeline and uses rawler
for sensor decoding, metadata and the direct `PPGDemosaic` operation on our
unclipped normalized FP32 sensor. Auditing `src/imgop/sensor/bayer/ppg.rs`
confirmed gradient/hue interpolation without range clipping, gamma or tone
mapping, and ROI-relative CFA phase adjustment. It has `width-6`/`height-6`
interior loops, hence the guarded small-image fallback.

The X-Trans implementation was also audited: it averages every channel in a
5×5 window, including the measured channel, with zero fallback for missing
neighbors. It is not a quality equivalent to Bayer PPG. Its different sensor
level/crop calibration has not been validated here, so X-Trans stays explicitly
unsupported in float mode. Source is available in the
[pinned crate](https://docs.rs/crate/rawler/0.8.0/source/); Context7 had no matching
rawler documentation entry when this path was implemented.

Synthetic unit tests cover binary FP32 preservation below zero/above one,
sub-8-bit differences, nonfinite rejection before output, matrix neutral/range,
bilinear edge behavior, all four Bayer PPG phases with odd ROI offsets,
negative/overwhite PPG reference values, two-pass streaming equivalence, the
64 MP header bound and WB validation. Run them in CI with
`cargo test --locked --manifest-path components/raw-decoder/Cargo.toml`.
This change has not been locally compiled or tested; real-camera color fidelity,
full-resolution performance and broader sensor validation remain outstanding.

The optional version-1 `raw_calibration` header retains `xyz_to_camera_d65`,
`camera_to_working`, green-normalized `as_shot_gains` (null when unavailable),
and `applied_gains`. This is the same validated D65 matrix used for development,
not a second color interpretation. The parent can recover demosaiced normalized
camera values by inverting `camera_to_working` and dividing by `applied_gains`.
No extra sensor plane is transmitted. A source neutral-patch picker therefore
samples demosaiced RGB, not original independent CFA photosites; it must reject
near-black/saturated patches and operate before edit geometry/display mapping.

The parent's sensor WB preparation supports explicitly selected CIE daylight
(4000-25000 K) or Kang (2002) Planckian (1667-25000 K) approximations, with tint
specified in CIE 1960 v units. These use the one D65 camera matrix and do not
claim proprietary DCP dual-illuminant interpolation or spectrally exact camera
response under arbitrary LED/fluorescent sources. The stored edit value remains
three normalized sensor gains; source pixels are never modified.
