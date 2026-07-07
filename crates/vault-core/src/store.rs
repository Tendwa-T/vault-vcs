use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::error::{Result, VaultError};
use crate::objects::{Commit, ConflictObject, Tree};

// Store module placeholder
pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn new(root: &Path) -> Self {
        ObjectStore {
            root: root.to_path_buf(),
        }
    }

    fn blob_path(&self, hash: &str) -> PathBuf {
        self.root.join("objects").join("blobs").join(hash)
    }

    fn tree_path(&self, hash: &str) -> PathBuf {
        self.root.join("objects").join("trees").join(hash)
    }

    fn commit_path(&self, hash: &str) -> PathBuf {
        self.root.join("objects").join("commits").join(hash)
    }

    fn conflict_path(&self, hash: &str) -> PathBuf {
        self.root.join("objects").join("conflicts").join(hash)
    }

    fn branch_path(&self, name: &str) -> PathBuf {
        self.root.join("refs").join("heads").join(name)
    }

    // Blobs
    pub fn write_blob(&self, content: &[u8]) -> Result<String> {
        let hash = blake3::hash(content).to_hex().to_string();
        let path = self.blob_path(&hash);
        if !path.exists() {
            fs::write(&path, content)?;
        }
        Ok(hash)
    }

    pub fn read_blob(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.blob_path(hash);
        fs::read(&path).map_err(|_| VaultError::ObjectNotFound(hash.to_string()))
    }

    // trees
    pub fn write_tree(&self, tree: &Tree) -> Result<String> {
        let bytes = serde_json::to_vec(tree)?;
        let hash = blake3::hash(&bytes).to_hex().to_string();
        let path = self.tree_path(&hash);
        if !path.exists() {
            fs::write(&path, &bytes)?;
        }
        Ok(hash)
    }

    pub fn read_tree(&self, hash: &str) -> Result<Tree> {
        let path = self.tree_path(hash);
        let bytes = fs::read(&path).map_err(|_| VaultError::ObjectNotFound(hash.to_string()))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    //commits
    pub fn write_commit(&self, commit: &Commit) -> Result<String> {
        let bytes = serde_json::to_vec(commit)?;
        let hash = blake3::hash(&bytes).to_hex().to_string();
        let path = self.commit_path(&hash);
        if !path.exists() {
            fs::write(&path, &bytes)?;
        }
        Ok(hash)
    }

    pub fn read_commit(&self, hash: &str) -> Result<Commit> {
        let path = self.commit_path(hash);
        let bytes = fs::read(&path).map_err(|_| VaultError::ObjectNotFound(hash.to_string()))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    // conflicts
    pub fn write_conflict(&self, conflict: &ConflictObject) -> Result<String> {
        let bytes = serde_json::to_vec(conflict)?;
        let hash = blake3::hash(&bytes).to_hex().to_string();
        let path = self.conflict_path(&hash);
        if !path.exists() {
            fs::write(&path, &bytes)?;
        }
        Ok(hash)
    }

    pub fn read_conflicts(&self, hash: &str) -> Result<ConflictObject> {
        let path = self.conflict_path(hash);
        let bytes = fs::read(&path).map_err(|_| VaultError::ObjectNotFound(hash.to_string()))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn read_object_raw(&self, kind_dir: &str, hash: &str) -> Result<Vec<u8>> {
        let path = self.root.join("objects").join(kind_dir).join(hash);
        fs::read(&path).map_err(|_| VaultError::ObjectNotFound(hash.to_string()))
    }

    // Branches
    pub fn write_branch(&self, name: &str, commit_hash: &str) -> Result<()> {
        let path = self.branch_path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, commit_hash)?;
        Ok(())
    }

    pub fn read_branch(&self, name: &str) -> Result<String> {
        let path = self.branch_path(name);
        fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .map_err(|_| VaultError::BranchNotFound(name.to_string()))
    }

    pub fn branch_exists(&self, name: &str) -> bool {
        self.branch_path(name).exists()
    }

    pub fn list_branches(&self) -> Result<Vec<String>> {
        let dir = self.root.join("refs").join("heads");
        let mut branches = Vec::new();
        if dir.exists() {
            self.collect_branches_rec(&dir, &dir, &mut branches)?;
        }
        Ok(branches)
    }

    fn collect_branches_rec(&self, base: &Path, current: &Path, branches: &mut Vec<String>) -> Result<()> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.collect_branches_rec(base, &path, branches)?;
            } else {
                if let Ok(rel_path) = path.strip_prefix(base) {
                    branches.push(rel_path.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        Ok(())
    }

    // Head
    pub fn write_head(&self, content: &str) -> Result<()> {
        fs::write(self.root.join("HEAD"), content)?;
        Ok(())
    }

    pub fn read_head(&self) -> Result<String> {
        Ok(fs::read_to_string(self.root.join("HEAD"))?
            .trim()
            .to_string())
    }

    // - a way to figure out which commit hash HEAD currently points to
    pub fn resolve_head(&self) -> Result<Option<String>> {
        let head = self.read_head()?;
        if head.starts_with("ref: ") {
            let branch = head.trim_start_matches("ref: refs/heads/");
            match self.read_branch(branch) {
                Ok(hash) => Ok(Some(hash)),
                Err(VaultError::BranchNotFound(_)) => Ok(None),
                Err(e) => Err(e),
            }
        } else if head.is_empty() {
            Ok(None)
        } else {
            Ok(Some(head))
        }
    }

    pub fn current_branch(&self) -> Result<Option<String>> {
        let head = self.read_head()?;
        if head.starts_with("ref: refs/heads/") {
            Ok(Some(
                head.trim_start_matches("ref: refs/heads/").to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    // Init
    pub fn init(vault_dir: &Path) -> Result<()> {
        for subdir in &[
            "objects/blobs",
            "objects/trees",
            "objects/commits",
            "objects/conflicts",
            "refs/heads",
            "oplog",
        ] {
            fs::create_dir_all(vault_dir.join(subdir))?;
        }
        Ok(())
    }
}
