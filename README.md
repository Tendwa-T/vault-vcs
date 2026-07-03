# VaultVCS

VaultVCS (`vault`) is a lightweight, Git-like version control system written in Rust. It provides a simple command-line interface to track, diff, branch, merge, log, and manage project history.

## Table of Contents

- [Features](#features)
- [Architecture & Design](#architecture--design)
- [Installation & Build](#installation--build)
- [CLI Reference](#cli-reference)
- [Internal Directory Structure](#internal-directory-structure)

---

## Features

- **Snapshots (`save`):** Instantly capture the state of the working directory.
- **Diffing & Status:** Check modified files and view line-level diffs against the last snapshot.
- **Oplogs & Undoing:** Revert the last action using an operational log (`oplog`) and standard `undo` mechanism.
- **Branch Management:** Create, switch, and merge branches, with support for nested/hierarchical branch names (e.g. `feature/auth`).
- **Conflict Resolution:** Safely handles branching conflicts by detecting and reporting them, allowing automated merges where possible.

---

## Architecture & Design

VaultVCS is composed of two primary crates:

1. **`vault-core` (Library):** The core engine. It manages object stores, file-diffing algorithms (LCS-based), three-way merging, DAG ancestry checks, and oplog generation.
2. **`vault-cli` (Executable):** The command-line interface. Built with [clap](https://crates.io/crates/clap) for parsing arguments and [colored](https://crates.io/crates/colored) for terminal presentation.

### Data Model

Like Git, Vault is content-addressed and records states as trees of objects:

- **Blobs:** File contents indexed by their Blake3 hashes under `objects/blobs`.
- **Trees:** Directory listings containing names, types, and hashes of children, under `objects/trees`.
- **Commits:** Metadata snapshots linking a root tree, parent commit hashes, message, author, and timestamp, under `objects/commits`.
- **Conflicts:** Object representation of active conflicts under `objects/conflicts`.

---

## Installation & Build

### 1. Quick Install on Linux (For clean systems without Rust)

If your Linux system does not have Rust or compilation tools installed, run the following commands to install dependencies, the Rust toolchain, and compile `vault` globally:

```bash
# Update package list and install system build essentials & git
# For Debian/Ubuntu-based systems:
sudo apt-get update && sudo apt-get install -y build-essential curl git

# For Fedora/RHEL/CentOS-based systems:
sudo dnf groupinstall -y "Development Tools" && sudo dnf install -y curl git

# Install the Rust toolchain (Rustup) non-interactively
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Configure your current shell session environment
source "$HOME/.cargo/env"

# Clone the repository
git clone https://github.com/Tendwa-T/vault-vcs.git
cd vault-vcs

# Install Vault globally (compiles and copies to ~/.cargo/bin/)
cargo install --path crates/vault-cli
```

The installer automatically places the compiled `vault` binary inside `~/.cargo/bin/`, which Rustup appends to your shell's `PATH`. You can verify the installation by running:
```bash
vault --version
```

---

### 2. Manual Build (If Rust is already installed)

If you already have Rust and Cargo set up, build the project manually:

```bash
# Clone the repository
git clone https://github.com/Tendwa-T/vault-vcs.git
cd vault-vcs

# Build the workspace in release mode
cargo build --release

# Run the test suite
cargo test
```

The compiled binary is generated at `target/release/vault`. Copy it to a folder in your path (such as `/usr/local/bin/` or `$HOME/.local/bin/`) to use it globally.

---

## CLI Reference

### 1. Initialize a Repository

Initialize a new Vault workspace. Creates a `.vault` folder under the directory.

```bash
vault init --name "Your Name" --email "your@email.com"
```

### 2. Check Workspace Status

Show untracked, added, modified, or deleted files since the last snapshot.

```bash
vault status
```

### 3. View Line-Level Diff

Show modifications line-by-line using a side-by-side diff engine.

```bash
vault diff
```

### 4. Commit Changes (Save Snapshot)

Snapshot the current working directory.

```bash
vault save --message "Commit message here"
```

### 5. Check History & Logs

View the graph history of commits.

```bash
vault log
```

### 6. Create or Switch Branches

Create a branch:

```bash
vault branch feature/auth
```

Switch to an existing branch:

```bash
vault switch feature/auth
```

### 7. Branch Merges

Merge another branch into the active branch:

```bash
vault merge feature/auth
```

### 8. Operations Log (Oplog) & Reversions

Display all operations:

```bash
vault oplog
```

Undo the last command (e.g. undo a switch or save):

```bash
vault undo
```

### 9. Inspect a Commit

Inspect details of a specific commit:

```bash
vault show <commit_id>
```

---

## Internal Directory Structure

An initialized vault directory looks like this:

```text
.vault/
├── HEAD               # Reference to current active branch (e.g., ref: refs/heads/main)
├── oplog/             # Local database of operations
├── refs/
│   └── heads/         # Pointer files to commit hashes per branch
└── objects/
    ├── blobs/         # Content-addressed raw files
    ├── trees/         # Directory structural data
    ├── commits/       # Commit metadata files
    └── conflicts/     # Outstanding conflict references
```
