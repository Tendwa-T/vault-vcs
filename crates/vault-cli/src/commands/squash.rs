use anyhow::Result;
use colored::Colorize;
use std::env;
use std::io::{self, Write};
use vault_core::Repo;

pub fn run(n: usize, message: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    let msg = match message {
        Some(m) => m.to_string(),
        None => {
            print!("Squashed commit message: ");
            io::stdout().flush()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
            let trimmed = buf.trim().to_string();
            if trimmed.is_empty() {
                anyhow::bail!("Commit message cannot be empty");
            }
            trimmed
        }
    };

    let new_hash = repo.squash(n, &msg)?;
    println!(
        "{} Squashed {} commits → [{}] {}",
        "✓".green(),
        n,
        &new_hash[..8].yellow(),
        msg
    );
    Ok(())
}
