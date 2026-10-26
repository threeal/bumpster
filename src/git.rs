//! Wrappers around the `git` command.
//!
//! These run real `git` commands, so they are excluded from coverage instead
//! of being tested with throwaway repositories.

#![cfg(not(tarpaulin_include))]

use crate::command::run;
use anyhow::Result;

/// Returns the name of the current branch.
pub fn current_branch() -> Result<String> {
    run("git", &["branch", "--show-current"])
}

/// Returns whether the working tree has no changes, including untracked files.
pub fn is_clean() -> Result<bool> {
    Ok(run("git", &["status", "--porcelain"])?.is_empty())
}

/// Creates a new branch and switches to it.
pub fn create_branch(name: &str) -> Result<()> {
    run("git", &["switch", "-c", name]).map(|_| ())
}

/// Commits the given files with a sign-off and signature, skipping git hooks.
pub fn commit(message: &str, paths: &[&str]) -> Result<()> {
    let mut args = vec!["commit", "-sSn", "-m", message, "--"];
    args.extend_from_slice(paths);
    run("git", &args).map(|_| ())
}

/// Pushes the given branch to `origin` and sets it as the upstream.
pub fn push(branch: &str) -> Result<()> {
    run("git", &["push", "-u", "origin", branch]).map(|_| ())
}
