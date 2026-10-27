# Bumpster

A command-line tool for bumping tool versions that Dependabot can't, such as Node.js, pnpm, or dprint plugins. Run it inside a repository, and it updates the version, commits the change, and opens a pull request authored by you, so you decide when each bump lands.

## Installation

Bumpster is built from source, so install [Rust](https://rustup.rs/) first. Then, from the root of a clone of this repository, run:

```sh
cargo install --path .
```

This installs the `bumpster` command into `~/.cargo/bin`, so it can be run from any repository. To update it, pull the latest changes and run the same command again, since Cargo always rebuilds and reinstalls from a path.

## Usage

Run Bumpster on the `main` branch of a repository with a clean working tree, with [GitHub CLI](https://cli.github.com/) logged in, passing the tool to bump and the version to bump to:

```sh
bumpster <tool> <version>
```

It updates the version, commits the change to a new branch, pushes it, and opens a pull request for it, logging each step as it runs and ending with the pull request's URL. The supported tools are:

- **`dprint <plugin>`** — a dprint plugin in the `plugins` field of `dprint.json`, where `<plugin>` is one of `json`, `markdown`, `toml`, or `yaml` (for `g-plane/pretty_yaml`).
- **`node`** — Node.js, in the `devEngines.runtime` field of `package.json`, with `pnpm-lock.yaml` updated to match.
- **`pnpm`** — pnpm, in the `devEngines.packageManager` field of `package.json` with `pnpm-lock.yaml` updated to match, or in the `packageManager` field if that's not set.

## License

This project is licensed under the [MIT License](./LICENSE).

Copyright © 2026 [Alfi Maulana](https://github.com/threeal)
