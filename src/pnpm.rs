//! Bumping the version of pnpm in the `devEngines.packageManager` or
//! `packageManager` field of a `package.json` file.

use crate::tool::{self, Edit, replace_once};
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

impl Field {
    /// Finds the field that declares pnpm in a parsed `package.json` file,
    /// which is `devEngines.packageManager`, or `packageManager` if the former
    /// is not set.
    fn find(package_json: &serde_json::Value) -> Result<Self> {
        if package_json.pointer("/devEngines/packageManager").is_some() {
            return Ok(Self::DevEngines);
        }
        if package_json.get("packageManager").is_some() {
            return Ok(Self::PackageManager);
        }
        bail!("neither devEngines.packageManager nor packageManager is set in package.json");
    }

    /// Returns the name of the field that holds the version.
    const fn name(&self) -> &'static str {
        match self {
            Self::DevEngines => "devEngines.packageManager.version",
            Self::PackageManager => "packageManager",
        }
    }

    /// Returns the files that a bump of pnpm in the field edits.
    const fn paths(&self) -> &'static [&'static str] {
        match self {
            Self::DevEngines => &["package.json", "pnpm-lock.yaml"],
            Self::PackageManager => &["package.json"],
        }
    }
}

/// The tags that pnpm can be bumped to instead of a version, where the first is
/// the default.
pub const TAGS: [&str; 1] = ["latest"];

/// A bump of pnpm, prepared up to the edit of `package.json` that it makes.
pub struct Bump {
    /// The version that pnpm is bumped to.
    #[cfg_attr(tarpaulin_include, allow(dead_code))] // Read only by `apply`.
    version: String,
    /// The edit of `package.json`.
    edit: Edit,
    /// The field of `package.json` that the edit bumps pnpm in.
    field: Field,
}

/// Prepares a bump of pnpm in `package.json` to the given target, which is
/// either a version or one of [`TAGS`], without changing anything yet.
///
/// pnpm is bumped in the `devEngines.packageManager` field, or in the
/// `packageManager` field if the former is not set.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bump, if the
/// target is not a released version or a known tag, or if pnpm can't be
/// bumped to it.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub fn prepare(target: &str) -> Result<Bump> {
    println!("checking that the repository is ready");
    let package_json = tool::read("package.json")?;
    let value = parse_package_json(&package_json)?;
    let field = Field::find(&value)?;
    tool::check_ready(field.paths())?;
    let version = tool::resolve_target("pnpm", "pnpm/pnpm", "v", target, &TAGS, |_| {
        tool::latest_release("pnpm/pnpm", "v")
    })?;
    let edit = bump_package_json(&package_json, &value, &field, &version)?;
    Ok(Bump {
        version,
        edit,
        field,
    })
}

impl Bump {
    /// Returns the change that the bump makes, naming the field and file it
    /// edits along with the values it changes from and to.
    #[must_use]
    pub fn change(&self) -> String {
        format!(
            "{} in package.json: {} -> {}",
            self.field.name(),
            self.edit.from,
            self.edit.to
        )
    }

    /// Applies the bump, with `pnpm-lock.yaml` updated to match if pnpm is
    /// bumped in the `devEngines.packageManager` field, then commits the
    /// change to a new branch, pushes it, and opens a pull request for it.
    ///
    /// Returns the URL of the opened pull request.
    ///
    /// # Errors
    ///
    /// Returns an error if any step of it fails.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn apply(self) -> Result<String> {
        let paths = self.field.paths();
        let branch = tool::branch_name("pnpm", &self.version);
        println!("creating branch {branch}");
        git::create_branch(&branch)?;
        println!("updating {}", self.change());
        tool::write("package.json", &self.edit.content)?;
        if self.field == Field::DevEngines {
            println!("updating pnpm-lock.yaml (pnpm install --lockfile-only)");
            command::run("pnpm", &["install", "--lockfile-only"])?;
        }
        println!("committing {}", paths.join(" and "));
        git::commit(&tool::commit_message("pnpm", &self.version), paths)?;
        println!("pushing {branch} to origin");
        git::push(&branch)?;

        println!("opening a pull request");
        tool::open_pull_request("pnpm", "pnpm/pnpm", "v", &self.version)
    }
}

/// Parses the content of a `package.json` file.
fn parse_package_json(package_json: &str) -> Result<serde_json::Value> {
    serde_json::from_str(package_json).context("failed to parse package.json")
}

