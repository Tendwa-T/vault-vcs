use std::collections::HashMap;

use crate::{
    error::Result,
    objects::{EntryKind, FlatTree},
    store::ObjectStore,
};

// Flatten the tree recursively to be like path -> (kind, hash)
pub fn flatten_tree(
    store: &ObjectStore,
    tree_hash: &str,
    prefix: &str,
    out: &mut FlatTree,
) -> Result<()> {
    let tree = store.read_tree(tree_hash)?;
    for entry in &tree.entries {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", prefix, entry.name)
        };
        match entry.kind {
            EntryKind::Tree => {
                flatten_tree(store, &entry.hash, &path, out)?;
            }
            _ => {
                out.insert(path, (entry.kind.clone(), entry.hash.clone()));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub enum DiffKind {
    Added,
    Deleted,
    Modified,
}

#[derive(Debug, Clone)]
pub enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: String,
    pub kind: DiffKind,
    pub hunks: Vec<DiffHunk>,
}

// Comp 2 flat trees and return per-file diffs
pub fn diff_trees(
    store: &ObjectStore,
    old_tree: Option<&str>,
    new_tree: Option<&str>,
) -> Result<Vec<FileDiff>> {
    let mut old_flat: FlatTree = HashMap::new();
    let mut new_flat: FlatTree = HashMap::new();

    if let Some(h) = old_tree {
        flatten_tree(store, h, "", &mut old_flat)?;
    }
    if let Some(h) = new_tree {
        flatten_tree(store, h, "", &mut new_flat)?;
    }

    let mut diffs = Vec::new();

    //Loop and mark files
    //Deleted
    for (path, (_, old_hash)) in &old_flat {
        if !new_flat.contains_key(path.as_str()) {
            let content = store.read_blob(old_hash)?;
            let text = String::from_utf8_lossy(&content).to_string();
            diffs.push(FileDiff {
                path: path.clone(),
                kind: DiffKind::Deleted,
                hunks: make_deletion_hunks(&text),
            });
        }
    }

    //Added lines
    for (path, (kind, new_hash)) in &new_flat {
        if matches!(kind, EntryKind::Conflict) {
            continue; // conflicts will be shown separately 
        }
        match old_flat.get(path.as_str()) {
            None => {
                let content = store.read_blob(new_hash)?;
                let text = String::from_utf8_lossy(&content).to_string();
                diffs.push(FileDiff {
                    path: path.clone(),
                    kind: DiffKind::Added,
                    hunks: make_addition_hunks(&text),
                });
            }
            Some((_, old_hash)) if old_hash != new_hash => {
                let old_bytes = store.read_blob(old_hash)?;
                let new_bytes = store.read_blob(new_hash)?;
                let old_text = String::from_utf8_lossy(&old_bytes).to_string();
                let new_text = String::from_utf8_lossy(&new_bytes).to_string();

                let hunks = line_diff(&old_text, &new_text);
                if !hunks.is_empty() {
                    diffs.push(FileDiff {
                        path: path.clone(),
                        kind: DiffKind::Modified,
                        hunks,
                    });
                }
            }
            _ => {}
        }
    }
    diffs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(diffs)
}

// create simple hunks
pub fn line_diff(old: &str, new: &str) -> Vec<DiffHunk> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    // Build LCS table
    let m = old_lines.len();
    let n = new_lines.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in (0..m).rev() {
        for j in (0..n).rev() {
            dp[i][j] = if old_lines[i] == new_lines[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    // Trace back to get edit sequence
    let mut edits: Vec<DiffLine> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < m || j < n {
        if i < m && j < n && old_lines[i] == new_lines[j] {
            edits.push(DiffLine::Context(old_lines[i].to_string()));
            i += 1;
            j += 1;
        } else if j < n && (i >= m || dp[i][j + 1] >= dp[i + 1][j]) {
            edits.push(DiffLine::Added(new_lines[j].to_string()));
            j += 1;
        } else {
            edits.push(DiffLine::Removed(old_lines[i].to_string()));
            i += 1;
        }
    }

    // Group into hunks with 3 lines of context
    const CTX: usize = 3;
    let change_positions: Vec<usize> = edits
        .iter()
        .enumerate()
        .filter(|(_, l)| !matches!(l, DiffLine::Context(_)))
        .map(|(i, _)| i)
        .collect();

    if change_positions.is_empty() {
        return Vec::new();
    }

    // Merge overlapping windows
    let mut windows: Vec<(usize, usize)> = Vec::new();
    for &pos in &change_positions {
        let start = pos.saturating_sub(CTX);
        let end = (pos + CTX + 1).min(edits.len());
        if let Some(last) = windows.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        windows.push((start, end));
    }

    let mut hunks = Vec::new();
    let mut old_line = 1usize;
    let mut new_line = 1usize;

    // Track line numbers properly
    let mut old_cursor = 0usize;

    for (start, end) in windows {
        // Advance cursors to `start`
        for edit in &edits[old_cursor.min(start)..start] {
            match edit {
                DiffLine::Context(_) | DiffLine::Removed(_) => old_line += 1,
                _ => {}
            }
            match edit {
                DiffLine::Context(_) | DiffLine::Added(_) => new_line += 1,
                _ => {}
            }
        }

        let hunk_lines: Vec<DiffLine> = edits[start..end].to_vec();
        hunks.push(DiffHunk {
            old_start: old_line,
            new_start: new_line,
            lines: hunk_lines.clone(),
        });

        for line in &hunk_lines {
            match line {
                DiffLine::Context(_) | DiffLine::Removed(_) => old_line += 1,
                _ => {}
            }
            match line {
                DiffLine::Context(_) | DiffLine::Added(_) => new_line += 1,
                _ => {}
            }
        }
        old_cursor = end;
    }

    hunks
}

fn make_addition_hunks(text: &str) -> Vec<DiffHunk> {
    let lines: Vec<DiffLine> = text
        .lines()
        .map(|l| DiffLine::Added(l.to_string()))
        .collect();
    if lines.is_empty() {
        return Vec::new();
    }
    vec![DiffHunk {
        old_start: 0,
        new_start: 1,
        lines,
    }]
}

fn make_deletion_hunks(text: &str) -> Vec<DiffHunk> {
    let lines: Vec<DiffLine> = text
        .lines()
        .map(|l| DiffLine::Removed(l.to_string()))
        .collect();
    if lines.is_empty() {
        return Vec::new();
    }
    vec![DiffHunk {
        old_start: 1,
        new_start: 0,
        lines,
    }]
}
