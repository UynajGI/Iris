// SPDX-License-Identifier: GPL-3.0-or-later
//! Prepared file work is executed outside the shared database lock. A render job
//! is an immutable edit/source snapshot; later slider changes cannot alter it.
use super::{EditRecord, Services};
use anyhow::{ensure, Context, Result};
use iris_render::{
    document::SourceBinding,
    engine::{RenderEngine, RenderRequest, RenderedFrame},
    Frame, RenderContext,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

mod export;
mod lens;
mod mask_preview;
mod region;
mod validation;
mod white_balance;
pub use export::*;
pub use lens::*;
pub use region::*;
pub use validation::SourceValidation;
pub use white_balance::*;

pub struct EditSourceJob {
    pub photo_id: i64,
    pub source: PathBuf,
}
pub struct EditSource {
    pub binding: SourceBinding,
    pub metadata: iris_render::source::SourceMetadata,
    pub frame: Arc<Frame>,
}
pub struct EditRenderJob {
    pub record: EditRecord,
    pub source: PathBuf,
    pub resources: BTreeMap<String, Vec<u8>>,
}

impl Services {
    pub fn prepare_edit_source(&self, photo_id: i64) -> Result<EditSourceJob> {
        Ok(EditSourceJob {
            photo_id,
            source: self.photo_path(photo_id)?,
        })
    }

    pub fn prepare_edit_render(&self, id: &str, expected_revision: u64) -> Result<EditRenderJob> {
        let record = self.edit_document(id)?;
        ensure!(
            record.revision == expected_revision,
            "edit revision changed"
        );
        self.edit_render_job(record)
    }

    /// Validate a transient slider document against an existing revision. This
    /// reads catalogue/resources only: no edit history or command receipt writes.
    pub fn prepare_draft_edit_render(
        &self,
        id: &str,
        base_revision: u64,
        document: iris_render::document::EditDocument,
    ) -> Result<EditRenderJob> {
        let mut record = self.edit_document(id)?;
        ensure!(record.revision == base_revision, "edit revision changed");
        document.validate()?;
        super::editing::bounded_json(&document)?;
        ensure!(
            super::editing::same_source(&record.document.source, &document.source),
            "source binding is immutable; explicit rebase required"
        );
        self.verify_edit_resources(&document)?;
        record.document = document;
        self.edit_render_job(record)
    }

    fn edit_render_job(&self, record: EditRecord) -> Result<EditRenderJob> {
        let source = self.photo_path(record.photo_id)?;
        let mut resources = BTreeMap::new();
        let mut total = 0_usize;
        for hash in record.document.resources() {
            let bytes = self.read_edit_resource(&hash)?;
            total = total
                .checked_add(bytes.len())
                .context("resource size overflow")?;
            ensure!(total <= 128 * 1024 * 1024, "render resources exceed budget");
            resources.insert(hash, bytes);
        }
        Ok(EditRenderJob {
            record,
            source,
            resources,
        })
    }
}

impl EditSourceJob {
    pub fn decode(&self, context: &RenderContext) -> Result<EditSource> {
        let fingerprint = source_digest(&self.source, context)?;
        let decoded = decode_source(&self.source, None, context)?;
        ensure!(
            source_digest(&self.source, context)? == fingerprint,
            "source changed during decode"
        );
        let binding = SourceBinding {
            photo_id: self.photo_id.to_string(),
            fingerprint,
            width: decoded.frame.width,
            height: decoded.frame.height,
            reference_kind: decoded.metadata.reference_kind,
            raw_white_balance: None,
        };
        let frame = Arc::new(decoded.frame);
        context.track_frame(&frame)?;
        Ok(EditSource {
            binding,
            metadata: decoded.metadata,
            frame,
        })
    }
}

impl EditRenderJob {
    /// Browser transport is terminal. Proof simulation always returns tagged
    /// sRGB; neither JPEG nor proof pixels are ever put into the working cache.
    pub fn encode_edit_preview(
        &self,
        engine: &RenderEngine,
        request: &RenderRequest,
        context: &RenderContext,
        proof_icc: Option<&[u8]>,
    ) -> Result<(Vec<u8>, RenderedFrame)> {
        use iris_render::{color, output};
        let rendered = self.preview(engine, request, context)?;
        let mut config = self.record.document.output.clone();
        // A normal browser preview uses sRGB, independently of export primaries.
        if proof_icc.is_none() {
            config.space = color::OutputSpace::Srgb;
        }
        let target = color::output(&rendered.frame, &config, context)?;
        let _target_memory = context.reserve(target.byte_len())?;
        let encoded = if let Some(icc) = proof_icc {
            let proof = color::soft_proof(&target, config.space, icc, context)?;
            let _proof_memory = context.reserve(proof.byte_len())?;
            config.space = color::OutputSpace::Srgb;
            output::encode_output(
                &proof,
                &config,
                output::OutputFormat::Jpeg8 { quality: 90 },
                context,
            )?
        } else {
            output::encode_output(
                &target,
                &config,
                output::OutputFormat::Jpeg8 { quality: 90 },
                context,
            )?
        };
        engine.check_request(&request.session, request.sequence)?;
        context.check_cancelled()?;
        Ok((encoded, rendered))
    }

    pub fn preview(
        &self,
        engine: &RenderEngine,
        request: &RenderRequest,
        context: &RenderContext,
    ) -> Result<RenderedFrame> {
        self.preview_document(engine, request, context, &self.record.document)
    }

    fn preview_document(
        &self,
        engine: &RenderEngine,
        request: &RenderRequest,
        context: &RenderContext,
        document: &iris_render::document::EditDocument,
    ) -> Result<RenderedFrame> {
        ensure!(
            request.revision == self.record.revision,
            "preview revision mismatch"
        );
        let force = request.quality == iris_render::engine::RenderQuality::Full;
        let (identity, _) =
            validation::validate(&self.source, &document.source.fingerprint, force, context)?;
        let result = engine.render_with_context(
            document,
            request,
            |context| {
                let decoded =
                    decode_source(&self.source, document.source.raw_white_balance, context)?;
                // Even with a warm metadata hint, a source-cache miss must not
                // decode and cache changed bytes under the old fingerprint.
                self.verify_source(context)?;
                Ok(decoded.frame)
            },
            &self.resources,
            context,
        )?;
        validation::check_unchanged(&self.source, &identity, context)?;
        if force {
            self.verify_source(context)?;
        }
        Ok(result)
    }

    /// Full resolution graph output, before display mapping or quantization.
    pub fn full_resolution(&self, context: &RenderContext) -> Result<Arc<Frame>> {
        self.verify_source(context)?;
        let decoded = decode_source(
            &self.source,
            self.record.document.source.raw_white_balance,
            context,
        )?;
        let result = iris_render::graph::render(
            &self.record.document,
            Arc::new(decoded.frame),
            &self.resources,
            context,
        )?;
        self.verify_source(context)?;
        Ok(result)
    }

    pub fn verify_source(&self, context: &RenderContext) -> Result<()> {
        validation::validate(
            &self.source,
            &self.record.document.source.fingerprint,
            true,
            context,
        )?;
        Ok(())
    }

    /// Explicit validation mode for hosts and diagnostics. MetadataCache is a
    /// bounded warm-interaction hint, not a new content-integrity guarantee.
    pub fn validate_preview_source(
        &self,
        context: &RenderContext,
        force_validation: bool,
    ) -> Result<SourceValidation> {
        Ok(validation::validate(
            &self.source,
            &self.record.document.source.fingerprint,
            force_validation,
            context,
        )?
        .1)
    }
    /// Refresh hint state before a subsequent interactive request. Full-quality
    /// requests and exports already bypass the hint and verify all bytes.
    pub fn clear_source_validation(&self) -> Result<()> {
        validation::invalidate(&self.source)
    }
}

fn decode_source(
    path: &Path,
    white_balance: Option<[f32; 3]>,
    context: &RenderContext,
) -> Result<iris_render::source::DecodedSource> {
    if crate::vision::supported_format(path) == Some("raw") {
        crate::vision::decode_raw_linear(path, white_balance, context)
    } else {
        ensure!(
            white_balance.is_none(),
            "sensor white balance requires a RAW input"
        );
        iris_render::source::decode_file(path, context)
    }
}

pub fn source_digest(path: &Path, context: &RenderContext) -> Result<String> {
    let mut file = File::open(path)?;
    ensure!(
        file.metadata()?.len() <= 1024 * 1024 * 1024,
        "editing source exceeds file limit"
    );
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        context.check_cancelled()?;
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        hasher.update(&buffer[..length]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
