// keeping track of the data types

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub struct Blob {
    pub content: Vec<u8>,
}

impl Blob {
    pub fn hash(content: &[u8]) -> String {
        blake3::hash(content).to_hex().to_string()
    }
}

// Trees

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Blob,
    Tree,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub name: String,
    pub kind: EntryKind,
    pub hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    pub fn new() -> Self {
        Tree {
            entries: Vec::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&TreeEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn hash_of(entries: &[TreeEntry]) -> Result<String, serde_json::Error> {
        let t = Tree {
            entries: entries.to_vec(),
        };
        let bytes = serde_json::to_vec(&t)?;
        Ok(blake3::hash(&bytes).to_hex().to_string())
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

// Conflict Object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictObject {
    pub ours: String,
    pub theirs: String,
    pub ancestor: String,
}

// Commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub tree: String,
    pub parents: Vec<String>,
    pub author: Author,
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub change_id: String,
}

impl Commit {
    pub fn hash_of(commit: &Commit) -> Result<String, serde_json::Error> {
        let bytes = serde_json::to_vec(commit)?;
        Ok(blake3::hash(&bytes).to_hex().to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Untracked,
}

#[derive(Debug, Clone)]
pub struct StatusEntry {
    pub path: String,
    pub status: FileStatus,
}

// Flat tree
pub type FlatTree = HashMap<String, (EntryKind, String)>;
