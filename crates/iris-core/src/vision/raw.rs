//! Local RAW previews: ExifTool extraction, then an isolated PPM converter.
use super::{decode_jpeg_preview, orient, resize_preview, DecodedPreview};
use anyhow::{Context, Result};
use image::ImageDecoder;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_INPUT: u64 = 512 * 1024 * 1024;
const MAX_PREVIEW: u64 = 32 * 1024 * 1024;
const MAX_DEVELOP_PIXELS: u64 = 24_000_000;
const MAX_FLOAT_PIXELS: u64 = 64_000_000;

/// Native RGB crop plus oriented source geometry, with no preview fallback.
pub(super) fn native_region(
    path: &Path,
    region: [f32; 4],
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(image::RgbImage, u32, u32)> {
    anyhow::ensure!(
        std::fs::metadata(path)?.len() <= MAX_INPUT,
        "RAW input exceeds safety limit"
    );
    let mode = format!(
        "region:{},{},{},{}",
        region[0], region[1], region[2], region[3]
    );
    let bytes = convert(path, &mode, 4_000_000 * 3 + 4096, Some(cancel))?;
    parse_region(&bytes)
}

fn parse_region(bytes: &[u8]) -> Result<(image::RgbImage, u32, u32)> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Header {
        protocol: String,
        source_width: u32,
        source_height: u32,
        orientation: u32,
    }
    let split = bytes
        .iter()
        .take(1024)
        .position(|b| *b == b'\n')
        .context("RAW region protocol header missing")?;
    let header: Header = serde_json::from_slice(&bytes[..split])?;
    anyhow::ensure!(
        header.protocol == "iris-raw-region-v1"
            && (1..=8).contains(&header.orientation)
            && header.source_width > 0
            && header.source_height > 0
            && u64::from(header.source_width) * u64::from(header.source_height) <= 60_000_000,
        "invalid RAW region protocol geometry"
    );
    let mut reader = image::ImageReader::with_format(
        std::io::Cursor::new(&bytes[split + 1..]),
        image::ImageFormat::Pnm,
    );
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(4_000_000 * 3);
    reader.limits(limits);
    let decoder = reader.into_decoder()?;
    let (w, h) = decoder.dimensions();
    anyhow::ensure!(
        w >= 16
            && h >= 16
            && u64::from(w) * u64::from(h) <= 4_000_000
            && decoder.color_type() == image::ColorType::Rgb8,
        "RAW region exceeds native crop safety limit"
    );
    let image = orient(
        image::DynamicImage::from_decoder(decoder)?.to_rgb8(),
        header.orientation,
    );
    anyhow::ensure!(
        image.width() <= header.source_width && image.height() <= header.source_height,
        "RAW crop exceeds source geometry"
    );
    Ok((image, header.source_width, header.source_height))
}

pub(super) fn converter_revision() -> String {
    developer_executable()
        .and_then(|path| {
            let metadata = std::fs::metadata(&path)?;
            let time = metadata.modified()?.duration_since(std::time::UNIX_EPOCH)?;
            Ok(format!(
                "{}:{}:{}",
                path.display(),
                metadata.len(),
                time.as_nanos()
            ))
        })
        .unwrap_or_else(|_| "missing-raw-converter".into())
}

fn developer_executable() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("IRIS_RAW_DECODER_PATH") {
        let path = PathBuf::from(path);
        anyhow::ensure!(
            path.is_absolute() && path.is_file(),
            "explicit RAW converter must be an existing absolute path"
        );
        return Ok(path);
    }
    let name = format!("iris-raw-decoder{}", std::env::consts::EXE_SUFFIX);
    let executable = std::env::current_exe()?;
    let path = executable
        .parent()
        .context("executable has no parent")?
        .join(&name);
    if path.is_file() {
        return Ok(path);
    }
    #[cfg(debug_assertions)]
    {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../components/raw-decoder/target/release")
            .join(name);
        if path.is_file() {
            return Ok(path);
        }
    }
    anyhow::bail!("RAW converter missing; keep iris-raw-decoder beside the application")
}

