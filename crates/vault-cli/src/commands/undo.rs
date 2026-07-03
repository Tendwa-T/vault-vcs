use anyhow::{Ok, Result};
use colored::Colorize;
use std::env;
use vault_core::{Repo, repo::UndoOutcome};

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    match repo.undo()? {
        UndoOutcome::RestoredToEmpty => {
            println!(
                "{} Undone -- reposiotry restored to empty state",
                "✓".green()
            );
        }
        UndoOutcome::Restored(hash) => {
            println!(
                "{}  Undone -- restored to [{}]",
                "✓".green(),
                &hash[..8].yellow()
            );
        }
    }

    Ok(())
}
