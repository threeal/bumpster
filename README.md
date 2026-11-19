# Bumpster

A command-line tool for bumping tool versions that Dependabot can't, such as Go, Node.js, pnpm, dprint plugins, or the JDK in `.sdkmanrc`. Run it inside a repository, and it updates the version, commits the change, and opens a pull request authored by you, so you decide when each bump lands.

## Installation

Bumpster is built from source, so install [Rust](https://rustup.rs/) first. Then, from the root of a clone of this repository, run:

```sh
cargo install --path .
```

This installs the `bumpster` command into `~/.cargo/bin`, so it can be run from any repository. To update it, pull the latest changes and run the same command again, since Cargo always rebuilds and reinstalls from a path.

## Usage

Run Bumpster in a repository with [GitHub CLI](https://cli.github.com/) logged in, passing the tool to bump and optionally the version to bump to:

```sh
bumpster <tool> [version]
```

The version can also be a tag that resolves to a released version, such as `latest`, with each tool choosing what to bump to when no version is given. It first checks that GitHub CLI is logged in, failing before anything is changed if it's not, then gets onto the repository's default branch on GitHub, such as `main`, with a clean working tree. If it's on another branch with a clean working tree, it switches to the default branch without asking, since nothing is lost. If the working tree has uncommitted changes, it shows them and asks whether to discard them, switching to the default branch too if needed, with no as the default answer since the changes can't be recovered. It then pulls the default branch from `origin`, failing if it can't be brought in sync with its counterpart on `origin`, such as when it has unpushed commits. It then checks that the tool can be bumped, showing that it's up to date and exiting successfully if it's already at that version, or failing if it's already at a newer one, then shows the change and asks whether to open a pull request for it, with yes as the default answer. Once confirmed, it updates the version, commits the change to a new branch, replacing any branch of the same name left by an earlier run, pushes it, replacing that branch on `origin` too unless it has commits pushed from elsewhere, and opens the pull request against the default branch, or updates the title and description of the one already open for that branch, logging each step as it runs and ending with the pull request's URL. To answer yes to every question, such as when running without a terminal, pass `--yes`. The commit is only signed off or signed when asked with `--signoff` (`-s`) or `--gpg-sign` (`-S`), the same as with `git commit`, so without `--gpg-sign` it is still signed if git's `commit.gpgSign` setting asks for it.

Run `bumpster` without a tool to check every supported tool the repository uses against the version its own bump defaults to, showing the ones already up to date and skipping tools the repository doesn't use without a word. A tool whose version can't be read or resolved, such as when its release lookup fails, is skipped with a warning instead of stopping the others. It then asks once whether to open a pull request for each outdated tool, and runs each bump in turn from the default branch, the same as bumping that tool alone, stopping at the first one that fails and ending on the last bump's branch. After merging one of those pull requests, the others may conflict with it, and running `bumpster` again rebuilds them from the updated default branch.

The supported tools are:

- **`dprint <plugin>`** — a dprint plugin in the `plugins` field of `dprint.json`, where `<plugin>` is one of `json`, `markdown`, `toml`, or `yaml` (for `g-plane/pretty_yaml`). It defaults to the `latest` tag, the plugin's latest GitHub release. Without a plugin, `dprint` checks every plugin in `dprint.json` against its latest release instead, warning about any plugin it doesn't support and showing the ones already up to date, then asks once whether to open a pull request for each outdated plugin. Each bump then runs in turn from the default branch, the same as bumping that plugin alone, stopping at the first one that fails and ending on the last bump's branch.
- **`go`** — Go, in the `go` directive of `go.mod`, removing a `toolchain` directive that the new version makes redundant, since it names a Go release no newer than that version. It defaults to the newest patch release of the minor version already in `go.mod`, since the `go` directive is also the minimum Go version every user of the module needs, and also supports the `latest` tag, the newest stable release. A version with only a major, or a major and minor, such as `1` or `1.28`, bumps to the newest stable release matching it. Its versions are looked up in Go's list of releases on `go.dev`.
- **`node`** — Node.js, in the `devEngines.runtime` field of `package.json`, with `pnpm-lock.yaml` updated to match if the repository has one. It defaults to the newest release of the major version already in `package.json`, and also supports the `lts` tag, the newest LTS release, and `latest`, the newest release. A version with only a major, or a major and minor, such as `24` or `24.22`, bumps to the newest release matching it.
- **`pnpm`** — pnpm, in the `devEngines.packageManager` field of `package.json` with `pnpm-lock.yaml` updated to match, or in the `packageManager` field if that's not set. It defaults to the newest release of the major version already in `package.json`, and also supports the `latest` tag, the version pnpm marks as latest on npm. A version with only a major, or a major and minor, such as `10` or `10.18`, bumps to the newest release matching it. Its versions are looked up in the npm registry, where pnpm is installed from.
- **`sdkman <candidate>`** — a [SDKMAN!](https://sdkman.io/) candidate in `.sdkmanrc`, where `<candidate>` is `java`, pinned to an [Eclipse Temurin](https://adoptium.net/temurin/) identifier such as `25.0.5.1+2-tem`, with the pull request linking to its release notes on the Adoptium website. Java from any other vendor isn't supported. Its identifiers are looked up in SDKMAN's list for Linux x64. It defaults to the newest release of the major version already in `.sdkmanrc`, such as `25.0.5.0+8-tem` to `25.0.5.1+2-tem`, since a JDK is usually kept on an LTS release on purpose, and also supports the `latest` tag, the newest release of any major version. A version, such as `27` or `25.0.5`, bumps to the newest release matching it. Without a candidate, `sdkman` checks every candidate in `.sdkmanrc` against the newest release of its major version instead, warning about any candidate it doesn't support, such as `gradle`, `maven`, or Java from another vendor, and showing the ones already up to date, then asks once whether to open a pull request for each outdated candidate.

## License

This project is licensed under the [MIT License](./LICENSE).

Copyright © 2026 [Alfi Maulana](https://github.com/threeal)
