use anyhow::{Context, Result};
use caldawarrior::caldav_adapter::{CalDavClient, RealCalDavClient};
use caldawarrior::tw_adapter::{RealTaskRunner, TwAdapter};
use caldawarrior::{config, output, sync};
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync TaskWarrior tasks with CalDAV
    Sync {
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Sync { dry_run } => {
            // Load configuration
            let config = config::load(cli.config.as_deref())
                .context("Failed to load configuration")?;

            // Create TW adapter
            let tw = TwAdapter::new(RealTaskRunner)
                .context("Failed to initialize TaskWarrior adapter")?;

            // Create CalDAV client
            let caldav = RealCalDavClient::new(
                config.server_url.clone(),
                config.username.clone(),
                config.password.clone(),
                config.caldav_timeout_seconds,
                config.allow_insecure_tls,
            )
            .context("Failed to initialize CalDAV client")?;

            // List all TW tasks
            let tw_tasks = tw.list_all().context("Failed to list TaskWarrior tasks")?;

            // Fetch VTODOs from each configured calendar
            let mut vtodos_by_calendar = HashMap::new();
            for calendar in &config.calendars {
                let vtodos = caldav
                    .list_vtodos(&calendar.url)
                    .with_context(|| {
                        format!("Failed to list VTODOs from calendar '{}'", calendar.url)
                    })?;
                vtodos_by_calendar.insert(calendar.url.clone(), vtodos);
            }

            // Run sync
            let result = sync::run_sync(
                &tw_tasks,
                &vtodos_by_calendar,
                &config,
                &tw,
                &caldav,
                dry_run,
                Utc::now(),
            );

            // Print formatted output (errors and warnings go to stderr, summary to stdout)
            output::print_result(&result, dry_run);

            // Exit non-zero if there were errors
            if !result.errors.is_empty() {
                process::exit(1);
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    #[allow(unused_imports)]
    use caldawarrior::caldav_adapter::CalDavClient;

    #[test]
    fn sync_dry_run_true() {
        let cli = Cli::try_parse_from(["caldawarrior", "sync", "--dry-run"]).expect("parse");
        match cli.command {
            Commands::Sync { dry_run } => assert!(dry_run, "expected dry_run to be true"),
        }
    }

    #[test]
    fn sync_dry_run_false() {
        let cli = Cli::try_parse_from(["caldawarrior", "sync"]).expect("parse");
        match cli.command {
            Commands::Sync { dry_run } => assert!(!dry_run, "expected dry_run to be false"),
        }
    }

    #[test]
    fn sync_with_config_path() {
        let cli =
            Cli::try_parse_from(["caldawarrior", "--config", "/tmp/cfg.toml", "sync"])
                .expect("parse");
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/tmp/cfg.toml")),
            "expected config path to be /tmp/cfg.toml"
        );
    }

    #[test]
    fn sync_subcommand_exists() {
        let cmd = Cli::command();
        assert!(
            cmd.find_subcommand("sync").is_some(),
            "expected 'sync' subcommand to exist"
        );
    }
}
