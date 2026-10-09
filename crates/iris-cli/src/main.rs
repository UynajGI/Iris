use clap::{Parser, Subcommand};
use iris_core::{AcceptCategory, AcceptRequest, Action, PhotoFilter};
use iris_daemon::{analyze_project, ApiDoc, AppState, Job, JobProgress};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

#[derive(Parser)]
#[command(about = "IrisVision headless photo culling")]
struct Args {
    #[arg(long, default_value = ".iris/library.sqlite3")]
    database: PathBuf,
    #[arg(long, default_value = "models")]
    model_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Projects,
    /// Open a known project and move it to the front of recent history.
    Open {
        project: i64,
    },
    /// Hide a recent project without deleting its data or photographs.
    Hide {
        project: i64,
    },
    Scan {
        root: PathBuf,
    },
    Photos {
        project: i64,
    },
    Analyze {
        project: i64,
    },
    /// Read project settings, or replace them from a validated JSON file.
    Settings {
        project: i64,
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Inspect detector artifacts without initializing model inference.
    Models {
        project: i64,
    },
    Decide {
        project: i64,
        action: String,
        photos: Vec<i64>,
    },
    Undo {
        project: i64,
    },
    Accept {
        project: i64,
        /// Omit for all active photos; provide no values for an empty scope.
        #[arg(long, num_args = 0.., value_delimiter = ',')]
        photo_ids: Option<Vec<i64>>,
        #[arg(long, default_value = "all", value_parser = ["all", "recommend", "reject_suggest"])]
        category: String,
    },
    Groups {
        project: i64,
    },
    Export {
        project: i64,
        #[arg(value_parser=["xmp","copy","csv"])]
        format: String,
        #[arg(long)]
        destination: Option<PathBuf>,
        #[arg(long, default_value = "keep")]
        scope: String,
        #[arg(long)]
        overwrite: bool,
    },
    Import {
        project: i64,
        source: PathBuf,
    },
    QuarantinePreview {
        project: i64,
    },
    QuarantineCommit {
        manifest: String,
    },
    QuarantineRestore {
        manifest: String,
    },
    Cache {
        project: i64,
    },
    CacheMigrate {
        project: i64,
        destination: PathBuf,
    },
    CacheHistory {
        project: i64,
    },
    /// Confirm cleanup of files in one saved migration's old-cache manifest.
    CacheCleanupOld {
        project: i64,
        migration: String,
    },
    Openapi,
}
fn main() -> anyhow::Result<()> {
    use utoipa::OpenApi;
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
    if let Command::Models { project } = args.command {
        println!(
            "{}",
            serde_json::to_string_pretty(&iris_daemon::models_status(&state, project)?)?
        );
        return Ok(());
    }
    if let Command::Analyze { project } = args.command {
        let job = Job {
            progress: JobProgress::default(),
            cancel: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
        };
        let result = analyze_project(&state, project, &job)?;
        println!("{result}");
        anyhow::ensure!(
            result["failed"].as_u64().unwrap_or(0) == 0,
            "one or more photos failed; successful results were retained"
        );
        return Ok(());
    }
    let mut s = state
        .services
        .lock()
        .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
    let value = match args.command {
        Command::Projects => serde_json::to_value(s.projects()?)?,
        Command::Open { project } => serde_json::to_value(s.open_project(project)?)?,
        Command::Hide { project } => serde_json::to_value(s.hide_project(project)?)?,
        Command::Scan { root } => {
            let p = s.create_project(root)?;
            serde_json::json!({"project":p,"scan":s.scan(p.id)?})
        }
        Command::Photos { project } => {
            serde_json::to_value(s.photos(project, PhotoFilter::default())?)?
        }
        Command::Settings { project, file } => match file {
            Some(path) => {
                s.set_settings(project, serde_json::from_slice(&std::fs::read(path)?)?)?
            }
            None => s.settings(project)?,
        },
        Command::Decide {
            project,
            action,
            photos,
        } => serde_json::to_value(s.decisions(
            project,
            &photos,
            action.parse::<Action>()?,
            "human",
            true,
        )?)?,
        Command::Undo { project } => serde_json::to_value(s.undo(project)?)?,
        Command::Accept {
            project,
            photo_ids,
            category,
        } => serde_json::to_value(s.accept_scoped(
            project,
            AcceptRequest {
                photo_ids,
                category: match category.as_str() {
                    "recommend" => AcceptCategory::Recommend,
                    "reject_suggest" => AcceptCategory::RejectSuggest,
                    _ => AcceptCategory::All,
                },
            },
        )?)?,
        Command::Groups { project } => serde_json::to_value(s.groups(project)?)?,
        Command::Export {
            project,
            format,
            destination,
            scope,
            overwrite,
        } => serde_json::to_value(match format.as_str() {
            "xmp" => s.export_xmp(project, &scope, overwrite)?,
            "copy" => s.export_copy(
                project,
                &destination.ok_or_else(|| anyhow::anyhow!("--destination required"))?,
                &scope,
            )?,
            _ => s.export_csv(
                project,
                &destination.ok_or_else(|| anyhow::anyhow!("--destination required"))?,
            )?,
        })?,
        Command::Import { project, source } => {
            serde_json::to_value(s.import_csv(project, &source)?)?
        }
        Command::QuarantinePreview { project } => {
            serde_json::to_value(s.quarantine_preview(project)?)?
        }
        Command::QuarantineCommit { manifest } => {
            serde_json::to_value(s.quarantine_commit(&manifest)?)?
        }
        Command::QuarantineRestore { manifest } => {
            serde_json::to_value(s.quarantine_restore(&manifest)?)?
        }
        Command::Cache { project } => serde_json::to_value(s.cache_status(project)?)?,
        Command::CacheMigrate {
            project,
            destination,
        } => serde_json::to_value(s.cache_migrate(project, &destination)?)?,
        Command::CacheHistory { project } => serde_json::to_value(s.cache_migrations(project)?)?,
        Command::CacheCleanupOld { project, migration } => {
            serde_json::to_value(s.cache_cleanup_old(project, &migration)?)?
        }
        Command::Analyze { .. } | Command::Models { .. } | Command::Openapi => unreachable!(),
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    anyhow::ensure!(
        value
            .get("scan")
            .and_then(|v| v.get("errors"))
            .and_then(serde_json::Value::as_array)
            .is_none_or(Vec::is_empty),
        "scan encountered errors; successful records were retained"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adoption_scope_distinguishes_omitted_empty_and_explicit_ids() {
        for (arguments, expected) in [
            (vec!["iris", "accept", "1"], None),
            (vec!["iris", "accept", "1", "--photo-ids"], Some(vec![])),
            (
                vec![
                    "iris",
                    "accept",
                    "1",
                    "--photo-ids",
                    "2,3",
                    "--category",
                    "recommend",
                ],
                Some(vec![2, 3]),
            ),
        ] {
            let args = Args::try_parse_from(arguments).unwrap();
            let Command::Accept { photo_ids, .. } = args.command else {
                panic!("wrong command")
            };
            assert_eq!(photo_ids, expected);
        }
        assert!(Args::try_parse_from(["iris", "accept", "1", "--photo-ids", "typo"]).is_err());
    }
}
