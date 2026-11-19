//! Bumping every supported tool in a repository that is behind, each to the
//! version its own bump defaults to.

use crate::tool::{self, PullRequest};
use crate::{dprint, go, node, pnpm, sdkman};
use anyhow::Result;

/// A bump of any supported tool.
pub enum Bump {
    /// A bump of a dprint plugin.
    Dprint(dprint::Bump<'static>),
    /// A bump of Go.
    Go(go::Bump),
    /// A bump of Node.js.
    Node(node::Bump),
    /// A bump of pnpm.
    Pnpm(pnpm::Bump),
    /// A bump of a SDKMAN! candidate.
    Sdkman(sdkman::Bump<'static>),
}

/// The bumps of every supported tool in the repository that is behind the
/// version its own bump defaults to.
#[derive(Default)]
pub struct Outdated {
    /// The bumps of dprint plugins, Go, Node.js, pnpm, then SDKMAN!
    /// candidates, which is empty if every tool that wasn't skipped is up to
    /// date.
    pub bumps: Vec<Bump>,
    /// Whether any tool was skipped because finding whether it is behind
    /// failed.
    pub skipped: bool,
}

/// Prepares a bump of every supported tool in the repository that is behind
/// the version its own bump defaults to, without changing anything yet,
/// showing each one that is already up to date.
///
/// Tools the repository doesn't use are skipped without showing anything,
/// while a tool whose file can't be read or whose version can't be read or
/// resolved is skipped with a warning, without stopping the other tools.
///
/// If the repository is not on its default branch or its working tree is not
/// clean, it asks whether to switch to the default branch or discard the
/// changes first, unless `yes` is set.
///
/// Returns the bumps of the tools that are behind, along with whether any tool
/// was skipped.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bumps.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub fn prepare_outdated(yes: bool) -> Result<Outdated> {
    println!("checking that the repository is ready");
    let base = tool::prepare_repository(yes)?;
    let mut outdated = Outdated::default();
    if let Some(dprint_json) = outdated.or_skip("dprint", tool::read_if_exists("dprint.json")) {
        let found = outdated.or_skip("dprint", dprint::find_outdated(&dprint_json, &base));
        outdated.bumps.extend(found.into_iter().map(Bump::Dprint));
    }
    if let Some(go_mod) = outdated.or_skip("Go", tool::read_if_exists("go.mod")) {
        let found = outdated.or_skip("Go", go::find_outdated(&go_mod, &base));
        outdated.bumps.extend(found.map(Bump::Go));
    }
    let package_json = tool::read_if_exists("package.json");
    if let Some(package_json) = outdated.or_skip("Node.js and pnpm", package_json) {
        let found = outdated.or_skip("Node.js", node::find_outdated(&package_json, &base));
        outdated.bumps.extend(found.map(Bump::Node));
        let found = outdated.or_skip("pnpm", pnpm::find_outdated(&package_json, &base));
        outdated.bumps.extend(found.map(Bump::Pnpm));
    }
    if let Some(sdkmanrc) = outdated.or_skip("SDKMAN!", tool::read_if_exists(".sdkmanrc")) {
        let found = outdated.or_skip("SDKMAN!", sdkman::find_outdated(&sdkmanrc, &base));
        outdated.bumps.extend(found.into_iter().map(Bump::Sdkman));
    }
    Ok(outdated)
}

impl Outdated {
    /// Returns what was found for the tools with the given name, or, if
    /// finding it failed, warns that those tools are skipped, records the
    /// skip, and returns nothing found, so the other tools can still be bumped.
    fn or_skip<T: Default>(&mut self, name: &str, found: Result<T>) -> T {
        found.unwrap_or_else(|err| {
            println!("warning: skipping {name}: {err:#}");
            self.skipped = true;
            T::default()
        })
    }
}

impl Bump {
    /// Returns the change that the bump makes, as the bump of its tool does.
    #[must_use]
    pub fn change(&self) -> String {
        match self {
            Self::Dprint(bump) => bump.change(),
            Self::Go(bump) => bump.change(),
            Self::Node(bump) => bump.change(),
            Self::Pnpm(bump) => bump.change(),
            Self::Sdkman(bump) => bump.change(),
        }
    }

