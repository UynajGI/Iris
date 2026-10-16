// SPDX-License-Identifier: GPL-3.0-or-later
//! A cold request decodes RAW; warm temperature/as-shot requests reuse bounded
//! calibration metadata. No measured latency claim is made. Warm identity checks
//! inherit validation.rs's metadata-cache caveat; clear_source_validation forces
//! the next source check to hash bytes. Neutral picking always strongly validates
//! and decodes the original. No decoded image is retained by this cache.
use super::{decode_source, validation, EditRenderJob};
use crate::Services;
use anyhow::{ensure, Context, Result};
use iris_render::{
    region::PixelRect,
    white_balance::{self, RawCalibration, WhiteBalanceLocus, WhiteBalanceTemperature},
    RenderContext,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawWhiteBalanceRequest {
    AsShot,
    /// Interpret draft sensor gains without changing the saved edit.
    Gains {
        gains_rgb: [f32; 3],
        locus: WhiteBalanceLocus,
    },
    Temperature {
        kelvin: f64,
        tint_delta_v: f64,
        locus: WhiteBalanceLocus,
    },
    /// Original orientation-normalized RAW pixels, before edit geometry.
    Neutral {
        rect: PixelRect,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PreparedRawWhiteBalance {
    pub gains_rgb: [f32; 3],
    pub revision: u64,
    pub source_fingerprint: String,
    pub calibration: RawCalibration,
    pub sampled_pixels: Option<u32>,
    /// None means the gains cannot be expressed within the supported model.
    pub temperature: Option<WhiteBalanceTemperature>,
}

pub struct RawWhiteBalanceJob {
    render: EditRenderJob,
}

impl Services {
    /// Capture revision/path while holding catalog access; call job.prepare
    /// outside that lock. Preparing gains does not mutate the edit document.
    pub fn prepare_raw_white_balance(
        &self,
        id: &str,
        expected_revision: u64,
    ) -> Result<RawWhiteBalanceJob> {
        let record = self.edit_document(id)?;
        ensure!(
            record.revision == expected_revision,
            "edit revision changed"
        );
        let source = self.photo_path(record.photo_id)?;
        ensure!(
            crate::vision::supported_format(&source) == Some("raw"),
            "sensor white balance requires RAW input"
        );
        Ok(RawWhiteBalanceJob {
            render: EditRenderJob {
                record,
                source,
                resources: Default::default(),
            },
        })
    }
}

impl RawWhiteBalanceJob {
    /// The first request decodes the original; later temperature/as-shot requests
    /// reuse at most 64 verified calibration records after source identity checks.
    /// Neutral patches always decode and strongly hash the full original.
    pub fn prepare(
        &self,
        context: &RenderContext,
        request: &RawWhiteBalanceRequest,
    ) -> Result<PreparedRawWhiteBalance> {
        validate_request(request)?;
        let (gains_rgb, calibration) = if matches!(request, RawWhiteBalanceRequest::Neutral { .. })
        {
            self.decode_gains(context, request)?
        } else {
            self.metadata_gains(context, request)?
        };
        validate_savable_gains(gains_rgb)?;
        let temperature = match request {
            RawWhiteBalanceRequest::Temperature {
                kelvin,
                tint_delta_v,
                locus,
            } => Some(WhiteBalanceTemperature {
                kelvin: *kelvin,
                tint_delta_v: *tint_delta_v,
                locus: *locus,
            }),
            _ => white_balance::temperature_from_gains(
                &calibration,
                gains_rgb,
                match request {
                    RawWhiteBalanceRequest::Gains { locus, .. } => *locus,
                    _ => WhiteBalanceLocus::Planckian,
                },
            )?,
        };
        context.check_cancelled()?;
        Ok(PreparedRawWhiteBalance {
            temperature,
            gains_rgb,
            revision: self.render.record.revision,
            source_fingerprint: self.render.record.document.source.fingerprint.clone(),
            calibration,
            sampled_pixels: match request {
                RawWhiteBalanceRequest::Neutral { rect } => Some(rect.width * rect.height),
                _ => None,
            },
        })
    }

    fn metadata_gains(
        &self,
        context: &RenderContext,
        request: &RawWhiteBalanceRequest,
    ) -> Result<([f32; 3], RawCalibration)> {
        // Same validator as validate_preview_source(false); retain its identity
        // token so we can compare the exact observed file again after lookup.
        let (identity, mode) = validation::validate(
            &self.render.source,
            &self.render.record.document.source.fingerprint,
            false,
            context,
        )?;
        let key = self.cache_key()?;
        let calibration = calibration_cache()?.get(&key, &identity);
        if mode == validation::SourceValidation::MetadataCache {
            if let Some(calibration) = calibration {
                let gains = metadata_gains(request, &calibration)?;
                validation::check_unchanged(&self.render.source, &identity, context)?;
                return Ok((gains, calibration));
            }
        }
        // StrongHash also follows explicit refresh; decode rather than reusing
        // a potentially old decoder calibration when the validation hint ended.
        self.decode_gains(context, request)
    }

    fn cache_key(&self) -> Result<CalibrationKey> {
        Ok(CalibrationKey {
            canonical: self.render.source.canonicalize()?,
            fingerprint: self.render.record.document.source.fingerprint.clone(),
        })
    }

    fn decode_gains(
        &self,
        context: &RenderContext,
        request: &RawWhiteBalanceRequest,
    ) -> Result<([f32; 3], RawCalibration)> {
        self.render.verify_source(context)?;
        let identity = validation::SourceIdentity::capture(&self.render.source)?;
        let decoded = decode_source(&self.render.source, None, context)?;
        let _frame_memory = context.reserve(decoded.frame.byte_len())?;
        self.render.verify_source(context)?;
        validation::check_unchanged(&self.render.source, &identity, context)?;
        let calibration = decoded
            .metadata
            .raw_calibration
            .context("RAW decoder did not supply verified white balance calibration")?;
        calibration.validate()?;
        let gains = prepare_gains(request, &decoded.frame, &calibration, context)?;
        calibration_cache()?.remember(self.cache_key()?, identity, calibration.clone());
        Ok((gains, calibration))
    }
}

fn validate_savable_gains(gains: [f32; 3]) -> Result<()> {
    ensure!(
        gains.iter().all(|gain| gain.is_finite() && (0.01..=100.0).contains(gain)),
        "suggested sensor white balance exceeds the editable gain range 0.01..=100; no clamping was applied"
    );
    Ok(())
}

fn validate_request(request: &RawWhiteBalanceRequest) -> Result<()> {
    match request {
        RawWhiteBalanceRequest::AsShot => (),
        RawWhiteBalanceRequest::Gains { gains_rgb, .. } => {
            white_balance::validate_gains(*gains_rgb)?;
            validate_savable_gains(*gains_rgb)?;
        }
        RawWhiteBalanceRequest::Temperature {
            kelvin,
            tint_delta_v,
            locus,
        } => {
            white_balance::illuminant_xy(*kelvin, *tint_delta_v, *locus)?;
        }
        RawWhiteBalanceRequest::Neutral { rect } => {
            rect.validate()?;
            ensure!(
                rect.width <= 64 && rect.height <= 64,
                "neutral patch maximum is 64x64"
            );
        }
    }
    Ok(())
}

fn prepare_gains(
    request: &RawWhiteBalanceRequest,
    frame: &iris_render::Frame,
    calibration: &RawCalibration,
    context: &RenderContext,
) -> Result<[f32; 3]> {
    match request {
        RawWhiteBalanceRequest::Neutral { rect } => {
            white_balance::neutral_patch_gains(frame, calibration, *rect, context)
        }
        _ => metadata_gains(request, calibration),
    }
}

fn metadata_gains(
    request: &RawWhiteBalanceRequest,
    calibration: &RawCalibration,
) -> Result<[f32; 3]> {
    match request {
        RawWhiteBalanceRequest::AsShot => calibration
            .as_shot_gains
            .context("as-shot white balance unavailable"),
        RawWhiteBalanceRequest::Gains { gains_rgb, .. } => Ok(*gains_rgb),
        RawWhiteBalanceRequest::Temperature {
            kelvin,
            tint_delta_v,
            locus,
        } => white_balance::temperature_gains(calibration, *kelvin, *tint_delta_v, *locus),
        RawWhiteBalanceRequest::Neutral { .. } => {
            anyhow::bail!("neutral patch requires original pixels")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CalibrationKey {
    canonical: PathBuf,
    fingerprint: String,
}
struct CalibrationEntry {
    identity: validation::SourceIdentity,
    calibration: RawCalibration,
    touched: u64,
}
#[derive(Default)]
struct CalibrationCache {
    entries: BTreeMap<CalibrationKey, CalibrationEntry>,
    clock: u64,
}

fn calibration_cache() -> Result<std::sync::MutexGuard<'static, CalibrationCache>> {
    static CACHE: OnceLock<Mutex<CalibrationCache>> = OnceLock::new();
    CACHE
        .get_or_init(|| Mutex::new(CalibrationCache::default()))
        .lock()
        .map_err(|_| anyhow::anyhow!("RAW calibration cache poisoned"))
}
impl CalibrationCache {
    fn get(
        &mut self,
        key: &CalibrationKey,
        identity: &validation::SourceIdentity,
    ) -> Option<RawCalibration> {
        self.clock = self.clock.saturating_add(1);
        let entry = self.entries.get_mut(key)?;
        if entry.identity != *identity {
            return None;
        }
        entry.touched = self.clock;
        Some(entry.calibration.clone())
    }
    fn remember(
        &mut self,
        key: CalibrationKey,
        identity: validation::SourceIdentity,
        calibration: RawCalibration,
    ) {
        if self.entries.len() >= 64 && !self.entries.contains_key(&key) {
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(key, _)| key.clone())
            {
                self.entries.remove(&oldest);
            }
        }
        self.clock = self.clock.saturating_add(1);
        self.entries.insert(
            key,
            CalibrationEntry {
                identity,
                calibration,
                touched: self.clock,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calibration() -> RawCalibration {
        RawCalibration {
            version: 1,
            xyz_to_camera_d65: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            camera_to_working: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            as_shot_gains: Some([2.0, 1.0, 1.5]),
            applied_gains: [2.0, 1.0, 1.5],
        }
    }

    #[test]
    fn suggestions_must_fit_document_gain_limits_without_clamping() {
        assert!(validate_savable_gains([0.01, 1.0, 100.0]).is_ok());
        for gains in [[0.009, 1.0, 2.0], [2.0, 1.0, 101.0], [f32::NAN, 1.0, 1.0]] {
            assert!(validate_savable_gains(gains).is_err());
        }
    }

    #[test]
    fn interpreting_draft_gains_validates_and_preserves_them() {
        let gains = [2.0, 1.0, 1.5];
        let request = RawWhiteBalanceRequest::Gains {
            gains_rgb: gains,
            locus: WhiteBalanceLocus::Planckian,
        };
        validate_request(&request).unwrap();
        assert_eq!(metadata_gains(&request, &calibration()).unwrap(), gains);
        for invalid in [[2.0, 2.0, 1.5], [0.0, 1.0, 1.5], [101.0, 1.0, 1.5]] {
            assert!(validate_request(&RawWhiteBalanceRequest::Gains {
                gains_rgb: invalid,
                locus: WhiteBalanceLocus::Daylight,
            })
            .is_err());
        }
    }

    #[test]
    fn bounded_calibration_cache_keys_include_path_hash_and_identity() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("synthetic.bin");
        std::fs::write(&path, b"synthetic source identity").unwrap();
        let identity = validation::SourceIdentity::capture(&path).unwrap();
        let key = CalibrationKey {
            canonical: path.canonicalize().unwrap(),
            fingerprint: "a".repeat(64),
        };
        let mut cache = CalibrationCache::default();
        cache.remember(key.clone(), identity.clone(), calibration());
        assert_eq!(
            cache.get(&key, &identity).unwrap().as_shot_gains,
            Some([2.0, 1.0, 1.5])
        );
        let changed_hash = CalibrationKey {
            fingerprint: "b".repeat(64),
            ..key.clone()
        };
        assert!(cache.get(&changed_hash, &identity).is_none());
        let changed_path = CalibrationKey {
            canonical: temp.path().join("other.bin"),
            ..key.clone()
        };
        assert!(cache.get(&changed_path, &identity).is_none());
        std::fs::write(&path, b"changed size").unwrap();
        let changed_identity = validation::SourceIdentity::capture(&path).unwrap();
        assert!(cache.get(&key, &changed_identity).is_none());
    }

    #[test]
    fn calibration_cache_evicts_lru_and_replacement_does_not_evict_other_entries() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("synthetic.bin");
        std::fs::write(&path, b"synthetic").unwrap();
        let identity = validation::SourceIdentity::capture(&path).unwrap();
        let key = |i| CalibrationKey {
            canonical: path.clone(),
            fingerprint: format!("{i:064x}"),
        };
        let mut cache = CalibrationCache::default();
        for i in 0..64 {
            cache.remember(key(i), identity.clone(), calibration());
        }
        assert_eq!(cache.entries.len(), 64);
        assert!(cache.get(&key(0), &identity).is_some());
        cache.remember(key(64), identity.clone(), calibration());
        assert_eq!(cache.entries.len(), 64);
        assert!(cache.get(&key(1), &identity).is_none());
        assert!(cache.get(&key(0), &identity).is_some());
        cache.remember(key(64), identity.clone(), calibration());
        assert_eq!(cache.entries.len(), 64);
        assert!(cache.get(&key(2), &identity).is_some());
    }
}
