use anyhow::{Ok, Result};
use colored::Colorize;
use std::env;
use vault_core::{
    Repo,
    diff::{DiffKind, DiffLine},
};

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    let diffs = repo.diff()?;

    if diffs.is_empty() {
        println!("No changes");
        return Ok(());
    }

    for file_diff in &diffs {
        let header = match file_diff.kind {
            DiffKind::Added => format!("+ {}", file_diff.path).green().to_string(),
            DiffKind::Deleted => format!("- {}", file_diff.path).red().to_string(),
            DiffKind::Modified => format!("~ {}", file_diff.path).yellow().to_string(),
        };

        println!("\n{}", header);
        println!("{}", "--".repeat(60).dimmed());

        for hunk in &file_diff.hunks {
            println!(
                "{}",
                format!("@@ -{} +{} @@", hunk.old_start, hunk.new_start).cyan()
            );

            for line in &hunk.lines {
                match line {
                    DiffLine::Context(l) => println!("{}", l),
                    DiffLine::Added(l) => println!("{}", format!("+  {}", l).green()),
                    DiffLine::Removed(l) => println!("{}", format!("--  {}", l).red()),
                }
            }
        }
    }
    Ok(())
}
