use super::{
    manifest::{Manifest, VerifiedAssets},
    tokenization,
};
use anyhow::{Context, Result};
use image::{
    imageops::{self, FilterType},
    GenericImageView, RgbImage,
};
use ort::{
    session::Session,
    value::{Tensor, TensorElementType, ValueType},
};
use std::path::Path;

pub struct RetrievalEncoder {
    manifest: Manifest,
    pub space_key: String,
    image: Session,
    text: Session,
    tokenizer: tokenizers::Tokenizer,
}
impl RetrievalEncoder {
    pub fn load(dir: &Path) -> Result<Self> {
        let assets = VerifiedAssets::load(dir)?;
        super::super::runtime::initialize(dir)?;
        let image = super::super::runtime::session(&assets.image, "retrieval-image")?;
        let text = super::super::runtime::session(&assets.text, "retrieval-text")?;
        validate(&image, &text, &assets.manifest)?;
        let tokenizer = tokenization::tokenizer(&assets.tokenizer, &assets.manifest)?;
        let mut encoder = Self {
            manifest: assets.manifest,
            space_key: assets.space_key,
            image,
            text,
            tokenizer,
        };
        // Both outputs must inhabit the declared same-dimensional space, even before indexing.
        encoder.embed_image(&RgbImage::from_pixel(224, 224, image::Rgb([128; 3])))?;
        encoder.embed_text("portrait")?;
        Ok(encoder)
    }
    pub fn embed_image(&mut self, image: &RgbImage) -> Result<Vec<f32>> {
        anyhow::ensure!(
            image.width() > 0 && image.height() > 0 && image.width().max(image.height()) <= 1280,
            "retrieval requires a bounded preview"
        );
        let shortest = image.width().min(image.height()) as f64;
        let w = ((image.width() as f64 * 224. / shortest).round() as u32).max(224);
        let h = ((image.height() as f64 * 224. / shortest).round() as u32).max(224);
        anyhow::ensure!(
            u64::from(w) * u64::from(h) <= 4_000_000,
            "retrieval aspect ratio exceeds resize limit"
        );
        let resized = imageops::resize(image, w, h, FilterType::CatmullRom);
        let crop = imageops::crop_imm(&resized, (w - 224) / 2, (h - 224) / 2, 224, 224);
        let mut values = vec![0.; 3 * 224 * 224];
        let mean = [0.48145466, 0.4578275, 0.40821073];
        let std = [0.26862954, 0.2613026, 0.2757771];
        for (i, pixel) in crop.pixels().enumerate() {
            for c in 0..3 {
                values[c * 224 * 224 + i] = (f32::from(pixel.2[c]) / 255. - mean[c]) / std[c];
            }
        }
        let output = self
            .image
            .run(ort::inputs!["pixel_values"=>Tensor::from_array(([1usize,3,224,224],values))?])?;
        extract(&output["image_embeds"], self.manifest.dimensions)
    }
    pub fn embed_text(&mut self, text: &str) -> Result<Vec<f32>> {
        let (ids, mask) = tokenization::encode(&self.tokenizer, text, &self.manifest)?;
        let shape = [1usize, self.manifest.context_length];
        let output = if self.text.inputs().len() == 2 {
            self.text.run(ort::inputs!["input_ids"=>Tensor::from_array((shape,ids))?,"attention_mask"=>Tensor::from_array((shape,mask))?])?
        } else {
            self.text
                .run(ort::inputs!["input_ids"=>Tensor::from_array((shape,ids))?])?
        };
        extract(&output["text_embeds"], self.manifest.dimensions)
    }
}
fn extract(value: &ort::value::DynValue, dimensions: usize) -> Result<Vec<f32>> {
    let (shape, values) = value.try_extract_tensor::<f32>()?;
    anyhow::ensure!(
        shape[..] == [1, dimensions as i64],
        "retrieval encoder returned an incompatible output shape"
    );
    normalize(values, dimensions)
}
pub fn normalize(values: &[f32], dimensions: usize) -> Result<Vec<f32>> {
    anyhow::ensure!(
        values.len() == dimensions && values.iter().all(|v| v.is_finite()),
        "retrieval vector dimensions or values invalid"
    );
    let norm = values
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    anyhow::ensure!(
        norm.is_finite() && norm > 1e-12,
        "retrieval vector has zero/invalid norm"
    );
    Ok(values
        .iter()
        .map(|v| (f64::from(*v) / norm) as f32)
        .collect())
}
fn tensor(value: &ValueType, ty: TensorElementType, shape: &[i64]) -> bool {
    matches!(value,ValueType::Tensor {ty:actual,shape:actual_shape,..} if *actual==ty && actual_shape.len()==shape.len() && actual_shape.iter().zip(shape).all(|(a,b)|*a==*b || *a == -1))
}
fn validate(image: &Session, text: &Session, manifest: &Manifest) -> Result<()> {
    anyhow::ensure!(
        image.inputs().len() == 1
            && image.inputs()[0].name() == "pixel_values"
            && tensor(
                image.inputs()[0].dtype(),
                TensorElementType::Float32,
                &[1, 3, 224, 224]
            ),
        "image encoder requires float32 pixel_values [1,3,224,224]"
    );
    anyhow::ensure!(
        (1..=2).contains(&text.inputs().len())
            && text.inputs().iter().all(|input| matches!(
                input.name(),
                "input_ids" | "attention_mask"
            ) && tensor(
                input.dtype(),
                TensorElementType::Int64,
                &[1, manifest.context_length as i64]
            ))
            && text
                .inputs()
                .iter()
                .any(|input| input.name() == "input_ids"),
        "text encoder requires int64 input_ids and optional attention_mask"
    );
    for (session, name) in [(image, "image_embeds"), (text, "text_embeds")] {
        let output = session
            .outputs()
            .iter()
            .find(|o| o.name() == name)
            .context("retrieval encoder output name incompatible")?;
        anyhow::ensure!(
            tensor(
                output.dtype(),
                TensorElementType::Float32,
                &[1, manifest.dimensions as i64]
            ),
            "retrieval output must be float32 [1,dimensions]"
        );
    }
    Ok(())
}
