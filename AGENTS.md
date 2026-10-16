# AGENTS.md

Rules for AI agents working on Thumbring. The workflow for every contributor
is in [CONTRIBUTING.md](CONTRIBUTING.md); what Thumbring does and how to build
it are in [README.md](README.md).

## Build and check

Windows only: stable Rust with the `x86_64-pc-windows-msvc` target, no older
than `rust-version` in the root `Cargo.toml`. Before each commit, run what CI
runs:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

## Where code goes

- `core/` (`thumbring-core`): pure logic with no Windows APIs: ring state
  machine, gestures, commands, profiles and the `config.toml` format. Logic
  that can live without Windows goes here, with its tests in `core/tests/`.
- `engine/` (`thumbring.exe`): the Windows side: mouse hook, ring overlay,
  tray icon, command runner.
- `settings/` (`thumbring-settings.exe`): the settings editor, on Tauri 2. Its
  interface in `settings/ui/` is plain HTML, CSS and JavaScript modules,
  loaded as they are: no Node, no npm, no build step.

## Invariants

- The hook callbacks in `engine/src/hook.rs` only decide whether to block an
  event and send a note over the channel; actions, I/O and anything slow run
  on the worker thread. Windows skips a slow callback and, if that keeps
  happening, removes the hook without notice.
- Blocking is symmetric: a button's release is blocked exactly when its press
  was.
- Every event the engine injects carries `input::SIGNATURE` in `dwExtraInfo`,
  so the hook recognizes it and lets it through.
- A change that would make an older `config.toml` mean something else raises
  `FORMAT_VERSION` (`core/src/config.rs`) and converts older files in
  `read_version` (`core/src/config/load.rs`), with a test. New keys also go
  into `core/src/config/default.toml`, with a comment.
- The settings file is saved atomically, through `write_atomic` in
  `settings/src/file.rs`: the engine reloads it on every change and must never
  read it half written.
- Thumbring makes no network connections and has no telemetry, as the README
  and SECURITY.md promise.
- Generic names only: code, tests, examples and docs name no brand or product
  of another company (write `browser.exe`, `editor.exe`). The exceptions are
  what building and running need: Windows, GitHub, the build tools and the
  Lucide credit.
- Icons come only from [Lucide](https://lucide.dev), unmodified, in
  `engine/assets/icons/` or `settings/ui/icons/`, next to `LICENSE-LUCIDE`.

## Conventions

- English in code, comments, commit messages and docs. `README.pt-BR.md` is
  the one Portuguese file.
- One branch per change (`type/short-description`), small pull requests,
  squash merged, with the title in the imperative ("Add tray menu").
- Edit manifests such as `Cargo.toml` with targeted changes. Rewriting the
  whole file loses its comments and fields such as `license`.
- A new dependency needs a reason, written as a comment above it in
  `Cargo.toml`, like the others, and in the pull request.
- A change people will notice updates `CHANGELOG.md` under "Unreleased", and
  both `README.md` and `README.pt-BR.md`.

## Left to people

Pushes, tags and releases are done by a person: commit on your branch and stop
there. A `v*` tag builds and drafts a release.
