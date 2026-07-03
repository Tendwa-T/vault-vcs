use crate::error::Result;
use crate::objects::Commit;
use crate::store::ObjectStore;
use std::collections::{HashSet, VecDeque};

// handling walking the commit graph
// moving towards root

pub fn ancestors(store: &ObjectStore, start: &str) -> Result<Vec<String>> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut result = Vec::new();

    queue.push_back(start.to_string());

    while let Some(hash) = queue.pop_front() {
        if visited.contains(&hash) {
            continue;
        }
        visited.insert(hash.clone());
        result.push(hash.clone());
        let commit = store.read_commit(&hash)?;
        for parent in commit.parents {
            queue.push_back(parent);
        }
    }
    Ok(result)
}

//find the lowest common ancestor of 2 commits. Return None if the graphs are disjoint
pub fn merge_base(store: &ObjectStore, a: &str, b: &str) -> Result<Option<String>> {
    let ancestors_a: HashSet<String> = ancestors(store, a)?.into_iter().collect();
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    queue.push_back(b.to_string());

    while let Some(hash) = queue.pop_front() {
        if visited.contains(&hash) {
            continue;
        }
        visited.insert(hash.clone());

        if ancestors_a.contains(&hash) {
            return Ok(Some(hash));
        }

        let commit = store.read_commit(&hash)?;
        for parent in commit.parents {
            queue.push_back(parent);
        }
    }
    Ok(None)
}

// return the commit hash for HEAD or none for a fresh REPO
pub fn head_commit(store: &ObjectStore) -> Result<Option<String>> {
    store.resolve_head()
}

// Walk from start in reverse timestamp order starting with parent
pub fn log_walk(store: &ObjectStore, start: &str) -> Result<Vec<(String, Commit)>> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = vec![start.to_string()];

    while let Some(hash) = stack.pop() {
        if visited.contains(&hash) {
            continue;
        }
        visited.insert(hash.clone());
        let commit = store.read_commit(&hash)?;
        for parent in commit.parents.iter().rev() {
            stack.push(parent.clone());
        }
        result.push((hash, commit));
    }
    Ok(result)
}
