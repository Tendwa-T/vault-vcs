use anyhow::{Ok, Result};
use colored::Colorize;
use std::env;
use vault_core::{Repo, oplog::OpLog};

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    let log = OpLog::new(&repo.vault_dir);
    let entries = log.read_all()?;

    if entries.is_empty() {
        println!("Operation log is empty");
        return Ok(());
    }

    println!("{}", "Operation Log".bold().underline());
    println!();

    for entry in entries.iter().rev() {
        let mark = if entry.undone {
            "↩️".dimmed()
        } else {
            "●".green()
        };
        let op = if entry.undone {
            entry.op.dimmed().to_string()
        } else {
            entry.op.cyan().bold().to_string()
        };

        let detail = match entry.op.as_str() {
            "save" => entry.message.clone().unwrap_or_default(),
            "branch" => format!("branch '{}'", entry.branch.clone().unwrap_or_default()),
            "switch" => format!("-> '{}'", entry.branch.clone().unwrap_or_default()),
            "merge" => format!("merge '{}'", entry.extra.clone().unwrap_or_default()),
            "init" => "repository created".to_string(),
            "undo" => "undone".to_string(),
            _ => String::new(),
        };

        let hash_part = entry
            .head_after
            .as_ref()
            .map(|h| format!("[{}]", &h[..8.min(h.len())]).yellow().to_string())
            .unwrap_or_default();

        println!(
            "{}  {:12}  {}  {}  {}",
            mark,
            op,
            entry.timestamp.format("%m-%d %H:%M").to_string().dimmed(),
            hash_part,
            detail
        );
    }

    Ok(())
}
