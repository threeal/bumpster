//! Bumping the version of pnpm in the `devEngines.packageManager` or
//! `packageManager` field of a `package.json` file.

use crate::tool::{self, Edit, PullRequest, replace_once};
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

/// The tags that pnpm can be bumped to instead of a version.
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
/// A version with only a major, or a major and minor, such as `10` or `10.18`,
/// is resolved to the newest release matching it.
///
/// If no target is given, pnpm is bumped to the newest release of the major
/// version it's already on in `package.json`.
///
/// Versions and tags are looked up in the npm registry, where pnpm is
/// installed from.
///
/// pnpm is bumped in the `devEngines.packageManager` field, or in the
/// `packageManager` field if the former is not set.
///
/// If the repository is not on the main branch or its working tree is not
/// clean, it asks whether to switch to the main branch or discard the changes
/// first, unless `yes` is set.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bump, if the
/// target is not a released version or a known tag, or if pnpm can't be
/// bumped to it.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub fn prepare(target: Option<&str>, yes: bool) -> Result<Bump> {
    println!("checking that the repository is ready");
    tool::prepare_repository(yes)?;
    let package_json = tool::read("package.json")?;
    let value = parse_package_json(&package_json)?;
    let field = Field::find(&value)?;
    tool::check_paths(field.paths())?;
    let version = match target {
        Some(target) => resolve_version(target)?,
        None => resolve_version(&current_major(&value, &field)?)?,
    };
    let edit = bump_package_json(&package_json, &value, &field, &version)?;
    Ok(Bump {
        version,
        edit,
        field,
    })
}

