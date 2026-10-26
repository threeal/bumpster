//! Bumping the version of Node.js in the `devEngines.runtime` field of a
//! `package.json` file.

use crate::tool::{Tool, replace_once};
use anyhow::{Context, Result, bail};

/// Node.js, bumped in the `devEngines.runtime` field of `package.json`, with
/// `pnpm-lock.yaml` updated to match.
pub const TOOL: Tool = Tool {
    id: "node",
    name: "Node.js",
    repository: "nodejs/node",
    file: "package.json",
    edit: bump_package_json,
    command: Some(("pnpm", &["install", "--lockfile-only"])),
    paths: &["package.json", "pnpm-lock.yaml"],
};

/// Returns the content of a `package.json` file with Node.js bumped to the
/// given version, keeping the range prefix and editing only the version text
/// to keep the rest of the file as is.
fn bump_package_json(package_json: &str, version: &str) -> Result<String> {
    let range = read_range(package_json)?;
    let (prefix, current) = split_range(&range)?;
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

/// Splits a version range into its prefix (`^`, `~`, or none) and version,
/// refusing anything more complex.
fn split_range(range: &str) -> Result<(&str, &str)> {
    let version = range.trim_start_matches(['^', '~']);
    let prefix = &range[..range.len() - version.len()];

    if prefix.len() > 1
        || version.is_empty()
        || !version.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        bail!("devEngines.runtime.version is not a simple range: {range}");
    }

    Ok((prefix, version))
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

    #[test]
    fn format_output() {
        assert_eq!(TOOL.branch_name("24.22.1"), "bump-node-24.22.1");
        assert_eq!(
            TOOL.commit_message("24.22.1"),
            "bump Node.js to version 24.22.1"
        );
        assert_eq!(
            TOOL.pull_request("24.22.1"),
            "Bump Node.js to version 24.22.1\n\nThis pull request bumps Node.js to version [24.22.1](https://github.com/nodejs/node/releases/tag/v24.22.1)."
        );
    }
}
