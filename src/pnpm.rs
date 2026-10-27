//! Bumping the version of pnpm in the `packageManager` field of a
//! `package.json` file.

use crate::git;
use crate::tool::{self, replace_once};
use anyhow::{Context, Result, bail};

/// Bumps pnpm in the `packageManager` field of `package.json` to the given
/// version, then commits and pushes the change to a new branch.
///
/// Returns the title and description of the pull request to open for it.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bump, or if any
/// step of it fails.
#[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
pub fn bump(version: &str) -> Result<String> {
    tool::check_ready(&["package.json"])?;
    let content = bump_package_json(&tool::read("package.json")?, version)?;

    let branch = tool::branch_name("pnpm", version);
    git::create_branch(&branch)?;
    tool::write("package.json", &content)?;
    git::commit(&tool::commit_message("pnpm", version), &["package.json"])?;
    git::push(&branch)?;

    Ok(tool::pull_request("pnpm", "pnpm/pnpm", "v", version))
}

/// Returns the content of a `package.json` file with pnpm bumped to the given
/// version, editing only the version text to keep the rest of the file as is.
fn bump_package_json(package_json: &str, version: &str) -> Result<String> {
    let current = read_version(package_json)?;
    if current == version {
        bail!("pnpm is already at version {version}");
    }

    replace_once(
        package_json,
        &format!("\"pnpm@{current}\""),
        &format!("\"pnpm@{version}\""),
    )
}

/// Reads the pnpm version from the `packageManager` field of a `package.json`
/// file.
fn read_version(package_json: &str) -> Result<String> {
    let value: serde_json::Value =
        serde_json::from_str(package_json).context("failed to parse package.json")?;

    let package_manager = value
        .get("packageManager")
        .and_then(serde_json::Value::as_str)
        .context("packageManager is not set in package.json")?;

    let Some(version) = package_manager.strip_prefix("pnpm@") else {
        bail!("packageManager is not pnpm: {package_manager}");
    };
    if version.contains('+') {
        bail!("packageManager with a hash is not supported: {package_manager}");
    }

    Ok(version.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_package_json_version() {
        let package_json = "{\n  \"name\": \"foo\",\n  \"packageManager\": \"pnpm@11.28.7\"\n}\n";
        assert_eq!(
            bump_package_json(package_json, "11.28.8").unwrap(),
            "{\n  \"name\": \"foo\",\n  \"packageManager\": \"pnpm@11.28.8\"\n}\n"
        );
    }

    #[test]
    fn bump_package_json_same_version() {
        let package_json = r#"{ "packageManager": "pnpm@11.28.7" }"#;
        assert_eq!(
            bump_package_json(package_json, "11.28.7")
                .unwrap_err()
                .to_string(),
            "pnpm is already at version 11.28.7"
        );
    }

    #[test]
    fn bump_package_json_ambiguous_version() {
        let package_json =
            r#"{ "packageManager": "pnpm@11.28.7", "devEngines": { "pnpm": "pnpm@11.28.7" } }"#;
        assert_eq!(
            bump_package_json(package_json, "11.28.8")
                .unwrap_err()
                .to_string(),
            "expected exactly one \"pnpm@11.28.7\", found 2"
        );
    }

    #[test]
    fn read_version_invalid() {
        let cases = [
            ("{", "failed to parse package.json"),
            (
                r#"{ "name": "foo" }"#,
                "packageManager is not set in package.json",
            ),
            (
                r#"{ "packageManager": 1 }"#,
                "packageManager is not set in package.json",
            ),
            (
                r#"{ "packageManager": "yarn@4.0.0" }"#,
                "packageManager is not pnpm: yarn@4.0.0",
            ),
            (
                r#"{ "packageManager": "pnpm@11.28.7+sha512.abc" }"#,
                "packageManager with a hash is not supported: pnpm@11.28.7+sha512.abc",
            ),
        ];
        for (package_json, error) in cases {
            assert_eq!(read_version(package_json).unwrap_err().to_string(), error);
        }
    }
}
