use clap::Parser;
use iris_daemon::{router, ApiDoc, AppState, DEFAULT_WORKER_LIMIT};
use std::path::PathBuf;
use utoipa::OpenApi;

#[derive(Parser)]
#[command(about = "IrisVision local authenticated photo service")]
struct Args {
    #[arg(long, default_value = ".iris")]
    data_dir: PathBuf,
    #[arg(long, default_value = "models")]
    model_dir: PathBuf,
    #[arg(long, default_value_t = 0)]
    port: u16,
    /// Maximum workers per project analysis (also bounded by available CPUs and pending photos).
    #[arg(long, default_value_t = DEFAULT_WORKER_LIMIT, value_parser = clap::value_parser!(u8).range(1..=16))]
    worker_limit: u8,
    #[arg(long)]
    openapi: bool,
    #[arg(long, hide = true)]
    internal_analysis_worker: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.internal_analysis_worker {
        return iris_daemon::worker::run(&args.model_dir);
    }
    if args.openapi {
        println!("{}", ApiDoc::openapi().to_pretty_json()?);
        return Ok(());
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(serve(args))
}

async fn serve(args: Args) -> anyhow::Result<()> {
    std::fs::create_dir_all(&args.data_dir)?;
    let _owner =
        iris_daemon::ownership::DatabaseOwner::acquire(&args.data_dir.join("library.sqlite3"))?;
    let state = AppState::new(&args.data_dir.join("library.sqlite3"), args.model_dir)?
        .with_worker_limit(usize::from(args.worker_limit))?;
    let listener =
        tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, args.port)).await?;
    println!(
        "{}",
        serde_json::json!({"base_url":format!("http://{}",listener.local_addr()?),"token":state.token,"version":env!("CARGO_PKG_VERSION")})
    );
    axum::serve(listener,router(state.clone())).with_graceful_shutdown(async move {
        use tokio::io::AsyncBufReadExt;
        let mut lines=tokio::io::BufReader::new(tokio::io::stdin()).lines();
        tokio::select! {
            _=tokio::signal::ctrl_c()=>{},
            _=async {while let Ok(Some(line))=lines.next_line().await {if line.trim()=="shutdown" {return}}std::future::pending::<()>().await;}=>{},
        }
        if let Ok(jobs)=state.jobs.lock(){for job in jobs.values(){job.cancel.store(true,std::sync::atomic::Ordering::Relaxed);job.pause.store(false,std::sync::atomic::Ordering::Relaxed);}}
    }).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_limit_defaults_to_twelve_and_rejects_invalid_values_during_parsing() {
        assert_eq!(
            Args::try_parse_from(["iris-daemon"]).unwrap().worker_limit,
            12
        );
        for limit in ["1", "2", "8", "12", "16"] {
            assert_eq!(
                Args::try_parse_from(["iris-daemon", "--worker-limit", limit])
                    .unwrap()
                    .worker_limit,
                limit.parse::<u8>().unwrap()
            );
        }
        for invalid in ["0", "17", "256", "-1", "1.5", "invalid"] {
            assert!(
                Args::try_parse_from(["iris-daemon", "--worker-limit", invalid]).is_err(),
                "{invalid}"
            );
        }
    }
}
