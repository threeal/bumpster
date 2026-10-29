//! Steps shared by the bump flows of the supported tools.

use crate::{gh, git};
use anyhow::{Context, Result, bail};

/// Checks that the current repository is ready for a bump: on the main branch,
/// with a clean working tree, with every given path existing, and with `gh`
/// logged in to open the pull request.
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
    gh::check_auth()
}

/// Checks that the given version of a tool is released, by requesting its
/// release page in the given GitHub repository, where tags are the version
/// prefixed by `tag_prefix`.
#[cfg(not(tarpaulin_include))] // Sends a request, so it is not unit tested.
pub fn check_release(name: &str, repository: &str, tag_prefix: &str, version: &str) -> Result<()> {
    let url = release_url(repository, tag_prefix, version);
    match ureq::head(&url).call() {
        Ok(_) => Ok(()),
        Err(ureq::Error::StatusCode(404)) => {
            bail!("{name} {version} is not released: {url} is not found")
        }
        Err(err) => Err(err).with_context(|| format!("failed to request {url}")),
    }
}

/// Resolves the target of a bump to the version to bump a tool to, where the
/// target is either a version, which is checked to be released in the given
/// GitHub repository, or one of the tool's `tags`, which is resolved to a
/// version by `resolve_tag`.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub fn resolve_target(
    name: &str,
    repository: &str,
    tag_prefix: &str,
    target: &str,
    tags: &[&str],
    resolve_tag: impl FnOnce(&str) -> Result<String>,
) -> Result<String> {
    if is_version(target) {
        println!("checking that {name} {target} is released");
        check_release(name, repository, tag_prefix, target)?;
        return Ok(target.to_string());
    }

    check_tag(name, target, tags)?;
    println!("finding the {target} version of {name}");
    let version = resolve_tag(target)?;
    println!("found {name} {version}");
    Ok(version)
}

