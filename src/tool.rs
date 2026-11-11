//! Steps shared by the bump flows of the supported tools.

use crate::{gh, git, prompt};
use anyhow::{Context, Result, bail};
use std::cmp::Ordering;
use std::io::IsTerminal;

/// Prepares the current repository for a bump, by getting it onto the main
/// branch with a clean working tree, switching to the main branch and
/// discarding its changes after asking unless `yes` is set, then checking that
/// `gh` is logged in to open the pull request, then pulling the main branch
/// from `origin` so the bump starts from its latest commit.
#[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
pub fn prepare_repository(yes: bool) -> Result<()> {
    let on_main = git::current_branch()? == "main";
    let changes = git::changes()?;
    match (on_main, changes.is_empty()) {
        (true, true) => {}
        (false, true) => {
            println!("switching to the main branch");
            git::switch("main")?;
        }
        (false, false) => {
            println!("uncommitted changes:\n{changes}");
            confirm_fix(
                "not on the main branch and the working tree is not clean",
                "discard the changes and switch to the main branch",
                yes,
            )?;
            println!("discarding the changes");
            git::discard_changes()?;
            println!("switching to the main branch");
            git::switch("main")?;
        }
        (true, false) => {
            println!("uncommitted changes:\n{changes}");
            confirm_fix("the working tree is not clean", "discard the changes", yes)?;
            println!("discarding the changes");
            git::discard_changes()?;
        }
    }
    gh::check_auth()?;

    println!("pulling the main branch from origin");
    git::pull("main")?;
    if git::is_ahead_of_origin("main")? {
        bail!("the main branch has commits not pushed to origin");
    }
    Ok(())
}

/// Confirms making the given fix to the given problem with the repository,
/// asking whether to with no as the default answer unless `yes` is set, since
/// the fix can't be undone.
///
/// Returns an error naming the problem if the fix is declined or there's no
/// terminal to ask on.
#[cfg(not(tarpaulin_include))] // Reads from the terminal, so it is not unit tested.
fn confirm_fix(problem: &str, fix: &str, yes: bool) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        bail!("{problem}, and no terminal to confirm on, pass --yes to {fix}");
    }
    if !prompt::confirm(&format!("{problem}, {fix}"), false)? {
        bail!("{problem}");
    }
    Ok(())
}

