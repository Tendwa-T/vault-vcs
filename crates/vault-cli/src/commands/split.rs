use anyhow::{Ok, Result, bail};
use chrono::Utc;
use colored::Colorize;
use crossterm::event::KeyCode;
use std::{collections::HashMap, env};
use uuid::Uuid;
use vault_core::{
    Repo,
    diff::flatten_tree,
    objects::{Commit, EntryKind, FileStatus},
};

use crate::tui::{Term, palette as p};

struct SplitEntry {
    path: String,
    status: FileStatus,
    // false -> commit1, true -> commit 2
    in_second: bool,
}

pub fn run(msg1: Option<&str>, msg2: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    let entries_raw = repo.status()?;
    if entries_raw.is_empty() {
        println!("Nothing to split -- Working directory clean.");
        return Ok(());
    }
    if entries_raw.len() < 2 {
        bail!("split needs at least 2 changed files. Use 'vault save' instead.");
    }

    let mut entries: Vec<SplitEntry> = entries_raw
        .into_iter()
        .map(|e| SplitEntry {
            path: e.path,
            status: e.status,
            in_second: false,
        })
        .collect();

    let mut cursor: usize = 0;

    let message1 = match msg1 {
        Some(m) => m.to_string(),
        None => prompt_message("Commit 1 message")?,
    };

    let message2 = match msg2 {
        Some(m) => m.to_string(),
        None => prompt_message("Commit 2 message")?,
    };

    let term = Term::enter()?;
    let result = run_tui(&term, &mut entries, &mut cursor, &message1, &message2);
    term.leave()?;

    match result? {
        SplitAction::Confirm => {
            let first: Vec<&str> = entries
                .iter()
                .filter(|e| !e.in_second)
                .map(|e| e.path.as_str())
                .collect();

            let second: Vec<&str> = entries
                .iter()
                .filter(|e| !e.in_second)
                .map(|e| e.path.as_str())
                .collect();

            if first.is_empty() {
                bail!("Commit 1 has no files. Toggle at least one file to stay in commit 1");
            }
            if second.is_empty() {
                bail!("Commit 2 has no files. Toggle at least one file to commit 2");
            }

            commit_split(&repo, &first, &message1, &second, &message2)?;
            println!("{} Split into 2 commits.", "✓".green());
        }
        SplitAction::Cancel => {
            println!("Split cancelled.");
        }
    }
    Ok(())
}

fn run_tui(
    term: &Term,
    entries: &mut [SplitEntry],
    cursor: &mut usize,
    message1: &str,
    message2: &str,
) -> anyhow::Result<SplitAction> {
    loop {
        draw(term, entries, *cursor, message1, message2)?;

        match Term::read_key()? {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(SplitAction::Cancel),
            KeyCode::Enter => return Ok(SplitAction::Confirm),

            KeyCode::Up | KeyCode::Char('k') => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if *cursor + 1 < entries.len() {
                    *cursor += 1;
                }
            }
            KeyCode::Char(' ') => {
                entries[*cursor].in_second = !entries[*cursor].in_second;
            }
            _ => {}
        }
    }
}

fn draw(
    term: &Term,
    entries: &[SplitEntry],
    cursor: usize,
    message1: &str,
    message2: &str,
) -> anyhow::Result<()> {
    term.clear()?;
    let w = term.cols as usize;

    // Title bar
    term.draw_full_row(
        0,
        " nya split — Distribute changes across two commits",
        p::MAUVE,
        p::CRUST,
        true,
    )?;

    // Column headers
    let col1_w = w / 2 - 2;
    let col2_w = w - col1_w - 4;
    let header1 = format!(" Commit 1: {}", Term::fit(message1, col1_w - 12));
    let header2 = format!(" Commit 2: {}", Term::fit(message2, col2_w - 12));
    term.draw_row(
        0,
        2,
        &format!("{:<col1_w$} │ {}", header1, header2, col1_w = col1_w),
        p::BLUE,
        p::SURFACE0,
        true,
    )?;
    term.draw_hline(3, '─', p::SURFACE1, p::BASE)?;

    // File rows
    for (i, entry) in entries.iter().enumerate() {
        let row = (i + 4) as u16;
        if row >= term.rows - 3 {
            break;
        }

        let is_cursor = i == cursor;
        let bg = if is_cursor { p::SURFACE0 } else { p::BASE };

        let status_ch = match entry.status {
            FileStatus::Added => "+",
            FileStatus::Modified => "~",
            FileStatus::Deleted => "-",
            FileStatus::Untracked => "?",
        };
        let status_color = match entry.status {
            FileStatus::Added => p::GREEN,
            FileStatus::Modified => p::YELLOW,
            FileStatus::Deleted => p::RED,
            FileStatus::Untracked => p::OVERLAY0,
        };

        let (left_mark, right_mark) = if entry.in_second {
            (
                "                               ".to_string(),
                format!("  {} {}", status_ch, entry.path),
            )
        } else {
            (format!("  {} {}", status_ch, entry.path), "".to_string())
        };

        let left_padded = Term::fit(&left_mark, col1_w);
        let right_padded = Term::fit(&right_mark, col2_w);
        let full = format!("{} │ {}", left_padded, right_padded);
        term.draw_full_row(row, &full, status_color, bg, is_cursor)?;
    }

    // Hint bar
    let hint_row = term.rows - 1;
    term.draw_full_row(
        hint_row,
        " ↑/↓ or j/k: move   Space: toggle side   Enter: confirm   q/Esc: cancel",
        p::SUBTEXT0,
        p::CRUST,
        false,
    )?;

    io_flush()
}

