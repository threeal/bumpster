//! Bumping the version of Go in the `go` directive of a `go.mod` file.

use crate::git;
use crate::tool::{self, Edit, PullRequest};
use anyhow::{Context, Result, bail};

/// The tags that Go can be bumped to instead of a version.
pub const TAGS: [&str; 1] = ["latest"];

/// A bump of Go, prepared up to the edit of `go.mod` that it makes.
pub struct Bump {
    /// The version that Go is bumped to.
    #[cfg_attr(tarpaulin_include, allow(dead_code))] // Read only by `apply` and `refresh`.
    version: String,
    /// The edit of `go.mod`.
    edit: Edit,
    /// The default branch that the bump's pull request targets.
    #[cfg_attr(tarpaulin_include, allow(dead_code))] // Read only by `apply`.
    base: String,
}

/// Prepares a bump of Go in the `go` directive of `go.mod` to the given
/// target, which is either a version or one of [`TAGS`], without changing
/// anything yet.
///
/// A version with only a major, or a major and minor, such as `1` or `1.28`,
/// is resolved to the newest stable release matching it.
///
/// If no target is given, Go is bumped to the newest patch release of the
/// minor version it's already on in `go.mod`, since the `go` directive is also
/// the minimum Go version every user of the module needs, so raising the minor
/// version is left to be asked for.
///
/// If the repository is not on its default branch or its working tree is not
/// clean, it asks whether to switch to the default branch or discard the
/// changes first, unless `yes` is set.
///
/// Returns `None`, showing that Go is up to date, if it's already at the
/// version the target resolves to.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bump, if the
/// target is not a released version or a known tag, or if Go can't be bumped
/// to it, such as when it's already at a newer version.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub fn prepare(target: Option<&str>, yes: bool) -> Result<Option<Bump>> {
    println!("checking that the repository is ready");
    let base = tool::prepare_repository(yes)?;
    tool::check_paths(&["go.mod"])?;
    let go_mod = tool::read("go.mod")?;
    let version = match target {
        Some(target) => resolve_version(target)?,
        None => resolve_version(&current_minor(&go_mod)?)?,
    };
    let current = current_version(&go_mod)?;
    if tool::compare_versions(&version, current).is_eq() {
        println!("Go is up to date at version {current}");
        return Ok(None);
    }
    Bump::new(&go_mod, version, base).map(Some)
}

/// Finds whether Go in the given `go.mod` file is behind the newest patch
/// release of the minor version it's already on, showing it if it's already
/// up to date.
///
/// Returns a bump of it to that release, with a pull request against the given
/// base branch, or `None` if it's up to date or if `go.mod` has no `go`
/// directive.
///
/// # Errors
///
/// Returns an error if the version of Go can't be read or resolved.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub(crate) fn find_outdated(go_mod: &str, base: &str) -> Result<Option<Bump>> {
    if !declares_go(go_mod) {
        return Ok(None);
    }
    let current = current_version(go_mod)?;
    let version = resolve_version(&current_minor(go_mod)?)?;
    if tool::compare_versions(&version, current).is_le() {
        println!("Go is up to date at version {current}");
        return Ok(None);
    }
    Bump::new(go_mod, version, base.to_string()).map(Some)
}

/// Resolves the target of a bump to the version to bump Go to, where the
/// target is a version, a version with only a major or a major and minor, or
/// one of [`TAGS`].
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
fn resolve_version(target: &str) -> Result<String> {
    if tool::is_partial_version(target) {
        println!("finding the newest {target}.x version of Go");
        let version = newest_version(&fetch_releases()?, target)?;
        println!("found Go {version}");
        return Ok(version);
    }

    tool::resolve_target(
        "Go",
        target,
        &TAGS,
        |version| check_released(&fetch_releases()?, version),
        |_| latest_version(&fetch_releases()?),
    )
}

impl Bump {
    /// Prepares a bump of Go to the given version, editing the given `go.mod`
    /// file, with a pull request against the given base branch.
    pub(crate) fn new(go_mod: &str, version: String, base: String) -> Result<Self> {
        let edit = bump_go_mod(go_mod, &version)?;
        Ok(Self {
            version,
            edit,
            base,
        })
    }

