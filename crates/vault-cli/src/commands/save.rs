use anyhow::{Ok, Result};
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
            print!("Commit message: ");
            io::stdout().flush()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
            buf.trim().to_string()
        }
    };

    if msg.is_empty() {
        eprintln!("{} Commit message cannot be empty", "error:".red());
        std::process::exit(1);
    }

    let hash = repo.save(&msg)?;
    let short = &hash[..8];
    println!("{} [{}] {}", "●".green(), short.yellow(), msg);
    Ok(())
}
