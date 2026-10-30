//! Bumping the version of Node.js in the `devEngines.runtime` field of a
//! `package.json` file.

use crate::tool::{self, Edit, replace_once};
use crate::{command, git};
use anyhow::{Context, Result, bail};

/// The tags that Node.js can be bumped to instead of a version, where the
/// first is the default.
pub const TAGS: [&str; 2] = ["lts", "latest"];

/// The files that a bump of Node.js edits.
const PATHS: [&str; 2] = ["package.json", "pnpm-lock.yaml"];

/// A bump of Node.js, prepared up to the edit of `package.json` that it makes.
pub struct Bump {
    /// The version that Node.js is bumped to.
    #[cfg_attr(tarpaulin_include, allow(dead_code))] // Read only by `apply`.
    version: String,
    /// The edit of `package.json`.
    edit: Edit,
}

/// Prepares a bump of Node.js in the `devEngines.runtime` field of
/// `package.json` to the given target, which is either a version or one of
/// [`TAGS`], without changing anything yet.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bump, if the
/// target is not a released version or a known tag, or if Node.js can't be
/// bumped to it.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub fn prepare(target: &str) -> Result<Bump> {
    println!("checking that the repository is ready");
    tool::check_ready(&PATHS)?;
    let version = tool::resolve_target("Node.js", "nodejs/node", "v", target, &TAGS, |tag| {
        tag_version(&fetch_index()?, tag)
    })?;
    let edit = bump_package_json(&tool::read("package.json")?, &version)?;
    Ok(Bump { version, edit })
}

impl Bump {
    /// Returns the change that the bump makes, naming the field and file it
    /// edits along with the values it changes from and to.
    #[must_use]
    pub fn change(&self) -> String {
        format!(
            "devEngines.runtime.version in package.json: {} -> {}",
            self.edit.from, self.edit.to
        )
    }

    /// Applies the bump, with `pnpm-lock.yaml` updated to match, then commits
    /// the change to a new branch, pushes it, and opens a pull request for it.
    ///
    /// Returns the URL of the opened pull request.
    ///
    /// # Errors
    ///
    /// Returns an error if any step of it fails.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn apply(self) -> Result<String> {
        let branch = tool::branch_name("node", &self.version);
        println!("creating branch {branch}");
        git::create_branch(&branch)?;
        println!("updating {}", self.change());
        tool::write("package.json", &self.edit.content)?;
        println!("updating pnpm-lock.yaml (pnpm install --lockfile-only)");
        command::run("pnpm", &["install", "--lockfile-only"])?;
        println!("committing {}", PATHS.join(" and "));
        git::commit(&tool::commit_message("Node.js", &self.version), &PATHS)?;
        println!("pushing {branch} to origin");
        git::push(&branch)?;

        println!("opening a pull request");
        tool::open_pull_request("Node.js", "nodejs/node", "v", &self.version)
    }
}

/// Returns the edit of a `package.json` file with Node.js bumped to the given
/// version, keeping the range prefix and editing only the version text to keep
/// the rest of the file as is.
fn bump_package_json(package_json: &str, version: &str) -> Result<Edit> {
    let range = read_range(package_json)?;
    let Some((prefix, current)) = tool::split_range(&range) else {
        bail!("devEngines.runtime.version in package.json is not a simple range: {range}");
    };
    tool::check_newer("Node.js", current, version)?;

    let to = format!("{prefix}{version}");
    let content = replace_once(package_json, &format!("\"{range}\""), &format!("\"{to}\""))?;
    Ok(Edit {
        content,
        from: range,
        to,
    })
}

/// Fetches the index of Node.js releases, listing every release from the
/// newest.
#[cfg(not(tarpaulin_include))] // Sends a request, so it is not unit tested.
fn fetch_index() -> Result<String> {
    let url = "https://nodejs.org/dist/index.json";
    ureq::get(url)
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .with_context(|| format!("failed to request {url}"))
}

/// Returns the version of Node.js that a tag refers to in the index of Node.js
/// releases: the newest release for `latest`, or the newest LTS release for
/// `lts`.
fn tag_version(index: &str, tag: &str) -> Result<String> {
    let index: serde_json::Value =
        serde_json::from_str(index).context("failed to parse the index of Node.js releases")?;
    let Some(releases) = index.as_array() else {
        bail!("the index of Node.js releases is not an array");
    };

    let release = releases
        .iter()
        .find(|release| tag == "latest" || release.get("lts").is_some_and(|lts| lts != false))
        .with_context(|| format!("no {tag} release is found in the index of Node.js releases"))?;
    let version = release
        .get("version")
        .and_then(serde_json::Value::as_str)
        .and_then(|version| version.strip_prefix('v'))
        .filter(|version| tool::is_version(version))
        .context("a release has no plain version in the index of Node.js releases")?;

    Ok(version.to_string())
}

