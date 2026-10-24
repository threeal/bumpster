# Bumpster

A command-line tool for bumping tool versions that Dependabot can't, such as Node.js, pnpm, or dprint plugins. Run it inside a repository, and it updates the version, commits the change, and opens a pull request authored by you, so you decide when each bump lands.

## Usage

Run Bumpster in a repository with the tool to bump, and optionally the version to bump to:

```sh
bumpster <tool> [version]
```

If no version is given, it bumps the tool to its latest version. No tools are supported yet.

## License

This project is licensed under the [MIT License](./LICENSE).

Copyright © 2026 [Alfi Maulana](https://github.com/threeal)
