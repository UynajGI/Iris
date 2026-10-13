use crate::args::Command;
use iris_core::{AcceptCategory, AcceptRequest, Action, PhotoFilter, Services};
use iris_daemon::{analyze_project, AppState, Job, JobProgress};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

/// Only orchestration lives here; service command families are independently reviewable.
pub(crate) fn execute(state: &AppState, command: Command) -> anyhow::Result<()> {
    if let Command::Edit { command } = command {
        let value = crate::editing::execute(state, command)?;
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    if let Command::Models { project } = command {
        println!(
            "{}",
            serde_json::to_string_pretty(&iris_daemon::models_status(state, project)?)?
        );
        return Ok(());
    }
    if let Command::Analyze { project } = command {
        return analyze(state, project);
    }
    let mut services = state
        .services
        .lock()
        .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
    let value = dispatch(&mut services, command)?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    anyhow::ensure!(
        value
            .get("scan")
            .and_then(|v| v.get("errors"))
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty),
        "scan encountered errors; successful records were retained"
    );
    Ok(())
}

fn analyze(state: &AppState, project: i64) -> anyhow::Result<()> {
    let job = Job {
        progress: JobProgress::default(),
        cancel: Arc::new(AtomicBool::new(false)),
        pause: Arc::new(AtomicBool::new(false)),
    };
    let result = analyze_project(state, project, &job)?;
    println!("{result}");
    anyhow::ensure!(
        result["failed"].as_u64().unwrap_or(0) == 0,
        "one or more photos failed; successful results were retained"
    );
    Ok(())
}

fn dispatch(s: &mut Services, command: Command) -> anyhow::Result<Value> {
    match command {
        c @ (Command::Projects | Command::Open { .. } | Command::Hide { .. }) => {
            project_history(s, c)
        }
        c @ (Command::Scan { .. } | Command::Photos { .. } | Command::Settings { .. }) => {
            projects(s, c)
        }
        c @ (Command::Decide { .. }
        | Command::Undo { .. }
        | Command::Accept { .. }
        | Command::Groups { .. }) => decisions(s, c),
        c @ (Command::Export { .. }
        | Command::Import { .. }
        | Command::QuarantinePreview { .. }
        | Command::QuarantineCommit { .. }
        | Command::QuarantineRestore { .. }) => files(s, c),
        c @ (Command::Cache { .. }
        | Command::CacheMigrate { .. }
        | Command::CacheHistory { .. }
        | Command::CacheCleanupOld { .. }) => cache(s, c),
        Command::Edit { .. }
        | Command::Analyze { .. }
        | Command::Models { .. }
        | Command::Openapi => unreachable!(),
    }
}

fn project_history(s: &mut Services, command: Command) -> anyhow::Result<Value> {
    Ok(match command {
        Command::Projects => serde_json::to_value(s.projects()?)?,
        Command::Open { project } => serde_json::to_value(s.open_project(project)?)?,
        Command::Hide { project } => serde_json::to_value(s.hide_project(project)?)?,
        _ => unreachable!("command routed to wrong family"),
    })
}

fn projects(s: &mut Services, command: Command) -> anyhow::Result<Value> {
    Ok(match command {
        Command::Scan { root } => scan(s, root)?,
        Command::Photos { project } => {
            serde_json::to_value(s.photos(project, PhotoFilter::default())?)?
        }
        Command::Settings { project, file } => settings(s, project, file)?,
        _ => unreachable!("command routed to wrong family"),
    })
}

fn scan(s: &mut Services, root: PathBuf) -> anyhow::Result<Value> {
    let project = s.create_project(root)?;
    Ok(serde_json::json!({"project":project,"scan":s.scan(project.id)?}))
}

fn settings(s: &mut Services, project: i64, file: Option<PathBuf>) -> anyhow::Result<Value> {
    match file {
        Some(path) => s.set_settings(project, serde_json::from_slice(&std::fs::read(path)?)?),
        None => s.settings(project),
    }
}

fn acceptance_category(category: &str) -> AcceptCategory {
    match category {
        "recommend" => AcceptCategory::Recommend,
        "reject_suggest" => AcceptCategory::RejectSuggest,
        _ => AcceptCategory::All,
    }
}

fn decisions(s: &mut Services, command: Command) -> anyhow::Result<Value> {
    Ok(match command {
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
                category: acceptance_category(&category),
            },
        )?)?,
        Command::Groups { project } => serde_json::to_value(s.groups(project)?)?,
        _ => unreachable!("command routed to wrong family"),
    })
}

fn files(s: &mut Services, command: Command) -> anyhow::Result<Value> {
    Ok(match command {
        Command::Export {
            project,
            format,
            destination,
            scope,
            overwrite,
        } => export(s, project, &format, destination, &scope, overwrite)?,
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
        _ => unreachable!("command routed to wrong family"),
    })
}

fn export(
    s: &mut Services,
    project: i64,
    format: &str,
    destination: Option<PathBuf>,
    scope: &str,
    overwrite: bool,
) -> anyhow::Result<Value> {
    let report = match format {
        "xmp" => s.export_xmp(project, scope, overwrite)?,
        "copy" => s.export_copy(
            project,
            &destination.ok_or_else(|| anyhow::anyhow!("--destination required"))?,
            scope,
        )?,
        _ => s.export_csv(
            project,
            &destination.ok_or_else(|| anyhow::anyhow!("--destination required"))?,
        )?,
    };
    Ok(serde_json::to_value(report)?)
}

fn cache(s: &mut Services, command: Command) -> anyhow::Result<Value> {
    Ok(match command {
        Command::Cache { project } => serde_json::to_value(s.cache_status(project)?)?,
        Command::CacheMigrate {
            project,
            destination,
        } => serde_json::to_value(s.cache_migrate(project, &destination)?)?,
        Command::CacheHistory { project } => serde_json::to_value(s.cache_migrations(project)?)?,
        Command::CacheCleanupOld { project, migration } => {
            serde_json::to_value(s.cache_cleanup_old(project, &migration)?)?
        }
        _ => unreachable!("command routed to wrong family"),
    })
}