    /// Prepares the bump again from the latest default branch, editing
    /// `go.mod` as it is there, so a bump made after another one starts from
    /// the default branch rather than the branch of the one before.
    ///
    /// If the repository is not on its default branch or its working tree is
    /// not clean, it asks whether to switch to the default branch or discard
    /// the changes first, unless `yes` is set.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository is not ready for the bump, or if Go
    /// can no longer be bumped to the version.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn refresh(self, yes: bool) -> Result<Self> {
        println!("checking that the repository is ready");
        let base = tool::prepare_repository(yes)?;
        tool::check_paths(&["go.mod"])?;
        Self::new(&tool::read("go.mod")?, self.version, base)
    }

    /// Returns the change that the bump makes, naming the directive and file
    /// it edits along with the values it changes from and to.
    #[must_use]
    pub fn change(&self) -> String {
        format!(
            "go directive in go.mod: {} -> {}",
            self.edit.from, self.edit.to
        )
    }

    /// Applies the bump, then commits the change to a new branch, pushes it,
    /// and opens a pull request for it, or updates the one already open.
    ///
    /// Returns the pull request, which is the one already open for the bump if
    /// there is one.
    ///
    /// # Errors
    ///
    /// Returns an error if any step of it fails.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn apply(self, options: tool::CommitOptions) -> Result<PullRequest> {
        let branch = tool::branch_name("go", &self.version);
        println!("creating branch {branch}");
        git::create_branch(&branch)?;
        println!("updating {}", self.change());
        tool::write("go.mod", &self.edit.content)?;
        println!("committing go.mod");
        git::commit(
            &tool::commit_message("Go", &self.version),
            &["go.mod"],
            options,
        )?;
        println!("pushing {branch} to origin");
        git::push(&branch)?;

        tool::open_or_update_pull_request(
            &branch,
            &self.base,
            "Go",
            &self.version,
            &release_url(&self.version),
        )
    }
}

/// Returns the URL of the entry for the given version of Go in its release
/// history, which summarizes every release, including patch releases, unlike
/// the tags of its GitHub repository, which have no release notes.
fn release_url(version: &str) -> String {
    format!("https://go.dev/doc/devel/release#go{version}")
}

/// Returns the edit of a `go.mod` file with Go bumped to the given version in
/// its `go` directive, editing only the version text to keep the rest of the
/// file as is.
///
/// A `toolchain` directive that names a Go release no newer than the given
/// version is removed, since the `go` directive then already asks for at least
/// that toolchain, as Go itself does when it updates the `go` directive.
fn bump_go_mod(go_mod: &str, version: &str) -> Result<Edit> {
    let current = current_version(go_mod)?;
    tool::check_newer("Go", current, version)?;

    let content = go_mod
        .split_inclusive('\n')
        .filter_map(|line| {
            if directive(line, "go").is_some() {
                return Some(line.replacen(current, version, 1));
            }
            let outdated_toolchain = directive(line, "toolchain")
                .and_then(|toolchain| toolchain.strip_prefix("go"))
                .is_some_and(|toolchain| {
                    tool::is_version(toolchain)
                        && tool::compare_versions(toolchain, version).is_le()
                });
            (!outdated_toolchain).then(|| line.to_string())
        })
        .collect();
    Ok(Edit {
        content,
        from: current.to_string(),
        to: version.to_string(),
    })
}

/// Returns the value of a line of a `go.mod` file if the line is the
/// single-value directive with the given name, such as `1.27.2` for the `go`
/// directive in `go 1.27.2`, ignoring a trailing comment.
fn directive<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let line = line.split("//").next().unwrap_or_default();
    let mut words = line.split_whitespace();
    if words.next()? != name {
        return None;
    }
    let value = words.next()?;
    words.next().is_none().then_some(value)
}

/// Returns whether a `go.mod` file has a `go` directive.
fn declares_go(go_mod: &str) -> bool {
    go_mod.lines().any(|line| directive(line, "go").is_some())
}

/// Returns the version of Go in the `go` directive of a `go.mod` file, such as
/// `1.27.2` for `go 1.27.2`.
fn current_version(go_mod: &str) -> Result<&str> {
    let versions: Vec<&str> = go_mod
        .lines()
        .filter_map(|line| directive(line, "go"))
        .collect();
    let [version] = versions[..] else {
        bail!(
            "expected exactly one go directive in go.mod, found {}",
            versions.len()
        );
    };
    if !tool::is_version(version) {
        bail!("go directive in go.mod is not a plain version: {version}");
    }
    Ok(version)
}

/// Returns the minor version of Go in the `go` directive of a `go.mod` file,
/// such as `1.27` for `go 1.27.2`.
fn current_minor(go_mod: &str) -> Result<String> {
    let current = current_version(go_mod)?;
    Ok(current.split('.').take(2).collect::<Vec<_>>().join("."))
}

/// Fetches the list of Go releases, listing every release from the newest,
/// including unstable ones and those no longer supported.
#[cfg(not(tarpaulin_include))] // Sends a request, so it is not unit tested.
fn fetch_releases() -> Result<String> {
    let url = "https://go.dev/dl/?mode=json&include=all";
    tool::http_agent()
        .get(url)
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .with_context(|| format!("failed to request {url}"))
}

