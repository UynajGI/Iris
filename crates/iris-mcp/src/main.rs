use anyhow::Result;
use clap::Parser;
use iris_daemon::{ownership::DatabaseOwner, AppState};
use rmcp::ServiceExt;
use std::{path::PathBuf, sync::atomic::Ordering, time::Duration};

#[derive(Parser)]
#[command(about = "Iris headless photo-culling MCP server (stdio; no desktop window)")]
struct Args {
    #[arg(long, default_value = ".iris-mcp")]
    data_dir: PathBuf,
    #[arg(long, default_value = "models")]
    model_dir: PathBuf,
    /// Repeat to authorize photo input and export locations. Existing absolute directories.
    #[arg(long, required_unless_present = "internal_analysis_worker")]
    allow_root: Vec<PathBuf>,
    /// Disable mutating MCP tools; engine caches may still be generated on reads.
    #[arg(long)]
    read_only: bool,
    #[arg(long, default_value_t=12, value_parser=clap::value_parser!(u8).range(1..=16))]
    worker_limit: u8,
    #[arg(long, hide = true)]
    internal_analysis_worker: bool,
}
fn main() -> Result<()> {
    let args = Args::parse();
    if args.internal_analysis_worker {
        return iris_daemon::worker::run(&args.model_dir);
    }
    std::fs::create_dir_all(&args.data_dir)?;
    let data_dir = args.data_dir.canonicalize()?;
    let _owner = DatabaseOwner::acquire(&data_dir.join("library.sqlite3"))?;
    let state = AppState::new(&data_dir.join("library.sqlite3"), args.model_dir)?
        .with_worker_limit(usize::from(args.worker_limit))?;
    if let Some(notice) = &state.recovery_notice {
        eprintln!("{notice}");
    }
    let server =
        iris_mcp::IrisMcp::new(state.clone(), &data_dir, &args.allow_root, args.read_only)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result: Result<()> = runtime.block_on(async {
        let service = server.serve(rmcp::transport::stdio()).await?;
        service.waiting().await?;
        Ok(())
    });
    if let Ok(jobs) = state.jobs.lock() {
        for job in jobs.values() {
            job.cancel.store(true, Ordering::Relaxed);
            job.pause.store(false, Ordering::Relaxed);
        }
    }
    // Workers observe cancellation before hard deadlines; no GUI owns this runtime.
    runtime.shutdown_timeout(Duration::from_secs(10));
    result
}
