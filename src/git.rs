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

/// Returns the changes in the working tree, including untracked files, in
/// git's short status format, one file per line, or an empty string if there
/// are none.
pub fn changes() -> Result<String> {
    run("git", &["status", "--short"])
}

/// Discards the changes in the working tree, by resetting tracked files,
/// including staged changes, and removing untracked files, keeping ignored
/// files.
pub fn discard_changes() -> Result<()> {
    run("git", &["reset", "--hard"])?;
    run("git", &["clean", "-fd"]).map(|_| ())
}

/// Fast-forwards the current branch to the given branch of `origin`, failing
/// instead if it can't be fast-forwarded.
pub fn pull(branch: &str) -> Result<()> {
    run("git", &["pull", "--ff-only", "origin", branch]).map(|_| ())
}

/// Returns whether the given branch has commits that its counterpart on
/// `origin` doesn't.
pub fn is_ahead_of_origin(branch: &str) -> Result<bool> {
    let range = format!("origin/{branch}..{branch}");
    Ok(run("git", &["rev-list", "--count", &range])? != "0")
}

/// Switches to an existing branch.
pub fn switch(branch: &str) -> Result<()> {
    run("git", &["switch", branch]).map(|_| ())
}

/// Creates a new branch and switches to it.
pub fn create_branch(name: &str) -> Result<()> {
    run("git", &["switch", "-c", name]).map(|_| ())
}

/// Commits the given files with a sign-off and signature, skipping git hooks.
pub fn commit(message: &str, paths: &[&str]) -> Result<()> {
    let mut args = vec!["commit", "-nsS", "-m", message, "--"];
    args.extend_from_slice(paths);
    run("git", &args).map(|_| ())
}

/// Pushes the given branch to `origin` and sets it as the upstream.
pub fn push(branch: &str) -> Result<()> {
    run("git", &["push", "-u", "origin", branch]).map(|_| ())
}
