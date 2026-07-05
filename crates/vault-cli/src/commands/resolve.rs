use anyhow::{Ok, Result, bail};
use colored::Colorize;
use crossterm::event::KeyCode;
use std::env;
use std::{collections::HashMap, fs};
use vault_core::merge::build_tree_from_flat_pub;
use vault_core::objects::Commit;
use vault_core::{
    Repo,
    diff::flatten_tree,
    objects::EntryKind,
};

use crate::tui::{Term, palette as p};

pub fn run(file: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;

    let head_hash = repo
        .store
        .resolve_head()?
        .ok_or_else(|| anyhow::anyhow!("No commits yet"))?;
    let head_commit = repo.store.read_commit(&head_hash)?;

    let mut flat: HashMap<String, (EntryKind, String)> = HashMap::new();
    flatten_tree(&repo.store, &head_commit.tree, "", &mut flat)?;

    let conflict_paths: Vec<String> = flat
        .iter()
        .filter(|(_, (kind, _))| matches!(kind, EntryKind::Conflict))
        .map(|(path, _)| path.clone())
        .collect();

    if conflict_paths.is_empty() {
        println!("{} No conflicts to resolve.", "✓".green());
        return Ok(());
    }

    let target_path = match file {
        Some(f) => {
            if !conflict_paths.contains(&f.to_string()) {
                bail!("'{}' is not in conflict", f);
            }
            f.to_string()
        }
        None => conflict_paths[0].clone(),
    };

    let (_, conflict_hash) = flat.get(&target_path).unwrap();
    let conflict = repo.store.read_conflicts(conflict_hash)?;
    let ours_bytes = repo.store.read_blob(&conflict.ours)?;
    let theirs_bytes = repo.store.read_blob(&conflict.theirs)?;
    let ancestor_bytes = if conflict.ancestor.is_empty() {
        Vec::new()
    } else {
        repo.store.read_blob(&conflict.ancestor).unwrap_or_default()
    };

    let ours_text = String::from_utf8_lossy(&ours_bytes).to_string();
    let theirs_text = String::from_utf8_lossy(&theirs_bytes).to_string();
    let ancestor_text = String::from_utf8_lossy(&ancestor_bytes).to_string();

    let ours_lines: Vec<&str> = ours_text.lines().collect();
    let theirs_lines: Vec<&str> = theirs_text.lines().collect();
    let ancestor_lines: Vec<&str> = ancestor_text.lines().collect();

    let term = Term::enter()?;
    let resolved = run_resolve_tui(
        &term,
        &target_path,
        &ours_lines,
        &ancestor_lines,
        &theirs_lines,
    );
    term.leave()?;

    match resolved? {
        None => {
            println!("Resolve cancelled.");
        }
        Some(merged_lines) => {
            let content = merged_lines.join("\n");
            let blob_hash = repo.store.write_blob(content.as_bytes())?;
            let abs = repo.work_dir.join(&target_path);
            fs::write(&abs, &content)?;
            update_tree_resolve(&repo, &head_hash, &target_path, &blob_hash)?;

            println!(
                "{} Conflict in '{}' resolved",
                "✓".green(),
                target_path.cyan()
            );

            let remaining = conflict_paths.len() - 1;
            if remaining > 0 {
                println!(
                    " {} conflicts(s) remaining. Run 'vault resolve' again.",
                    remaining
                );
            } else {
                println!(" All conflicts resolved. Run 'vault save' to record the resolution.");
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
enum Resolution {
    Ours,
    Theirs,
    Both,
    Unresolved,
}

fn run_resolve_tui(
    term: &Term,
    path: &str,
    ours: &[&str],
    ancestor: &[&str],
    theirs: &[&str],
) -> Result<Option<Vec<String>>> {
    let mut choice = Resolution::Unresolved;
    let mut scroll: usize = 0;

    loop {
        draw_resolve(term, path, ours, ancestor, theirs, &choice, scroll)?;

        match Term::read_key()? {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),

            KeyCode::Enter => match choice {
                Resolution::Unresolved => {}
                Resolution::Ours => return Ok(Some(ours.iter().map(|s| s.to_string()).collect())),
                Resolution::Theirs => {
                    return Ok(Some(theirs.iter().map(|s| s.to_string()).collect()));
                }
                Resolution::Both => {
                    let mut merged: Vec<String> = ours.iter().map(|s| s.to_string()).collect();
                    merged.extend(theirs.iter().map(|s| s.to_string()));
                    return Ok(Some(merged));
                }
            },

            KeyCode::Char(' ') | KeyCode::Char('o') => {
                choice = Resolution::Ours;
            }

            KeyCode::Char('t') => {
                choice = Resolution::Theirs;
            }

            KeyCode::Char('b') => {
                choice = Resolution::Both;
            }

            KeyCode::Down | KeyCode::Char('j') => {
                let max_lines = ours.len().max(theirs.len()).max(ancestor.len());
                if scroll + 1 < max_lines {
                    scroll += 1;
                }
            }

            KeyCode::Up | KeyCode::Char('k') => {
                scroll = scroll.saturating_sub(1);
            }
            _ => {}
        }
    }
}

fn draw_resolve(
    term: &Term,
    path: &str,
    ours: &[&str],
    ancestor: &[&str],
    theirs: &[&str],
    choice: &Resolution,
    scroll: usize,
) -> Result<()> {
    term.clear()?;
    let w = term.cols as usize;
    let col = w / 3;

    term.draw_full_row(
        0,
        &format!(" vault resolve '─' {}", path),
        p::MAUVE,
        p::CRUST,
        true,
    )?;

    let h1 = Term::fit("  OURS", col);
    let h2 = Term::fit("  ANCESTOR", col);
    let h3 = Term::fit("  THEIRS", col);

    term.draw_row(
        0,
        2,
        &format!("{} | {} | {}", h1, h2, h3),
        p::BLUE,
        p::SURFACE0,
        true,
    )?;
    term.draw_hline(3, '─', p::SURFACE1, p::BASE)?;

    let visible_rows = (term.rows - 6) as usize;

    for row_idx in 0..visible_rows {
        let src_idx = scroll + row_idx;
        let screen_row = (row_idx + 4) as u16;

        let our_line = ours.get(src_idx).copied().unwrap_or("");
        let anc_line = ancestor.get(src_idx).copied().unwrap_or("");
        let their_line = theirs.get(src_idx).copied().unwrap_or("");

        let our_fg = if our_line != anc_line {
            p::GREEN
        } else {
            p::TEXT
        };
        let their_fg = if their_line != anc_line {
            p::RED
        } else {
            p::TEXT
        };

        let c1 = Term::fit(&format!("     {}", our_line), col);
        let c2 = Term::fit(&format!("     {}", anc_line), col);
        let c3 = Term::fit(&format!("     {}", their_line), col);

        // Draw each column separately with its own colour
        term.draw_row(0, screen_row, &c1, our_fg, p::BASE, false)?;
        term.draw_row(col as u16, screen_row, "│", p::SURFACE1, p::BASE, false)?;
        term.draw_row(col as u16 + 1, screen_row, &c2, p::SUBTEXT0, p::BASE, false)?;
        term.draw_row(col as u16 * 2, screen_row, "│", p::SURFACE1, p::BASE, false)?;
        term.draw_row(
            col as u16 * 2 + 1,
            screen_row,
            &c3,
            their_fg,
            p::BASE,
            false,
        )?;
    }

    // Hint bar
    let status_str = match choice {
        Resolution::Unresolved => "No choice yet".to_string(),
        Resolution::Ours => "→ Accept OURS".to_string(),
        Resolution::Theirs => "→ Accept THEIRS".to_string(),
        Resolution::Both => "→ Accept BOTH (concatenated)".to_string(),
    };
    term.draw_full_row(
        term.rows - 2,
        &format!(" Current: {}", status_str),
        p::YELLOW,
        p::SURFACE0,
        false,
    )?;
    term.draw_full_row(
        term.rows - 1,
        " Space/o: accept ours   t: accept theirs   b: accept both   Enter: confirm   q: cancel",
        p::SUBTEXT0,
        p::CRUST,
        false,
    )?;

    use std::io::Write;
    std::io::stdout().flush()?;
    Ok(())
}

fn update_tree_resolve(repo: &Repo, head_hash: &str, path: &str, blob_hash: &str) -> Result<()> {
    let head_commit = repo.store.read_commit(head_hash)?;
    let mut flat: HashMap<String, (EntryKind, String)> = HashMap::new();
    flatten_tree(&repo.store, &head_commit.tree, "", &mut flat)?;

    // Replace conflict entry with blob
    flat.insert(path.to_string(), (EntryKind::Blob, blob_hash.to_string()));

    // Rebuild tree and overwrite HEAD commit's tree pointer
    let new_tree_hash = build_tree_from_flat_pub(&repo.store, &flat)?;
    let new_commit = Commit {
        tree: new_tree_hash,
        parents: head_commit.parents.clone(),
        author: head_commit.author.clone(),
        timestamp: head_commit.timestamp,
        message: head_commit.message.clone(),
        change_id: head_commit.change_id.clone(),
    };
    let new_hash = repo.store.write_commit(&new_commit)?;
    let branch = repo.store.current_branch()?.unwrap_or("main".to_string());
    repo.store.write_branch(&branch, &new_hash)?;

    Ok(())
}
