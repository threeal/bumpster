//! Bumping the version of pnpm in the `packageManager` field of a
//! `package.json` file.

use anyhow::{Context, Result, bail};

/// Bumps pnpm in the current repository to the given version, then commits
/// and pushes the change to a new branch.
///
/// Returns the title and description of the pull request to open for it.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bump, or if any
/// step of it fails.
#[cfg(not(tarpaulin_include))] // Runs git commands, so it is not unit tested.
pub fn bump(version: &str) -> Result<String> {
    use crate::git;

    if git::current_branch()? != "main" {
        bail!("not on the main branch");
    }
    if !git::is_clean()? {
        bail!("the working tree is not clean");
    }

    let package_json =
        std::fs::read_to_string("package.json").context("failed to read package.json")?;
    let package_json = bump_package_json(&package_json, version)?;

    let branch = branch_name(version);
    git::create_branch(&branch)?;
    std::fs::write("package.json", package_json).context("failed to write package.json")?;
    git::commit(&commit_message(version), "package.json")?;
    git::push(&branch)?;

    Ok(pull_request(version))
}

/// Returns the content of a `package.json` file with pnpm bumped to the given
/// version, editing only the version text to keep the rest of the file as is.
fn bump_package_json(package_json: &str, version: &str) -> Result<String> {
    let current = read_version(package_json)?;
    if current == version {
        bail!("pnpm is already at version {version}");
    }

    let from = format!("\"pnpm@{current}\"");
    let count = package_json.matches(&from).count();
    if count != 1 {
        bail!("expected exactly one {from} in package.json, found {count}");
    }

    Ok(package_json.replacen(&from, &format!("\"pnpm@{version}\""), 1))
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

fn branch_name(version: &str) -> String {
    format!("bump-pnpm-{version}")
}

fn commit_message(version: &str) -> String {
    format!("bump pnpm to version {version}")
}

fn pull_request(version: &str) -> String {
    format!(
        "Bump pnpm to version {version}\n\nThis pull request bumps pnpm to version [{version}](https://github.com/pnpm/pnpm/releases/tag/v{version})."
    )
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
            "expected exactly one \"pnpm@11.28.7\" in package.json, found 2"
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

    #[test]
    fn format_output() {
        assert_eq!(branch_name("11.28.8"), "bump-pnpm-11.28.8");
        assert_eq!(commit_message("11.28.8"), "bump pnpm to version 11.28.8");
        assert_eq!(
            pull_request("11.28.8"),
            "Bump pnpm to version 11.28.8\n\nThis pull request bumps pnpm to version [11.28.8](https://github.com/pnpm/pnpm/releases/tag/v11.28.8)."
        );
    }
}
