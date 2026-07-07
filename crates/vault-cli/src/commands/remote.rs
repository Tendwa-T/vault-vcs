use anyhow::Result;
use colored::Colorize;
use std::env;
use vault_core::{
    Repo,
    remote::{self, add_remote},
};

pub fn run_add(name: &str, url: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    add_remote(&repo.vault_dir, name, url)?;
    println!(
        "{} Remote '{}' → {}",
        "✓".green(),
        name.cyan(),
        url.yellow()
    );
    Ok(())
}

pub fn run_list() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    let remotes = remote::list_remotes(&repo.vault_dir)?;

    if remotes.is_empty() {
        println!("No remotes configured.");
        return Ok(());
    }
    for r in &remotes {
        println!("  {}  {}", r.name.cyan().bold(), r.url.yellow());
    }
    Ok(())
}
