use anyhow::Context as _;
use clap::{Parser, Subcommand, ValueHint};
use std::env;
use std::path::PathBuf;
use std::process::exit;

use autosave::*;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List directories in watch list
    List,
    /// Remove specified directory from watch list
    Remove {
        #[arg(long, short, value_hint = ValueHint::DirPath)]
        path: Option<PathBuf>,
        #[arg(long, help = "Remove all path from watch list")]
        all: bool,
    },
    /// Run command in specified branch worktree
    Run {
        #[arg(help = "New branch name")]
        branch: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, value_hint = ValueHint::CommandWithArguments, help = "Command to execute")]
        args: Option<Vec<String>>,
    },
    /// Kill autosave daemon
    Kill,
}

fn main() {
    use tracing_subscriber::{EnvFilter, filter::LevelFilter, prelude::*};
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .boxed();
    let (layer, reload_handle) = tracing_subscriber::reload::Layer::new(layer);
    tracing_subscriber::registry().with(layer).init();

    let parsed = Cli::parse();

    let daemon_check = match daemon::check_daemon() {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("{e:?}");
            exit(1);
        }
    };
    if daemon_check {
        tracing::info!("daemon is already running");
    } else {
        tracing::info!("daemon is not running; start daemon");
        if let Err(e) = daemon::start_daemon(&reload_handle) {
            tracing::error!("{e:?}");
            exit(1);
        }
    }

    let current_dir = match env::current_dir().context("failed to get current dir") {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("{e:?}");
            exit(1);
        }
    };

    match parsed.command {
        None => {
            tracing::info!("add current dir to the watch list");

            let resp = client::change_watch_list(types::ChangeWatchRequest::Add {
                path: current_dir,
                config: config::Config::default(),
            })
            .context("failed to add current dir to watch list");
            if let Err(e) = resp {
                tracing::error!("{e:?}");
                exit(1);
            }
            tracing::info!("current dir added to the watch list");
        }
        Some(Command::List) => {
            tracing::info!("list current paths in the watch list");
            let resp = client::get_watch_list().context("failed to get current watch list");
            match resp {
                Ok(paths) => {
                    for path in paths {
                        println!("{}", path.display());
                    }
                }
                Err(e) => {
                    tracing::error!("{e:?}");
                    exit(1);
                }
            }
        }
        Some(Command::Remove { path, all }) => {
            let paths = if all {
                let resp = client::get_watch_list().context("failed to get current watch list");
                match resp {
                    Ok(paths) => paths,
                    Err(e) => {
                        tracing::error!("{e:?}");
                        exit(1);
                    }
                }
            } else {
                match path {
                    Some(v) => vec![v],
                    None => vec![current_dir],
                }
            };
            tracing::info!("remove path(s) from the watch list: {paths:?}");
            for path in paths {
                let resp = client::change_watch_list(types::ChangeWatchRequest::Remove { path })
                    .context("failed to remove dir to watch list");
                if let Err(e) = resp {
                    tracing::error!("{e:?}");
                    exit(1);
                }
            }
            tracing::info!("successfully deleted the path from the watch list");
        }
        Some(Command::Run { branch, args }) => {
            let args = match args {
                Some(v) => v,
                None => match env::var("SHELL") {
                    Ok(v) => vec![v],
                    Err(_) => {
                        tracing::error!("$SHELL is not set");
                        exit(1);
                    }
                },
            };
            match client::do_worktree(&args, &branch, &current_dir) {
                Ok(v) => exit(v),
                Err(e) => {
                    tracing::error!("{e:?}");
                    exit(1);
                }
            };
        }
        Some(Command::Kill) => {
            let resp = client::kill().context("failed to kill the daemon");
            if let Err(e) = resp {
                tracing::error!("{e:?}");
                exit(1);
            }
        }
    }
    exit(0);
}