/// Returns the edit of a `package.json` file, parsed as `value`, with pnpm
/// bumped to the given version in the given field, editing only the
/// version text to keep the rest of the file as is.
fn bump_package_json(
    package_json: &str,
    value: &serde_json::Value,
    field: &Field,
    version: &str,
) -> Result<Edit> {
    match field {
        Field::DevEngines => {
            let range = read_dev_engines_range(&value["devEngines"]["packageManager"])?;
            let Some((prefix, current)) = tool::split_range(range) else {
                bail!(
                    "devEngines.packageManager.version in package.json is not a simple range: {range}"
                );
            };
            tool::check_newer("pnpm", current, version)?;

            let to = format!("{prefix}{version}");
            let content =
                replace_once(package_json, &format!("\"{range}\""), &format!("\"{to}\""))?;
            Ok(Edit {
                content,
                from: range.to_string(),
                to,
            })
        }
        Field::PackageManager => {
            let current = read_package_manager_version(&value["packageManager"])?;
            tool::check_newer("pnpm", current, version)?;

            let from = format!("pnpm@{current}");
            let to = format!("pnpm@{version}");
            let content = replace_once(package_json, &format!("\"{from}\""), &format!("\"{to}\""))?;
            Ok(Edit { content, from, to })
        }
    }
}

/// Reads the pnpm version range from the value of the
/// `devEngines.packageManager` field.
fn read_dev_engines_range(package_manager: &serde_json::Value) -> Result<&str> {
    let Some(fields) = package_manager.as_object() else {
        bail!("devEngines.packageManager in package.json is not an object: {package_manager}");
    };

    if fields.get("name").and_then(serde_json::Value::as_str) != Some("pnpm") {
        bail!("devEngines.packageManager in package.json is not pnpm: {package_manager}");
    }

    fields
        .get("version")
        .and_then(serde_json::Value::as_str)
        .context("devEngines.packageManager.version is not set in package.json")
}

