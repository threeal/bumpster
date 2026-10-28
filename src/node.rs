//! Bumping the version of Node.js in the `devEngines.runtime` field of a
//! `package.json` file.

use crate::tool::{self, Edit, replace_once};
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
/// Returns an error if the repository is not ready for the bump, if the
/// version is not released, or if any step of it fails.
#[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
pub fn bump(version: &str) -> Result<String> {
    let paths = ["package.json", "pnpm-lock.yaml"];
    println!("checking that the repository is ready");
    tool::check_ready(&paths)?;
    println!("checking that Node.js {version} is released");
    tool::check_release("Node.js", "nodejs/node", "v", version)?;
    let edit = bump_package_json(&tool::read("package.json")?, version)?;

    let branch = tool::branch_name("node", version);
    println!("creating branch {branch}");
    git::create_branch(&branch)?;
    println!(
        "updating devEngines.runtime.version in package.json: {} -> {}",
        edit.from, edit.to
    );
    tool::write("package.json", &edit.content)?;
    println!("updating pnpm-lock.yaml (pnpm install --lockfile-only)");
    command::run("pnpm", &["install", "--lockfile-only"])?;
    println!("committing {}", paths.join(" and "));
    git::commit(&tool::commit_message("Node.js", version), &paths)?;
    println!("pushing {branch} to origin");
    git::push(&branch)?;

    println!("opening a pull request");
    tool::open_pull_request("Node.js", "nodejs/node", "v", version)
}

/// Returns the edit of a `package.json` file with Node.js bumped to the given
/// version, keeping the range prefix and editing only the version text to keep
/// the rest of the file as is.
fn bump_package_json(package_json: &str, version: &str) -> Result<Edit> {
    let range = read_range(package_json)?;
    let (prefix, current) = tool::split_range(&range, "devEngines.runtime.version")?;
    if current == version {
        bail!("Node.js is already at version {version}");
    }

    let to = format!("{prefix}{version}");
    let content = replace_once(package_json, &format!("\"{range}\""), &format!("\"{to}\""))?;
    Ok(Edit {
        content,
        from: range,
        to,
    })
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
            let edit =
                bump_package_json(&package_json(&format!("{prefix}24.22.0")), "24.22.1").unwrap();
            assert_eq!(edit.content, package_json(&format!("{prefix}24.22.1")));
            assert_eq!(edit.from, format!("{prefix}24.22.0"));
            assert_eq!(edit.to, format!("{prefix}24.22.1"));
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