    /// Prepares the bump again from the latest default branch, as the bump of
    /// its tool does.
    ///
    /// # Errors
    ///
    /// Returns an error if the bump of its tool can't be prepared again.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn refresh(self, yes: bool) -> Result<Self> {
        match self {
            Self::Dprint(bump) => bump.refresh(yes).map(Self::Dprint),
            Self::Go(bump) => bump.refresh(yes).map(Self::Go),
            Self::Node(bump) => bump.refresh(yes).map(Self::Node),
            Self::Pnpm(bump) => bump.refresh(yes).map(Self::Pnpm),
            Self::Sdkman(bump) => bump.refresh(yes).map(Self::Sdkman),
        }
    }

    /// Applies the bump, committing it with the given options, as the bump of
    /// its tool does.
    ///
    /// # Errors
    ///
    /// Returns an error if any step of it fails.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn apply(self, options: tool::CommitOptions) -> Result<PullRequest> {
        match self {
            Self::Dprint(bump) => bump.apply(options),
            Self::Go(bump) => bump.apply(options),
            Self::Node(bump) => bump.apply(options),
            Self::Pnpm(bump) => bump.apply(options),
            Self::Sdkman(bump) => bump.apply(options),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outdated_or_skip_found() {
        let mut outdated = Outdated::default();
        let found = outdated.or_skip("Node.js", Ok(Some("24.12.0")));
        assert_eq!(found, Some("24.12.0"));
        assert!(!outdated.skipped);
    }

    #[test]
    fn outdated_or_skip_failed() {
        let mut outdated = Outdated::default();
        let found: Result<Option<&str>> = Err(anyhow::anyhow!("failed to request"));
        assert_eq!(outdated.or_skip("Node.js", found), None);
        assert!(outdated.skipped);
    }

    #[test]
    fn bump_change() {
        let (_, plugin) = &dprint::PLUGINS[2];
        let dprint_json = r#"{ "plugins": ["https://plugins.dprint.dev/toml-0.9.0.wasm"] }"#;
        let package_json = r#"{
  "devEngines": {
    "runtime": { "name": "node", "version": "^24.11.0" },
    "packageManager": { "name": "pnpm", "version": "^10.18.0" }
  }
}"#;
        let cases = [
            (
                Bump::Dprint(
                    dprint::Bump::new(
                        plugin,
                        dprint_json,
                        "0.10.0".to_string(),
                        "main".to_string(),
                    )
                    .unwrap(),
                ),
                "dprint-plugin-toml in dprint.json: 0.9.0 -> 0.10.0",
            ),
            (
                Bump::Go(
                    go::Bump::new(
                        "module example.com/foo\n\ngo 1.27.2\n",
                        "1.27.4".to_string(),
                        "main".to_string(),
                    )
                    .unwrap(),
                ),
                "go directive in go.mod: 1.27.2 -> 1.27.4",
            ),
            (
                Bump::Node(
                    node::Bump::new(package_json, "24.12.0".to_string(), "main".to_string())
                        .unwrap(),
                ),
                "devEngines.runtime.version in package.json: ^24.11.0 -> ^24.12.0",
            ),
            (
                Bump::Pnpm(
                    pnpm::Bump::new(package_json, "10.19.0".to_string(), "main".to_string())
                        .unwrap(),
                ),
                "devEngines.packageManager.version in package.json: ^10.18.0 -> ^10.19.0",
            ),
            (
                Bump::Sdkman(
                    sdkman::Bump::new(
                        &sdkman::CANDIDATES[0],
                        "java=25.0.5.0+8-tem\n",
                        "25.0.5.1+2-tem".to_string(),
                        "main".to_string(),
                    )
                    .unwrap(),
                ),
                "java in .sdkmanrc: 25.0.5.0+8-tem -> 25.0.5.1+2-tem",
            ),
        ];
        for (bump, change) in cases {
            assert_eq!(bump.change(), change);
        }
    }
}
