mod cd;
mod git;
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Cd,
    Save,
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Cd => {
            println!(
                "{:?}",
                git::GitRepo::new(".").unwrap().get_current_head_name()
            )
        }
        Commands::Save => {
            println!(
                "{:?}",
                git::GitRepo::new(".").unwrap().save("tmp/autosave", "auto")
            )
        }
    }
}
