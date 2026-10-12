//! Tiny generated ONNX graphs, not downloaded model weights or photographic fixtures.
use super::tokenization::tests::{fixture, manifest};
use super::{status, RetrievalEncoder, VerifiedAssets};
use sha2::{Digest, Sha256};

fn varint(mut value: u64) -> Vec<u8> {
    let mut bytes = vec![];
    while value >= 128 {
        bytes.push(value as u8 | 128);
        value >>= 7;
    }
    bytes.push(value as u8);
    bytes
}
fn integer(field: u64, value: u64) -> Vec<u8> {
    let mut b = varint(field << 3);
    b.extend(varint(value));
    b
}
fn blob(field: u64, value: impl AsRef<[u8]>) -> Vec<u8> {
    let value = value.as_ref();
    let mut b = varint((field << 3) | 2);
    b.extend(varint(value.len() as u64));
    b.extend(value);
    b
}
fn message(parts: impl IntoIterator<Item = Vec<u8>>) -> Vec<u8> {
    parts.into_iter().flatten().collect()
}
fn value_info(name: &str, ty: u64, shape: &[u64]) -> Vec<u8> {
    let shape = message(shape.iter().map(|d| blob(1, integer(1, *d))));
    message([
        blob(1, name),
        blob(2, blob(1, message([integer(1, ty), blob(2, shape)]))),
    ])
}
fn attribute(name: &str, values: &[u64], kind: u64) -> Vec<u8> {
    let mut parts = vec![blob(1, name), integer(20, kind)];
    parts.extend(
        values
            .iter()
            .map(|v| integer(if kind == 2 { 3 } else { 8 }, *v)),
    );
    message(parts)
}
fn node(op: &str, inputs: &[&str], output: &str, attributes: Vec<Vec<u8>>) -> Vec<u8> {
    let mut parts: Vec<_> = inputs.iter().map(|i| blob(1, i)).collect();
    parts.extend([blob(2, output), blob(4, op)]);
    parts.extend(attributes.into_iter().map(|a| blob(5, a)));
    message(parts)
}
fn model(image: bool) -> Vec<u8> {
    let mut graph = vec![blob(2, "iris-synthetic-retrieval")];
    if image {
        graph.push(blob(
            1,
            node(
                "ReduceMean",
                &["pixel_values"],
                "image_embeds",
                vec![
                    attribute("axes", &[2, 3], 7),
                    attribute("keepdims", &[0], 2),
                ],
            ),
        ));
        graph.push(blob(11, value_info("pixel_values", 1, &[1, 3, 224, 224])));
        graph.push(blob(12, value_info("image_embeds", 1, &[1, 3])));
    } else {
        let table: [f32; 21] = [
            0., 0., 0., 0., 0., 0., 0., 0., 0., 1., 1., 1., 1., 0., 0., 0., 0., 1., 1., 1., 1.,
        ];
        let tensor = message([
            integer(1, 7),
            integer(1, 3),
            integer(2, 1),
            blob(8, "table"),
            blob(
                9,
                table
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>(),
            ),
        ]);
        graph.push(blob(5, tensor));
        graph.push(blob(
            1,
            node(
                "Gather",
                &["table", "input_ids"],
                "tokens",
                vec![attribute("axis", &[0], 2)],
            ),
        ));
        graph.push(blob(
            1,
            node(
                "ReduceSum",
                &["tokens"],
                "text_embeds",
                vec![attribute("axes", &[1], 7), attribute("keepdims", &[0], 2)],
            ),
        ));
        graph.push(blob(11, value_info("input_ids", 7, &[1, 4])));
        graph.push(blob(12, value_info("text_embeds", 1, &[1, 3])));
    }
    message([
        integer(1, 7),
        blob(2, "Iris generated test fixture"),
        blob(7, message(graph)),
        blob(8, integer(2, 11)),
    ])
}

#[test]
fn manifest_hash_license_and_tokenizer_contract_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    write_assets(temp.path());
    let assets = VerifiedAssets::load(temp.path()).unwrap();
    assert_eq!(assets.manifest.dimensions, 3);
    assert_eq!(status(temp.path()).state, "ready");
    let config = temp.path().join("optional/retrieval/config.json");
    let mut manifest = assets.manifest;
    manifest.license_reviewed = false;
    std::fs::write(&config, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(status(temp.path()).state, "invalid");
    manifest.license_reviewed = true;
    std::fs::write(config, serde_json::to_vec(&manifest).unwrap()).unwrap();
    std::fs::write(
        temp.path().join("optional/retrieval/image.onnx"),
        b"tampered",
    )
    .unwrap();
    assert!(VerifiedAssets::load(temp.path())
        .err()
        .unwrap()
        .to_string()
        .contains("SHA-256"));
}
fn write_assets(root: &std::path::Path) {
    let dir = root.join("optional/retrieval");
    std::fs::create_dir_all(&dir).unwrap();
    let mut manifest = manifest();
    for (name, bytes, hash) in [
        ("image.onnx", model(true), &mut manifest.image_sha256),
        ("text.onnx", model(false), &mut manifest.text_sha256),
        ("tokenizer.json", fixture(), &mut manifest.tokenizer_sha256),
    ] {
        *hash = format!("{:x}", Sha256::digest(&bytes));
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    std::fs::write(
        dir.join("config.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires explicitly configured IRIS_TEST_RUNTIME; uses generated ONNX graphs, no downloaded optional model"]
fn actual_local_dual_encoders_rank_synthetic_colors_without_network_or_quality_models() {
    let temp = tempfile::tempdir().unwrap();
    write_assets(temp.path());
    let runtime = std::path::PathBuf::from(
        std::env::var_os("IRIS_TEST_RUNTIME").expect("set the absolute pinned runtime path"),
    );
    assert!(runtime.is_absolute());
    std::fs::copy(&runtime, temp.path().join(runtime.file_name().unwrap())).unwrap();
    let mut encoder = RetrievalEncoder::load(temp.path()).unwrap();
    let red = encoder
        .embed_image(&image::RgbImage::from_pixel(
            320,
            240,
            image::Rgb([255, 0, 0]),
        ))
        .unwrap();
    let blue = encoder
        .embed_image(&image::RgbImage::from_pixel(
            240,
            320,
            image::Rgb([0, 0, 255]),
        ))
        .unwrap();
    let query = encoder.embed_text("red").unwrap();
    let dot = |v: &[f32]| v.iter().zip(&query).map(|(a, b)| a * b).sum::<f32>();
    assert!(dot(&red) > dot(&blue));
    assert!(encoder.embed_text("人像 blue").is_ok());
    assert!(encoder.embed_text("").is_err());
}
