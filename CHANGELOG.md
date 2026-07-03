# Changelog

All notable changes to the VaultVCS project will be documented in this file.

## [Unreleased] - 2026-07-03

### Fixed

- **Branch Subdirectories Support:** Fixed `No such file or directory` (os error 2) when creating nested branches containing path separators (e.g. `feature/auth`). `write_branch` now automatically ensures that parent directories under `refs/heads/` are created recursively before writing the branch reference file.
- **Hierarchical Branch Listing:** Fixed listing of nested/sub-directory branches. Replaced flat directory iteration in `list_branches` with a recursive directory walker (`collect_branches_rec`) so that sub-directory branches are fully traversed and formatted with relative slash-separated paths instead of mistakenly reporting parent folder names as branch names.
- **Tree Object Serialization Directory Mismatch:** Fixed `ObjectNotFound` error in `vault status` by correcting a bug in `write_tree` where tree objects were incorrectly saved under the `objects/commits/` directory instead of the `objects/trees/` directory.
- **Branch Reference Format Mismatch:** Standardized the HEAD reference path format. Fixed a bug in `switch` where the target branch reference was written as `ref: ref/heads/<branch>` (singular `ref`) instead of `ref: refs/heads/<branch>` (plural `refs`), resolving conflicts with `resolve_head` and `current_branch` parsing.
- **Compiler Warnings Cleanup:** 
  - Removed unused imports (`flatten_tree`, `FlatTree`, `HashMap`, `read_to_string`, and `walkdir::WalkDir`) from `repo.rs`.
  - Removed unused local variables (`new_cursor`) and redundant re-assignments to (`old_cursor`, `new_cursor`) inside `diff.rs`.
  - Removed unreachable match arm `(None, None, Some(_))` from the file resolution logic in `merge.rs`.
  - Prefixed unused variables (`ours` and `theirs`) with underscores in the `try_auto_merge` placeholder function to suppress unused variable warnings.
