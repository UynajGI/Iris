use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Iris headless photo culling")]
pub(crate) struct Args {
    #[arg(long, default_value = ".iris/library.sqlite3")]
    pub database: PathBuf,
    #[arg(long, default_value = "models")]
    pub model_dir: PathBuf,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub(crate) enum Command {
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
