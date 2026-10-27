//! Steps shared by the bump flows of the supported tools.

use crate::git;
use anyhow::{Context, Result, bail};

/// Checks that the current repository is ready for a bump: on the main branch,
/// with a clean working tree, and with every given path existing.
#[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
pub fn check_ready(paths: &[&str]) -> Result<()> {
    if git::current_branch()? != "main" {
        bail!("not on the main branch");
    }
    if !git::is_clean()? {
        bail!("the working tree is not clean");
    }
    for path in paths {
        if !std::path::Path::new(path).exists() {
            bail!("{path} does not exist");
        }
    }
    Ok(())
}

/// Reads the content of a file to edit.
#[cfg(not(tarpaulin_include))] // Reads a real file, so it is not unit tested.
pub fn read(path: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))
}

/// Writes the edited content of a file.
#[cfg(not(tarpaulin_include))] // Writes a real file, so it is not unit tested.
pub fn write(path: &str, content: &str) -> Result<()> {
    std::fs::write(path, content).with_context(|| format!("failed to write {path}"))
}

/// Returns the name of the branch for bumping a tool to the given version.
#[must_use]
pub fn branch_name(id: &str, version: &str) -> String {
    format!("bump-{id}-{version}")
}

/// Returns the commit message for bumping a tool to the given version.
#[must_use]
pub fn commit_message(name: &str, version: &str) -> String {
    format!("bump {name} to version {version}")
}

/// Returns the title and description of the pull request for bumping a tool
/// to the given version, linking to its release in the given GitHub
/// repository, where tags are the version prefixed by `tag_prefix`.
#[must_use]
pub fn pull_request(name: &str, repository: &str, tag_prefix: &str, version: &str) -> String {
    format!(
        "Bump {name} to version {version}\n\nThis pull request bumps {name} to version [{version}](https://github.com/{repository}/releases/tag/{tag_prefix}{version})."
    )
}

/// Replaces the only occurrence of `from` in `content` with `to`, so a version
/// can be edited without rewriting the rest of a file.
pub fn replace_once(content: &str, from: &str, to: &str) -> Result<String> {
    let count = content.matches(from).count();
    if count != 1 {
        bail!("expected exactly one {from}, found {count}");
    }
    Ok(content.replacen(from, to, 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_output() {
        assert_eq!(branch_name("node", "24.22.1"), "bump-node-24.22.1");
        assert_eq!(
            commit_message("Node.js", "24.22.1"),
            "bump Node.js to version 24.22.1"
        );
        assert_eq!(
            pull_request("Node.js", "nodejs/node", "v", "24.22.1"),
            "Bump Node.js to version 24.22.1\n\nThis pull request bumps Node.js to version [24.22.1](https://github.com/nodejs/node/releases/tag/v24.22.1)."
        );
    }

    #[test]
    fn replace_once_single() {
        assert_eq!(replace_once("a b c", "b", "d").unwrap(), "a d c");
    }

    #[test]
    fn replace_once_not_single() {
        assert_eq!(
            replace_once("a c", "b", "d").unwrap_err().to_string(),
            "expected exactly one b, found 0"
        );
        assert_eq!(
            replace_once("a b b", "b", "d").unwrap_err().to_string(),
            "expected exactly one b, found 2"
        );
    }
}
