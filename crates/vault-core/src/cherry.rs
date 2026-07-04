use crate::diff::{DiffKind, diff_trees, flatten_tree};
use crate::error::{Result, VaultError};
use crate::objects::{ConflictObject, EntryKind};
use crate::store::ObjectStore;
use std::collections::HashMap;

pub struct CherryResult {
    pub new_tree_hash: String,
    pub conflicts: Vec<String>,
}

pub fn compute_cherry(
    store: &ObjectStore,
    commit_hash: &str,
    onto_tree: &str,
) -> Result<CherryResult> {
    let commit = store.read_commit(commit_hash)?;
    let parent_hash = commit.parents.first().ok_or_else(|| {
        VaultError::ObjectNotFound(
            "cherry-pick: cannot pick an initial commit (No parent diff)".to_string(),
        )
    })?;

    let parent_commit = store.read_commit(parent_hash)?;
    let diffs = diff_trees(store, Some(&parent_commit.tree), Some(&commit.tree))?;

    let mut onto_flat: HashMap<String, (EntryKind, String)> = HashMap::new();
    flatten_tree(store, onto_tree, "", &mut onto_flat)?;

    let mut conflicts = Vec::new();

    for file_diff in &diffs {
        let path = &file_diff.path;
        match file_diff.kind {
            DiffKind::Added => {
                if let Some((_, existing_hash)) = onto_flat.get(path.as_str()) {
                    let mut onto_flat_parent: HashMap<String, (EntryKind, String)> = HashMap::new();
                    flatten_tree(store, &parent_commit.tree, "", &mut onto_flat_parent)?;

                    if !onto_flat_parent.contains_key(path.as_str()) {
                        let mut commit_flat = HashMap::new();
                        flatten_tree(store, &commit.tree, "", &mut commit_flat)?;
                        let ch = commit_flat
                            .get(path.as_str())
                            .map(|(_, h)| h.clone())
                            .unwrap_or_default();

                        let conflict = ConflictObject {
                            ours: existing_hash.clone(),
                            theirs: ch,
                            ancestor: String::new(),
                        };
                        let conflict_hash = store.write_conflict(&conflict)?;
                        onto_flat.insert(path.clone(), (EntryKind::Conflict, conflict_hash));
                        conflicts.push(path.clone());
                    } else {
                        let mut commit_flat: HashMap<String, (EntryKind, String)> = HashMap::new();
                        flatten_tree(store, &commit.tree, "", &mut commit_flat)?;
                        if let Some(entry) = commit_flat.get(path.as_str()) {
                            onto_flat.insert(path.clone(), entry.clone());
                        }
                    }
                } else {
                    let mut commit_flat: HashMap<String, (EntryKind, String)> = HashMap::new();
                    flatten_tree(store, &commit.tree, "", &mut commit_flat)?;
                    if let Some(entry) = commit_flat.get(path.as_str()) {
                        onto_flat.insert(path.clone(), entry.clone());
                    }
                }
            }
            DiffKind::Deleted => {
                let mut parent_flat: HashMap<String, (EntryKind, String)> = HashMap::new();
                flatten_tree(store, &parent_commit.tree, "", &mut parent_flat)?;
                let parent_hash_opt = parent_flat.get(path.as_str()).map(|(_, h)| h.clone());

                let onto_hash_opt = onto_flat.get(path.as_str()).map(|(_, h)| h.clone());

                match (parent_hash_opt, onto_hash_opt) {
                    (Some(ph), Some(oh)) if ph == oh => {
                        onto_flat.remove(path.as_str());
                    }
                    (ph_opt, Some(oh)) => {
                        let conflict = ConflictObject {
                            ours: oh,
                            theirs: String::new(),
                            ancestor: ph_opt.unwrap_or_default(),
                        };
                        let conflict_hash = store.write_conflict(&conflict)?;
                        onto_flat.insert(path.clone(), (EntryKind::Conflict, conflict_hash));
                        conflicts.push(path.clone());
                    }
                    _ => {}
                }
            }
            DiffKind::Modified => {
                let mut parent_flat: HashMap<String, (EntryKind, String)> = HashMap::new();
                flatten_tree(store, &parent_commit.tree, "", &mut parent_flat)?;

                let mut commit_flat: HashMap<String, (EntryKind, String)> = HashMap::new();
                flatten_tree(store, &commit.tree, "", &mut commit_flat)?;

                let parent_hash_opt = parent_flat.get(path.as_str()).map(|(_, h)| h.clone());
                let onto_hash_opt = onto_flat.get(path.as_str()).map(|(_, h)| h.clone());
                let commit_hash_opt = commit_flat.get(path.as_str()).map(|(_, h)| h.clone());

                match (parent_hash_opt, onto_hash_opt, commit_hash_opt) {
                    (Some(ph), Some(oh), Some(ch)) => {
                        if oh == ph {
                            onto_flat.insert(path.clone(), (EntryKind::Blob, ch));
                        } else if oh == ch {
                            // already there
                        } else {
                            let conflict = ConflictObject {
                                ours: oh,
                                theirs: ch,
                                ancestor: ph,
                            };
                            let conflict_hash = store.write_conflict(&conflict)?;
                            onto_flat.insert(path.clone(), (EntryKind::Conflict, conflict_hash));
                            conflicts.push(path.clone());
                        }
                    }
                    (ph_opt, oh_opt, ch_opt) => {
                        let conflict = ConflictObject {
                            ours: oh_opt.unwrap_or_default(),
                            theirs: ch_opt.unwrap_or_default(),
                            ancestor: ph_opt.unwrap_or_default(),
                        };
                        let conflict_hash = store.write_conflict(&conflict)?;
                        onto_flat.insert(path.clone(), (EntryKind::Conflict, conflict_hash));
                        conflicts.push(path.clone());
                    }
                }
            }
        }
    }
    let new_tree_hash = crate::merge::build_tree_from_flat_pub(store, &onto_flat)?;
    Ok(CherryResult {
        new_tree_hash,
        conflicts,
    })
}