/// Reads the pnpm version from the value of the `packageManager` field.
fn read_package_manager_version(package_manager: &serde_json::Value) -> Result<&str> {
    let Some(value) = package_manager.as_str() else {
        bail!("packageManager in package.json is not a string: {package_manager}");
    };

    let Some(version) = value.strip_prefix("pnpm@") else {
        bail!("packageManager in package.json is not pnpm: {value}");
    };
    if version.contains('+') {
        bail!("packageManager in package.json has a hash, which is not supported: {value}");
    }
    if !tool::is_version(version) {
        bail!("packageManager in package.json has no plain version: {value}");
    }

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bumps pnpm in a `package.json` file as `prepare` does, returning the
    /// edit along with the field it was bumped in.
    fn bump(package_json: &str, version: &str) -> Result<(Edit, Field)> {
        let value = parse_package_json(package_json)?;
        let field = Field::find(&value)?;
        let edit = bump_package_json(package_json, &value, &field, version)?;
        Ok((edit, field))
    }

    fn dev_engines_package_json(range: &str) -> String {
        format!(
            "{{\n  \"devEngines\": {{\n    \"packageManager\": {{\n      \"name\": \"pnpm\",\n      \"version\": \"{range}\",\n      \"onFail\": \"download\"\n    }}\n  }}\n}}\n"
        )
    }

    #[test]
    fn bump_dev_engines_version() {
        for prefix in ["^", "~", ""] {
            let (edit, field) = bump(
                &dev_engines_package_json(&format!("{prefix}11.28.7")),
                "12.15.0",
            )
            .unwrap();
            assert_eq!(
                edit.content,
                dev_engines_package_json(&format!("{prefix}12.15.0"))
            );
            assert_eq!(edit.from, format!("{prefix}11.28.7"));
            assert_eq!(edit.to, format!("{prefix}12.15.0"));
            assert_eq!(field, Field::DevEngines);
        }
    }

    #[test]
    fn bump_dev_engines_over_package_manager() {
        let package_json = r#"{ "packageManager": "pnpm@11.28.7", "devEngines": { "packageManager": { "name": "pnpm", "version": "^11.28.7" } } }"#;
        let (edit, field) = bump(package_json, "12.15.0").unwrap();
        assert_eq!(edit.content, package_json.replace("^11.28.7", "^12.15.0"));
        assert_eq!(edit.from, "^11.28.7");
        assert_eq!(edit.to, "^12.15.0");
        assert_eq!(field, Field::DevEngines);
    }

    #[test]
    fn bump_change() {
        let package_jsons = [
            (
                dev_engines_package_json("^11.28.7"),
                "devEngines.packageManager.version in package.json: ^11.28.7 -> ^12.15.0",
            ),
            (
                r#"{ "packageManager": "pnpm@11.28.7" }"#.to_string(),
                "packageManager in package.json: pnpm@11.28.7 -> pnpm@12.15.0",
            ),
        ];
        for (package_json, change) in package_jsons {
            let (edit, field) = bump(&package_json, "12.15.0").unwrap();
            let bump = Bump {
                version: "12.15.0".to_string(),
                edit,
                field,
            };
            assert_eq!(bump.change(), change);
        }
    }

    #[test]
    fn field_paths() {
        assert_eq!(
            Field::DevEngines.paths(),
            ["package.json", "pnpm-lock.yaml"]
        );
        assert_eq!(Field::PackageManager.paths(), ["package.json"]);
    }

    #[test]
    fn bump_dev_engines_same_version() {
        assert_eq!(
            bump(&dev_engines_package_json("^11.28.7"), "11.28.7")
                .unwrap_err()
                .to_string(),
            "pnpm is already at version 11.28.7"
        );
    }

    #[test]
    fn bump_dev_engines_older_version() {
        assert_eq!(
            bump(&dev_engines_package_json("^12.15.0"), "11.28.7")
                .unwrap_err()
                .to_string(),
            "pnpm is already at version 12.15.0, newer than 11.28.7"
        );
    }

    #[test]
    fn bump_dev_engines_complex_range() {
        for range in [">=11 <12", "^~11.28.7", "11.x"] {
            assert_eq!(
                bump(&dev_engines_package_json(range), "12.15.0")
                    .unwrap_err()
                    .to_string(),
                format!(
                    "devEngines.packageManager.version in package.json is not a simple range: {range}"
                )
            );
        }
    }

    #[test]
    fn bump_package_manager_version() {
        let package_json = "{\n  \"name\": \"foo\",\n  \"packageManager\": \"pnpm@11.28.7\"\n}\n";
        let (edit, field) = bump(package_json, "11.28.8").unwrap();
        assert_eq!(
            edit.content,
            "{\n  \"name\": \"foo\",\n  \"packageManager\": \"pnpm@11.28.8\"\n}\n"
        );
        assert_eq!(edit.from, "pnpm@11.28.7");
        assert_eq!(edit.to, "pnpm@11.28.8");
        assert_eq!(field, Field::PackageManager);
    }

    #[test]
    fn bump_package_manager_same_version() {
        let package_json = r#"{ "packageManager": "pnpm@11.28.7" }"#;
        assert_eq!(
            bump(package_json, "11.28.7").unwrap_err().to_string(),
            "pnpm is already at version 11.28.7"
        );
    }

    #[test]
    fn bump_package_manager_older_version() {
        let package_json = r#"{ "packageManager": "pnpm@12.15.0" }"#;
        assert_eq!(
            bump(package_json, "11.28.7").unwrap_err().to_string(),
            "pnpm is already at version 12.15.0, newer than 11.28.7"
        );
    }

    #[test]
    fn bump_package_manager_ambiguous_version() {
        let package_json =
            r#"{ "packageManager": "pnpm@11.28.7", "scripts": { "setup": "pnpm@11.28.7" } }"#;
        assert_eq!(
            bump(package_json, "11.28.8").unwrap_err().to_string(),
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
                "devEngines.packageManager in package.json is not an object: []",
            ),
            (
                r#"{ "devEngines": { "packageManager": { "name": "yarn" } } }"#,
                r#"devEngines.packageManager in package.json is not pnpm: {"name":"yarn"}"#,
            ),
            (
                r#"{ "devEngines": { "packageManager": { "name": "pnpm" } } }"#,
                "devEngines.packageManager.version is not set in package.json",
            ),
            (
                r#"{ "packageManager": 1 }"#,
                "packageManager in package.json is not a string: 1",
            ),
            (
                r#"{ "packageManager": "yarn@4.0.0" }"#,
                "packageManager in package.json is not pnpm: yarn@4.0.0",
            ),
            (
                r#"{ "packageManager": "pnpm@11.28.7+sha512.abc" }"#,
                "packageManager in package.json has a hash, which is not supported: pnpm@11.28.7+sha512.abc",
            ),
            (
                r#"{ "packageManager": "pnpm@11.0.0-rc.1" }"#,
                "packageManager in package.json has no plain version: pnpm@11.0.0-rc.1",
            ),
        ];
        for (package_json, error) in cases {
            assert_eq!(
                bump(package_json, "12.15.0").unwrap_err().to_string(),
                error
            );
        }
    }
}
