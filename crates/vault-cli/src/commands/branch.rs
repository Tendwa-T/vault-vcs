use anyhow::{Ok, Result};
use colored::Colorize;
use std::env;
use vault_core::Repo;

pub fn run(name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.create_branch(name)?;
    println!("{} Branch '{}' created", "✓".green(), name.cyan());
    Ok(())
}
