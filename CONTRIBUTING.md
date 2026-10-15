# Contributing to Thumbring

Thanks for helping! This guide explains how changes get into the project.

## Workflow

1. **One branch per change.** Branch off `main` and name it
   `type/short-description`, for example `feat/tray-menu`, `fix/hook-timeout`
   or `docs/readme`.
2. **Keep pull requests small.** One topic per pull request. A rename, a
   translation and a new feature are three pull requests, not one.
3. **Pull requests are squash merged.** All commits in a pull request become a
   single commit on `main`, and its message comes from the pull request title
   and description. Write the title in the imperative mood: "Add tray menu",
   not "Added tray menu".
4. **Delete the branch after the merge.** A squash merge creates a new commit,
   so continuing on the old branch leads to conflicts. Start a new branch from
   `main` instead.
5. **`main` is protected.** Changes land only through pull requests, after CI
   passes.

## Bugs and ideas

Open an issue with the bug report or feature request template. Please search
the open issues first. Security problems are the exception: report them
privately, as [SECURITY.md](SECURITY.md) explains.

## Before opening a pull request

If people will notice the change, add a line to [CHANGELOG.md](CHANGELOG.md)
under "Unreleased", in the section that fits (Added, Changed, Fixed, ...).

Run the same checks CI runs:

```bash
cargo fmt --all
```

```bash
cargo clippy --all-targets -- -D warnings
```

```bash
cargo test --all
```

## Releases

A maintainer publishes each version: pushing a tag such as `v0.1.0` runs the
[Release workflow](.github/workflows/release.yml), which builds the files and
drafts a release with the notes from [CHANGELOG.md](CHANGELOG.md).

## Language

Code, comments, commit messages and pull request titles are written in
English. Issues and discussions may be in English or Portuguese.

## License

By contributing, you agree that your contributions are dual licensed under
the MIT and Apache-2.0 licenses, as described in the [README](README.md#license),
without any additional terms or conditions.
