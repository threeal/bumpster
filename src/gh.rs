//! Wrappers around the `gh` command.
//!
//! These run real `gh` commands, so they are excluded from coverage instead of
//! being tested against GitHub.

#![cfg(not(tarpaulin_include))]

use crate::command::run;
use anyhow::Result;

/// Checks that `gh` is installed and logged in.
pub fn check_auth() -> Result<()> {
    run("gh", &["auth", "status"]).map(|_| ())
}

/// Returns the name of the default branch of the repository on GitHub.
pub fn default_branch() -> Result<String> {
    run(
        "gh",
        &[
            "repo",
            "view",
            "--json",
            "defaultBranchRef",
            "--jq",
            ".defaultBranchRef.name",
        ],
    )
}

/// Returns the URL of the open pull request whose head is the given branch, or
/// `None` if there is none.
pub fn find_pull_request(branch: &str) -> Result<Option<String>> {
    let url = run(
        "gh",
        &[
            "pr",
            "list",
            "--head",
            branch,
            "--state",
            "open",
            "--json",
            "url",
            "--jq",
            ".[0].url // empty",
        ],
    )?;
    Ok(Some(url).filter(|url| !url.is_empty()))
}

/// Edits the pull request at the given URL to target the given base branch
/// with the given title and body.
pub fn edit_pull_request(url: &str, base: &str, title: &str, body: &str) -> Result<()> {
    run(
        "gh",
        &[
            "pr", "edit", url, "--base", base, "--title", title, "--body", body,
        ],
    )
    .map(|_| ())
}

/// Opens a pull request for the current branch against the given base
/// branch, returning its URL.
pub fn create_pull_request(base: &str, title: &str, body: &str) -> Result<String> {
    run(
        "gh",
        &[
            "pr", "create", "--base", base, "--title", title, "--body", body,
        ],
    )
}
