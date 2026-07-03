use anyhow::{Ok, Result};
use colored::Colorize;
use std::env;
use vault_core::{Repo, repo::MergeOutcome};

pub fn run(branch: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    match repo.merge(branch)? {
        MergeOutcome::AlreadyUpToDate => {
            println!("{} Already up to date.", "✓".green());
        }
        MergeOutcome::FastForward(hash) => {
            println!("{} Fast-forward to {}", "✓".green(), &hash[..8].yellow());
        }
        MergeOutcome::Clean(hash) => {
            println!(
                "{} Merged '{}' -> [{}]",
                "✓".green(),
                branch.cyan(),
                &hash[..8].yellow()
            );
        }
        MergeOutcome::Conflicts(hash, paths) => {
            println!(
                "{} Merged '{}' -> [{}] with {} conflicts(s):",
                "!".yellow().bold(),
                branch.cyan(),
                &hash[..8].yellow(),
                paths.len().to_string().red().bold()
            );
            for path in &paths {
                println!("  {}  {}", "conflicts:".red(), path);
            }
            println!();
            println!(
                "Edit the conflicted files, then run {} to save the resolution.",
                "'vault save'".cyan()
            );
        }
    }
    Ok(())
}
