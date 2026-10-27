//! Bumping the version of dprint plugins pinned by their URLs in a
//! `dprint.json` file.

use crate::git;
use crate::tool::{self, replace_once};
use anyhow::{Result, bail};

/// A dprint plugin pinned by its URL in `dprint.json`.
pub struct Plugin {
    /// The name of the plugin, used in branch names, commit messages, and
    /// pull requests.
    name: &'static str,
    /// The GitHub repository of the plugin, used to link its release notes.
    repository: &'static str,
    /// The text before the version in the plugin's release tags, such as `v`.
    tag_prefix: &'static str,
    /// The text before the version in the plugin's URL.
    url_prefix: &'static str,
}

/// The supported dprint plugins, each paired with its short name.
pub static PLUGINS: [(&str, Plugin); 4] = [
    (
        "json",
        Plugin {
            name: "dprint-plugin-json",
            repository: "dprint/dprint-plugin-json",
            tag_prefix: "",
            url_prefix: "https://plugins.dprint.dev/json-",
        },
    ),
    (
        "markdown",
        Plugin {
            name: "dprint-plugin-markdown",
            repository: "dprint/dprint-plugin-markdown",
            tag_prefix: "",
            url_prefix: "https://plugins.dprint.dev/markdown-",
        },
    ),
    (
        "toml",
        Plugin {
            name: "dprint-plugin-toml",
            repository: "dprint/dprint-plugin-toml",
            tag_prefix: "",
            url_prefix: "https://plugins.dprint.dev/toml-",
        },
    ),
    (
        "yaml",
        Plugin {
            name: "pretty_yaml",
            repository: "g-plane/pretty_yaml",
            tag_prefix: "v",
            url_prefix: "https://plugins.dprint.dev/g-plane/pretty_yaml-v",
        },
    ),
];

/// Bumps a dprint plugin in the `plugins` field of `dprint.json` to the given
/// version, then commits the change to a new branch, pushes it, and opens a
/// pull request for it.
///
/// Returns the URL of the opened pull request.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bump, or if any
/// step of it fails.
#[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
pub fn bump(plugin: &Plugin, version: &str) -> Result<String> {
    tool::check_ready(&["dprint.json"])?;
    let content = bump_url(&tool::read("dprint.json")?, version, plugin.url_prefix)?;

    let branch = tool::branch_name(plugin.name, version);
    git::create_branch(&branch)?;
    tool::write("dprint.json", &content)?;
    git::commit(
        &tool::commit_message(plugin.name, version),
        &["dprint.json"],
    )?;
    git::push(&branch)?;

    tool::open_pull_request(plugin.name, plugin.repository, plugin.tag_prefix, version)
}

/// Returns the content of a `dprint.json` file with the plugin URL starting
/// with `prefix` bumped to the given version, editing it as text so comments
/// and formatting are kept.
fn bump_url(dprint_json: &str, version: &str, prefix: &str) -> Result<String> {
    let current = read_version(dprint_json, prefix)?;
    if current == version {
        bail!("{prefix}{version}.wasm is already in dprint.json");
    }

    replace_once(
        dprint_json,
        &format!("{prefix}{current}.wasm"),
        &format!("{prefix}{version}.wasm"),
    )
}

