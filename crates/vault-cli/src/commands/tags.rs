use anyhow::Result;
use colored::Colorize;
use std::env;
use vault_core::Repo;

pub fn run_create(name: &str, hash: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.create_tag(name, hash)?;
    println!("{} Tag '{}' created", "✓".green(), name.cyan());
    Ok(())
}

pub fn run_list() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    let tags = repo.list_tags()?;

    if tags.is_empty() {
        println!("No tags.");
        return Ok(());
    }
    println!("{}", "Tags".bold().underline());
    println!();
    for (name, hash) in &tags {
        println!("  {}  {}", name.cyan().bold(), &hash[..8].yellow());
    }
    Ok(())
}
