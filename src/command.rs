//! Running external commands.
//!
//! These run real commands, so they are excluded from coverage instead of
//! being tested.

#![cfg(not(tarpaulin_include))]

use anyhow::{Context, Result, bail};
use std::process::Command;

/// Runs a program with the given arguments and returns its trimmed output.
pub fn run(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {program}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = if stderr.trim().is_empty() {
            String::from_utf8_lossy(&output.stdout)
        } else {
            stderr
        };
        bail!("{program} {} failed: {}", args.join(" "), message.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