/// Reads the version from the plugin URL starting with `prefix` in a
/// `dprint.json` file.
fn read_version<'a>(dprint_json: &'a str, prefix: &str) -> Result<&'a str> {
    let mut parts = dprint_json.split(prefix).skip(1);
    let Some(rest) = parts.next() else {
        bail!("{prefix}<version>.wasm is not found in dprint.json");
    };
    if parts.next().is_some() {
        bail!("expected exactly one {prefix}<version>.wasm in dprint.json");
    }

    let version = rest.split_once(".wasm").map_or("", |(version, _)| version);
    if version.is_empty() || !version.chars().all(|c| c.is_ascii_digit() || c == '.') {
        bail!("{prefix}<version>.wasm has no plain version in dprint.json");
    }

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DPRINT_JSON: &str = r#"{
  // Formats JSON, Markdown, TOML, and YAML files.
  "plugins": [
    "https://plugins.dprint.dev/json-0.25.2.wasm",
    "https://plugins.dprint.dev/markdown-0.28.1.wasm",
    "https://plugins.dprint.dev/toml-0.9.0.wasm",
    "https://plugins.dprint.dev/g-plane/pretty_yaml-v0.6.0.wasm"
  ]
}
"#;

    #[test]
    fn bump_plugins() {
        let cases = [
            ("json", "json-0.25.2", "json-0.25.3"),
            ("markdown", "markdown-0.28.1", "markdown-0.28.3"),
            ("toml", "toml-0.9.0", "toml-0.9.3"),
            ("yaml", "pretty_yaml-v0.6.0", "pretty_yaml-v0.6.3"),
        ];
        for ((name, plugin), (short_name, from, to)) in PLUGINS.iter().zip(cases) {
            assert_eq!(*name, short_name);
            let version = to.rsplit_once('-').unwrap().1.trim_start_matches('v');
            assert_eq!(
                bump_url(DPRINT_JSON, version, plugin.url_prefix).unwrap(),
                DPRINT_JSON.replace(from, to)
            );
        }
    }

    #[test]
    fn bump_url_same_version() {
        assert_eq!(
            bump_url(DPRINT_JSON, "0.25.2", "https://plugins.dprint.dev/json-")
                .unwrap_err()
                .to_string(),
            "https://plugins.dprint.dev/json-0.25.2.wasm is already in dprint.json"
        );
    }

    #[test]
    fn read_version_invalid() {
        let prefix = "https://plugins.dprint.dev/json-";
        let cases = [
            (
                r#"{ "plugins": [] }"#,
                "https://plugins.dprint.dev/json-<version>.wasm is not found in dprint.json",
            ),
            (
                r#"{ "plugins": ["https://plugins.dprint.dev/json-0.25.2.wasm", "https://plugins.dprint.dev/json-0.25.1.wasm"] }"#,
                "expected exactly one https://plugins.dprint.dev/json-<version>.wasm in dprint.json",
            ),
            (
                r#"{ "plugins": ["https://plugins.dprint.dev/json-latest.wasm"] }"#,
                "https://plugins.dprint.dev/json-<version>.wasm has no plain version in dprint.json",
            ),
            (
                r#"{ "plugins": ["https://plugins.dprint.dev/json-0.25.2"] }"#,
                "https://plugins.dprint.dev/json-<version>.wasm has no plain version in dprint.json",
            ),
        ];
        for (dprint_json, error) in cases {
            assert_eq!(
                read_version(dprint_json, prefix).unwrap_err().to_string(),
                error
            );
        }
    }

    #[test]
    fn format_output() {
        let cases = [
            (
                "dprint-plugin-json",
                "https://github.com/dprint/dprint-plugin-json/releases/tag/0.25.3",
            ),
            (
                "dprint-plugin-markdown",
                "https://github.com/dprint/dprint-plugin-markdown/releases/tag/0.25.3",
            ),
            (
                "dprint-plugin-toml",
                "https://github.com/dprint/dprint-plugin-toml/releases/tag/0.25.3",
            ),
            (
                "pretty_yaml",
                "https://github.com/g-plane/pretty_yaml/releases/tag/v0.25.3",
            ),
        ];
        for ((_, plugin), (name, url)) in PLUGINS.iter().zip(cases) {
            assert_eq!(plugin.name, name);
            assert_eq!(
                tool::pull_request_body(
                    plugin.name,
                    plugin.repository,
                    plugin.tag_prefix,
                    "0.25.3"
                ),
                format!("This pull request bumps {name} to version [0.25.3]({url}).")
            );
        }
    }
}
