mod args;
mod commands;

use args::{Args, Command};
use clap::Parser;
use iris_daemon::{ApiDoc, AppState};
use utoipa::OpenApi;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if matches!(args.command, Command::Openapi) {
        println!("{}", ApiDoc::openapi().to_pretty_json()?);
        return Ok(());
    }
    if let Some(parent) = args.database.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _owner = iris_daemon::ownership::DatabaseOwner::acquire(&args.database)?;
    let state = AppState::new(&args.database, args.model_dir)?;
    if let Some(notice) = &state.recovery_notice {
        eprintln!("{notice}");
    }
    commands::execute(&state, args.command)
}
