use crate::cherry::compute_cherry;
use crate::diff::{DiffKind, FileDiff, diff_trees, flatten_tree};
use crate::error::Result;
use crate::merge::{build_tree_from_flat_pub, three_way_merge};
use crate::objects::{
    Author, Commit, EntryKind, FileStatus, FlatTree, StatusEntry, Tree, TreeEntry,
};
use crate::oplog::{OpEntry, OpLog};
use crate::snapshot::{SnapshotEntry, SnapshotStore};
use crate::stash::{StashEntry, StashStore};
use crate::store::ObjectStore;
use crate::{VaultError, dag};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct Repo {
    pub work_dir: PathBuf,
    pub vault_dir: PathBuf,
    pub store: ObjectStore,
    pub oplog: OpLog,
}

pub struct AmendOutcome {
    pub old_hash: String,
    pub new_hash: String,
}

pub struct SplitOutcome {
    pub first_hash: String,
    pub second_hash: String,
}

impl Repo {
    pub fn amend(&self, new_message: Option<&str>) -> Result<AmendOutcome> {
        let old_hash = self
            .store
            .resolve_head()?
            .ok_or_else(|| VaultError::ObjectNotFound("HEAD - Nothing to amend".into()))?;

        let old_commit = self.store.read_commit(&old_hash)?;
        let new_tree = self.snapshot_working_dir()?;
        let new_commit = Commit {
            tree: new_tree,
            parents: old_commit.parents.clone(),
            author: old_commit.author.clone(),
            timestamp: chrono::Utc::now(),
            message: new_message.unwrap_or(&old_commit.message).to_string(),
            change_id: old_commit.change_id.clone(),
        };

        let new_hash = self.store.write_commit(&new_commit)?;
        let branch = self
            .store
            .current_branch()?
            .unwrap_or_else(|| "main".to_string());
        self.store.write_branch(&branch, &new_hash)?;

        self.oplog.append(&OpEntry {
            op: "ammend".to_string(),
            timestamp: Utc::now(),
            head_before: Some(old_hash.clone()),
            head_after: Some(new_hash.clone()),
            branch: Some(branch),
            message: new_message.map(|s| s.to_string()),
            extra: None,
            undone: false,
        })?;

        Ok(AmendOutcome { old_hash, new_hash })
    }

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
            if let Some(suffix) = pattern.strip_prefix("*")
                && rel_path.ends_with(suffix)
            {
                return true;
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

    pub fn squash(&self, n: usize, new_message: &str) -> Result<String> {
        if n < 2 {
            return Err(VaultError::ObjectNotFound(
                "squash requires at least 2 commits".to_string(),
            ));
        }

        let head_hash = self
            .store
            .resolve_head()?
            .ok_or_else(|| VaultError::ObjectNotFound("HEAD".to_string()))?;

        // walk back the commits
        let mut commits = Vec::new();
        let mut cursor = head_hash.clone();
        for _ in 0..n {
            let c = self.store.read_commit(&cursor)?;
            let next = c.parents.first().cloned();
            commits.push((cursor.clone(), c));
            match next {
                Some(p) => cursor = p,
                None => break,
            }
        }
        if commits.len() < n {
            return Err(VaultError::ObjectNotFound(format!(
                "only {} commit(s) in history, cannot squash {}",
                commits.len(),
                n
            )));
        }

        let (_, oldest) = &commits[commits.len() - 1];
        let (_, newest) = &commits[0];

        // the parent of the squashed result becomes the parent of the olderst squash commit
        let new_parents = oldest.parents.clone();

        let squashed = Commit {
            tree: newest.tree.clone(),
            parents: new_parents,
            author: self.author(),
            timestamp: Utc::now(),
            message: new_message.to_string(),
            change_id: oldest.change_id.clone(),
        };

        let new_hash = self.store.write_commit(&squashed)?;
        let branch = self.store.current_branch()?.unwrap_or("main".to_string());
        self.store.write_branch(&branch, &new_hash)?;

        self.oplog.append(&OpEntry {
            op: "squash".to_string(),
            timestamp: Utc::now(),
            head_before: Some(head_hash),
            head_after: Some(new_hash.clone()),
            branch: Some(branch),
            message: Some(new_message.to_string()),
            extra: Some(format!("squashed {} commits", n)),
            undone: false,
        })?;

        Ok(new_hash)
    }

    //Tags
    pub fn create_tag(&self, name: &str, hash: Option<&str>) -> Result<()> {
        let target = match hash {
            Some(h) => h.to_string(),
            None => self
                .store
                .resolve_head()?
                .ok_or_else(|| VaultError::ObjectNotFound("HEAD".to_string()))?,
        };

        let tag_path = self.vault_dir.join("refs").join("tags").join(name);
        if tag_path.exists() {
            return Err(VaultError::BranchExists(format!("tag '{}'", name)));
        }

        fs::create_dir_all(self.vault_dir.join("refs").join("tags"))?;

        fs::write(&tag_path, &target)?;
        Ok(())
    }

    pub fn list_tags(&self) -> Result<Vec<(String, String)>> {
        let dir = self.vault_dir.join("refs").join("tags");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut tags = Vec::new();

        for entry in fs::read_dir(dir)? {
            let e = entry?;
            let name = e.file_name().to_string_lossy().to_string();
            let hash = fs::read_to_string(e.path())?.trim().to_string();
            tags.push((name, hash));
        }
        Ok(tags)
    }

    // Stash
    pub fn stash_save(&self, name: &str) -> Result<()> {
        let branch = self.store.current_branch()?.unwrap_or("main".to_string());
        let tree = self.snapshot_working_dir()?;
        let store = StashStore::new(&self.vault_dir);

        store.save(StashEntry {
            name: name.to_string(),
            tree_hash: tree.clone(),
            created_at: Utc::now(),
            branch,
        })?;

        // Restore the HEAD tree
        if let Some(head) = self.store.resolve_head()? {
            let head_commit = self.store.read_commit(&head)?;
            self.restore_tree(&head_commit.tree)?;
        }

        self.oplog.append(&OpEntry {
            op: "stash-save".to_string(),
            timestamp: Utc::now(),
            head_before: self.store.resolve_head()?,
            head_after: self.store.resolve_head()?,
            branch: Some(self.store.current_branch()?.unwrap_or_default()),
            message: Some(name.to_string()),
            extra: None,
            undone: false,
        })?;

        Ok(())
    }

    pub fn stash_list(&self) -> Result<Vec<StashEntry>> {
        StashStore::new(&self.vault_dir).list()
    }

    pub fn stash_restore(&self, name: &str) -> Result<()> {
        let store = StashStore::new(&self.vault_dir);
        let entry = store.get(name)?;
        self.restore_tree(&entry.tree_hash)?;
        Ok(())
    }

    pub fn stash_drop(&self, name: &str) -> Result<()> {
        StashStore::new(&self.vault_dir).drop(name)
    }

    // ignore
    pub fn ignore_add(&self, pattern: &str) -> Result<()> {
        let path = self.work_dir.join(".vaultignore");
        let current = fs::read_to_string(&path).unwrap_or_default();
        if current.lines().any(|l| l.trim() == pattern) {
            return Ok(());
        }
        let new = format!("{}\n{}\n", current.trim_end(), pattern);
        fs::write(path, new)?;
        Ok(())
    }

    pub fn ignore_list(&self) -> Result<Vec<String>> {
        let path = self.work_dir.join(".vaultignore");
        if !path.exists() {
            return Ok(Vec::new());
        }
        Ok(fs::read_to_string(path)?
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with("#"))
            .map(|l| l.to_string())
            .collect())
    }

