# Bumpster

A command-line tool for bumping tool versions that Dependabot can't, such as Node.js, pnpm, or dprint plugins. Run it inside a repository, and it updates the version, commits the change, and opens a pull request authored by you, so you decide when each bump lands.

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

The version can also be a tag that resolves to a released version, such as `latest`, with each tool choosing what to bump to when no version is given. It first gets onto the `main` branch with a clean working tree. If it's on another branch, it asks whether to switch to `main`, with yes as the default answer. If the working tree has uncommitted changes, it shows them and asks whether to discard them, switching to `main` too if needed, with no as the default answer since the changes can't be recovered. It then pulls `main` from `origin`, failing if it can't be brought in sync with `origin/main`, such as when it has unpushed commits. It then checks that the tool can be bumped, failing if it's already at that version or newer, then shows the change and asks whether to open a pull request for it, with yes as the default answer. Once confirmed, it updates the version, commits the change to a new branch, replacing any branch of the same name left by an earlier run, pushes it, replacing that branch on `origin` too unless it has commits pushed from elsewhere, and opens the pull request, logging each step as it runs and ending with the pull request's URL. To answer yes to every question, such as when running without a terminal, pass `--yes`. The supported tools are:

- **`dprint <plugin>`** — a dprint plugin in the `plugins` field of `dprint.json`, where `<plugin>` is one of `json`, `markdown`, `toml`, or `yaml` (for `g-plane/pretty_yaml`). It defaults to the `latest` tag, the plugin's latest GitHub release.
- **`node`** — Node.js, in the `devEngines.runtime` field of `package.json`, with `pnpm-lock.yaml` updated to match. It defaults to the newest release of the major version already in `package.json`, and also supports the `lts` tag, the newest LTS release, and `latest`, the newest release. A version with only a major, or a major and minor, such as `24` or `24.22`, bumps to the newest release matching it.
- **`pnpm`** — pnpm, in the `devEngines.packageManager` field of `package.json` with `pnpm-lock.yaml` updated to match, or in the `packageManager` field if that's not set. It defaults to the newest release of the major version already in `package.json`, and also supports the `latest` tag, the version pnpm marks as latest on npm. A version with only a major, or a major and minor, such as `10` or `10.18`, bumps to the newest release matching it. Its versions are looked up in the npm registry, where pnpm is installed from.

## License

This project is licensed under the [MIT License](./LICENSE).

Copyright © 2026 [Alfi Maulana](https://github.com/threeal)
