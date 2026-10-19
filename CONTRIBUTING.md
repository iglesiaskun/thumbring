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

## Building from source

Requirements (Windows only):

- Stable Rust with the `x86_64-pc-windows-msvc` target (installed by `rustup`).
- Visual Studio or Build Tools with the **Desktop development with C++**
  workload (provides the MSVC linker and the Windows SDK).

```bash
cargo test
```

Development build, with the log in a console (`Ctrl+C` quits):

```bash
cargo run -p thumbring
```

Release build of both programs, which run without a console:

```bash
cargo build --release -p thumbring -p thumbring-settings
```

Then start `target\release\thumbring.exe`. `--dir <folder>` keeps the
settings file and the log in one folder, which is handy for testing without
touching your real settings. The release files are made by
[`packaging/package.ps1`](packaging/package.ps1).

The settings window is a separate program, which needs the WebView2 Runtime
(it comes with Windows 11 and with the updates of Windows 10). Its interface
is plain HTML, CSS and JavaScript in [`settings/ui`](settings/ui), with no
build step:

```bash
cargo run -p thumbring-settings -- --dir <folder>
```

`--dir` works as in the engine; without it, the window edits your real
settings file. The engine's tray menu > Open settings opens the window next to
`thumbring.exe` and passes on its own `--dir`, so both use the same file.

## Project layout

```
core/       library (thumbring-core): pure, testable logic with no Windows
            APIs: ring state machine, gestures, commands, profiles and
            the settings file
engine/     Windows program (thumbring.exe): mouse hook, tray icon,
            command runner and the ring, drawn on the GPU
settings/   settings window (thumbring-settings.exe): edits the settings
            file and keeps the user's comments; interface in settings/ui
packaging/  release packaging: installer, license notices
```

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
