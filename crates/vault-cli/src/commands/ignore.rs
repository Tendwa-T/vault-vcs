use anyhow::{Ok, Result};
use colored::Colorize;
use std::env;
use vault_core::Repo;

pub fn run_add(pattern: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.ignore_add(pattern)?;
    println!(
        "{} Added '{}' to .vaultignore",
        "✓".green(),
        pattern.yellow()
    );
    Ok(())
}

pub fn run_list() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    let patterns = repo.ignore_list()?;

    if patterns.is_empty() {
        println!("No ignore patterns. (.vaultignore is empty or missing)");
        return Ok(());
    }

    println!("{}", ".vaultignore".bold().underline());
    println!();
    for p in &patterns {
        println!("  {}", p.yellow());
    }

    Ok(())
}

pub fn run_check(file: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    match repo.ignore_check(file)? {
        Some(pattern) => {
            println!(
                "{} '{}' is ignored by pattern '{}'",
                "✓".yellow(),
                file,
                pattern.yellow()
            );
        }
        None => {
            println!("{} '{}' is not ignored", "✓".green(), file);
        }
    }
    Ok(())
}
