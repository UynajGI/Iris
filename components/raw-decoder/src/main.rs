use anyhow::{bail, Context, Result};
use rawler::{decoders::RawDecodeParams, imgop::develop::RawDevelop, rawsource::RawSource};
use std::{io::Write, path::Path};

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 || (args[1] != "metadata" && args[1] != "develop") {
        bail!("usage: iris-raw-decoder metadata|develop <RAW file>");
    }
    let path = Path::new(&args[2]);
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
    anyhow::ensure!(
        image.width() > 0 && image.height() > 0,
        "RAW development produced empty image"
    );
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    write!(out, "P6\n{} {}\n255\n", image.width(), image.height())?;
    out.write_all(image.as_raw())?;
    out.flush()?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
