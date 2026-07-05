use anyhow::Result;
use colored::Colorize;
use std::env;
use vault_core::Repo;

pub fn run_save(name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.stash_save(name)?;
    println!(
        "{} Stashed as '{}' — working directory restored to HEAD",
        "✓".green(),
        name.cyan()
    );
    Ok(())
}

pub fn run_list() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    let stashes = repo.stash_list()?;

    if stashes.is_empty() {
        println!("No stashes.");
        return Ok(());
    }

    println!("{}", "Stashes".bold().underline());
    println!();
    for s in &stashes {
        println!(
            "  {} {}  {}  {}",
            "◆".mauve(),
            s.name.cyan().bold(),
            s.created_at.format("%Y-%m-%d %H:%M").to_string().dimmed(),
            format!("on {}", s.branch).dimmed(),
        );
    }
    Ok(())
}

pub fn run_restore(name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.stash_restore(name)?;
    println!("{} Restored stash '{}'", "✓".green(), name.cyan());
    Ok(())
}

pub fn run_drop(name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.stash_drop(name)?;
    println!("{} Dropped stash '{}'", "✓".green(), name.cyan());
    Ok(())
}

// Add this trait locally for the .mauve() colour shorthand
trait ColorizeExtra {
    fn mauve(self) -> colored::ColoredString;
}
impl ColorizeExtra for &str {
    fn mauve(self) -> colored::ColoredString {
        self.truecolor(203, 166, 247)
    }
}
