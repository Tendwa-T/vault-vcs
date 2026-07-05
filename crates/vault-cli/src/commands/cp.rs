use anyhow::Result;
use colored::Colorize;
use std::env;
use std::io::{self, Write};
use vault_core::dag::log_walk;
use vault_core::{Repo, repo::CherryPickOutcome};

pub fn run(commit: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    // Resolve short hash
    let full_hash = resolve_hash(&repo, commit)?;

    match repo.cherry_pick(&full_hash, false)? {
        CherryPickOutcome::Clean(hash) => {
            println!(
                "{} Cherry-picked [{}] → [{}]",
                "✓".green(),
                &full_hash[..8].yellow(),
                &hash[..8].yellow()
            );
        }
        CherryPickOutcome::Conflict(paths) => {
            println!(
                "{} Cherry-pick conflict(s) detected — {} file(s) conflicted:",
                "✗".red().bold(),
                paths.len().to_string().red()
            );
            for p in &paths {
                println!("  {} {}", "conflict:".red(), p);
            }
            println!();
            print!(
                "Would you like to continue and save (commit with conflict markers) or abort? [y/N]: "
            );
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let choice = input.trim().to_lowercase();
            if choice == "y" || choice == "yes" {
                match repo.cherry_pick(&full_hash, true)? {
                    CherryPickOutcome::ConflictSaved(hash, _) => {
                        println!(
                            "{} Cherry-picked with conflicts [{}] → [{}]",
                            "✓".green(),
                            &full_hash[..8].yellow(),
                            &hash[..8].yellow()
                        );
                    }
                    _ => unreachable!(),
                }
            } else {
                println!("{} Cherry-pick aborted.", "!".yellow());
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn resolve_hash(repo: &Repo, prefix: &str) -> Result<String> {
    let head = match repo.store.resolve_head()? {
        Some(h) => h,
        None => anyhow::bail!("No commits in repository"),
    };
    if head.starts_with(prefix) {
        return Ok(head);
    }
    let commits = log_walk(&repo.store, &head)?;
    for (hash, _) in commits {
        if hash.starts_with(prefix) {
            return Ok(hash);
        }
    }
    anyhow::bail!("No commit found with prefix '{}'", prefix)
}
