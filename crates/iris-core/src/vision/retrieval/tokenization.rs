use super::manifest::Manifest;
use anyhow::Result;
use tokenizers::{Tokenizer, TruncationParams};

pub(super) fn tokenizer(bytes: &[u8], manifest: &Manifest) -> Result<Tokenizer> {
    let mut tokenizer =
        Tokenizer::from_bytes(bytes).map_err(|e| anyhow::anyhow!("tokenizer: {e}"))?;
    tokenizer.with_padding(None);
    tokenizer
        .with_truncation(Some(TruncationParams {
            max_length: manifest.context_length,
            ..Default::default()
        }))
        .map_err(|e| anyhow::anyhow!("tokenizer truncation: {e}"))?;
    Ok(tokenizer)
}
pub(super) fn encode(
    tokenizer: &Tokenizer,
    text: &str,
    manifest: &Manifest,
) -> Result<(Vec<i64>, Vec<i64>)> {
    anyhow::ensure!(
        !text.trim().is_empty() && text.len() <= 4096,
        "enter a search query of 1..=4096 UTF-8 bytes"
    );
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("tokenization: {e}"))?;
    let tokens = encoding.get_ids();
    anyhow::ensure!(
        tokens.len() >= 2
            && tokens.len() <= manifest.context_length
            && tokens.first() == Some(&manifest.bos_token_id)
            && tokens.last() == Some(&manifest.eos_token_id),
        "tokenizer special tokens do not match the CLIP manifest"
    );
    let mut ids: Vec<_> = tokens.iter().map(|id| i64::from(*id)).collect();
    let mut mask = vec![1; ids.len()];
    ids.resize(manifest.context_length, i64::from(manifest.pad_token_id));
    mask.resize(manifest.context_length, 0);
    Ok((ids, mask))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    pub(crate) fn fixture() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"version":"1.0","truncation":null,"padding":null,"added_tokens":[],
            "normalizer":{"type":"Lowercase"},"pre_tokenizer":{"type":"Whitespace"},"decoder":null,
            "post_processor":{"type":"TemplateProcessing","single":[{"SpecialToken":{"id":"<s>","type_id":0}},{"Sequence":{"id":"A","type_id":0}},{"SpecialToken":{"id":"</s>","type_id":0}}],"pair":[{"Sequence":{"id":"A","type_id":0}},{"Sequence":{"id":"B","type_id":1}}],
                "special_tokens":{"<s>":{"id":"<s>","ids":[1],"tokens":["<s>"]},"</s>":{"id":"</s>","ids":[2],"tokens":["</s>"]}}},
            "model":{"type":"WordLevel","vocab":{"[UNK]":0,"<s>":1,"</s>":2,"portrait":3,"red":4,"blue":5,"人像":6},"unk_token":"[UNK]"}})).unwrap()
    }
    pub(crate) fn manifest() -> Manifest {
        Manifest {
            provider: "clip_dual_encoder_v1".into(),
            label: "Synthetic test, no retrieval accuracy claim".into(),
            license_reviewed: true,
            image_sha256: String::new(),
            text_sha256: String::new(),
            tokenizer_sha256: String::new(),
            dimensions: 3,
            context_length: 4,
            bos_token_id: 1,
            eos_token_id: 2,
            pad_token_id: 0,
        }
    }
    #[test]
    fn configured_tokenizer_preserves_unicode_special_tokens_truncation_and_padding() {
        let manifest = manifest();
        let tokenizer = tokenizer(&fixture(), &manifest).unwrap();
        assert_eq!(
            encode(&tokenizer, "RED", &manifest).unwrap(),
            (vec![1, 4, 2, 0], vec![1, 1, 1, 0])
        );
        assert_eq!(
            encode(&tokenizer, "人像 blue portrait red", &manifest)
                .unwrap()
                .0,
            vec![1, 6, 5, 2]
        );
        assert!(encode(&tokenizer, "   ", &manifest).is_err());
        let invalid = Manifest {
            bos_token_id: 999,
            ..manifest
        };
        assert!(encode(&tokenizer, "portrait", &invalid).is_err());
    }
}
