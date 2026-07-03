use anyhow::{Ok, Result, bail};
use colored::Colorize;
use std::env;
use vault_core::{
    Repo, dag,
    diff::{DiffKind, DiffLine},
};

pub fn run(id: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    let hash = resolve_short_hash(&repo, id)?;
    let result = repo.show(&hash)?;
    let c = &result.commit;

    println!("{} {}", "commit".yellow(), hash.yellow());
    println!("change    {}", &c.change_id[..8].dimmed());
    println!("author    {} <{}>", c.author.name, c.author.email);
    println!("date      {}", c.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));

    if c.parents.len() > 1 {
        println!(
            "merge     {}",
            c.parents
                .iter()
                .map(|p| &p[..8])
                .collect::<Vec<_>>()
                .join(" <-> ")
        );
    }
    println!();
    println!("      {}", c.message.bold());
    println!();

    for file_diff in &result.diffs {
        let header = match file_diff.kind {
            DiffKind::Added => format!("+   {}", file_diff.path).green().to_string(),
            DiffKind::Deleted => format!("-     {}", file_diff.path).red().to_string(),
            DiffKind::Modified => format!("~    {}", file_diff.path).yellow().to_string(),
        };
        println!("{}", header);
        println!("{}", "--".repeat(50).dimmed());
        for hunk in &file_diff.hunks {
            println!(
                "{}",
                format!("@@ -{} +{} @@", hunk.old_start, hunk.new_start).cyan()
            );
            for line in &hunk.lines {
                match line {
                    DiffLine::Context(l) => println!("{}", l),
                    DiffLine::Added(l) => println!("{}", format!("+   {}", l).green()),
                    DiffLine::Removed(l) => println!("{}", format!("--    {}", l).red()),
                }
            }
        }
        println!();
    }
    Ok(())
}

fn resolve_short_hash(repo: &Repo, prefix: &str) -> Result<String> {
    let head = match repo.store.resolve_head()? {
        Some(h) => h,
        None => bail!("No commits in repository"),
    };

    if head.starts_with(prefix) {
        return Ok(head.clone());
    }

    let commits = dag::log_walk(&repo.store, &head)?;
    for (hash, _) in commits {
        if hash.starts_with(prefix) {
            return Ok(hash);
        }
    }
    bail!("No commit found with prefix: '{}'", prefix)
}
