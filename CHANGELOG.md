# Changelog

All notable changes to Thumbring are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a
minor version (0.2.0, 0.3.0) may change the settings file format; when that
happens, Thumbring updates older files by itself and this file says what
changed.

## [Unreleased]

### Added

- "Start with Windows as administrator" in the tray menu, so gestures and the
  ring also work in windows that run as administrator, such as Task Manager.
  It asks for permission once (the UAC prompt) and creates a Task Scheduler
  task that starts Thumbring with administrator rights at sign-in. The apps
  Thumbring opens still run without administrator rights, and Back and
  Forward presses simulated by other programs are ignored, so a program
  without those rights cannot use Thumbring to reach windows that have them.
  It and "Start with Windows" exclude each other.
- Choose which mouse button runs the gestures and which opens the ring: Back,
  Forward or the middle button (pressing the wheel), with `button` under
  `[gestures]` and under `[ring]` in the settings file. Back and Forward stay
  the defaults, so older settings files work as before. A quick tap, and the
  `"native"` command in a gesture or a ring action, send the click of the
  button that was held.

### Changed

- Uninstalling the `.msi` also stops Thumbring from starting with Windows.

## [0.1.0] - 2026-10-14

First public release. Thumbring 0.1.0 is configured through its settings file
(`config.toml`); a visual settings editor is planned for 0.2.0.

### Added

- Mouse gestures: hold the Back button, drag up, down, left or right and
  release to run a command. A quick tap still goes back.
- Action ring: hold the Forward button to open a ring of 8 slots around the
  cursor. Each slot holds an action or a folder with up to 9 actions. The ring
  is drawn on the GPU, with "viscous" (default) or "elastic" animations whose
  speed and timings can be changed.
- Commands for gestures and ring actions: keyboard shortcuts, media and volume
  keys, opening apps, files, folders and URLs, and built-in actions
  (screenshot, emoji panel, File Explorer, lock screen).
- Per-app gesture profiles, a list of apps where Thumbring stays off, and no
  gestures while an app is fullscreen.
- Settings file at `%APPDATA%\thumbring\config.toml`, created with comments on
  the first run and reloaded on every save. Invalid values fall back to their
  defaults; problems go to the log and to a tray notification, once.
- `version = 1` at the top of the settings file, so later versions can update
  older files when the format changes. A file without it counts as version 1.
- Tray icon to pause and resume, open the settings file and the log, start
  with Windows and quit. On the first run, a notification says where it is.
- The mouse hook is installed again after sleep, after unlocking the session
  and when the display turns on, in case Windows removed it.
- On an unexpected error, Thumbring writes it to the log, gives the mouse
  buttons back and quits with a message.
- Release files: a portable `.zip` and an `.msi` installer for the current
  user, with `SHA256SUMS`, build provenance attestations and the licenses of
  the third-party crates.

Gestures and the ring do not work in windows that run as administrator yet;
see "Known limitations" in the README.

[unreleased]: https://github.com/iglesiaskun/thumbring/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/iglesiaskun/thumbring/releases/tag/v0.1.0
