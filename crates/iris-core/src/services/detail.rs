use super::*;
use crate::vision::{
    detail::{self, DetailEvidence, DetailRequest},
    VisionAnalysis,
};

/// Prepared under the service lock, computed through the bounded media queue.
pub struct DetailJob {
    source: PathBuf,
    destination: PathBuf,
    mtime: i64,
    size: u64,
    region: [f32; 4],
}
impl DetailJob {
    pub fn render(&self, cancel: &AtomicBool) -> Result<Vec<u8>> {
        self.verify(cancel)?;
        if self.destination.is_file() {
            let bytes = fs::read(&self.destination)?;
            if bytes.len() <= 65536 && serde_json::from_slice::<DetailEvidence>(&bytes).is_ok() {
                self.verify(cancel)?;
                return Ok(bytes);
            }
            fs::remove_file(&self.destination)?;
        }
        let evidence = detail::measure(&self.source, self.region, cancel)?;
        self.verify(cancel)?;
        let bytes = serde_json::to_vec(&evidence)?;
        super::media::publish_cached_image(&self.destination, &bytes)?;
        Ok(bytes)
    }
    fn verify(&self, cancel: &AtomicBool) -> Result<()> {
        anyhow::ensure!(!cancel.load(Ordering::Relaxed), "detail cancelled");
        let metadata = fs::metadata(&self.source)?;
        anyhow::ensure!(
            modified(&metadata)? == self.mtime && metadata.len() == self.size,
            "detail source changed; rescan the project"
        );
        Ok(())
    }
}

impl Services {
    pub fn prepare_detail(&self, photo_id: i64, request: &DetailRequest) -> Result<DetailJob> {
        anyhow::ensure!(
            request.face_index.is_none() || request.region.is_none(),
            "choose a face or a region"
        );
        let source = self.photo_path(photo_id)?;
        let photo = self.photo(photo_id)?;
        let region = requested_region(&photo, request)?;
        detail::validate_region(region)?;
        let metadata = fs::metadata(&source)?;
        anyhow::ensure!(
            modified(&metadata)? == photo.mtime && metadata.len() == photo.size_bytes,
            "detail source changed; rescan the project"
        );
        let cache_root: String = self.store.conn.query_row(
            "SELECT cache_root FROM projects WHERE id=?1",
            [photo.project_id],
            |row| row.get(0),
        )?;
        fs::create_dir_all(&cache_root)?;
        let key = format!(
            "{}:{}:{}:{}:{:?}",
            detail::DETAIL_METHOD,
            photo_id,
            photo.mtime,
            photo.size_bytes,
            region.map(f32::to_bits)
        );
        let name = format!("detail-{:x}.json", Sha256::digest(key.as_bytes()));
        Ok(DetailJob {
            source,
            destination: safe_destination(Path::new(&cache_root), &name)?,
            mtime: photo.mtime,
            size: photo.size_bytes,
            region,
        })
    }
}

fn requested_region(photo: &Photo, request: &DetailRequest) -> Result<[f32; 4]> {
    let Some(index) = request.face_index else {
        return Ok(request.region.unwrap_or([0.25, 0.25, 0.5, 0.5]));
    };
    anyhow::ensure!(
        photo.analysis_status == AnalysisStatus::Current,
        "face detail requires a current analysis"
    );
    let analysis: VisionAnalysis =
        serde::Deserialize::deserialize(photo.analysis.as_ref().context("analysis unavailable")?)?;
    let face = analysis
        .faces
        .iter()
        .find(|face| face.index == index)
        .context("face unavailable")?;
    anyhow::ensure!(
        analysis.width > 0 && analysis.height > 0,
        "invalid analysis geometry"
    );
    let [x, y, w, h] = face.bbox;
    let right = ((x + w) / analysis.width as f32).clamp(0., 1.);
    let bottom = ((y + h) / analysis.height as f32).clamp(0., 1.);
    let x = (x / analysis.width as f32).clamp(0., 1.);
    let y = (y / analysis.height as f32).clamp(0., 1.);
    Ok([x, y, right - x, bottom - y])
}
