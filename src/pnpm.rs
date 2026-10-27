//! Bumping the version of pnpm in the `devEngines.packageManager` or
//! `packageManager` field of a `package.json` file.

use crate::tool::{self, replace_once};
use crate::{command, git};
use anyhow::{Context, Result, bail};

/// The field of `package.json` that declares pnpm.
#[derive(Debug, PartialEq, Eq)]
enum Field {
    /// The `devEngines.packageManager` field, which pnpm also records in
    /// `pnpm-lock.yaml`.
    DevEngines,
    /// The `packageManager` field.
    PackageManager,
}

/// Bumps pnpm in `package.json` to the given version, then commits the change
/// to a new branch, pushes it, and opens a pull request for it.
///
/// pnpm is bumped in the `devEngines.packageManager` field, with
/// `pnpm-lock.yaml` updated to match, or in the `packageManager` field if the
/// former is not set.
///
/// Returns the URL of the opened pull request.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bump, or if any
/// step of it fails.
#[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
pub fn bump(version: &str) -> Result<String> {
    let (content, field) = bump_package_json(&tool::read("package.json")?, version)?;
    let paths: &[&str] = match field {
        Field::DevEngines => &["package.json", "pnpm-lock.yaml"],
        Field::PackageManager => &["package.json"],
    };
    tool::check_ready(paths)?;

    let branch = tool::branch_name("pnpm", version);
    git::create_branch(&branch)?;
    tool::write("package.json", &content)?;
    if field == Field::DevEngines {
        command::run("pnpm", &["install", "--lockfile-only"])?;
    }
    git::commit(&tool::commit_message("pnpm", version), paths)?;
    git::push(&branch)?;

    tool::open_pull_request("pnpm", "pnpm/pnpm", "v", version)
}

/// Returns the content of a `package.json` file with pnpm bumped to the given
/// version, along with the field it was bumped in, editing only the version
/// text to keep the rest of the file as is.
fn bump_package_json(package_json: &str, version: &str) -> Result<(String, Field)> {
    let value: serde_json::Value =
        serde_json::from_str(package_json).context("failed to parse package.json")?;

    if let Some(package_manager) = value.pointer("/devEngines/packageManager") {
        let range = read_dev_engines_range(package_manager)?;
        let (prefix, current) = tool::split_range(range, "devEngines.packageManager.version")?;
        if current == version {
            bail!("pnpm is already at version {version}");
        }

        let content = replace_once(
            package_json,
            &format!("\"{range}\""),
            &format!("\"{prefix}{version}\""),
        )?;
        return Ok((content, Field::DevEngines));
    }

    let Some(package_manager) = value.get("packageManager") else {
        bail!("neither devEngines.packageManager nor packageManager is set in package.json");
    };
    let current = read_package_manager_version(package_manager)?;
    if current == version {
        bail!("pnpm is already at version {version}");
    }

    let content = replace_once(
        package_json,
        &format!("\"pnpm@{current}\""),
        &format!("\"pnpm@{version}\""),
    )?;
    Ok((content, Field::PackageManager))
}

/// Reads the pnpm version range from the value of the
/// `devEngines.packageManager` field.
fn read_dev_engines_range(package_manager: &serde_json::Value) -> Result<&str> {
    let Some(package_manager) = package_manager.as_object() else {
        bail!("devEngines.packageManager is not an object");
    };

    if package_manager
        .get("name")
        .and_then(serde_json::Value::as_str)
        != Some("pnpm")
    {
        bail!("devEngines.packageManager is not pnpm");
    }

    package_manager
        .get("version")
        .and_then(serde_json::Value::as_str)
        .context("devEngines.packageManager.version is not set")
}

