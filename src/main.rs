mod cd;
mod config;
mod git;
mod watcher;
use clap::{Parser, Subcommand};

#[derive(Debug)]
pub enum Error {
    WatchError(notify::Error),
    GitError(git::GitError),
}

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Cd,
    Save {
        path: Option<String>,
    },
    Run {
        path: Option<String>,
        config: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Cd => {
            println!(
                "{:?}",
                git::GitRepo::new(".").unwrap().get_current_head_name()
            )
        }
        Commands::Save { path } => {
            let p = path.unwrap_or(".".to_string());
            git::GitRepo::new(&p).unwrap().save("tmp/autosave", "auto");
        }
        Commands::Run { path, config } => {
            let p = path.unwrap_or(".".to_string());
            let c = config::Config::default();
            let mut repo_watcher = watcher::RepoWatcher::new(&p, &c.branch(), c.message()).unwrap();
            repo_watcher.watch().unwrap();
            loop {}
        }
    }
}