/// Returns whether the target of a bump is a plain version rather than a tag.
#[must_use]
pub fn is_version(target: &str) -> bool {
    !target.is_empty() && target.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Checks that a tag is one of the tags supported by a tool.
pub fn check_tag(name: &str, tag: &str, tags: &[&str]) -> Result<()> {
    if !tags.contains(&tag) {
        bail!(
            "{tag} is neither a version nor a tag of {name}, expected a version or one of: {}",
            tags.join(", ")
        );
    }
    Ok(())
}

/// Finds the version of the latest release of a tool in the given GitHub
/// repository, where tags are the version prefixed by `tag_prefix`, from where
/// its latest release page redirects to.
#[cfg(not(tarpaulin_include))] // Sends a request, so it is not unit tested.
pub fn latest_release(repository: &str, tag_prefix: &str) -> Result<String> {
    let url = format!("https://github.com/{repository}/releases/latest");
    let response = ureq::head(&url)
        .config()
        .max_redirects(0)
        .build()
        .call()
        .with_context(|| format!("failed to request {url}"))?;
    let location = response
        .headers()
        .get("location")
        .and_then(|location| location.to_str().ok())
        .with_context(|| format!("{url} does not redirect to a release"))?;
    release_version(repository, tag_prefix, location)
}

/// Returns the version of a release from the URL of its release page in the
/// given GitHub repository, where tags are the version prefixed by
/// `tag_prefix`.
pub fn release_version(repository: &str, tag_prefix: &str, url: &str) -> Result<String> {
    match url.strip_prefix(&release_url(repository, tag_prefix, "")) {
        Some(version) if is_version(version) => Ok(version.to_string()),
        _ => bail!("{url} is not a release of a plain version"),
    }
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

/// Opens the pull request for bumping a tool to the given version, linking to
/// its release in the given GitHub repository, where tags are the version
/// prefixed by `tag_prefix`.
///
/// Returns the URL of the opened pull request.
#[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
pub fn open_pull_request(
    name: &str,
    repository: &str,
    tag_prefix: &str,
    version: &str,
) -> Result<String> {
    gh::create_pull_request(
        &pull_request_title(name, version),
        &pull_request_body(name, repository, tag_prefix, version),
    )
}

/// Returns the title of the pull request for bumping a tool to the given
/// version.
#[must_use]
pub fn pull_request_title(name: &str, version: &str) -> String {
    format!("Bump {name} to version {version}")
}

/// Returns the description of the pull request for bumping a tool to the
/// given version, linking to its release in the given GitHub repository,
/// where tags are the version prefixed by `tag_prefix`.
#[must_use]
pub fn pull_request_body(name: &str, repository: &str, tag_prefix: &str, version: &str) -> String {
    format!(
        "This pull request bumps {name} to version [{version}]({}).",
        release_url(repository, tag_prefix, version)
    )
}

/// Returns the URL of the release of the given version in the given GitHub
/// repository, where tags are the version prefixed by `tag_prefix`.
#[must_use]
pub fn release_url(repository: &str, tag_prefix: &str, version: &str) -> String {
    format!("https://github.com/{repository}/releases/tag/{tag_prefix}{version}")
}

/// The edited content of a file, along with the values it changed, to show the
/// change in the log.
#[derive(Debug)]
pub struct Edit {
    /// The content of the file after the edit.
    pub content: String,
    /// The value before the edit.
    pub from: String,
    /// The value after the edit.
    pub to: String,
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

/// Splits a version range into its prefix (`^`, `~`, or none) and version,
/// refusing anything more complex, naming `field` in the error.
pub fn split_range<'a>(range: &'a str, field: &str) -> Result<(&'a str, &'a str)> {
    let version = range.trim_start_matches(['^', '~']);
    let prefix = &range[..range.len() - version.len()];

    if prefix.len() > 1
        || version.is_empty()
        || !version.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        bail!("{field} is not a simple range: {range}");
    }

    Ok((prefix, version))
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
            pull_request_title("Node.js", "24.22.1"),
            "Bump Node.js to version 24.22.1"
        );
        assert_eq!(
            pull_request_body("Node.js", "nodejs/node", "v", "24.22.1"),
            "This pull request bumps Node.js to version [24.22.1](https://github.com/nodejs/node/releases/tag/v24.22.1)."
        );
        assert_eq!(
            release_url("nodejs/node", "v", "24.22.1"),
            "https://github.com/nodejs/node/releases/tag/v24.22.1"
        );
    }

    #[test]
    fn is_version_or_tag() {
        for target in ["24.22.1", "24", "0.6.0"] {
            assert!(is_version(target), "{target}");
        }
        for target in ["", "latest", "lts", "v24.22.1", "24.x"] {
            assert!(!is_version(target), "{target}");
        }
    }

    #[test]
    fn check_tag_supported() {
        check_tag("Node.js", "lts", &["lts", "latest"]).unwrap();
    }

    #[test]
    fn check_tag_unsupported() {
        assert_eq!(
            check_tag("Node.js", "next", &["lts", "latest"])
                .unwrap_err()
                .to_string(),
            "next is neither a version nor a tag of Node.js, expected a version or one of: lts, latest"
        );
    }

    #[test]
    fn release_version_valid() {
        assert_eq!(
            release_version(
                "g-plane/pretty_yaml",
                "v",
                "https://github.com/g-plane/pretty_yaml/releases/tag/v0.6.0"
            )
            .unwrap(),
            "0.6.0"
        );
    }

    #[test]
    fn release_version_invalid() {
        for url in [
            "https://github.com/g-plane/pretty_yaml/releases",
            "https://github.com/g-plane/pretty_yaml/releases/tag/0.6.0",
            "https://github.com/g-plane/pretty_yaml/releases/tag/v0.6.0-beta.1",
        ] {
            assert_eq!(
                release_version("g-plane/pretty_yaml", "v", url)
                    .unwrap_err()
                    .to_string(),
                format!("{url} is not a release of a plain version")
            );
        }
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
