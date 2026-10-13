//! One process-wide render pool, independent of Rayon's unbounded default pool.
//! Multiple render requests share this pool; tiles never create nested pools.
use anyhow::{anyhow, Result};
use std::sync::OnceLock;
static POOL: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
pub(crate) fn pool() -> Result<&'static rayon::ThreadPool> {
    POOL.get_or_init(|| {
        let available = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1);
        rayon::ThreadPoolBuilder::new()
            .num_threads((available / 2).clamp(1, 8))
            .thread_name(|i| format!("iris-render-{i}"))
            .build()
            .map_err(|e| e.to_string())
    })
    .as_ref()
    .map_err(|e| anyhow!("creating bounded render pool: {e}"))
}
