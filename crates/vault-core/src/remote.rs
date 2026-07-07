use crate::error::{Result, VaultError};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

pub fn list_remotes(vault_dir: &Path) -> Result<Vec<Remote>> {
    let content = fs::read_to_string(vault_dir.join("config")).unwrap_or_default();
    let mut remotes = Vec::new();
    let mut cur_name: Option<String> = None;
    let mut cur_url: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("[remote \"") {
            if let (Some(n), Some(u)) = (cur_name.take(), cur_url.take()) {
                remotes.push(Remote { name: n, url: u });
            }
            cur_name = Some(
                line.trim_start_matches("[remote \"")
                    .trim_end_matches("\"]")
                    .to_string(),
            );
        } else if let Some(url) = line.strip_prefix("url = ") {
            cur_url = Some(url.trim().to_string());
        }
    }
    if let (Some(n), Some(u)) = (cur_name, cur_url) {
        remotes.push(Remote { name: n, url: u });
    }
    Ok(remotes)
}

pub fn add_remote(nya_dir: &Path, name: &str, url: &str) -> Result<()> {
    let config_path = nya_dir.join("config");
    let mut content = fs::read_to_string(&config_path).unwrap_or_default();
    content.push_str(&format!("\n[remote \"{}\"]\nurl = {}\n", name, url));
    fs::write(config_path, content)?;
    Ok(())
}

pub fn get_remote(nya_dir: &Path, name: &str) -> Result<Remote> {
    list_remotes(nya_dir)?
        .into_iter()
        .find(|r| r.name == name)
        .ok_or_else(|| VaultError::BranchNotFound(format!("remote '{}'", name)))
}
