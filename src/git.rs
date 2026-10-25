//! Wrappers around the `git` command.
//!
//! These run real `git` commands, so they are excluded from coverage instead
//! of being tested with throwaway repositories.

#![cfg(not(tarpaulin_include))]

use anyhow::{Context, Result, bail};
use std::process::Command;

/// Runs `git` with the given arguments and returns its trimmed output.
fn run(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("failed to run git")?;

    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Returns the name of the current branch.
pub fn current_branch() -> Result<String> {
    run(&["branch", "--show-current"])
}

/// Returns whether the working tree has no changes, including untracked files.
pub fn is_clean() -> Result<bool> {
    Ok(run(&["status", "--porcelain"])?.is_empty())
}

/// Creates a new branch and switches to it.
pub fn create_branch(name: &str) -> Result<()> {
    run(&["switch", "-c", name]).map(|_| ())
}

/// Commits the given file with a sign-off and signature, skipping git hooks.
pub fn commit(message: &str, path: &str) -> Result<()> {
    run(&["commit", "-sSn", "-m", message, "--", path]).map(|_| ())
}

/// Pushes the given branch to `origin` and sets it as the upstream.
pub fn push(branch: &str) -> Result<()> {
    run(&["push", "-u", "origin", branch]).map(|_| ())
}
