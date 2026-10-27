# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## About This Repository

A Rust CLI that bumps a tool's version in the current repository and opens a PR authored by the user, covering updates Dependabot can't do (e.g. Node.js, pnpm, dprint plugins). It runs `git` and `gh` as commands rather than using libraries, so the user's commit signing, hooks, and auth apply. Each supported tool has its own module in `src/`, built from the shared steps in `src/tool.rs`.

## Gotchas

- `lefthook run pre-commit` skips every job when nothing is staged, even ones that ignore the staged file list — pass `--all-files` to run it outside an actual commit.
- The pre-commit hook fixes files in place, and the run fails if any file changes. Report that failure and leave the fixes for the user to review and re-stage before committing again.
- The test command is `cargo tarpaulin`, not `cargo test` — only it applies the 100% coverage threshold from `tarpaulin.toml`.
- `.gitignore` ignores every dotfile and dotdir (`.*`) — a new one needs an explicit `!` exception there or it silently won't be tracked.
