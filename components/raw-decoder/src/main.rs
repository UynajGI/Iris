use anyhow::{bail, Context, Result};
use rawler::{decoders::RawDecodeParams, imgop::develop::RawDevelop, rawsource::RawSource};
use std::{io::Write, path::Path};

mod region;

fn valid_mode(mode: &std::ffi::OsStr) -> bool {
    mode == "metadata" || mode == "develop" || mode.to_string_lossy().starts_with("region:")
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 || !valid_mode(&args[1]) {
        bail!("usage: iris-raw-decoder metadata|develop|region:x,y,w,h <RAW file>");
    }
    let path = Path::new(&args[2]);
    if let Some(request) = args[1].to_string_lossy().strip_prefix("region:") {
        anyhow::ensure!(
            pixels <= 60_000_000,
            "Native regional RAW detail exceeds the 60MP sensor limit"
        );
        return region::develop(decoder.raw_image(&input, &params, false)?, request);
    }
    anyhow::ensure!(
        std::fs::metadata(path)?.len() <= 512 * 1024 * 1024,
        "RAW exceeds compressed input safety limit"
    );
    let input = RawSource::new(path)?;
    let decoder = rawler::get_decoder(&input)?;
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
    anyhow::ensure!(
        pixels <= 24_000_000,
        "RAW sensor exceeds 24MP development safety limit"
    );
    let raw = decoder.raw_image(&input, &params, false)?;
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
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
