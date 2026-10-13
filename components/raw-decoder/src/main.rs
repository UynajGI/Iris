use anyhow::{bail, Context, Result};
use rawler::{decoders::RawDecodeParams, imgop::develop::RawDevelop, rawsource::RawSource};
use std::{io::Write, path::Path};

mod float;
mod region;

fn valid_mode(mode: &std::ffi::OsStr) -> bool {
    mode == "metadata"
        || mode == "develop"
        || mode == "develop-float"
        || mode.to_string_lossy().starts_with("region:")
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    let wb = validate_arguments(&args)?;
    let path = Path::new(&args[2]);
    anyhow::ensure!(
        std::fs::metadata(path)?.len() <= 512 * 1024 * 1024,
        "RAW exceeds compressed input safety limit"
    );
    let input = RawSource::new(path)?;
    let decoder = rawler::get_decoder(&input)?;
    if args[1] == "develop-float" {
        float::validate_calibration(decoder.as_ref())?;
    }
    let params = RawDecodeParams::default();
    let header = decoder.raw_image(&input, &params, true)?;
    let pixels = header.width as u64 * header.height as u64;
    anyhow::ensure!(
        pixels > 0 && pixels <= 120_000_000,
        "RAW exceeds pixel safety limit"
    );
    if args[1] == "metadata" {
        println!(
            "{}",
            serde_json::json!({
                "width": header.width, "height": header.height,
                "orientation": header.orientation.to_u16()
            })
        );
        return Ok(());
    }
    if let Some(request) = args[1].to_string_lossy().strip_prefix("region:") {
        anyhow::ensure!(
            pixels <= 60_000_000,
            "Native regional RAW detail exceeds the 60MP sensor limit"
        );
        return region::develop(decoder.raw_image(&input, &params, false)?, request);
    }
    let max_pixels = if args[1] == "develop-float" {
        float::MAX_PIXELS as u64
    } else {
        24_000_000
    };
    anyhow::ensure!(
        pixels <= max_pixels,
        "RAW sensor exceeds mode development safety limit"
    );
    let raw = decoder.raw_image(&input, &params, false)?;
    if args[1] == "develop-float" {
        return float::develop(raw, wb);
    }
    develop_preview(&raw)
}

fn develop_preview(raw: &rawler::RawImage) -> Result<()> {
    let image = RawDevelop::default()
        .develop_intermediate(&raw)?
        .to_dynamic_image()
        .context("RAW development produced no image")?
        .to_rgb8();
    write_ppm(image.width(), image.height(), image.as_raw())
}

fn write_ppm(width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    anyhow::ensure!(
        width > 0 && height > 0,
        "RAW development produced empty image"
    );
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    write!(out, "P6\n{} {}\n255\n", width, height)?;
    out.write_all(pixels)?;
    out.flush()?;
    Ok(())
}

fn main() {
    let float_mode = std::env::args_os()
        .nth(1)
        .is_some_and(|mode| mode == "develop-float");
    if float_mode {
        std::panic::set_hook(Box::new(|_| {}));
    }
    let result = if float_mode {
        std::panic::catch_unwind(run).unwrap_or_else(|_| {
            Err(anyhow::anyhow!(
                "RAW decoder panicked; unsupported or malformed sensor data"
            ))
        })
    } else {
        run()
    };
    if let Err(error) = result {
        if float_mode {
            eprintln!(
                "{}",
                serde_json::json!({"version": 1, "error": "raw_float_decode_failed", "message": format!("{error:#}")})
            );
        } else {
            eprintln!("{error:#}");
        }
        std::process::exit(1);
    }
}

fn validate_arguments(args: &[std::ffi::OsString]) -> Result<Option<[f32; 3]>> {
    if args.len() < 3
        || !valid_mode(&args[1])
        || (args.len() != 3 && !(args[1] == "develop-float" && args.len() == 4))
    {
        bail!("usage: iris-raw-decoder metadata|develop|develop-float|region:x,y,w,h <RAW file> [--wb=r,g,b]");
    }
    let wb = if args[1] == "develop-float" {
        float::parse_wb(args.get(3).map(|s| s.to_string_lossy()).as_deref())?
    } else {
        None
    };
    Ok(wb)
}
