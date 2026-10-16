# Changelog

All notable changes to Thumbring are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a
minor version (0.2.0, 0.3.0) may change the settings file format; when that
happens, Thumbring updates older files by itself and this file says what
changed.

## [Unreleased]

## [0.1.0] - 2026-10-15

First public release. Thumbring gives any mouse on Windows 10 and 11 two
things: gestures on the Back button and a ring of actions on the Forward
button. A quick tap on either button still goes back or forward, as always.

### Added

- **Gestures.** Hold Back, drag up, down, left or right and release to run a
  command. By default: Task View, show the desktop, and the next and previous
  virtual desktop. Profiles change the gestures in the apps you choose.
- **Action ring.** Hold Forward to open a ring of 8 slots around the cursor.
  Each slot holds an action or a folder with up to 9 actions. The ring is
  drawn on the GPU, with smooth animations whose style and speed you can
  change.
- **Settings window.** Tray menu > Open settings edits the ring with a live
  preview (drag actions onto the slots, pick icons or use your own SVG or PNG,
  record shortcuts), the gestures, the apps and their profiles, the behavior,
  the colors and size of the ring and the animation. "Show on my screen" shows
  the ring before you save. Everything is kept in one settings file, which you
  can also edit by hand; the window keeps its comments.
- **Commands** for gestures and ring actions: keyboard shortcuts, media and
  volume keys, opening apps, files, folders and URLs, and built-in actions
  (screenshot, emoji panel, File Explorer, lock screen).
- **Any button for either job**: Back, Forward or the middle button (pressing
  the wheel), for mice without side buttons.
- **Stays out of the way**: it turns itself off in the apps you exclude and
  while an app is fullscreen, and pauses with one click on the tray icon.
  Settings apply as soon as they are saved; a wrong value falls back to its
  default and says so once, in a notification.
- **Start with Windows**, from the tray menu, normally or as administrator.
- **Downloads**: an `.msi` that installs for your user only, without
  administrator rights, and a portable `.zip`. Both come with `SHA256SUMS`,
  build provenance attestations and the licenses of the third-party code.

### Known limitations

- The files are not code-signed yet, so Windows SmartScreen ("Windows
  protected your PC") and some antivirus programs may warn about them. The
  README explains how to check a download.
- Windows that run as administrator, such as Task Manager, do not get gestures
  or the ring unless Thumbring runs as administrator too. Read the trade-off
  in the README before turning that on.
- The settings window needs the WebView2 Runtime. Windows 11 has it, and
  Windows Update installs it on most Windows 10 computers; where it is
  missing, the window says where to get it, and Thumbring itself keeps
  working.
- Some anti-cheat systems flag programs that hook the mouse. Pause Thumbring
  or exclude the game before playing online.
- If your mouse's own software remaps the side buttons, Thumbring no longer
  sees them as Back and Forward.
- Windows only, and the interface is in English.

[unreleased]: https://github.com/iglesiaskun/thumbring/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/iglesiaskun/thumbring/releases/tag/v0.1.0
