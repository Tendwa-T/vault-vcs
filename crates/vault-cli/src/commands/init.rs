use anyhow::Result;
use colored::Colorize;
use std::env;
use vault_core::Repo;

pub fn run(name: &str, email: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    Repo::init(&cwd, name, email)?;
    println!(
        "{} Initialized empty vault repository in {}",
        "✓".green(),
        cwd.display()
    );
    println!(" Author: {} <{}>", name, email);
    println!(" Branch: main");
    Ok(())
}
