use crate::diff::flatten_tree;
use crate::error::Result;
use crate::objects::{ConflictObject, EntryKind, Tree, TreeEntry};
use crate::store::ObjectStore;
use std::collections::{HashMap, HashSet};

pub struct MergeResult {
    pub tree_hash: String,
    pub had_conflicts: bool,
    pub conflict_paths: Vec<String>,
}

pub fn three_way_merge(
    store: &ObjectStore,
    ours_tree: &str,
    theirs_tree: &str,
    ancestor_tree: Option<&str>,
) -> Result<MergeResult> {
    let mut ours_flat = HashMap::new();
    let mut theirs_flat = HashMap::new();
    let mut ancestor_flat = HashMap::new();

    flatten_tree(store, ours_tree, "", &mut ours_flat)?;
    flatten_tree(store, theirs_tree, "", &mut theirs_flat)?;

    if let Some(anc) = ancestor_tree {
        flatten_tree(store, anc, "", &mut ancestor_flat)?;
    }

    let all_paths: HashSet<String> = ours_flat
        .keys()
        .chain(theirs_flat.keys())
        .chain(ancestor_flat.keys())
        .cloned()
        .collect();

    let mut result_flat: HashMap<String, (EntryKind, String)> = HashMap::new();
    let mut conflict_paths = Vec::new();

    for path in &all_paths {
        let ours_entry = ours_flat.get(path.as_str());
        let theirs_entry = theirs_flat.get(path.as_str());
        let ancestor_entry = ancestor_flat.get(path.as_str());

        let ours_hash = ours_entry.map(|(_, h)| h.as_str());
        let theirs_hash = theirs_entry.map(|(_, h)| h.as_str());
        let ancestor_hash = ancestor_entry.map(|(_, h)| h.as_str());

        let resolved = resolve_file(store, path, ours_hash, theirs_hash, ancestor_hash)?;

        match resolved {
            FileResolution::Unchanged => {}
            FileResolution::TakeOurs(hash) => {
                result_flat.insert(path.clone(), (EntryKind::Blob, hash));
            }
            FileResolution::TakeTheirs(hash) => {
                result_flat.insert(path.clone(), (EntryKind::Blob, hash));
            }
            FileResolution::AutoMerged(hash) => {
                result_flat.insert(path.clone(), (EntryKind::Blob, hash));
            }
            FileResolution::Conflict {
                ours,
                theirs,
                ancestor,
            } => {
                let conflict = ConflictObject {
                    ours,
                    theirs,
                    ancestor,
                };
                let hash = store.write_conflict(&conflict)?;
                result_flat.insert(path.clone(), (EntryKind::Conflict, hash));
                conflict_paths.push(path.clone());
            }
        }
    }
    let tree_hash = build_tree_from_flat(store, &result_flat)?;

    Ok(MergeResult {
        tree_hash,
        had_conflicts: !conflict_paths.is_empty(),
        conflict_paths,
    })
}

enum FileResolution {
    Unchanged,
    TakeOurs(String),
    TakeTheirs(String),
    AutoMerged(String),
    Conflict {
        ours: String,
        theirs: String,
        ancestor: String,
    },
}

fn resolve_file(
    store: &ObjectStore,
    _path: &str,
    ours: Option<&str>,
    theirs: Option<&str>,
    ancestor: Option<&str>,
) -> Result<FileResolution> {
    match (ours, theirs, ancestor) {
        // deleted both
        (None, None, _) => Ok(FileResolution::Unchanged),

        // only ours has the file
        (Some(o), None, None) => Ok(FileResolution::TakeOurs(o.to_string())),
        (Some(_), None, Some(_)) => Ok(FileResolution::Unchanged), // they deleted but ancestor has
        // it

        //only theirs has the file
        (None, Some(t), None) => Ok(FileResolution::TakeTheirs(t.to_string())),
        (None, Some(t), Some(_)) => Ok(FileResolution::TakeTheirs(t.to_string())),

        // both have file
        (Some(o), Some(t), anc) => {
            if o == t {
                return Ok(FileResolution::TakeOurs(o.to_string()));
            }
            let anc_hash = anc.unwrap_or("");
            if o == anc_hash {
                return Ok(FileResolution::TakeTheirs(t.to_string()));
            }
            if t == anc_hash {
                return Ok(FileResolution::TakeOurs(o.to_string()));
            }

            let ours_bytes = store.read_blob(o)?;
            let theirs_bytes = store.read_blob(t)?;
            let ours_text = String::from_utf8_lossy(&ours_bytes).to_string();
            let theirs_text = String::from_utf8_lossy(&theirs_bytes).to_string();

            if let Some(merged) = try_auto_merge(&ours_text, &theirs_text) {
                let hash = store.write_blob(merged.as_bytes())?;
                return Ok(FileResolution::AutoMerged(hash));
            }

            Ok(FileResolution::Conflict {
                ours: o.to_string(),
                theirs: t.to_string(),
                ancestor: anc.unwrap_or("").to_string(),
            })
        }
    }
}

fn try_auto_merge(_ours: &str, _theirs: &str) -> Option<String> {
    None
}

fn build_tree_from_flat(
    store: &ObjectStore,
    flat: &HashMap<String, (EntryKind, String)>,
) -> Result<String> {
    let mut dirs: HashMap<String, HashMap<String, (EntryKind, String)>> = HashMap::new();
    let mut root_entries: Vec<TreeEntry> = Vec::new();
    for (path, (kind, hash)) in flat {
        if let Some(slash_pos) = path.find('/') {
            let dir = &path[..slash_pos];
            let rest = &path[slash_pos + 1..];
            dirs.entry(dir.to_string())
                .or_default()
                .insert(rest.to_string(), (kind.clone(), hash.clone()));
        } else {
            root_entries.push(TreeEntry {
                name: path.clone(),
                kind: kind.clone(),
                hash: hash.clone(),
            });
        }
    }

    for (dir_name, subtree_flat) in dirs {
        let subtree_hash = build_tree_from_flat(store, &subtree_flat)?;
        root_entries.push(TreeEntry {
            name: dir_name,
            kind: EntryKind::Tree,
            hash: subtree_hash,
        });
    }

    root_entries.sort_by(|a, b| a.name.cmp(&b.name));
    let tree = Tree {
        entries: root_entries,
    };
    store.write_tree(&tree)
}