/// Resolves the target of a bump to the version to bump pnpm to, where the
/// target is a version, a version with only a major or a major and minor, or
/// one of [`TAGS`].
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
fn resolve_version(target: &str) -> Result<String> {
    if tool::is_partial_version(target) {
        println!("finding the newest {target}.x version of pnpm");
        let version = newest_version(&fetch_registry()?, target)?;
        println!("found pnpm {version}");
        return Ok(version);
    }

    tool::resolve_target(
        "pnpm",
        target,
        &TAGS,
        |version| check_published(&fetch_registry()?, version),
        |tag| tag_version(&fetch_registry()?, tag),
    )
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
    /// change to a new branch, pushes it, and opens a pull request for it if
    /// there isn't one already.
    ///
    /// Returns the pull request, which is the one already open for the bump if
    /// there is one, as pushing the branch updated it.
    ///
    /// # Errors
    ///
    /// Returns an error if any step of it fails.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn apply(self) -> Result<PullRequest> {
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

        tool::find_or_open_pull_request(&branch, "pnpm", "pnpm/pnpm", "v", &self.version)
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
            let (prefix, current) = split_dev_engines_range(range)?;
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

/// Returns the major version of pnpm in the given field of a `package.json`
/// file, parsed as `value`, such as `10` for `^10.18.0` or `pnpm@10.18.0`.
fn current_major(value: &serde_json::Value, field: &Field) -> Result<String> {
    let current = match field {
        Field::DevEngines => {
            let range = read_dev_engines_range(&value["devEngines"]["packageManager"])?;
            split_dev_engines_range(range)?.1
        }
        Field::PackageManager => read_package_manager_version(&value["packageManager"])?,
    };
    Ok(current.split('.').next().unwrap_or_default().to_string())
}

/// Splits the pnpm version range from the `devEngines.packageManager` field
/// into its prefix and plain version, as [`tool::split_range`] does.
fn split_dev_engines_range(range: &str) -> Result<(&str, &str)> {
    tool::split_range(range).with_context(|| {
        format!("devEngines.packageManager.version in package.json is not a simple range: {range}")
    })
}

/// Fetches the document of pnpm in the npm registry, in its abbreviated form
/// that lists every published version along with the dist-tags.
#[cfg(not(tarpaulin_include))] // Sends a request, so it is not unit tested.
fn fetch_registry() -> Result<String> {
    let url = "https://registry.npmjs.org/pnpm";
    ureq::get(url)
        .header("Accept", "application/vnd.npm.install-v1+json")
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .with_context(|| format!("failed to request {url}"))
}

/// Parses the document of pnpm in the npm registry.
fn parse_registry(registry: &str) -> Result<serde_json::Value> {
    serde_json::from_str(registry).context("failed to parse the npm registry document of pnpm")
}

/// Returns the `versions` field of the parsed document of pnpm in the npm
/// registry, keyed by every published version.
fn published_versions(
    registry: &serde_json::Value,
) -> Result<&serde_json::Map<String, serde_json::Value>> {
    registry
        .get("versions")
        .and_then(serde_json::Value::as_object)
        .context("the npm registry document of pnpm has no versions")
}

/// Checks that a version of pnpm is published to the npm registry, as listed
/// in the `versions` field of its document.
fn check_published(registry: &str, version: &str) -> Result<()> {
    let registry = parse_registry(registry)?;
    if !published_versions(&registry)?.contains_key(version) {
        bail!("pnpm {version} is not released: it's not in the npm registry");
    }
    Ok(())
}

/// Returns the newest plain version of pnpm in its document in the npm
/// registry that matches a partial version on whole parts, so `1` matches
/// `1.43.1` but not `10.34.9`.
///
/// The newest version is found by comparing every match, since the registry
/// doesn't list versions in version order, and pnpm keeps releasing older
/// majors alongside the newest one.
fn newest_version(registry: &str, partial: &str) -> Result<String> {
    let registry = parse_registry(registry)?;
    let prefix = format!("{partial}.");
    published_versions(&registry)?
        .keys()
        .filter(|version| tool::is_version(version) && version.starts_with(&prefix))
        .max_by(|a, b| tool::compare_versions(a, b))
        .cloned()
        .with_context(|| format!("no {partial}.x version of pnpm is found in the npm registry"))
}

/// Returns the version of pnpm that a tag refers to in the `dist-tags` field
/// of its document in the npm registry, such as the version pnpm marks as
/// `latest`.
fn tag_version(registry: &str, tag: &str) -> Result<String> {
    parse_registry(registry)?
        .get("dist-tags")
        .and_then(|tags| tags.get(tag))
        .and_then(serde_json::Value::as_str)
        .filter(|version| tool::is_version(version))
        .map(str::to_string)
        .with_context(|| format!("no plain {tag} version of pnpm is found in the npm registry"))
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

    const REGISTRY: &str = r#"{
  "name": "pnpm",
  "dist-tags": { "latest": "12.16.1", "next-12": "12.17.0-rc.1" },
  "versions": { "11.28.9": {}, "12.16.1": {}, "12.17.0-rc.1": {} }
}"#;

    #[test]
    fn check_published_valid() {
        for version in ["11.28.9", "12.16.1"] {
            check_published(REGISTRY, version).unwrap();
        }
    }

    #[test]
    fn check_published_invalid() {
        let cases = [
            (
                "{",
                "12.16.1",
                "failed to parse the npm registry document of pnpm",
            ),
            (
                "{}",
                "12.16.1",
                "the npm registry document of pnpm has no versions",
            ),
            (
                REGISTRY,
                "12.16.2",
                "pnpm 12.16.2 is not released: it's not in the npm registry",
            ),
            (
                REGISTRY,
                "12",
                "pnpm 12 is not released: it's not in the npm registry",
            ),
        ];
        for (registry, version, error) in cases {
            assert_eq!(
                check_published(registry, version).unwrap_err().to_string(),
                error
            );
        }
    }

    const VERSIONS: &str = r#"{
  "versions": {
    "11.28.9": {},
    "12.16.1": {},
    "12.9.0": {},
    "12.17.0-rc.1": {},
    "1.43.1": {},
    "10.34.9": {}
  }
}"#;

    #[test]
    fn newest_version_valid() {
        let cases = [("12", "12.16.1"), ("12.9", "12.9.0"), ("1", "1.43.1")];
        for (partial, version) in cases {
            assert_eq!(newest_version(VERSIONS, partial).unwrap(), version);
        }
    }

    #[test]
    fn newest_version_invalid() {
        let cases = [
            (
                "{",
                "12",
                "failed to parse the npm registry document of pnpm",
            ),
            (
                "{}",
                "12",
                "the npm registry document of pnpm has no versions",
            ),
            (
                VERSIONS,
                "13",
                "no 13.x version of pnpm is found in the npm registry",
            ),
            (
                VERSIONS,
                "12.17",
                "no 12.17.x version of pnpm is found in the npm registry",
            ),
        ];
        for (registry, partial, error) in cases {
            assert_eq!(
                newest_version(registry, partial).unwrap_err().to_string(),
                error
            );
        }
    }

    #[test]
    fn tag_version_valid() {
        assert_eq!(tag_version(REGISTRY, "latest").unwrap(), "12.16.1");
    }

    #[test]
    fn tag_version_invalid() {
        let cases = [
            (
                "{",
                "latest",
                "failed to parse the npm registry document of pnpm",
            ),
            (
                "{}",
                "latest",
                "no plain latest version of pnpm is found in the npm registry",
            ),
            (
                r#"{ "dist-tags": { "latest": 12 } }"#,
                "latest",
                "no plain latest version of pnpm is found in the npm registry",
            ),
            (
                REGISTRY,
                "next-12",
                "no plain next-12 version of pnpm is found in the npm registry",
            ),
        ];
        for (registry, tag, error) in cases {
            assert_eq!(tag_version(registry, tag).unwrap_err().to_string(), error);
        }
    }

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

    /// Returns the major version of pnpm in a `package.json` file as `prepare`
    /// finds it.
    fn major(package_json: &str) -> Result<String> {
        let value = parse_package_json(package_json)?;
        let field = Field::find(&value)?;
        current_major(&value, &field)
    }

    #[test]
    fn current_major_valid() {
        let cases = [
            (dev_engines_package_json("^10.18.0"), "10"),
            (dev_engines_package_json("~11.2"), "11"),
            (dev_engines_package_json("12"), "12"),
            (r#"{ "packageManager": "pnpm@10.18.0" }"#.to_string(), "10"),
        ];
        for (package_json, current) in cases {
            assert_eq!(major(&package_json).unwrap(), current);
        }
    }

    #[test]
    fn current_major_invalid() {
        let cases = [
            (
                dev_engines_package_json(">=10 <11"),
                "devEngines.packageManager.version in package.json is not a simple range: >=10 <11",
            ),
            (
                r#"{ "devEngines": { "packageManager": { "name": "yarn" } } }"#.to_string(),
                r#"devEngines.packageManager in package.json is not pnpm: {"name":"yarn"}"#,
            ),
            (
                r#"{ "packageManager": "pnpm@11.0.0-rc.1" }"#.to_string(),
                "packageManager in package.json has no plain version: pnpm@11.0.0-rc.1",
            ),
        ];
        for (package_json, error) in cases {
            assert_eq!(major(&package_json).unwrap_err().to_string(), error);
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
