use anyhow::{Ok, Result, anyhow};
use colored::Colorize;
use std::env;
use std::io::{self, Write};
use vault_core::Repo;

pub fn run(message: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    let msg = match message {
        Some(m) => m.to_string(),
        None => {
            let head = repo
                .store
                .resolve_head()?
                .ok_or_else(|| anyhow!("Nothing to amend"))?;
            let old = repo.store.read_commit(&head)?;
            println!("{} {}", "Current message:".dimmed(), old.message);
            print!("New message (blank to keep): ");
            io::stdout().flush()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
            let trimmed = buf.trim().to_string();
            if trimmed.is_empty() {
                old.message.clone()
            } else {
                trimmed
            }
        }
    };

    let outcome = repo.amend(Some(&msg))?;

    println!(
        "{} Amended {} -> {}",
        "●".yellow(),
        &outcome.old_hash[..8].dimmed(),
        &outcome.new_hash[..8].yellow()
    );
    println!("  Message     {}", msg);
    Ok(())
}