    pub fn ignore_check(&self, file: &str) -> Result<Option<String>> {
        let patterns = self.ignored_patterns();
        for p in &patterns {
            if file == p || file.starts_with(p.as_str()) {
                return Ok(Some(p.clone()));
            }
            if p.starts_with("*") && file.ends_with(&p[1..]) {
                return Ok(Some(p.clone()));
            }
        }
        Ok(None)
    }

    // Snapshot
    pub fn snapshot_save(&self, name: &str, note: Option<&str>) -> Result<()> {
        let tree = self.snapshot_working_dir()?;
        let store = SnapshotStore::new(&self.vault_dir);
        store.save(SnapshotEntry {
            name: name.to_string(),
            tree_hash: tree,
            created_at: Utc::now(),
            note: note.map(|s| s.to_string()),
        })
    }

    pub fn snapshot_list(&self) -> Result<Vec<SnapshotEntry>> {
        SnapshotStore::new(&self.vault_dir).list()
    }

    pub fn snapshot_restore(&self, name: &str) -> Result<()> {
        let store = SnapshotStore::new(&self.vault_dir);
        let entry = store.get(name)?;
        self.restore_tree(&entry.tree_hash)
    }

    pub fn snapshot_drop(&self, name: &str) -> Result<()> {
        SnapshotStore::new(&self.vault_dir).drop(name)
    }