/// Reads the Node.js version range from the `devEngines.runtime` field of a
/// `package.json` file.
fn read_range(package_json: &str) -> Result<String> {
    let value: serde_json::Value =
        serde_json::from_str(package_json).context("failed to parse package.json")?;

    let runtime = value
        .pointer("/devEngines/runtime")
        .context("devEngines.runtime is not set in package.json")?;
    let Some(fields) = runtime.as_object() else {
        bail!("devEngines.runtime in package.json is not an object: {runtime}");
    };

    if fields.get("name").and_then(serde_json::Value::as_str) != Some("node") {
        bail!("devEngines.runtime in package.json is not node: {runtime}");
    }

    let range = fields
        .get("version")
        .and_then(serde_json::Value::as_str)
        .context("devEngines.runtime.version is not set in package.json")?;

    Ok(range.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package_json(range: &str) -> String {
        format!(
            "{{\n  \"engines\": {{\n    \"node\": \">=24\"\n  }},\n  \"devEngines\": {{\n    \"runtime\": {{\n      \"name\": \"node\",\n      \"version\": \"{range}\",\n      \"onFail\": \"download\"\n    }}\n  }}\n}}\n"
        )
    }

    #[test]
    fn bump_package_json_version() {
        for prefix in ["^", "~", ""] {
            let edit =
                bump_package_json(&package_json(&format!("{prefix}24.22.0")), "24.22.1").unwrap();
            assert_eq!(edit.content, package_json(&format!("{prefix}24.22.1")));
            assert_eq!(edit.from, format!("{prefix}24.22.0"));
            assert_eq!(edit.to, format!("{prefix}24.22.1"));
        }
    }

    #[test]
    fn bump_change() {
        let bump = Bump {
            version: "24.22.1".to_string(),
            edit: bump_package_json(&package_json("^24.22.0"), "24.22.1").unwrap(),
        };
        assert_eq!(
            bump.change(),
            "devEngines.runtime.version in package.json: ^24.22.0 -> ^24.22.1"
        );
    }

    #[test]
    fn bump_package_json_same_version() {
        assert_eq!(
            bump_package_json(&package_json("^24.22.0"), "24.22.0")
                .unwrap_err()
                .to_string(),
            "Node.js is already at version 24.22.0"
        );
    }

    #[test]
    fn bump_package_json_older_version() {
        assert_eq!(
            bump_package_json(&package_json("^25.1.0"), "24.22.1")
                .unwrap_err()
                .to_string(),
            "Node.js is already at version 25.1.0, newer than 24.22.1"
        );
    }

    #[test]
    fn bump_package_json_ambiguous_version() {
        let package_json = r#"{ "devEngines": { "runtime": { "name": "node", "version": "^24.22.0" } }, "devDependencies": { "@types/node": "^24.22.0" } }"#;
        assert_eq!(
            bump_package_json(package_json, "24.22.1")
                .unwrap_err()
                .to_string(),
            "expected exactly one \"^24.22.0\", found 2"
        );
    }

    #[test]
    fn bump_package_json_complex_range() {
        for range in [">=24 <25", "^~24.22.0", "^", "24.x"] {
            assert_eq!(
                bump_package_json(&package_json(range), "24.22.1")
                    .unwrap_err()
                    .to_string(),
                format!(
                    "devEngines.runtime.version in package.json is not a simple range: {range}"
                )
            );
        }
    }

    const INDEX: &str = r#"[
  { "version": "v26.1.0", "lts": false },
  { "version": "v24.22.1", "lts": "Krypton" },
  { "version": "v22.30.0", "lts": "Jod" }
]"#;

    #[test]
    fn tag_version_valid() {
        assert_eq!(tag_version(INDEX, "latest").unwrap(), "26.1.0");
        assert_eq!(tag_version(INDEX, "lts").unwrap(), "24.22.1");
    }

    #[test]
    fn tag_version_invalid() {
        let cases = [
            ("{", "lts", "failed to parse the index of Node.js releases"),
            ("{}", "lts", "the index of Node.js releases is not an array"),
            (
                "[]",
                "latest",
                "no latest release is found in the index of Node.js releases",
            ),
            (
                r#"[{ "version": "v26.1.0", "lts": false }]"#,
                "lts",
                "no lts release is found in the index of Node.js releases",
            ),
            (
                r#"[{ "lts": "Krypton" }]"#,
                "lts",
                "a release has no plain version in the index of Node.js releases",
            ),
            (
                r#"[{ "version": "26.1.0" }]"#,
                "latest",
                "a release has no plain version in the index of Node.js releases",
            ),
            (
                r#"[{ "version": "v26.0.0-rc.1" }]"#,
                "latest",
                "a release has no plain version in the index of Node.js releases",
            ),
        ];
        for (index, tag, error) in cases {
            assert_eq!(tag_version(index, tag).unwrap_err().to_string(), error);
        }
    }

    #[test]
    fn read_range_invalid() {
        let cases = [
            ("{", "failed to parse package.json"),
            (
                r#"{ "name": "foo" }"#,
                "devEngines.runtime is not set in package.json",
            ),
            (
                r#"{ "devEngines": { "runtime": [] } }"#,
                "devEngines.runtime in package.json is not an object: []",
            ),
            (
                r#"{ "devEngines": { "runtime": { "name": "bun" } } }"#,
                r#"devEngines.runtime in package.json is not node: {"name":"bun"}"#,
            ),
            (
                r#"{ "devEngines": { "runtime": { "name": "node" } } }"#,
                "devEngines.runtime.version is not set in package.json",
            ),
        ];
        for (package_json, error) in cases {
            assert_eq!(read_range(package_json).unwrap_err().to_string(), error);
        }
    }
}
