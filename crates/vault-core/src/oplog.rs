use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpEntry {
    pub op: String,
    pub timestamp: DateTime<Utc>,
    pub head_before: Option<String>,
    pub head_after: Option<String>,
    pub branch: Option<String>,
    pub message: Option<String>,
    pub extra: Option<String>,
    pub undone: bool,
}

pub struct OpLog {
    path: PathBuf,
}

impl OpLog {
    pub fn new(vault_dir: &Path) -> Self {
        OpLog {
            path: vault_dir.join("oplog").join("ops.jsonl"),
        }
    }

    pub fn append(&self, entry: &OpEntry) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(entry)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    pub fn read_all(&self) -> Result<Vec<OpEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: OpEntry = serde_json::from_str(&line)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    pub fn undo_last(&self) -> Result<Option<Option<String>>> {
        let mut entries = self.read_all()?;
        let idx = entries
            .iter()
            .rposition(|e| !e.undone && e.head_before != e.head_after);
        if let Some(i) = idx {
            let head_before = entries[i].head_before.clone();
            entries[i].undone = true;
            let mut file = File::create(&self.path)?;
            for entry in &entries {
                let line = serde_json::to_string(entry)?;
                writeln!(file, "{}", line)?;
            }
            Ok(Some(head_before))
        } else {
            Ok(None)
        }
    }
}
