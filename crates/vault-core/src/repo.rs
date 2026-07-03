use crate::diff::{DiffKind, FileDiff, diff_trees};
use crate::error::Result;
use crate::merge::three_way_merge;
use crate::objects::{
    Author, Commit, EntryKind, FileStatus, StatusEntry, Tree, TreeEntry,
};
use crate::oplog::{OpEntry, OpLog};
use crate::store::ObjectStore;
use crate::{VaultError, dag};
use chrono::Utc;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct Repo {
    pub work_dir: PathBuf,
    pub vault_dir: PathBuf,
    pub store: ObjectStore,
    pub oplog: OpLog,
}

impl Repo {
    pub fn open(path: &Path) -> Result<Self> {
        let mut current = path.to_path_buf();
        loop {
            let vault = current.join(".vault");
            if vault.is_dir() {
                return Ok(Repo {
                    store: ObjectStore::new(&vault),
                    oplog: OpLog::new(&vault),
                    work_dir: current,
                    vault_dir: vault,
                });
            }
            if !current.pop() {
                return Err(VaultError::NotARepo);
            }
        }
    }

    pub fn init(path: &Path, author_name: &str, author_email: &str) -> Result<Self> {
        let vault_dir = path.join(".vault");
        if vault_dir.exists() {
            return Err(VaultError::AlreadyInit(path.display().to_string()));
        }
        ObjectStore::init(&vault_dir)?;
        let store = ObjectStore::new(&vault_dir);
        store.write_head("ref: refs/heads/main")?;

        let config = format!("[user]\nname = {}\nemail = {}\n", author_name, author_email);

        fs::write(vault_dir.join("config"), config)?;

        let ignore = ".vault/\n.git/\ntarget/\nnode_modules/\n*.class\n";
        let ignore_path = path.join(".vaultignore");
        if !ignore_path.exists() {
            fs::write(ignore_path, ignore)?;
        }

        let oplog = OpLog::new(&vault_dir);
        let repo = Repo {
            store,
            oplog,
            work_dir: path.to_path_buf(),
            vault_dir,
        };

        repo.oplog.append(&OpEntry {
            op: "init".to_string(),
            timestamp: Utc::now(),
            head_before: None,
            head_after: None,
            branch: Some("main".to_string()),
            message: None,
            extra: None,
            undone: false,
        })?;
        Ok(repo)
    }

    pub fn author(&self) -> Author {
        let config_path = self.vault_dir.join("config");
        let content = fs::read_to_string(&config_path).unwrap_or_default();
        let mut name = "Unknown".to_string();
        let mut email = "unknown@vault.com".to_string();
        for line in content.lines() {
            if let Some(v) = line.trim().strip_prefix("name = ") {
                name = v.trim().to_string();
            }
            if let Some(v) = line.trim().strip_prefix("email = ") {
                email = v.trim().to_string();
            }
        }
        Author { name, email }
    }

