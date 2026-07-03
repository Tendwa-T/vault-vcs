use anyhow::{Ok, Result};
use colored::Colorize;
use std::env;
use vault_core::{Repo, objects::FileStatus};

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    let branch = repo.store.current_branch()?.unwrap_or("HEAD".to_string());

    println!("On branch {}", branch.cyan());

    let entries = repo.status()?;

    if entries.is_empty() {
        println!(
            "Nothing to commit -- working directory clean {}",
            "✓".green()
        );
        return Ok(());
    }

    println!();
    for e in &entries {
        let (symbol, colored_path) = match e.status {
            FileStatus::Added => ("+".green(), e.path.green()),
            FileStatus::Modified => ("~".yellow(), e.path.yellow()),
            FileStatus::Deleted => ("-".red(), e.path.red()),
            FileStatus::Untracked => ("?".dimmed(), e.path.dimmed()),
        };
        println!(" {} {}", symbol, colored_path);
    }
    println!();
    println!(" {} file(s) changed", entries.len().to_string().bold());
    Ok(())
}
