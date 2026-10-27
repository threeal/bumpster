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

/// Opens a pull request for the current branch against `main`, returning its
/// URL.
pub fn create_pull_request(title: &str, body: &str) -> Result<String> {
    run(
        "gh",
        &[
            "pr", "create", "--base", "main", "--title", title, "--body", body,
        ],
    )
}
