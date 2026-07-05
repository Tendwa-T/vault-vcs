use anyhow::Result;
use colored::Colorize;
use std::env;
use std::io::{self, Write};
use vault_core::Repo;

pub fn run(message: Option<&str>, no_edit: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    let msg = if no_edit {
        None
    } else {
        match message {
            Some(m) => Some(m.to_string()),
            None => {
                // Show current message and allow override
                if let Some(h) = repo.store.resolve_head()? {
                    let c = repo.store.read_commit(&h)?;
                    println!("Current message: {}", c.message.dimmed());
                }
                print!("New message (Enter to keep current): ");
                io::stdout().flush()?;
                let mut buf = String::new();
                io::stdin().read_line(&mut buf)?;
                let trimmed = buf.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }
        }
    };

    let outcome = repo.amend(msg.as_deref())?;
    println!("{} amended [{}]", "●".green(), &outcome.new_hash[..8].yellow());
    Ok(())
}
