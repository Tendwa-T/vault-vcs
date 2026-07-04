use crate::error::{Result, VaultError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashEntry {
    pub name: String,
    pub tree_hash: String,
    pub created_at: DateTime<Utc>,
    pub branch: String,
}

pub struct StashStore {
    index_path: PathBuf,
}

impl StashStore {
    pub fn new(vault_dir: &Path) -> Self {
        let dir = vault_dir.join("stash");
        let _ = fs::create_dir_all(&dir);
        StashStore {
            index_path: dir.join("index.json"),
        }
    }

    pub fn list(&self) -> Result<Vec<StashEntry>> {
        if !self.index_path.exists() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(&self.index_path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn save(&self, entry: StashEntry) -> Result<()> {
        let mut entries = self.list()?;
        if entries.iter().any(|e| e.name == entry.name) {
            return Err(VaultError::ObjectNotFound(format!(
                "stash '{}' already exists -- drop it first",
                entry.name
            )));
        }
        entries.push(entry);
        fs::write(&self.index_path, serde_json::to_vec_pretty(&entries)?)?;
        Ok(())
    }

    pub fn get(&self, name: &str) -> Result<StashEntry> {
        self.list()?
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| VaultError::ObjectNotFound(format!("stash '{}' ", name)))
    }

    pub fn drop(&self, name: &str) -> Result<()> {
        let mut entries = self.list()?;
        let before = entries.len();
        entries.retain(|e| e.name != name);

        if entries.len() == before {
            return Err(VaultError::ObjectNotFound(format!("stash '{}' ", name)));
        }

        fs::write(&self.index_path, serde_json::to_vec_pretty(&entries)?)?;
        Ok(())
    }
}
