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
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Cd => {
            println!(
                "{:?}",
                String::from_utf8(git::get_current_head_name().unwrap()).unwrap()
            )
        }
    }
}
