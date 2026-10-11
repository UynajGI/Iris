use super::*;

impl AnalysisSettings {
    pub fn validate(&self) -> Result<()> {
        self.validate_device()?;
        self.validate_embedding()?;
        self.validate_occlusion()?;
        self.validate_detector()?;
        self.validate_scoring()
    }

    fn validate_device(&self) -> Result<()> {
        if self
            .directml_device_id
            .is_some_and(|id| id > i32::MAX as u32)
        {
            bail!("directml_device_id must fit a nonnegative 32-bit device index");
        }
        Ok(())
    }

    fn validate_embedding(&self) -> Result<()> {
        if let Some(hash) = &self.embedding_model_sha256 {
            if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                bail!("embedding_model_sha256 must contain exactly 64 hexadecimal characters");
            }
        }
        if let Some(value) = self.semantic_similarity_threshold {
            if !value.is_finite() || value <= 0. || value > 1. {
                bail!("semantic_similarity_threshold must be finite and satisfy 0 < value <= 1");
            }
        }
        if self.embedding_provider == EmbeddingProvider::Dinov3Vits16
            && (self.embedding_model_sha256.is_none()
                || self.semantic_similarity_threshold.is_none())
        {
            bail!("DINOv3 requires an explicit model SHA-256 and semantic similarity threshold");
        }
        Ok(())
    }

    fn validate_occlusion(&self) -> Result<()> {
        if let Some(hash) = &self.occlusion_model_sha256 {
            if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                bail!("occlusion_model_sha256 must contain exactly 64 hexadecimal characters");
            }
        }
        if let Some(value) = self.occlusion_min_visible_fraction {
            if !value.is_finite() || value <= 0. || value > 1. {
                bail!("occlusion_min_visible_fraction must be finite and satisfy 0 < value <= 1");
            }
        }
        if self.occlusion_provider == OcclusionProvider::Faceocc
            && (self.occlusion_model_sha256.is_none()
                || self.occlusion_min_visible_fraction.is_none())
        {
            bail!("FaceOcc requires an explicit model SHA-256 and minimum visible fraction; no calibrated default exists");
        }
        Ok(())
    }

    fn validate_detector(&self) -> Result<()> {
        if let Some(hash) = &self.scrfd_model_sha256 {
            if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                bail!("scrfd_model_sha256 must contain exactly 64 hexadecimal characters");
            }
        }
        if self.face_detector == FaceDetectorProvider::Scrfd500m
            && self.scrfd_model_sha256.is_none()
        {
            bail!("SCRFD requires an explicit scrfd_model_sha256");
        }
        Ok(())
    }

    fn validate_scoring(&self) -> Result<()> {
        let weights = [
            self.eyes_weight,
            self.sharpness_weight,
            self.face_weight,
            self.exposure_weight,
            self.smile_weight,
        ];
        if weights
            .iter()
            .any(|w| !w.is_finite() || *w < 0. || *w > 100.)
            || weights.iter().sum::<f64>() <= 0.
        {
            bail!("weights must be finite 0..100 with positive total");
        }
        if !(0.1..=0.99).contains(&self.face_confidence) || !(1..=10).contains(&self.max_faces) {
            bail!("invalid face confidence or face limit");
        }
        if !(0.0..=1.0).contains(&self.niqe_weight) {
            bail!("NIQE weight must be a fraction in 0..1");
        }
        if !(0.0..=100.0).contains(&self.reject_threshold)
            || !(0.0..=100.0).contains(&self.recommend_threshold)
            || self.reject_threshold >= self.recommend_threshold
        {
            bail!("verdict thresholds must satisfy 0 <= reject < recommend <= 100");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_keeps_device_then_model_then_score_error_precedence() {
        let mut settings = AnalysisSettings::default();
        settings.directml_device_id = Some(u32::MAX);
        settings.embedding_model_sha256 = Some("invalid".into());
        settings.occlusion_model_sha256 = Some("invalid".into());
        settings.scrfd_model_sha256 = Some("invalid".into());
        settings.eyes_weight = -1.;
        assert!(settings
            .validate()
            .unwrap_err()
            .to_string()
            .starts_with("directml_device_id"));
        settings.directml_device_id = None;
        assert!(settings
            .validate()
            .unwrap_err()
            .to_string()
            .starts_with("embedding_model_sha256"));
        settings.embedding_model_sha256 = None;
        assert!(settings
            .validate()
            .unwrap_err()
            .to_string()
            .starts_with("occlusion_model_sha256"));
        settings.occlusion_model_sha256 = None;
        assert!(settings
            .validate()
            .unwrap_err()
            .to_string()
            .starts_with("scrfd_model_sha256"));
        settings.scrfd_model_sha256 = None;
        assert!(settings
            .validate()
            .unwrap_err()
            .to_string()
            .starts_with("weights"));
        settings.eyes_weight = 25.;
        settings.validate().unwrap();
    }
}