fn io_flush() -> anyhow::Result<()> {
    use std::io::Write;
    std::io::stdout().flush()?;
    Ok(())
}

fn prompt_message(label: &str) -> anyhow::Result<String> {
    use std::io::{self, Write};
    print!("{} : ", label);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let msg = buf.trim().to_string();
    if msg.is_empty() {
        bail!("{} cannot be empty", label);
    }
    Ok(msg)
}

fn commit_split(
    repo: &Repo,
    first: &[&str],
    message1: &str,
    second: &[&str],
    message2: &str,
) -> anyhow::Result<()> {
    let head_hash = repo.store.resolve_head()?;
    let base_tree = match &head_hash {
        Some(h) => Some(repo.store.read_commit(h)?.tree),
        None => None,
    };

    let work_tree = repo.snapshot_working_dir()?;
    let mut base_flat: HashMap<String, (EntryKind, String)> = HashMap::new();
    if let Some(ref bt) = base_tree {
        flatten_tree(&repo.store, bt, "", &mut base_flat)?;
    }

    let mut work_flat: HashMap<String, (EntryKind, String)> = HashMap::new();
    flatten_tree(&repo.store, &work_tree, "", &mut work_flat)?;

    let mut tree1_flat = base_flat.clone();
    for path in first {
        match work_flat.get(*path) {
            Some(entry) => {
                tree1_flat.insert(path.to_string(), entry.clone());
            }
            None => {
                tree1_flat.remove(*path);
            }
        }
    }

    let mut tree2_flat = tree1_flat.clone();
    for path in second {
        match work_flat.get(*path) {
            Some(entry) => {
                tree2_flat.insert(path.to_string(), entry.clone());
            }
            None => {
                tree2_flat.remove(*path);
            }
        }
    }

    let tree1_hash = vault_core::merge::build_tree_from_flat_pub(&repo.store, &tree1_flat)?;
    let tree2_hash = vault_core::merge::build_tree_from_flat_pub(&repo.store, &tree2_flat)?;

    let author = repo.author();
    let branch = repo.store.current_branch()?.unwrap_or("main".to_string());
    let parents = head_hash
        .as_ref()
        .map(|h| vec![h.clone()])
        .unwrap_or_default();

    let c1 = Commit {
        tree: tree1_hash,
        parents: parents.clone(),
        author: author.clone(),
        timestamp: Utc::now(),
        message: message1.to_string(),
        change_id: Uuid::now_v7().to_string(),
    };
    let hash1 = repo.store.write_commit(&c1)?;

    let c2 = Commit {
        tree: tree2_hash,
        parents: vec![hash1.clone()],
        author: author.clone(),
        timestamp: Utc::now(),
        message: message2.to_string(),
        change_id: Uuid::now_v7().to_string(),
    };
    let hash2 = repo.store.write_commit(&c2)?;

    repo.store.write_branch(&branch, &hash2)?;

    println!("{} [{}] {}", "●".green(), &hash1[..8].yellow(), message1);
    println!("{} [{}] {}", "●".green(), &hash2[..8].yellow(), message2);

    Ok(())
}


enum SplitAction {
    Confirm,
    Cancel,
}