/// Parses the list of Go releases.
fn parse_releases(releases: &str) -> Result<Vec<serde_json::Value>> {
    let releases: serde_json::Value =
        serde_json::from_str(releases).context("failed to parse the list of Go releases")?;
    let serde_json::Value::Array(releases) = releases else {
        bail!("the list of Go releases is not an array");
    };
    Ok(releases)
}

/// Returns the plain versions of the stable releases in the list of Go
/// releases, without their `go` prefix.
///
/// Releases without a plain version, such as release candidates, are skipped,
/// since they are never something to bump to.
fn stable_versions(releases: &str) -> Result<Vec<String>> {
    Ok(parse_releases(releases)?
        .iter()
        .filter(|release| release.get("stable") == Some(&serde_json::Value::Bool(true)))
        .filter_map(|release| release.get("version")?.as_str()?.strip_prefix("go"))
        .filter(|version| tool::is_version(version))
        .map(str::to_string)
        .collect())
}

/// Checks that a version of Go is a stable release in the list of Go
/// releases.
fn check_released(releases: &str, version: &str) -> Result<()> {
    if !stable_versions(releases)?
        .iter()
        .any(|stable| stable == version)
    {
        bail!("Go {version} is not released: it's not in the list of Go releases");
    }
    Ok(())
}

/// Returns the newest stable version of Go in the list of Go releases that
/// matches a partial version on whole parts, so `1.2` matches `1.2.2` but not
/// `1.27.4`.
fn newest_version(releases: &str, partial: &str) -> Result<String> {
    let prefix = format!("{partial}.");
    stable_versions(releases)?
        .into_iter()
        .filter(|version| version.starts_with(&prefix))
        .max_by(|a, b| tool::compare_versions(a, b))
        .with_context(|| format!("no {partial}.x release is found in the list of Go releases"))
}

