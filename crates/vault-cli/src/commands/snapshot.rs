use anyhow::Result;
use colored::Colorize;
use std::env;
use vault_core::Repo;

pub fn run_save(name: &str, note: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.snapshot_save(name, note)?;
    println!("{} Snapshot '{}' saved", "✓".green(), name.cyan());
    Ok(())
}

pub fn run_list() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    let snapshots = repo.snapshot_list()?;

    if snapshots.is_empty() {
        println!("No snapshots.");
        return Ok(());
    }

    println!("{}", "Snapshots".bold().underline());
    println!();
    for s in &snapshots {
        let note = s.note.as_deref().unwrap_or("");
        println!(
            "  {}  {}  {}  {}",
            "◈".cyan(),
            s.name.bold(),
            s.created_at.format("%Y-%m-%d %H:%M").to_string().dimmed(),
            note.dimmed(),
        );
    }
    Ok(())
}

pub fn run_restore(name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.snapshot_restore(name)?;
    println!(
        "{} Snapshot '{}' restored to working directory",
        "✓".green(),
        name.cyan()
    );
    Ok(())
}

pub fn run_drop(name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.snapshot_drop(name)?;
    println!("{} Snapshot '{}' dropped", "✓".green(), name.cyan());
    Ok(())
}
