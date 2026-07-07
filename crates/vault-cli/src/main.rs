mod commands;
mod tui; // NEW

use clap::{Parser, Subcommand};
use commands::*;

use crate::commands::tags::{run_create, run_list};

#[derive(Parser)]
#[command(
    name = "vault",
    version = "0.2.0",
    about = "VaultVCS — intentional version control"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialise a new repository
    Init {
        #[arg(long, default_value = "Developer")]
        name: String,
        #[arg(long, default_value = "dev@vault.com")]
        email: String,
    },
    /// Snapshot the working directory
    Save {
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Show changed files vs last snapshot
    Status,
    /// Show line-level diff
    Diff {
        #[arg(long)]
        stat: bool,
        a: Option<String>,
        b: Option<String>,
    },
    /// Show commit history
    Log,
    /// Create a branch
    Branch { name: String },
    /// Switch branch
    Switch { name: String },
    /// Merge a branch
    Merge { name: String },
    /// Undo last operation
    Undo,
    /// Show operation log
    Oplog,
    /// Show a commit
    Show { id: String },

    /// Rewrite the last commit
    Amend {
        #[arg(short, long)]
        message: Option<String>,
        #[arg(long)]
        no_edit: bool,
    },
    /// Split working changes into two commits
    Split {
        #[arg(long)]
        msg1: Option<String>,
        #[arg(long)]
        msg2: Option<String>,
    },
    /// Squash last N commits into one
    Squash {
        n: usize,
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Resolve a conflict interactively
    Resolve {
        /// Specific file to resolve (defaults to first conflict)
        file: Option<String>,
    },
    /// Manage named stashes
    Stash {
        #[command(subcommand)]
        action: StashAction,
    },
    /// Manage .vaultignore
    Ignore {
        #[command(subcommand)]
        action: IgnoreAction,
    },
    /// Manage tags
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },

    /// Save/restore named working-dir snapshots
    Snapshot {
        #[command(subcommand)]
        action: SnapshotAction,
    },
    /// Restore a single file to HEAD (or a commit)
    Restore {
        path: String,
        #[arg(long)]
        from: Option<String>,
    },
    /// Cherry-pick a commit onto current branch (prompt/save on conflict)
    Cp { commit: String },
    /// Manage remotes
    Remote {
        #[command(subcommand)]
        action: RemoteAction,
    },
    /// Push commits to a remote
    Push {
        remote: String,
        branch: Option<String>,
    },
}

#[derive(Subcommand)]
enum StashAction {
    /// Save working directory as a named stash
    Save {
        name: String,
    },
    List,
    Restore {
        name: String,
    },
    Drop {
        name: String,
    },
}

#[derive(Subcommand)]
enum IgnoreAction {
    /// Add a pattern to .vaultignore
    Add {
        pattern: String,
    },
    List,
    Check {
        file: String,
    },
}

#[derive(Subcommand)]
enum TagAction {
    /// Create a tag at HEAD or a given hash
    Create {
        name: String,
        #[arg(long)]
        hash: Option<String>,
    },
    /// List all tags
    List,
}

#[derive(Subcommand)]
enum SnapshotAction {
    /// Save a named snapshot of the working directory
    Save {
        name: String,
        #[arg(long)]
        note: Option<String>,
    },
    /// List all snapshots
    List,
    /// Restore a snapshot to the working directory
    Restore { name: String },
    /// Delete a snapshot
    Drop { name: String },
}

#[derive(Subcommand)]
enum RemoteAction {
    /// Add a new remote
    Add {
        name: String,
        url: String,
    },
    /// List all remotes
    List,
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Init { name, email } => init::run(&name, &email),
        Commands::Save { message } => save::run(message.as_deref()),
        Commands::Status => status::run(),
        Commands::Diff { stat, a, b } => diff::run(stat, a.as_deref(), b.as_deref()),
        Commands::Log => log::run(),
        Commands::Branch { name } => branch::run(&name),
        Commands::Switch { name } => switch::run(&name),
        Commands::Merge { name } => merge::run(&name),
        Commands::Undo => undo::run(),
        Commands::Oplog => oplog::run(),
        Commands::Show { id } => show::run(&id),

        Commands::Amend { message, no_edit } => amend::run(message.as_deref(), no_edit),
        Commands::Split { msg1, msg2 } => split::run(msg1.as_deref(), msg2.as_deref()),
        Commands::Squash { n, message } => squash::run(n, message.as_deref()),
        Commands::Resolve { file } => resolve::run(file.as_deref()),
        Commands::Stash { action } => match action {
            StashAction::Save { name } => stash::run_save(&name),
            StashAction::List => stash::run_list(),
            StashAction::Restore { name } => stash::run_restore(&name),
            StashAction::Drop { name } => stash::run_drop(&name),
        },
        Commands::Ignore { action } => match action {
            IgnoreAction::Add { pattern } => ignore::run_add(&pattern),
            IgnoreAction::List => ignore::run_list(),
            IgnoreAction::Check { file } => ignore::run_check(&file),
        },
        Commands::Tag { action } => match action {
            TagAction::Create { name, hash } => run_create(&name, hash.as_deref()),
            TagAction::List => run_list(),
        },

        Commands::Snapshot { action } => match action {
            SnapshotAction::Save { name, note } => snapshot::run_save(&name, note.as_deref()),
            SnapshotAction::List => snapshot::run_list(),
            SnapshotAction::Restore { name } => snapshot::run_restore(&name),
            SnapshotAction::Drop { name } => snapshot::run_drop(&name),
        },
        Commands::Restore { path, from } => restore::run(&path, from.as_deref()),
        Commands::Cp { commit } => cp::run(&commit),
        Commands::Remote { action } => match action {
            RemoteAction::Add { name, url } => remote::run_add(&name, &url),
            RemoteAction::List => remote::run_list(),
        },
        Commands::Push { remote, branch } => push::run(&remote, branch.as_deref()),
    };

    if let Err(e) = result {
        eprintln!("{} {}", colored::Colorize::red("error:"), e);
        std::process::exit(1);
    }
}
