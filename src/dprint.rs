//! Bumping the version of dprint plugins pinned by their URLs in a
//! `dprint.json` file.

use crate::tool::{Tool, replace_once};
use anyhow::{Result, bail};

/// The supported dprint plugins, each paired with its short name.
pub static PLUGINS: [(&str, Tool); 4] = [
    (
        "json",
        Tool {
            id: "dprint-plugin-json",
            name: "dprint-plugin-json",
            repository: "dprint/dprint-plugin-json",
            tag_prefix: "",
            file: "dprint.json",
            edit: |content, version| bump_url(content, version, "https://plugins.dprint.dev/json-"),
            command: None,
            paths: &["dprint.json"],
        },
    ),
    (
        "markdown",
        Tool {
            id: "dprint-plugin-markdown",
            name: "dprint-plugin-markdown",
            repository: "dprint/dprint-plugin-markdown",
            tag_prefix: "",
            file: "dprint.json",
            edit: |content, version| {
                bump_url(content, version, "https://plugins.dprint.dev/markdown-")
            },
            command: None,
            paths: &["dprint.json"],
        },
    ),
    (
        "toml",
        Tool {
            id: "dprint-plugin-toml",
            name: "dprint-plugin-toml",
            repository: "dprint/dprint-plugin-toml",
            tag_prefix: "",
            file: "dprint.json",
            edit: |content, version| bump_url(content, version, "https://plugins.dprint.dev/toml-"),
            command: None,
            paths: &["dprint.json"],
        },
    ),
    (
        "yaml",
        Tool {
            id: "pretty_yaml",
            name: "pretty_yaml",
            repository: "g-plane/pretty_yaml",
            tag_prefix: "v",
            file: "dprint.json",
            edit: |content, version| {
                bump_url(
                    content,
                    version,
                    "https://plugins.dprint.dev/g-plane/pretty_yaml-v",
                )
            },
            command: None,
            paths: &["dprint.json"],
        },
    ),
];

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
        for ((plugin, tool), (name, from, to)) in PLUGINS.iter().zip(cases) {
            assert_eq!(*plugin, name);
            let version = to.rsplit_once('-').unwrap().1.trim_start_matches('v');
            assert_eq!(
                (tool.edit)(DPRINT_JSON, version).unwrap(),
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
        for ((_, tool), (name, url)) in PLUGINS.iter().zip(cases) {
            assert_eq!(tool.branch_name("0.25.3"), format!("bump-{name}-0.25.3"));
            assert_eq!(
                tool.commit_message("0.25.3"),
                format!("bump {name} to version 0.25.3")
            );
            assert_eq!(
                tool.pull_request("0.25.3"),
                format!(
                    "Bump {name} to version 0.25.3\n\nThis pull request bumps {name} to version [0.25.3]({url})."
                )
            );
        }
    }
}
