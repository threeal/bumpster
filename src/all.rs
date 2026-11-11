//! Bumping every supported tool in a repository that is behind, each to the
//! version its own bump defaults to.

use crate::tool::{self, PullRequest};
use crate::{dprint, node, pnpm};
use anyhow::Result;

/// A bump of any supported tool.
pub enum Bump {
    /// A bump of a dprint plugin.
    Dprint(dprint::Bump<'static>),
    /// A bump of Node.js.
    Node(node::Bump),
    /// A bump of pnpm.
    Pnpm(pnpm::Bump),
}

/// Prepares a bump of every supported tool in the repository that is behind
/// the version its own bump defaults to, without changing anything yet,
/// showing each one that is already up to date.
///
/// Tools the repository doesn't use are skipped without showing anything.
///
/// If the repository is not on the main branch or its working tree is not
/// clean, it asks whether to switch to the main branch or discard the changes
/// first, unless `yes` is set.
///
/// Returns the bumps of dprint plugins, Node.js, then pnpm, which is empty if
/// every tool is up to date.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bumps, or if a file
/// that declares a tool can't be read or the version of a tool can't be read
/// or resolved.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub fn prepare_outdated(yes: bool) -> Result<Vec<Bump>> {
    println!("checking that the repository is ready");
    tool::prepare_repository(yes)?;
    let mut bumps = Vec::new();
    if let Some(dprint_json) = tool::read_if_exists("dprint.json")? {
        let outdated = dprint::find_outdated(&dprint_json)?;
        bumps.extend(outdated.into_iter().map(Bump::Dprint));
    }
    if let Some(package_json) = tool::read_if_exists("package.json")? {
        bumps.extend(node::find_outdated(&package_json)?.map(Bump::Node));
        bumps.extend(pnpm::find_outdated(&package_json)?.map(Bump::Pnpm));
    }
    Ok(bumps)
}

impl Bump {
    /// Returns the change that the bump makes, as the bump of its tool does.
    #[must_use]
    pub fn change(&self) -> String {
        match self {
            Self::Dprint(bump) => bump.change(),
            Self::Node(bump) => bump.change(),
            Self::Pnpm(bump) => bump.change(),
        }
    }

    /// Prepares the bump again from the latest main branch, as the bump of its
    /// tool does.
    ///
    /// # Errors
    ///
    /// Returns an error if the bump of its tool can't be prepared again.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn refresh(self, yes: bool) -> Result<Self> {
        match self {
            Self::Dprint(bump) => bump.refresh(yes).map(Self::Dprint),
            Self::Node(bump) => bump.refresh(yes).map(Self::Node),
            Self::Pnpm(bump) => bump.refresh(yes).map(Self::Pnpm),
        }
    }

    /// Applies the bump, as the bump of its tool does.
    ///
    /// # Errors
    ///
    /// Returns an error if any step of it fails.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn apply(self) -> Result<PullRequest> {
        match self {
            Self::Dprint(bump) => bump.apply(),
            Self::Node(bump) => bump.apply(),
            Self::Pnpm(bump) => bump.apply(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                Bump::Dprint(dprint::Bump::new(plugin, dprint_json, "0.10.0".to_string()).unwrap()),
                "dprint-plugin-toml in dprint.json: 0.9.0 -> 0.10.0",
            ),
            (
                Bump::Node(node::Bump::new(package_json, "24.12.0".to_string()).unwrap()),
                "devEngines.runtime.version in package.json: ^24.11.0 -> ^24.12.0",
            ),
            (
                Bump::Pnpm(pnpm::Bump::new(package_json, "10.19.0".to_string()).unwrap()),
                "devEngines.packageManager.version in package.json: ^10.18.0 -> ^10.19.0",
            ),
        ];
        for (bump, change) in cases {
            assert_eq!(bump.change(), change);
        }
    }
}
