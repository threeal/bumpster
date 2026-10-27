//! Bumping the version of Node.js in the `devEngines.runtime` field of a
//! `package.json` file.

use crate::tool::{self, replace_once};
use crate::{command, git};
use anyhow::{Context, Result, bail};

/// Bumps Node.js in the `devEngines.runtime` field of `package.json` to the
/// given version, with `pnpm-lock.yaml` updated to match.
///
/// The change is then committed to a new branch, pushed, and opened as a pull
/// request.
///
/// Returns the URL of the opened pull request.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bump, or if any
/// step of it fails.
#[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
pub fn bump(version: &str) -> Result<String> {
    let paths = ["package.json", "pnpm-lock.yaml"];
    tool::check_ready(&paths)?;
    let content = bump_package_json(&tool::read("package.json")?, version)?;

    let branch = tool::branch_name("node", version);
    git::create_branch(&branch)?;
    tool::write("package.json", &content)?;
    command::run("pnpm", &["install", "--lockfile-only"])?;
    git::commit(&tool::commit_message("Node.js", version), &paths)?;
    git::push(&branch)?;

    tool::open_pull_request("Node.js", "nodejs/node", "v", version)
}

/// Returns the content of a `package.json` file with Node.js bumped to the
/// given version, keeping the range prefix and editing only the version text
/// to keep the rest of the file as is.
fn bump_package_json(package_json: &str, version: &str) -> Result<String> {
    let range = read_range(package_json)?;
    let (prefix, current) = tool::split_range(&range, "devEngines.runtime.version")?;
    if current == version {
        bail!("Node.js is already at version {version}");
    }

    replace_once(
        package_json,
        &format!("\"{range}\""),
        &format!("\"{prefix}{version}\""),
    )
}

/// Reads the Node.js version range from the `devEngines.runtime` field of a
/// `package.json` file.
fn read_range(package_json: &str) -> Result<String> {
    let value: serde_json::Value =
        serde_json::from_str(package_json).context("failed to parse package.json")?;

    let runtime = value
        .pointer("/devEngines/runtime")
        .context("devEngines.runtime is not set in package.json")?;
    let Some(runtime) = runtime.as_object() else {
        bail!("devEngines.runtime is not an object");
    };

    if runtime.get("name").and_then(serde_json::Value::as_str) != Some("node") {
        bail!("devEngines.runtime is not node");
    }

    let range = runtime
        .get("version")
        .and_then(serde_json::Value::as_str)
        .context("devEngines.runtime.version is not set")?;

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
            assert_eq!(
                bump_package_json(&package_json(&format!("{prefix}24.22.0")), "24.22.1").unwrap(),
                package_json(&format!("{prefix}24.22.1"))
            );
        }
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
                format!("devEngines.runtime.version is not a simple range: {range}")
            );
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
                "devEngines.runtime is not an object",
            ),
            (
                r#"{ "devEngines": { "runtime": { "name": "bun" } } }"#,
                "devEngines.runtime is not node",
            ),
            (
                r#"{ "devEngines": { "runtime": { "name": "node" } } }"#,
                "devEngines.runtime.version is not set",
            ),
        ];
        for (package_json, error) in cases {
            assert_eq!(read_range(package_json).unwrap_err().to_string(), error);
        }
    }
}