/// High-precision editing path. Stream directly into FP32 buffers so a complete
/// serialized copy of the image is not retained alongside the decoded image.
pub(crate) fn decode_linear(
    path: &Path,
    white_balance: Option<[f32; 3]>,
    context: &iris_render::RenderContext,
) -> Result<iris_render::source::DecodedSource> {
    let _worker = float_worker_permit(context)?;
    anyhow::ensure!(
        std::fs::metadata(path)?.len() <= MAX_INPUT,
        "RAW input exceeds safety limit"
    );
    context.check_cancelled()?;
    let mut command = Command::new(developer_executable()?);
    command.arg("develop-float").arg(path);
    float_white_balance(&mut command, white_balance)?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().context("start float RAW decoder")?;
    #[cfg(windows)]
    let process_tree = crate::owned_job::OwnedJob::attach(&mut child)?;
    let stdout = child.stdout.take().context("RAW stdout missing")?;
    let stderr = child.stderr.take().context("RAW stderr missing")?;
    let reader_context = context.clone();
    let reader = std::thread::spawn(move || {
        let stream = stdout.take(MAX_FLOAT_PIXELS * 16 + 65536 + 13);
        iris_render::source::decode_raw_stream(stream, &reader_context)
    });
    let errors = std::thread::spawn(move || bounded_float_errors(stderr));
    let status = wait_for_converter(&mut child, Some(context.cancellation_flag()));
    #[cfg(windows)]
    process_tree.terminate();
    finish_float_decode(reader, errors, status)
}

fn finish_float_decode(
    reader: std::thread::JoinHandle<Result<iris_render::source::DecodedSource>>,
    errors: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
    status: Result<std::process::ExitStatus>,
) -> Result<iris_render::source::DecodedSource> {
    let decoded = reader
        .join()
        .map_err(|_| anyhow::anyhow!("RAW float reader panicked"))?;
    let errors = errors
        .join()
        .map_err(|_| anyhow::anyhow!("RAW stderr reader panicked"))??;
    anyhow::ensure!(
        status?.success(),
        "RAW float decode failed: {}",
        String::from_utf8_lossy(&errors)
    );
    decoded
}

fn bounded_float_errors(mut stderr: std::process::ChildStderr) -> std::io::Result<Vec<u8>> {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stderr.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let keep = count.min(8192_usize.saturating_sub(retained.len()));
        retained.extend_from_slice(&buffer[..keep]);
        // Continue draining after the diagnostic cap so a noisy converter
        // cannot block forever on its stderr pipe while stdout is waiting.
    }
    Ok(retained)
}

// The separate decoder's image buffers are not owned by RenderContext. Admit
// only one high-precision RAW worker per process; waiters remain cancellable.
fn float_worker_permit(
    context: &iris_render::RenderContext,
) -> Result<std::sync::MutexGuard<'static, ()>> {
    static WORKER: std::sync::Mutex<()> = std::sync::Mutex::new(());
    loop {
        context.check_cancelled()?;
        match WORKER.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(_)) => anyhow::bail!("RAW worker gate poisoned"),
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(Duration::from_millis(10))
            }
        }
    }
}

fn float_white_balance(command: &mut Command, white_balance: Option<[f32; 3]>) -> Result<()> {
    if let Some(wb) = white_balance {
        anyhow::ensure!(
            wb.iter()
                .all(|v| v.is_finite() && (0.01..=100.0).contains(v)),
            "invalid sensor white balance"
        );
        command.arg(format!("--wb={},{},{}", wb[0], wb[1], wb[2]));
    }
    Ok(())
}

fn convert(
    path: &Path,
    mode: &str,
    limit: u64,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<u8>> {
    let mut command = Command::new(developer_executable()?);
    command
        .arg(mode)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().context("start standalone RAW converter")?;
    #[cfg(windows)]
    let process_tree = crate::owned_job::OwnedJob::attach(&mut child)?;
    let stdout = child
        .stdout
        .take()
        .context("RAW converter stdout missing")?;
    let stderr = child
        .stderr
        .take()
        .context("RAW converter stderr missing")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let errors = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.take(8192).read_to_end(&mut bytes).map(|_| bytes)
    });
    let status = wait_for_converter(&mut child, cancel);
    #[cfg(windows)]
    process_tree.terminate();
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("RAW converter reader panicked"))??;
    let errors = errors
        .join()
        .map_err(|_| anyhow::anyhow!("RAW converter stderr reader panicked"))??;
    anyhow::ensure!(
        bytes.len() as u64 <= limit,
        "RAW converter output exceeds safety limit"
    );
    anyhow::ensure!(
        status?.success(),
        "RAW converter failed: {}",
        String::from_utf8_lossy(&errors)
    );
    Ok(bytes)
}

