//! The bump flow shared by every supported tool.

use anyhow::{Result, bail};

/// A tool that can be bumped, described by the parts of the bump flow that
/// differ between tools.
pub struct Tool {
    /// The short name of the tool, used in branch names.
    pub id: &'static str,
    /// The display name of the tool, used in commit messages and pull
    /// requests.
    pub name: &'static str,
    /// The GitHub repository of the tool, used to link its release notes.
    pub repository: &'static str,
    /// The file that holds the version of the tool.
    pub file: &'static str,
    /// Returns the content of `file` with the tool bumped to the given
    /// version.
    pub edit: fn(&str, &str) -> Result<String>,
    /// A command to run after editing `file`, as a program and its arguments.
    pub command: Option<(&'static str, &'static [&'static str])>,
    /// The files to commit, which must all exist before the bump.
    pub paths: &'static [&'static str],
}

impl Tool {
    /// Bumps the tool in the current repository to the given version, then
    /// commits and pushes the change to a new branch.
    ///
    /// Returns the title and description of the pull request to open for it.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository is not ready for the bump, or if any
    /// step of it fails.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn bump(&self, version: &str) -> Result<String> {
        use crate::{command, git};
        use anyhow::Context;

        if git::current_branch()? != "main" {
            bail!("not on the main branch");
        }
        if !git::is_clean()? {
            bail!("the working tree is not clean");
        }
        for path in self.paths {
            if !std::path::Path::new(path).exists() {
                bail!("{path} does not exist");
            }
        }

        let content = std::fs::read_to_string(self.file)
            .with_context(|| format!("failed to read {}", self.file))?;
        let content = (self.edit)(&content, version)?;

        let branch = self.branch_name(version);
        git::create_branch(&branch)?;
        std::fs::write(self.file, content)
            .with_context(|| format!("failed to write {}", self.file))?;
        if let Some((program, args)) = self.command {
            command::run(program, args)?;
        }
        git::commit(&self.commit_message(version), self.paths)?;
        git::push(&branch)?;

        Ok(self.pull_request(version))
    }

    /// Returns the name of the branch for bumping to the given version.
    #[must_use]
    pub fn branch_name(&self, version: &str) -> String {
        format!("bump-{}-{version}", self.id)
    }

    /// Returns the commit message for bumping to the given version.
    #[must_use]
    pub fn commit_message(&self, version: &str) -> String {
        format!("bump {} to version {version}", self.name)
    }

    /// Returns the title and description of the pull request for bumping to
    /// the given version.
    #[must_use]
    pub fn pull_request(&self, version: &str) -> String {
        let name = self.name;
        let repository = self.repository;
        format!(
            "Bump {name} to version {version}\n\nThis pull request bumps {name} to version [{version}](https://github.com/{repository}/releases/tag/v{version})."
        )
    }
}

/// Replaces the only occurrence of `from` in `content` with `to`, so a version
/// can be edited without rewriting the rest of a file.
pub(crate) fn replace_once(content: &str, from: &str, to: &str) -> Result<String> {
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