/// Reads the pnpm version from the value of the `packageManager` field.
fn read_package_manager_version(package_manager: &serde_json::Value) -> Result<&str> {
    let Some(package_manager) = package_manager.as_str() else {
        bail!("packageManager is not a string");
    };

    let Some(version) = package_manager.strip_prefix("pnpm@") else {
        bail!("packageManager is not pnpm: {package_manager}");
    };
    if version.contains('+') {
        bail!("packageManager with a hash is not supported: {package_manager}");
    }

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev_engines_package_json(range: &str) -> String {
        format!(
            "{{\n  \"devEngines\": {{\n    \"packageManager\": {{\n      \"name\": \"pnpm\",\n      \"version\": \"{range}\",\n      \"onFail\": \"download\"\n    }}\n  }}\n}}\n"
        )
    }

    #[test]
    fn bump_dev_engines_version() {
        for prefix in ["^", "~", ""] {
            assert_eq!(
                bump_package_json(
                    &dev_engines_package_json(&format!("{prefix}11.28.7")),
                    "12.15.0"
                )
                .unwrap(),
                (
                    dev_engines_package_json(&format!("{prefix}12.15.0")),
                    Field::DevEngines
                )
            );
        }
    }

    #[test]
    fn bump_dev_engines_over_package_manager() {
        let package_json = r#"{ "packageManager": "pnpm@11.28.7", "devEngines": { "packageManager": { "name": "pnpm", "version": "^11.28.7" } } }"#;
        assert_eq!(
            bump_package_json(package_json, "12.15.0").unwrap(),
            (
                package_json.replace("^11.28.7", "^12.15.0"),
                Field::DevEngines
            )
        );
    }

    #[test]
    fn bump_dev_engines_same_version() {
        assert_eq!(
            bump_package_json(&dev_engines_package_json("^11.28.7"), "11.28.7")
                .unwrap_err()
                .to_string(),
            "pnpm is already at version 11.28.7"
        );
    }

    #[test]
    fn bump_dev_engines_complex_range() {
        for range in [">=11 <12", "^~11.28.7", "11.x"] {
            assert_eq!(
                bump_package_json(&dev_engines_package_json(range), "12.15.0")
                    .unwrap_err()
                    .to_string(),
                format!("devEngines.packageManager.version is not a simple range: {range}")
            );
        }
    }

    #[test]
    fn bump_package_manager_version() {
        let package_json = "{\n  \"name\": \"foo\",\n  \"packageManager\": \"pnpm@11.28.7\"\n}\n";
        assert_eq!(
            bump_package_json(package_json, "11.28.8").unwrap(),
            (
                "{\n  \"name\": \"foo\",\n  \"packageManager\": \"pnpm@11.28.8\"\n}\n".to_string(),
                Field::PackageManager
            )
        );
    }

    #[test]
    fn bump_package_manager_same_version() {
        let package_json = r#"{ "packageManager": "pnpm@11.28.7" }"#;
        assert_eq!(
            bump_package_json(package_json, "11.28.7")
                .unwrap_err()
                .to_string(),
            "pnpm is already at version 11.28.7"
        );
    }

    #[test]
    fn bump_package_manager_ambiguous_version() {
        let package_json =
            r#"{ "packageManager": "pnpm@11.28.7", "scripts": { "setup": "pnpm@11.28.7" } }"#;
        assert_eq!(
            bump_package_json(package_json, "11.28.8")
                .unwrap_err()
                .to_string(),
            "expected exactly one \"pnpm@11.28.7\", found 2"
        );
    }

    #[test]
    fn bump_package_json_invalid() {
        let cases = [
            ("{", "failed to parse package.json"),
            (
                r#"{ "name": "foo" }"#,
                "neither devEngines.packageManager nor packageManager is set in package.json",
            ),
            (
                r#"{ "devEngines": { "packageManager": [] } }"#,
                "devEngines.packageManager is not an object",
            ),
            (
                r#"{ "devEngines": { "packageManager": { "name": "yarn" } } }"#,
                "devEngines.packageManager is not pnpm",
            ),
            (
                r#"{ "devEngines": { "packageManager": { "name": "pnpm" } } }"#,
                "devEngines.packageManager.version is not set",
            ),
            (
                r#"{ "packageManager": 1 }"#,
                "packageManager is not a string",
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
            assert_eq!(
                bump_package_json(package_json, "12.15.0")
                    .unwrap_err()
                    .to_string(),
                error
            );
        }
    }
}
