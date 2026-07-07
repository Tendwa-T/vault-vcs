use anyhow::{Result, bail};
use colored::Colorize;
use reqwest::Identity;
use std::env;
use std::env::home_dir;
use vault_core::{Repo, dag, objects::EntryKind, remote, store::ObjectStore};

pub fn run(remote_name: &str, branch: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::open(&cwd)?;
    let rem = remote::get_remote(&repo.vault_dir, remote_name)?;
    let branch = branch
        .unwrap_or(&repo.store.current_branch()?.unwrap_or("main".to_string()))
        .to_string();

    let local_hash = repo.store.read_branch(&branch)?;
    let client = build_client(&repo)?;
    let repo_name = repo_name_from_url(&rem.url);
    let server_url = server_base_url_from_remote_url(&rem.url);

    println!("Pushing to {}...", rem.url.yellow());

    // Ask server what it already has for this ref
    let server_hash = fetch_server_ref(&client, &server_url, &repo_name, &branch)?;

    // Walk local commit graph, collect objects server doesn't have
    let have: Vec<String> = server_hash.into_iter().collect();
    let objects = collect_local_objects(&repo, &local_hash, &have)?;

    println!("  {} object(s) to send", objects.len().to_string().bold());

    if objects.is_empty() {
        println!("{} Already up to date.", "✓".green());
        return Ok(());
    }

    // Build and send packfile
    let mut pack_body = Vec::new();
    let mut pw = PackWriter::new(&mut pack_body);
    for (kind, hash) in &objects {
        let kind_dir = format!("{}s", kind);
        let data = repo.store.read_object_raw(&kind_dir, hash)?;
        pw.write(kind, hash, &data)?;
    }
    pw.flush()?;

    let push_url = format!("{}/repos/{}/push", server_url, repo_name);

    let resp = client
        .post(&push_url)
        .header("X-Vault-Ref", format!("heads/{}", branch))
        .header("X-Vault-Want", &local_hash)
        .body(pack_body)
        .send()?;

    if !resp.status().is_success() {
        bail!("push failed: {} {}", resp.status(), resp.text()?);
    }

    println!(
        "{} [{}] {} → {}",
        "✓".green(),
        &local_hash[..8].yellow(),
        branch.cyan(),
        rem.url.yellow()
    );
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_client(_repo: &Repo) -> Result<reqwest::blocking::Client> {
    // Load client cert from ~/.config/vaultd/certs/client.{crt,key}
    let home = home_dir().unwrap();
    let cert_dir = home.join(".config").join("vaultd").join("certs");
    let cert = std::fs::read(cert_dir.join("client.crt"))?;
    let key = std::fs::read(cert_dir.join("client.key"))?;
    let ca = std::fs::read(cert_dir.join("ca.crt"))?;

    let mut pem_data = cert;
    pem_data.extend_from_slice(b"\n");
    pem_data.extend_from_slice(&key);
    let identity = Identity::from_pem(&pem_data)?;
    let ca_cert = reqwest::Certificate::from_pem(&ca)?;

    Ok(reqwest::blocking::Client::builder()
        .identity(identity)
        .add_root_certificate(ca_cert)
        .danger_accept_invalid_hostnames(false)
        .timeout(std::time::Duration::from_secs(300))
        .build()?)
}

fn fetch_server_ref(
    client: &reqwest::blocking::Client,
    server_url: &str,
    repo: &str,
    branch: &str,
) -> Result<Option<String>> {
    let url = format!("{}/repos/{}/refs", server_url, repo);
    let resp = client.get(&url).send()?;
    if resp.status().as_u16() == 404 {
        return Ok(None);
    }

    let body: serde_json::Value = resp.json()?;
    let key = format!("heads/{}", branch);
    Ok(body["refs"][key].as_str().map(|s| s.to_string()))
}

fn collect_local_objects(
    repo: &Repo,
    local_hash: &str,
    have: &[String],
) -> Result<Vec<(String, String)>> {
    let have_set: std::collections::HashSet<String> = have.iter().cloned().collect();
    let commits = dag::log_walk(&repo.store, local_hash)?;
    let mut objects: Vec<(String, String)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (hash, commit) in commits {
        if have_set.contains(&hash) {
            break;
        }
        if seen.insert(hash.clone()) {
            objects.push(("commit".to_string(), hash));
        }
        // Walk tree
        collect_tree_objects(&repo.store, &commit.tree, &mut seen, &mut objects)?;
    }
    Ok(objects)
}

fn collect_tree_objects(
    store: &ObjectStore,
    hash: &str,
    seen: &mut std::collections::HashSet<String>,
    objects: &mut Vec<(String, String)>,
) -> Result<()> {
    let key = format!("tree:{}", hash);
    if !seen.insert(key) {
        return Ok(());
    }
    objects.push(("tree".to_string(), hash.to_string()));

    let tree = store.read_tree(hash)?;
    for entry in &tree.entries {
        match entry.kind {
            EntryKind::Blob => {
                if seen.insert(format!("blob:{}", entry.hash)) {
                    objects.push(("blob".to_string(), entry.hash.clone()));
                }
            }
            EntryKind::Tree => {
                collect_tree_objects(store, &entry.hash, seen, objects)?;
            }
            EntryKind::Conflict => {
                if seen.insert(format!("conflict:{}", entry.hash)) {
                    objects.push(("conflict".to_string(), entry.hash.clone()));
                }
            }
        }
    }
    Ok(())
}

fn repo_name_from_url(url: &str) -> String {
    url.trim_end_matches('/')
        .split('/')
        .last()
        .unwrap_or("repo")
        .to_string()
}

fn server_base_url_from_remote_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if let Some(pos) = trimmed.rfind('/') {
        trimmed[..pos].to_string()
    } else {
        trimmed.to_string()
    }
}

// Inline pack writer for push (mirrors sync/packfile.go)
struct PackWriter<'a> {
    buf: &'a mut Vec<u8>,
}
impl<'a> PackWriter<'a> {
    fn new(buf: &'a mut Vec<u8>) -> Self {
        PackWriter { buf }
    }
    fn write(&mut self, kind: &str, hash: &str, data: &[u8]) -> Result<()> {
        let hdr = serde_json::json!({"kind": kind, "hash": hash, "size": data.len()});
        self.buf.extend_from_slice(hdr.to_string().as_bytes());
        self.buf.push(b'\n');
        self.buf.extend_from_slice(data);
        self.buf.push(b'\n');
        Ok(())
    }
    fn flush(&self) -> Result<()> {
        Ok(())
    }
}
