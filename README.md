# Bumpster

A command-line tool for bumping tool versions that Dependabot can't, such as Node.js, pnpm, or dprint plugins. Run it inside a repository, and it updates the version, commits the change, and opens a pull request authored by you, so you decide when each bump lands.

## Installation

Bumpster is built from source, so install [Rust](https://rustup.rs/) first. Then, from the root of a clone of this repository, run:

```sh
cargo install --path .
```

This installs the `bumpster` command into `~/.cargo/bin`, so it can be run from any repository. To update it, pull the latest changes and run the same command again, since Cargo always rebuilds and reinstalls from a path.

## Usage

Run Bumpster on the `main` branch of a repository with a clean working tree, passing the tool to bump and the version to bump to:

```sh
bumpster pnpm <version>
```

It updates the version, commits the change to a new branch, pushes it, and prints the title and description of the pull request to open. pnpm, in the `packageManager` field of `package.json`, is the only supported tool so far.

## License

This project is licensed under the [MIT License](./LICENSE).

Copyright © 2026 [Alfi Maulana](https://github.com/threeal)
