mod config;
mod git;
mod watcher;
use anyhow::Result;
use clap::{Parser, Subcommand};
use config::Config;
use watcher::RepoWatcher;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        path: Option<String>,
        config: Option<String>,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { path, config } => {
            let p = path.unwrap_or(".".to_string());
            let conf = if let Some(cp) = config {
                Config::from_file_path(cp)
            } else {
                Config::from_dir_path(&p, ".autosave.toml")
            }
            .unwrap();
            let _watcher = RepoWatcher::new(&p, conf).unwrap();
            loop {}
        }
    }
}
