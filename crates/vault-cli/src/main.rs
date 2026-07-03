mod commands;

use clap::{Parser, Subcommand};
use commands::*;

#[derive(Parser)]
#[command(
    name = "vault",
    version = "0.1.0",
    about = "VaultVCS - version control",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // Initialize a new vault repository
    Init {
        #[arg(long, default_value = "Developer")]
        name: String,
        #[arg(long, default_value = "dev@vault.com")]
        email: String,
    },

    // Snap working dir
    Save {
        //commit message
        #[arg(short, long)]
        message: Option<String>,
    },

    // show changed files sv last snap
    Status,

    /// Show line-level diff vs last
    Diff,

    ///Show commit history
    Log,

    /// Creat a new branch
    Branch {
        name: String,
    },

    /// Switch to a branch
    Switch {
        name: String,
    },

    /// Merge a branch into current
    Merge {
        name: String,
    },

    ///Undo the last operation
    Undo,

    /// Show the operation log
    Oplog,

    /// Show a specific commit
    Show {
        id: String,
    },

    /// Amend
    Amend {
        #[arg(short, long)]
        message: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Init { name, email } => init::run(&name, &email),
        Commands::Save { message } => save::run(message.as_deref()),
        Commands::Status => status::run(),
        Commands::Diff => diff::run(),
        Commands::Log => log::run(),
        Commands::Branch { name } => branch::run(&name),
        Commands::Switch { name } => switch::run(&name),
        Commands::Merge { name } => merge::run(&name),
        Commands::Undo => undo::run(),
        Commands::Oplog => oplog::run(),
        Commands::Show { id } => show::run(&id),
        Commands::Amend { message } => amend::run(message.as_deref()),
    };

    if let Err(e) = result {
        eprintln!("{} : {}", colored::Colorize::red("error:"), e);
        std::process::exit(1);
    }
}