/// Returns the newest stable version of Go in the list of Go releases.
fn latest_version(releases: &str) -> Result<String> {
    stable_versions(releases)?
        .into_iter()
        .max_by(|a, b| tool::compare_versions(a, b))
        .context("no latest release is found in the list of Go releases")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn go_mod(version: &str) -> String {
        format!(
            "module github.com/threeal/go-starter\n\ngo {version}\n\nrequire github.com/stretchr/testify v1.11.1\n"
        )
    }

    #[test]
    fn release_url_of_version() {
        assert_eq!(
            release_url("1.27.4"),
            "https://go.dev/doc/devel/release#go1.27.4"
        );
    }

    #[test]
    fn bump_go_mod_version() {
        let edit = bump_go_mod(&go_mod("1.27.2"), "1.27.4").unwrap();
        assert_eq!(edit.content, go_mod("1.27.4"));
        assert_eq!(edit.from, "1.27.2");
        assert_eq!(edit.to, "1.27.4");
    }

    #[test]
    fn bump_go_mod_with_comment() {
        let go_mod = "module example.com/foo\n\ngo 1.27.2 // keep 1.27.2 for now\n";
        assert_eq!(
            bump_go_mod(go_mod, "1.27.4").unwrap().content,
            "module example.com/foo\n\ngo 1.27.4 // keep 1.27.2 for now\n"
        );
    }

    #[test]
    fn bump_go_mod_toolchain() {
        let cases = [
            ("go1.27.3", false),
            ("go1.27.4", false),
            ("go1.28.0", true),
            ("default", true),
            ("go1.27.3-custom", true),
        ];
        for (toolchain, kept) in cases {
            let go_mod = format!("go 1.27.2\ntoolchain {toolchain}\n");
            let expected = if kept {
                format!("go 1.27.4\ntoolchain {toolchain}\n")
            } else {
                "go 1.27.4\n".to_string()
            };
            assert_eq!(
                bump_go_mod(&go_mod, "1.27.4").unwrap().content,
                expected,
                "{toolchain}"
            );
        }
    }

    #[test]
    fn bump_change() {
        let bump = Bump::new(&go_mod("1.27.2"), "1.27.4".to_string(), "main".to_string()).unwrap();
        assert_eq!(bump.change(), "go directive in go.mod: 1.27.2 -> 1.27.4");
    }

    #[test]
    fn bump_go_mod_same_version() {
        assert_eq!(
            bump_go_mod(&go_mod("1.27.2"), "1.27.2")
                .unwrap_err()
                .to_string(),
            "Go is already at version 1.27.2"
        );
    }

    #[test]
    fn bump_go_mod_older_version() {
        assert_eq!(
            bump_go_mod(&go_mod("1.28.0"), "1.27.4")
                .unwrap_err()
                .to_string(),
            "Go is already at version 1.28.0, newer than 1.27.4"
        );
    }

    #[test]
    fn directive_valid_or_not() {
        let cases = [
            ("go 1.27.2", "go", Some("1.27.2")),
            ("\tgo  1.27.2 // comment\n", "go", Some("1.27.2")),
            ("toolchain go1.27.4", "toolchain", Some("go1.27.4")),
            ("toolchain go1.27.4", "go", None),
            ("go", "go", None),
            ("go 1.27.2 1.27.3", "go", None),
            ("// go 1.27.2", "go", None),
            ("require example.com/go v1.0.0", "go", None),
            ("", "go", None),
        ];
        for (line, name, value) in cases {
            assert_eq!(directive(line, name), value, "{line}");
        }
    }

    #[test]
    fn declares_go_or_not() {
        assert!(declares_go(&go_mod("1.27.2")));
        assert!(!declares_go("module example.com/foo\n"));
    }

    #[test]
    fn current_version_valid() {
        let cases = [("1.27.2", "1.27.2"), ("1.20", "1.20")];
        for (version, current) in cases {
            assert_eq!(current_version(&go_mod(version)).unwrap(), current);
        }
    }

    #[test]
    fn current_version_invalid() {
        let cases = [
            (
                "module example.com/foo\n".to_string(),
                "expected exactly one go directive in go.mod, found 0",
            ),
            (
                "go 1.27.2\ngo 1.27.3\n".to_string(),
                "expected exactly one go directive in go.mod, found 2",
            ),
            (
                go_mod("1.28rc1"),
                "go directive in go.mod is not a plain version: 1.28rc1",
            ),
        ];
        for (go_mod, error) in cases {
            assert_eq!(current_version(&go_mod).unwrap_err().to_string(), error);
        }
    }

    #[test]
    fn current_minor_valid() {
        let cases = [("1.27.2", "1.27"), ("1.20", "1.20"), ("1", "1")];
        for (version, minor) in cases {
            assert_eq!(current_minor(&go_mod(version)).unwrap(), minor);
        }
    }

    #[test]
    fn current_minor_invalid() {
        assert_eq!(
            current_minor("module example.com/foo\n")
                .unwrap_err()
                .to_string(),
            "expected exactly one go directive in go.mod, found 0"
        );
    }

    const RELEASES: &str = r#"[
  { "version": "go1.28rc1", "stable": false },
  { "version": "go1.27.4", "stable": true },
  { "version": "go1.27.3", "stable": true },
  { "version": "go1.27.5", "stable": false },
  { "stable": true },
  { "version": "1.26.9", "stable": true },
  { "version": "go1.2.2", "stable": true },
  { "version": "go1.2", "stable": true }
]"#;

    #[test]
    fn check_released_valid() {
        check_released(RELEASES, "1.27.3").unwrap();
    }

    #[test]
    fn check_released_invalid() {
        let cases = [
            ("{", "1.27.4", "failed to parse the list of Go releases"),
            ("{}", "1.27.4", "the list of Go releases is not an array"),
            (
                RELEASES,
                "1.27.5",
                "Go 1.27.5 is not released: it's not in the list of Go releases",
            ),
            (
                RELEASES,
                "1.26.9",
                "Go 1.26.9 is not released: it's not in the list of Go releases",
            ),
        ];
        for (releases, version, error) in cases {
            assert_eq!(
                check_released(releases, version).unwrap_err().to_string(),
                error
            );
        }
    }

    #[test]
    fn newest_version_valid() {
        let cases = [("1.27", "1.27.4"), ("1.2", "1.2.2"), ("1", "1.27.4")];
        for (partial, version) in cases {
            assert_eq!(newest_version(RELEASES, partial).unwrap(), version);
        }
    }

    #[test]
    fn newest_version_invalid() {
        let cases = [
            ("{", "1.27", "failed to parse the list of Go releases"),
            ("{}", "1.27", "the list of Go releases is not an array"),
            (
                RELEASES,
                "1.28",
                "no 1.28.x release is found in the list of Go releases",
            ),
            (
                RELEASES,
                "1.26",
                "no 1.26.x release is found in the list of Go releases",
            ),
        ];
        for (releases, partial, error) in cases {
            assert_eq!(
                newest_version(releases, partial).unwrap_err().to_string(),
                error
            );
        }
    }

    #[test]
    fn latest_version_valid() {
        assert_eq!(latest_version(RELEASES).unwrap(), "1.27.4");
    }

    #[test]
    fn latest_version_invalid() {
        let cases = [
            ("{", "failed to parse the list of Go releases"),
            ("{}", "the list of Go releases is not an array"),
            (
                r#"[{ "version": "go1.28rc1", "stable": false }]"#,
                "no latest release is found in the list of Go releases",
            ),
        ];
        for (releases, error) in cases {
            assert_eq!(latest_version(releases).unwrap_err().to_string(), error);
        }
    }
}
