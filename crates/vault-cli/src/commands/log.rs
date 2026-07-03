use anyhow::{Ok, Result};
use colored::Colorize;
use std::env;
use vault_core::{Repo, dag};

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    let head = match repo.store.resolve_head()? {
        Some(h) => h,
        None => {
            println!("No commits yet");
            return Ok(());
        }
    };

    let current_branch = repo.store.current_branch()?.unwrap_or_default();
    let commits = dag::log_walk(&repo.store, &head)?;

    for (hash, commit) in commits {
        let short = &hash[..8];

        let branches: Vec<String> = repo
            .store
            .list_branches()?
            .into_iter()
            .filter(|b| repo.store.read_branch(b).ok().as_deref() == Some(&hash))
            .map(|b| {
                if b == current_branch {
                    format!("HEAD -> {}", b).cyan().bold().to_string()
                } else {
                    b.yellow().to_string()
                }
            })
            .collect();

        let decoration = if branches.is_empty() {
            String::new()
        } else {
            format!("  ({})", branches.join(", "))
        };

        println!(
            "{}  {}{}",
            format!("●  {}", short).yellow(),
            commit.message.bold(),
            decoration
        );
        println!(
            "   {}  {}   {}",
            commit.author.name.dimmed(),
            commit
                .timestamp
                .format("%Y-%m-%d %H:%M")
                .to_string()
                .dimmed(),
            format!("change: {}", &commit.change_id[..8]).dimmed()
        );

        if commit.parents.len() > 1 {
            println!(
                "    {}",
                format!(
                    "Merge: {}",
                    commit
                        .parents
                        .iter()
                        .map(|p| &p[..8])
                        .collect::<Vec<_>>()
                        .join(" <-> ")
                )
                .dimmed()
            );
        }
        println!();
    }
    Ok(())
}