    pub fn split(
        &self,
        first_msg: &str,
        second_msg: &str,
        first_files: &[String],
        second_files: &[String],
    ) -> Result<SplitOutcome> {
        let head_hash = self
            .store
            .resolve_head()?
            .ok_or_else(|| VaultError::ObjectNotFound("HEAD".into()))?;
        let head_commit = self.store.read_commit(&head_hash)?;

        let full_tree_hash = self.snapshot_working_dir()?;
        let mut full_flat = HashMap::new();
        flatten_tree(&self.store, &full_tree_hash, "", &mut full_flat)?;

        let mut head_flat = HashMap::new();
        flatten_tree(&self.store, &head_commit.tree, "", &mut head_flat)?;

        let first_set: HashSet<&String> = first_files.iter().collect();
        let second_set: HashSet<&String> = second_files.iter().collect();

        // commit 1 tree: Head + first files change
        let commit1_tree = self.build_partial_tree(&head_flat, &full_flat, &first_set)?;
        let commit1 = Commit {
            tree: commit1_tree,
            parents: vec![head_hash.clone()],
            author: self.author(),
            timestamp: Utc::now(),
            message: first_msg.to_string(),
            change_id: Uuid::now_v7().to_string(),
        };

        let hash1 = self.store.write_commit(&commit1)?;

        // commit 2 tree:
        let mut commit1_flat = HashMap::new();
        flatten_tree(&self.store, &commit1.tree, "", &mut commit1_flat)?;
        let commit2_tree = self.build_partial_tree(&commit1_flat, &full_flat, &second_set)?;
        let commit2 = Commit {
            tree: commit2_tree,
            parents: vec![hash1.clone()],
            author: self.author(),
            timestamp: Utc::now(),
            message: second_msg.to_string(),
            change_id: Uuid::now_v7().to_string(),
        };

        let hash2 = self.store.write_commit(&commit2)?;

        let branch = self
            .store
            .current_branch()?
            .unwrap_or_else(|| "main".to_string());
        self.store.write_branch(&branch, &hash2)?;

        self.oplog.append(&OpEntry {
            op: "split".to_string(),
            timestamp: Utc::now(),
            head_before: Some(head_hash),
            head_after: Some(hash2.clone()),
            branch: Some(branch),
            message: Some(format!("{} / {}", first_msg, second_msg)),
            extra: None,
            undone: false,
        })?;

        Ok(SplitOutcome {
            first_hash: hash1,
            second_hash: hash2,
        })
    }

    // Restore
    // Restore a file
    pub fn restore_file(&self, rel_path: &str, from_hash: Option<&str>) -> Result<()> {
        let commit_hash = match from_hash {
            Some(h) => h.to_string(),
            None => self
                .store
                .resolve_head()?
                .ok_or_else(|| VaultError::ObjectNotFound("HEAD".to_string()))?,
        };

        let commit = self.store.read_commit(&commit_hash)?;
        let mut flat: HashMap<String, (EntryKind, String)> = HashMap::new();
        flatten_tree(&self.store, &commit.tree, "", &mut flat)?;

        match flat.get(rel_path) {
            None => {
                let abs = self.work_dir.join(rel_path);
                if abs.exists() {
                    fs::remove_file(abs)?;
                }
            }
            Some((_, blob_hash)) => {
                let content = self.store.read_blob(blob_hash)?;
                let abs = self.work_dir.join(rel_path);
                if let Some(parent) = abs.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(abs, content)?;
            }
        }
        Ok(())
    }

    // Cherry pick
    pub fn cherry_pick(
        &self,
        commit_hash: &str,
        force_commit_conflicts: bool,
    ) -> Result<CherryPickOutcome> {
        let head_hash = self
            .store
            .resolve_head()?
            .ok_or_else(|| VaultError::ObjectNotFound("HEAD".to_string()))?;
        let head_commit = self.store.read_commit(&head_hash)?;

        let result = compute_cherry(&self.store, commit_hash, &head_commit.tree)?;
        if !result.conflicts.is_empty() && !force_commit_conflicts {
            return Ok(CherryPickOutcome::Conflict(result.conflicts));
        }
        self.restore_tree(&result.new_tree_hash)?;

        // build a commit
        let source = self.store.read_commit(commit_hash)?;
        let new_msg = format!("cherry-pick: {}", source.message);
        let new_commit = Commit {
            tree: result.new_tree_hash,
            parents: vec![head_hash.clone()],
            author: self.author(),
            timestamp: Utc::now(),
            message: new_msg,
            change_id: source.change_id.clone(),
        };

        let new_hash = self.store.write_commit(&new_commit)?;
        let branch = self.store.current_branch()?.unwrap_or("main".to_string());
        self.store.write_branch(&branch, &new_hash)?;

        self.oplog.append(&OpEntry {
            op: "cherry-pick".to_string(),
            timestamp: Utc::now(),
            head_before: Some(head_hash),
            head_after: Some(new_hash.clone()),
            branch: Some(branch),
            message: Some(new_commit.message.clone()),
            extra: Some(commit_hash.to_string()),
            undone: false,
        })?;

        if !result.conflicts.is_empty() {
            Ok(CherryPickOutcome::ConflictSaved(new_hash, result.conflicts))
        } else {
            Ok(CherryPickOutcome::Clean(new_hash))
        }
    }

    fn build_partial_tree(
        &self,
        base: &FlatTree,
        full: &FlatTree,
        paths: &HashSet<&String>,
    ) -> Result<String> {
        let mut result: HashMap<String, (EntryKind, String)> = base.clone();

        for (path, (kind, hash)) in full {
            if paths.contains(path) {
                result.insert(path.clone(), (kind.clone(), hash.clone()));
            }
        }

        for path in paths.iter() {
            if !full.contains_key(path.as_str()) {
                result.remove(path.as_str());
            }
        }
        build_tree_from_flat_pub(&self.store, &result)
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

pub enum CherryPickOutcome {
    Clean(String),
    Conflict(Vec<String>),
    ConflictSaved(String, Vec<String>),
}
