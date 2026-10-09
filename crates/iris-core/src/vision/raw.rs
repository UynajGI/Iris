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

fn convert(path: &Path, mode: &str, limit: u64) -> Result<Vec<u8>> {
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
    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
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
    };
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

fn extract(executable: &Path, path: &Path, args: &[&str], limit: u64) -> Result<Vec<u8>> {
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

fn metadata(path: &Path, media: Option<&Path>) -> Result<(u32, u32, u32)> {
    anyhow::ensure!(
        std::fs::metadata(path)?.len() <= MAX_INPUT,
        "RAW exceeds compressed input safety limit"
    );
    if let Some(executable) = executable(media)? {
        let bytes = extract(
            &executable,
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
                return Ok((
                    width as u32,
                    height as u32,
                    row["Orientation"]
                        .as_u64()
                        .filter(|v| (1..=8).contains(v))
                        .unwrap_or(1) as u32,
                ));
            }
        }
    }
    let header: serde_json::Value = serde_json::from_slice(&convert(path, "metadata", 64 * 1024)?)?;
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

pub(super) fn dimensions(path: &Path, media: Option<&Path>) -> Result<(u32, u32)> {
    let (w, h, _) = metadata(path, media)?;
    Ok((w, h))
}

pub(super) fn decode(path: &Path, max_edge: u32, media: Option<&Path>) -> Result<DecodedPreview> {
    let (width, height, orientation) = metadata(path, media)?;
    if let Some(executable) = executable(media)? {
        for tag in ["-JpgFromRaw", "-PreviewImage", "-OtherImage"] {
            let Ok(bytes) = extract(&executable, path, &["-b", tag], MAX_PREVIEW) else {
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
    let bytes = convert(path, "develop", MAX_DEVELOP_PIXELS * 3 + 1024)?;
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
