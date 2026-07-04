use crate::error::{Result, VaultError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub name: String,
    pub tree_hash: String,
    pub created_at: DateTime<Utc>,
    pub note: Option<String>,
}

pub struct SnapshotStore {
    index_path: PathBuf,
}

impl SnapshotStore {
    pub fn new(vauld_dir: &Path) -> Self {
        let dir = vauld_dir.join("snapshots");
        let _ = fs::create_dir_all(&dir);
        SnapshotStore {
            index_path: dir.join("index.json"),
        }
    }

    pub fn list(&self) -> Result<Vec<SnapshotEntry>> {
        if !self.index_path.exists() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(&self.index_path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn save(&self, entry: SnapshotEntry) -> Result<()> {
        let mut entries = self.list()?;
        if entries.iter().any(|e| e.name == entry.name) {
            return Err(VaultError::ObjectNotFound(format!(
                "snapshot '{}' already exists",
                entry.name
            )));
        }
        entries.push(entry);
        fs::write(&self.index_path, serde_json::to_vec_pretty(&entries)?)?;
        Ok(())
    }

    pub fn get(&self, name: &str) -> Result<SnapshotEntry> {
        self.list()?
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| VaultError::ObjectNotFound(format!("snapshot '{}' ", name)))
    }

    pub fn drop(&self, name: &str) -> Result<()> {
        let mut entries = self.list()?;
        let before = entries.len();
        entries.retain(|e| e.name != name);
        if entries.len() == before {
            return Err(VaultError::ObjectNotFound(format!("snapshot '{}' ", name)));
        }

        fs::write(&self.index_path, serde_json::to_vec_pretty(&entries)?)?;
        Ok(())
    }
}
