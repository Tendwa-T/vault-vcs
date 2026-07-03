use anyhow::{Ok, Result};
use colored::Colorize;
use std::env;
use vault_core::Repo;

pub fn run(name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.switch(name)?;

    let head = repo.store.resolve_head()?.unwrap_or_default();
    let short = if head.len() >= 8 { &head[..8] } else { &head };

    println!(
        "{} Switched to branch '{}' ({})",
        "✓".green(),
        name.cyan(),
        short.yellow()
    );

    Ok(())
}
