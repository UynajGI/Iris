use super::*;
use crate::vision::retrieval::normalize;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RetrievalPhoto {
    pub photo_id: i64,
    pub mtime: i64,
    pub size_bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SearchHit {
    pub photo_id: i64,
    pub similarity: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RetrievalCoverage {
    pub total: usize,
    pub indexed: usize,
    pub pending: usize,
}

fn valid_space(space: &str) -> Result<()> {
    anyhow::ensure!(
        space.len() == 64 && space.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid retrieval space identity"
    );
    Ok(())
}
impl Services {
    pub fn retrieval_pending(&self, project: i64, space: &str) -> Result<Vec<RetrievalPhoto>> {
        valid_space(space)?;
        self.require_project(project)?;
        let mut query=self.store.conn.prepare("SELECT p.id,p.mtime,p.size_bytes FROM photos p LEFT JOIN retrieval_embeddings e ON e.photo_id=p.id
            WHERE p.project_id=?1 AND p.missing=0 AND p.quarantined=0 AND (e.photo_id IS NULL OR e.space_key!=?2 OR e.mtime!=p.mtime OR e.size_bytes!=p.size_bytes) ORDER BY p.id")?;
        let rows = query
            .query_map(params![project, space], |r| {
                Ok(RetrievalPhoto {
                    photo_id: r.get(0)?,
                    mtime: r.get(1)?,
                    size_bytes: r.get::<_, i64>(2)? as u64,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
    pub fn retrieval_coverage(&self, project: i64, space: &str) -> Result<RetrievalCoverage> {
        valid_space(space)?;
        self.require_project(project)?;
        let (total,indexed)=self.store.conn.query_row("SELECT COUNT(*),COALESCE(SUM(e.photo_id IS NOT NULL AND e.space_key=?2 AND e.mtime=p.mtime AND e.size_bytes=p.size_bytes),0) FROM photos p LEFT JOIN retrieval_embeddings e ON e.photo_id=p.id WHERE p.project_id=?1 AND p.missing=0 AND p.quarantined=0",params![project,space],|r|Ok((r.get::<_,i64>(0)? as usize,r.get::<_,i64>(1)? as usize)))?;
        Ok(RetrievalCoverage {
            total,
            indexed,
            pending: total - indexed,
        })
    }
    pub fn save_retrieval_embedding(
        &mut self,
        project: i64,
        photo: &RetrievalPhoto,
        space: &str,
        vector: &[f32],
        dimensions: usize,
    ) -> Result<()> {
        valid_space(space)?;
        self.require_available_photos(project, &[photo.photo_id])?;
        anyhow::ensure!(
            (2..=2048).contains(&dimensions),
            "invalid retrieval dimensions"
        );
        let vector = normalize(vector, dimensions)?;
        let current: (i64, u64) = self.store.conn.query_row(
            "SELECT mtime,size_bytes FROM photos WHERE id=?1",
            [photo.photo_id],
            |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)),
        )?;
        anyhow::ensure!(
            current == (photo.mtime, photo.size_bytes),
            "retrieval source changed during indexing"
        );
        let path = self.photo_path(photo.photo_id)?;
        let metadata = fs::metadata(path)?;
        anyhow::ensure!(
            modified(&metadata)? == photo.mtime && metadata.len() == photo.size_bytes,
            "retrieval source changed; rescan the project"
        );
        let bytes: Vec<_> = vector.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.store.conn.execute("INSERT INTO retrieval_embeddings(photo_id,space_key,mtime,size_bytes,dimensions,vector) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(photo_id) DO UPDATE SET space_key=excluded.space_key,mtime=excluded.mtime,size_bytes=excluded.size_bytes,dimensions=excluded.dimensions,vector=excluded.vector",params![photo.photo_id,space,photo.mtime,i64::try_from(photo.size_bytes)?,dimensions as i64,bytes])?;
        Ok(())
    }
    /// Streaming exact cosine for the explicitly indexed local catalog. O(N*d), O(k) results.
    pub fn search_embeddings(
        &self,
        project: i64,
        space: &str,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        valid_space(space)?;
        self.require_project(project)?;
        anyhow::ensure!(
            (1..=200).contains(&limit) && (2..=2048).contains(&query_vector.len()),
            "invalid search dimensions or limit"
        );
        let vector = normalize(query_vector, query_vector.len())?;
        let mut query=self.store.conn.prepare("SELECT p.id,e.vector FROM retrieval_embeddings e JOIN photos p ON p.id=e.photo_id WHERE p.project_id=?1 AND p.missing=0 AND p.quarantined=0 AND e.space_key=?2 AND e.mtime=p.mtime AND e.size_bytes=p.size_bytes AND e.dimensions=?3")?;
        let mut rows = query.query(params![project, space, vector.len() as i64])?;
        let mut hits: Vec<SearchHit> = vec![];
        while let Some(row) = rows.next()? {
            let bytes: Vec<u8> = row.get(1)?;
            if bytes.len() != vector.len() * 4 {
                continue;
            }
            let candidate: Vec<_> = bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().expect("four bytes")))
                .collect();
            let Ok(candidate) = normalize(&candidate, vector.len()) else {
                continue;
            };
            let similarity = vector
                .iter()
                .zip(&candidate)
                .map(|(a, b)| f64::from(*a) * f64::from(*b))
                .sum::<f64>()
                .clamp(-1., 1.);
            let hit = SearchHit {
                photo_id: row.get(0)?,
                similarity,
            };
            let position = hits.partition_point(|prior| {
                prior.similarity > similarity
                    || (prior.similarity == similarity && prior.photo_id < hit.photo_id)
            });
            if position < limit {
                hits.insert(position, hit);
                hits.truncate(limit);
            }
        }
        Ok(hits)
    }
}
