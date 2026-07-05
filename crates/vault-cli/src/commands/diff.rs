use anyhow::Result;
use colored::Colorize;
use std::env;
use vault_core::{
    Repo,
    dag::log_walk,
    diff::{DiffKind, DiffLine, diff_trees},
};

/// C2: --stat flag shows summary only
/// C3: optional <old> <new> commit hashes
pub fn run(stat: bool, commit_a: Option<&str>, commit_b: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    let diffs = match (commit_a, commit_b) {
        // C3: diff two explicit commits
        (Some(a), Some(b)) => {
            let hash_a = resolve_hash(&repo, a)?;
            let hash_b = resolve_hash(&repo, b)?;
            let ca = repo.store.read_commit(&hash_a)?;
            let cb = repo.store.read_commit(&hash_b)?;
            diff_trees(&repo.store, Some(&ca.tree), Some(&cb.tree))?
        }
        // Working dir vs HEAD (original behaviour)
        _ => repo.diff()?,
    };

    if diffs.is_empty() {
        println!("No changes.");
        return Ok(());
    }

    if stat {
        // C2: --stat summary
        let mut total_add = 0usize;
        let mut total_del = 0usize;
        let max_path = diffs.iter().map(|d| d.path.len()).max().unwrap_or(20);

        println!();
        for fd in &diffs {
            let adds: usize = fd
                .hunks
                .iter()
                .flat_map(|h| &h.lines)
                .filter(|l| matches!(l, DiffLine::Added(_)))
                .count();
            let dels: usize = fd
                .hunks
                .iter()
                .flat_map(|h| &h.lines)
                .filter(|l| matches!(l, DiffLine::Removed(_)))
                .count();
            total_add += adds;
            total_del += dels;

            let bar_add = "+".repeat(adds.min(40));
            let bar_del = "-".repeat(dels.min(40));
            let kind_sym = match fd.kind {
                DiffKind::Added => "+".green(),
                DiffKind::Deleted => "-".red(),
                DiffKind::Modified => "~".yellow(),
            };
            println!(
                "  {} {:path_w$}  {:>4}  {}{}",
                kind_sym,
                fd.path,
                format!("+{} -{}", adds, dels).dimmed(),
                bar_add.green(),
                bar_del.red(),
                path_w = max_path,
            );
        }
        println!();
        println!(
            "  {} file(s)  {} insertion(s)  {} deletion(s)",
            diffs.len().to_string().bold(),
            total_add.to_string().green(),
            total_del.to_string().red(),
        );
        return Ok(());
    }

    // Full diff output (original behaviour)
    for file_diff in &diffs {
        let header = match file_diff.kind {
            DiffKind::Added => format!("+ {}", file_diff.path).green().to_string(),
            DiffKind::Deleted => format!("- {}", file_diff.path).red().to_string(),
            DiffKind::Modified => format!("~ {}", file_diff.path).yellow().to_string(),
        };
        println!("\n{}", header);
        println!("{}", "─".repeat(60).dimmed());

        for hunk in &file_diff.hunks {
            println!(
                "{}",
                format!("@@ -{} +{} @@", hunk.old_start, hunk.new_start).cyan()
            );
            for line in &hunk.lines {
                match line {
                    DiffLine::Context(l) => println!("  {}", l),
                    DiffLine::Added(l) => println!("{}", format!("+  {}", l).green()),
                    DiffLine::Removed(l) => println!("{}", format!("-  {}", l).red()),
                }
            }
        }
    }

    Ok(())
}

fn resolve_hash(repo: &Repo, prefix: &str) -> Result<String> {
    let head = match repo.store.resolve_head()? {
        Some(h) => h,
        None => anyhow::bail!("No commits"),
    };
    if head.starts_with(prefix) {
        return Ok(head);
    }
    for (hash, _) in log_walk(&repo.store, &head)? {
        if hash.starts_with(prefix) {
            return Ok(hash);
        }
    }
    anyhow::bail!("No commit with prefix '{}'", prefix)
}