    fn ignored_patterns(&self) -> Vec<String> {
        let path = self.work_dir.join(".vaultignore");
        fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with("#"))
            .map(|l| l.trim().to_string())
            .collect()
    }

    fn is_ignored(&self, rel_path: &str, patterns: &[String]) -> bool {
        for pattern in patterns {
            if rel_path == pattern || rel_path.starts_with(pattern.as_str()) {
                return true;
            }
            if pattern.starts_with("*") {
                let suffix = &pattern[1..];
                if rel_path.ends_with(suffix) {
                    return true;
                }
            }
        }
        false
    }

    pub fn snapshot_working_dir(&self) -> Result<String> {
        let patterns = self.ignored_patterns();
        self.snapshot_dir(&self.work_dir.clone(), &patterns)
    }

    fn snapshot_dir(&self, dir: &Path, patterns: &[String]) -> Result<String> {
        let mut entries = Vec::new();
        let mut children: Vec<_> = fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
        children.sort_by_key(|e| e.file_name());

        for entry in children {
            let path = entry.path();
            let rel_path = path
                .strip_prefix(&self.work_dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");

            if self.is_ignored(&rel_path, patterns) {
                continue;
            }

            if path.is_dir() {
                let tree_hash = self.snapshot_dir(&path, patterns)?;
                entries.push(TreeEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    kind: EntryKind::Tree,
                    hash: tree_hash,
                });
            } else if path.is_file() {
                let content = fs::read(&path)?;
                let blob_hash = self.store.write_blob(&content)?;
                entries.push(TreeEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    kind: EntryKind::Blob,
                    hash: blob_hash,
                });
            }
        }
        let tree = Tree { entries };
        self.store.write_tree(&tree)
    }

    pub fn restore_tree(&self, tree_hash: &str) -> Result<()> {
        let pattern = self.ignored_patterns();
        self.clean_work_dir(&self.work_dir.clone(), &pattern)?;
        self.restore_dir(tree_hash, &self.work_dir.clone())
    }

    fn clean_work_dir(&self, dir: &Path, patterns: &[String]) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let rel_path = path
                .strip_prefix(&self.work_dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");

            if self.is_ignored(&rel_path, patterns) {
                continue;
            }

            if path.is_dir() {
                self.clean_work_dir(&path, patterns)?;
                let _ = fs::remove_dir(&path);
            } else {
                fs::remove_file(&path)?;
            }
        }
        Ok(())
    }

    fn restore_dir(&self, tree_hash: &str, target_dir: &Path) -> Result<()> {
        let tree = self.store.read_tree(tree_hash)?;
        fs::create_dir_all(target_dir)?;

        for entry in &tree.entries {
            let entry_path = target_dir.join(&entry.name);
            match entry.kind {
                EntryKind::Tree => {
                    self.restore_dir(&entry.hash, &entry_path)?;
                }
                EntryKind::Blob => {
                    let content = self.store.read_blob(&entry.hash)?;
                    fs::write(&entry_path, content)?;
                }
                EntryKind::Conflict => {
                    let conflict = self.store.read_conflicts(&entry.hash)?;
                    let ours = self.store.read_blob(&conflict.ours).unwrap_or_default();
                    let theirs = self.store.read_blob(&conflict.theirs).unwrap_or_default();
                    let marker = format!(
                        "<<<<<<<< ours\n{}\n================\n{}\n>>>>>>>> theirs\n",
                        String::from_utf8_lossy(&ours),
                        String::from_utf8_lossy(&theirs)
                    );
                    fs::write(&entry_path, marker)?;
                }
            }
        }
        Ok(())
    }

    pub fn save(&self, message: &str) -> Result<String> {
        let head_before = self.store.resolve_head()?;
        let tree_hash = self.snapshot_working_dir()?;

        if let Some(ref head) = head_before {
            let head_commit = self.store.read_commit(head)?;
            if head_commit.tree == tree_hash {
                println!("Nothing changed since last snapshot");
                return Ok(head.clone());
            }
        }

        let parents = head_before
            .as_ref()
            .map(|h| vec![h.clone()])
            .unwrap_or_default();

        let commit = Commit {
            tree: tree_hash,
            parents,
            author: self.author(),
            timestamp: Utc::now(),
            message: message.to_string(),
            change_id: Uuid::now_v7().to_string(),
        };

        let commit_hash = self.store.write_commit(&commit)?;

        let branch = self.store.current_branch()?.unwrap_or("main".to_string());
        self.store.write_branch(&branch, &commit_hash)?;
        self.oplog.append(&OpEntry {
            op: "save".to_string(),
            timestamp: Utc::now(),
            head_before,
            head_after: Some(commit_hash.clone()),
            branch: Some(branch),
            message: Some(message.to_string()),
            extra: None,
            undone: false,
        })?;

        Ok(commit_hash)
    }

    pub fn status(&self) -> Result<Vec<StatusEntry>> {
        let head_tree = match self.store.resolve_head()? {
            Some(h) => Some(self.store.read_commit(&h)?.tree),
            None => None,
        };

        let work_tree_hash = self.snapshot_working_dir()?;
        let diffs = diff_trees(&self.store, head_tree.as_deref(), Some(&work_tree_hash))?;
        Ok(diffs
            .iter()
            .map(|d| StatusEntry {
                path: d.path.clone(),
                status: match d.kind {
                    DiffKind::Added => FileStatus::Added,
                    DiffKind::Deleted => FileStatus::Deleted,
                    DiffKind::Modified => FileStatus::Modified,
                },
            })
            .collect())
    }

    pub fn diff(&self) -> Result<Vec<FileDiff>> {
        let head_tree = match self.store.resolve_head()? {
            Some(h) => Some(self.store.read_commit(&h)?.tree),
            None => None,
        };
        let work_tree = self.snapshot_working_dir()?;
        diff_trees(&self.store, head_tree.as_deref(), Some(&work_tree))
    }

    pub fn diff_commits(&self, old_hash: &str, new_hash: &str) -> Result<Vec<FileDiff>> {
        let old_commit = self.store.read_commit(old_hash)?;
        let new_commit = self.store.read_commit(new_hash)?;
        diff_trees(&self.store, Some(&old_commit.tree), Some(&new_commit.tree))
    }

    pub fn create_branch(&self, name: &str) -> Result<()> {
        if self.store.branch_exists(name) {
            return Err(VaultError::BranchExists(name.to_string()));
        }

        let head = self
            .store
            .resolve_head()?
            .ok_or_else(|| VaultError::ObjectNotFound("HEAD (no commits yet)".to_string()))?;
        self.store.write_branch(name, &head)?;

        self.oplog.append(&OpEntry {
            op: "branch".to_string(),
            timestamp: Utc::now(),
            head_before: Some(head.clone()),
            head_after: Some(head),
            branch: Some(name.to_string()),
            message: None,
            extra: None,
            undone: false,
        })?;
        Ok(())
    }

    pub fn switch(&self, branch: &str) -> Result<()> {
        if !self.store.branch_exists(branch) {
            return Err(VaultError::BranchNotFound(branch.to_string()));
        }
        let head_before = self.store.resolve_head()?;
        let target_hash = self.store.read_branch(branch)?;
        let target_commit = self.store.read_commit(&target_hash)?;

        self.restore_tree(&target_commit.tree)?;
        self.store
            .write_head(&format!("ref: refs/heads/{}", branch))?;

        self.oplog.append(&OpEntry {
            op: "switch".to_string(),
            timestamp: Utc::now(),
            head_before,
            head_after: Some(target_hash),
            branch: Some(branch.to_string()),
            message: None,
            extra: None,
            undone: false,
        })?;
        Ok(())
    }

    pub fn merge(&self, branch: &str) -> Result<MergeOutcome> {
        let ours_hash = self
            .store
            .resolve_head()?
            .ok_or_else(|| VaultError::ObjectNotFound("HEAD".to_string()))?;
        let theirs_hash = self.store.read_branch(branch)?;

        if ours_hash == theirs_hash {
            return Ok(MergeOutcome::AlreadyUpToDate);
        }

        let ours_commit = self.store.read_commit(&ours_hash)?;
        let theirs_commit = self.store.read_commit(&theirs_hash)?;

        let ours_ancestors: HashSet<String> = dag::ancestors(&self.store, &ours_hash)?
            .into_iter()
            .collect();
        if ours_ancestors.contains(&theirs_hash) {
            let cur_branch = self.store.current_branch()?.unwrap_or("main".to_string());
            self.store.write_branch(&cur_branch, &theirs_hash)?;
            self.restore_tree(&theirs_commit.tree)?;
            return Ok(MergeOutcome::FastForward(theirs_hash));
        }

        let base = dag::merge_base(&self.store, &ours_hash, &theirs_hash)?;
        let ancestor_tree = base
            .as_ref()
            .map(|b| self.store.read_commit(b).map(|c| c.tree))
            .transpose()?;

        let head_before = Some(ours_hash.clone());
        let result = three_way_merge(
            &self.store,
            &ours_commit.tree,
            &theirs_commit.tree,
            ancestor_tree.as_deref(),
        )?;

        let merge_commit = Commit {
            tree: result.tree_hash.clone(),
            parents: vec![ours_hash, theirs_hash],
            author: self.author(),
            timestamp: Utc::now(),
            message: format!("Merge branch '{}'", branch),
            change_id: Uuid::now_v7().to_string(),
        };

        let commit_hash = self.store.write_commit(&merge_commit)?;
        let cur_branch = self.store.current_branch()?.unwrap_or("main".to_string());
        self.store.write_branch(&cur_branch, &commit_hash)?;
        self.restore_tree(&result.tree_hash)?;
        self.oplog.append(&OpEntry {
            op: "merge".to_string(),
            timestamp: Utc::now(),
            head_before,
            head_after: Some(commit_hash.clone()),
            branch: Some(cur_branch),
            message: Some(format!("Merge branch '{}'", branch)),
            extra: Some(branch.to_string()),
            undone: false,
        })?;

        if result.had_conflicts {
            Ok(MergeOutcome::Conflicts(commit_hash, result.conflict_paths))
        } else {
            Ok(MergeOutcome::Clean(commit_hash))
        }
    }

    pub fn undo(&self) -> Result<UndoOutcome> {
        match self.oplog.undo_last()? {
            None => Err(VaultError::NothingToUndo),
            Some(None) => Ok(UndoOutcome::RestoredToEmpty),
            Some(Some(head_before)) => {
                let commit = self.store.read_commit(&head_before)?;
                let branch = self.store.current_branch()?.unwrap_or("main".to_string());
                self.store.write_branch(&branch, &head_before)?;
                self.restore_tree(&commit.tree)?;
                Ok(UndoOutcome::Restored(head_before))
            }
        }
    }

    pub fn show(&self, hash: &str) -> Result<ShowResult> {
        let commit = self.store.read_commit(hash)?;
        let diffs = if let Some(parent) = commit.parents.first() {
            diff_trees(
                &self.store,
                Some(&self.store.read_commit(parent)?.tree),
                Some(&commit.tree),
            )?
        } else {
            diff_trees(&self.store, None, Some(&commit.tree))?
        };
        Ok(ShowResult { commit, diffs })
    }
}

pub enum MergeOutcome {
    AlreadyUpToDate,
    FastForward(String),
    Clean(String),
    Conflicts(String, Vec<String>),
}

pub enum UndoOutcome {
    RestoredToEmpty,
    Restored(String),
}

pub struct ShowResult {
    pub commit: Commit,
    pub diffs: Vec<FileDiff>,
}