/// Checks that every given path to edit exists.
#[cfg(not(tarpaulin_include))] // Reads real files, so it is not unit tested.
pub fn check_paths(paths: &[&str]) -> Result<()> {
    for path in paths {
        if !std::path::Path::new(path).exists() {
            bail!("{path} does not exist");
        }
    }
    Ok(())
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
/// target is either a version, which is checked to be released by
/// `check_version`, or one of the tool's `tags`, which is resolved to a version
/// by `resolve_tag`.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub fn resolve_target(
    name: &str,
    target: &str,
    tags: &[&str],
    check_version: impl FnOnce(&str) -> Result<()>,
    resolve_tag: impl FnOnce(&str) -> Result<String>,
) -> Result<String> {
    if is_version(target) {
        println!("checking that {name} {target} is released");
        check_version(target)?;
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

/// Returns whether a target is a plain version with only a major, or a major
/// and minor, such as `24` or `24.22`, rather than a full version.
#[must_use]
pub fn is_partial_version(target: &str) -> bool {
    is_version(target) && target.split('.').count() < 3
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

/// Checks that a tool can be bumped from its current version to the given
/// version, which must be newer.
pub fn check_newer(name: &str, current: &str, version: &str) -> Result<()> {
    match compare_versions(version, current) {
        Ordering::Greater => Ok(()),
        Ordering::Equal => bail!("{name} is already at version {version}"),
        Ordering::Less => bail!("{name} is already at version {current}, newer than {version}"),
    }
}

/// Compares two plain versions by their numeric parts in order, where a
/// missing part counts as zero.
#[must_use]
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn parts(version: &str) -> Vec<&str> {
        version
            .split('.')
            .map(|part| part.trim_start_matches('0'))
            .collect()
    }
    let (a, b) = (parts(a), parts(b));

    (0..a.len().max(b.len()))
        .map(|i| {
            let a = a.get(i).copied().unwrap_or_default();
            let b = b.get(i).copied().unwrap_or_default();
            a.len().cmp(&b.len()).then_with(|| a.cmp(b))
        })
        .find(|ordering| ordering.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// Reads the content of a file to edit.
#[cfg(not(tarpaulin_include))] // Reads a real file, so it is not unit tested.
pub fn read(path: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))
}

/// Reads the content of a file to edit if it exists, showing that it's read,
/// or returns `None` without showing anything if it doesn't.
#[cfg(not(tarpaulin_include))] // Reads a real file, so it is not unit tested.
pub fn read_if_exists(path: &str) -> Result<Option<String>> {
    if !std::path::Path::new(path).exists() {
        return Ok(None);
    }
    println!("reading {path}");
    read(path).map(Some)
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

/// The pull request for a bump.
#[derive(Debug)]
pub struct PullRequest {
    /// The URL of the pull request.
    pub url: String,
    /// Whether the pull request was opened by the bump, rather than already
    /// open from an earlier run of it and updated.
    pub opened: bool,
}

/// Opens the pull request for bumping a tool to the given version on the given
/// branch, or updates the one already open there, linking to its release in
/// the given GitHub repository, where tags are the version prefixed by
/// `tag_prefix`.
///
/// An open pull request is given the same title, body, and base as opening one
/// would, as the push already gave it the new commit.
#[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
pub fn open_or_update_pull_request(
    branch: &str,
    name: &str,
    repository: &str,
    tag_prefix: &str,
    version: &str,
) -> Result<PullRequest> {
    let title = pull_request_title(name, version);
    let body = pull_request_body(name, repository, tag_prefix, version);
    if let Some(url) = gh::find_pull_request(branch)? {
        println!("updating the pull request");
        gh::edit_pull_request(&url, &title, &body)?;
        return Ok(PullRequest { url, opened: false });
    }
    println!("opening a pull request");
    let url = gh::create_pull_request(&title, &body)?;
    Ok(PullRequest { url, opened: true })
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

/// Splits a version range into its prefix (`^`, `~`, or none) and plain
/// version, or returns `None` if the range is anything more complex.
#[must_use]
pub fn split_range(range: &str) -> Option<(&str, &str)> {
    let version = range.trim_start_matches(['^', '~']);
    let prefix = &range[..range.len() - version.len()];
    (prefix.len() <= 1 && is_version(version)).then_some((prefix, version))
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
    fn is_partial_version_or_not() {
        for target in ["24", "24.22"] {
            assert!(is_partial_version(target), "{target}");
        }
        for target in ["24.22.1", "lts", ""] {
            assert!(!is_partial_version(target), "{target}");
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
    fn check_newer_versions() {
        check_newer("Node.js", "24.22.0", "24.22.1").unwrap();
        assert_eq!(
            check_newer("Node.js", "24.22.0", "24.22.0")
                .unwrap_err()
                .to_string(),
            "Node.js is already at version 24.22.0"
        );
        assert_eq!(
            check_newer("Node.js", "25.1.0", "24.22.1")
                .unwrap_err()
                .to_string(),
            "Node.js is already at version 25.1.0, newer than 24.22.1"
        );
    }

    #[test]
    fn compare_versions_order() {
        let cases = [
            ("24.22.1", "24.22.0", Ordering::Greater),
            ("24.22.0", "24.22.1", Ordering::Less),
            ("24.10.0", "24.9.0", Ordering::Greater),
            ("100.0.0", "99.99.99", Ordering::Greater),
            ("24.1", "24.0.9", Ordering::Greater),
            ("24", "24.0.0", Ordering::Equal),
            ("24.01.0", "24.1.0", Ordering::Equal),
            ("24.22.1", "24.22.1", Ordering::Equal),
        ];
        for (a, b, ordering) in cases {
            assert_eq!(compare_versions(a, b), ordering, "{a} vs {b}");
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