fn wait_for_converter(
    child: &mut std::process::Child,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<std::process::ExitStatus> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if cancel.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(anyhow::anyhow!("preview cancelled"));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            outcome => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(anyhow::anyhow!(
                    "RAW converter timeout or wait failure: {outcome:?}"
                ));
            }
        }
    }
}

fn executable(media: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = std::env::var_os("IRIS_EXIFTOOL_PATH") {
        let path = PathBuf::from(path);
        anyhow::ensure!(path.is_file(), "explicit ExifTool executable missing");
        return Ok(Some(path));
    }
    let models = if let Some(media) = media {
        media
            .parent()
            .context("media directory has no parent")?
            .to_owned()
    } else {
        std::env::current_exe()?
            .parent()
            .context("executable has no parent")?
            .join("models")
    };
    let path = models.join("raw/bin/exiftool.exe");
    if path.is_file() {
        return Ok(Some(path));
    }
    #[cfg(debug_assertions)]
    if media.is_none() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/raw/bin/exiftool.exe");
        if path.is_file() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn extract(
    executable: &Path,
    path: &Path,
    args: &[&str],
    limit: u64,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<u8>> {
    // Validate before spawning so invalid paths cannot leave a waiting child.
    let path = path.to_str().context("RAW path must be valid Unicode")?;
    anyhow::ensure!(
        !path.contains(['\r', '\n']),
        "RAW path contains an argument separator"
    );
    let mut command = Command::new(executable);
    command
        .args(["-config", "", "-charset", "filename=UTF8"])
        .args(args)
        .args(["-@", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().context("start local ExifTool")?;
    #[cfg(windows)]
    let process_tree = crate::owned_job::OwnedJob::attach(&mut child)?;
    // UTF-8 argument stream avoids Windows ANSI argv conversion for Unicode paths.
    if let Some(mut input) = child.stdin.take() {
        if let Err(error) = input.write_all(format!("{path}\n").as_bytes()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
    }
    let stdout = child.stdout.take().context("ExifTool stdout missing")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if cancel.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(anyhow::anyhow!("preview cancelled"));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            outcome => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(anyhow::anyhow!(
                    "ExifTool timeout or wait failure: {outcome:?}"
                ));
            }
        }
    };
    // Close descendant pipes as well before joining the output reader.
    #[cfg(windows)]
    process_tree.terminate();
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("ExifTool output reader panicked"))??;
    anyhow::ensure!(
        bytes.len() as u64 <= limit,
        "ExifTool output exceeds safety limit"
    );
    anyhow::ensure!(status?.success(), "ExifTool failed to read source");
    Ok(bytes)
}

fn metadata(
    path: &Path,
    media: Option<&Path>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(u32, u32, u32)> {
    anyhow::ensure!(
        std::fs::metadata(path)?.len() <= MAX_INPUT,
        "RAW exceeds compressed input safety limit"
    );
    if let Some(executable) = executable(media)? {
        if let Some(dimensions) = exif_metadata(&executable, path, cancel)? {
            return Ok(dimensions);
        }
    }
    let header: serde_json::Value =
        serde_json::from_slice(&convert(path, "metadata", 64 * 1024, cancel)?)?;
    let width = header["width"].as_u64().context("RAW width missing")?;
    let height = header["height"].as_u64().context("RAW height missing")?;
    anyhow::ensure!(
        width > 0
            && height > 0
            && width <= 65535
            && height <= 65535
            && width * height <= 120_000_000,
        "RAW exceeds pixel safety limit"
    );
    Ok((
        width as u32,
        height as u32,
        header["orientation"]
            .as_u64()
            .filter(|v| (1..=8).contains(v))
            .unwrap_or(1) as u32,
    ))
}

fn exif_metadata(
    executable: &Path,
    path: &Path,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Option<(u32, u32, u32)>> {
    let bytes = extract(
        executable,
        path,
        &[
            "-json",
            "-n",
            "-ImageWidth",
            "-ImageHeight",
            "-ExifImageWidth",
            "-ExifImageHeight",
            "-Orientation",
        ],
        64 * 1024,
        cancel,
    )?;
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes)?;
    if let Some(row) = rows.first() {
        let width = row["ExifImageWidth"]
            .as_u64()
            .or_else(|| row["ImageWidth"].as_u64())
            .unwrap_or(0);
        let height = row["ExifImageHeight"]
            .as_u64()
            .or_else(|| row["ImageHeight"].as_u64())
            .unwrap_or(0);
        if width > 0
            && height > 0
            && width <= 65535
            && height <= 65535
            && width * height <= 120_000_000
        {
            return Ok(Some((
                width as u32,
                height as u32,
                row["Orientation"]
                    .as_u64()
                    .filter(|v| (1..=8).contains(v))
                    .unwrap_or(1) as u32,
            )));
        }
    }
    Ok(None)
}

pub(super) fn dimensions(path: &Path, media: Option<&Path>) -> Result<(u32, u32)> {
    let (w, h, _) = metadata(path, media, None)?;
    Ok((w, h))
}

pub(super) fn decode(
    path: &Path,
    max_edge: u32,
    media: Option<&Path>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<DecodedPreview> {
    let (width, height, orientation) = metadata(path, media, cancel)?;
    if let Some(executable) = executable(media)? {
        for tag in ["-JpgFromRaw", "-PreviewImage", "-OtherImage"] {
            anyhow::ensure!(
                !cancel.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)),
                "preview cancelled"
            );
            let Ok(bytes) = extract(&executable, path, &["-b", tag], MAX_PREVIEW, cancel) else {
                continue;
            };
            if !bytes.starts_with(&[0xff, 0xd8]) {
                continue;
            }
            let mut temporary = tempfile::NamedTempFile::new()?;
            temporary.write_all(&bytes)?;
            // The embedded JPEG carries its own orientation when present. Apply
            // the RAW orientation only when the JPEG has none.
            let Ok(mut preview) = decode_jpeg_preview(temporary.path(), max_edge) else {
                continue;
            };
            let enough = preview.image.width().max(preview.image.height())
                >= max_edge.min(width.max(height)) / 2;
            if !enough {
                continue;
            }
            if preview.orientation == 1 {
                preview.image = orient(preview.image, orientation);
                preview.orientation = orientation;
            }
            preview.original_width = width;
            preview.original_height = height;
            preview.source = "raw_embedded_jpeg";
            return Ok(preview);
        }
    }
    anyhow::ensure!(
        u64::from(width) * u64::from(height) <= MAX_DEVELOP_PIXELS,
        "RAW has no adequate embedded JPEG; full development exceeds 24MP safety limit"
    );
    anyhow::ensure!(
        !cancel.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)),
        "preview cancelled"
    );
    let bytes = convert(path, "develop", MAX_DEVELOP_PIXELS * 3 + 1024, cancel)?;
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Pnm);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DEVELOP_PIXELS * 3);
    reader.limits(limits);
    let decoder = reader.into_decoder()?;
    let (w, h) = decoder.dimensions();
    anyhow::ensure!(
        u64::from(w) * u64::from(h) <= MAX_DEVELOP_PIXELS
            && decoder.color_type() == image::ColorType::Rgb8,
        "RAW converter must return bounded RGB8 PPM"
    );
    let developed = image::DynamicImage::from_decoder(decoder)?.to_rgb8();
    Ok(DecodedPreview {
        source: "raw_developed",
        image: resize_preview(orient(developed, orientation), max_edge),
        original_width: width,
        original_height: height,
        orientation,
    })
}

#[cfg(all(test, windows))]
mod cancellation_tests {
    use super::*;
    #[test]
    fn cancelling_native_converter_terminates_the_owned_child() {
        use std::os::windows::process::CommandExt;
        let mut child = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 30",
            ])
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let tree = crate::owned_job::OwnedJob::attach(&mut child).unwrap();
        let started = Instant::now();
        let error = wait_for_converter(&mut child, Some(&std::sync::atomic::AtomicBool::new(true)))
            .unwrap_err();
        tree.terminate();
        assert!(error.to_string().contains("cancelled"));
        assert!(child.try_wait().unwrap().is_some());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
