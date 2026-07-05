use anyhow::Result;
use colored::Colorize;
use std::env;
use vault_core::Repo;

pub fn run(path: &str, from: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    repo.restore_file(path, from)?;

    match from {
        Some(h) => println!(
            "{} '{}' restored from [{}]",
            "✓".green(),
            path.cyan(),
            &h[..8].yellow()
        ),
        None => println!("{} '{}' restored to HEAD", "✓".green(), path.cyan()),
    }
    Ok(())
}
