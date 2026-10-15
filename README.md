# Thumbring

[![CI](https://github.com/iglesiaskun/thumbring/actions/workflows/ci.yml/badge.svg)](https://github.com/iglesiaskun/thumbring/actions/workflows/ci.yml)

[Leia em português](README.pt-BR.md)

A lightweight Windows tool that gives any mouse **gestures** and a radial
**action menu**:

- **Hold the Back button and drag** to trigger a gesture (up, down, left or
  right).
- **Hold the Forward button** to open a ring of 8 slots around the cursor. Each
  slot is an action or a folder with up to 9 actions.
- **A quick tap** on either button still does what the button always did (go
  back or forward).
- **Other buttons** can take either job: Back, Forward or the middle button
  (pressing the wheel).

<!--
  GIF: the ring. Save it as docs/media/ring.gif and replace this comment with
  ![Holding Forward opens the action ring](docs/media/ring.gif)
-->

<!--
  GIF: the gestures. Save it as docs/media/gestures.gif and replace this
  comment with
  ![Holding Back and dragging runs a gesture](docs/media/gestures.gif)
-->

Thumbring 0.1.0 is the first public release. It is configured through a
settings file; a visual settings editor is planned for 0.2.0. What changed in
each version is in the [changelog](CHANGELOG.md).

## Install

Thumbring runs on Windows 10 and 11 (64-bit). By default it uses the two side
buttons of the mouse, Back and Forward; on a mouse without them, the middle
button can run the gestures or open the ring instead (see "Configuration").

1. Download the latest version from the
   [Releases page](https://github.com/iglesiaskun/thumbring/releases/latest):
   - **`thumbring-<version>-windows-x64.msi`** (recommended) installs Thumbring
     for your user only, without administrator rights, adds it to the Start
     menu and starts it. To update, run the newer `.msi`.
   - **`thumbring-<version>-windows-x64.zip`** runs without installing: unzip
     it anywhere and start `thumbring.exe`.
2. If Windows shows "Windows protected your PC", click **More info** and then
   **Run anyway**. The files are not code-signed yet; "Known limitations"
   below explains why and how to check a download.
3. On the first run, a notification says that Thumbring is running. Its icon
   is in the notification area, next to the clock; Windows may hide it behind
   the **^** arrow. To keep it in sight, drag it to the taskbar.
4. To start Thumbring with Windows, right-click its icon and check
   **Start with Windows**. To use it in windows that run as administrator
   too, check **Start with Windows as administrator** instead; "Known
   limitations" below explains it.

**Uninstall.** With the `.msi`, remove Thumbring in Settings > Apps >
Installed apps; that also stops it from starting with Windows. With the
`.zip`, first right-click the tray icon and uncheck **Start with Windows** (or
**Start with Windows as administrator**), then quit Thumbring and delete its
folder. Your settings and the log stay in `%APPDATA%\thumbring` and
`%LOCALAPPDATA%\thumbring`; delete those folders to remove everything.

## Using Thumbring

Hold the mouse Back button, drag about 3 cm and release (the buttons can be
changed; see "Configuration"):

| Gesture | Default |
|---|---|
| Up | `Win+Tab` (Task View) |
| Down | `Win+D` (show the desktop) |
| Left | `Ctrl+Win+Right` (next virtual desktop) |
| Right | `Ctrl+Win+Left` (previous virtual desktop) |
| Quick tap | Back, as usual |

Hold the Forward button for a moment: the ring opens around the cursor. Point
at an item and click it, or release Forward and click afterwards. A folder
opens when the cursor rests on it. A click outside the ring, a right click,
Esc, Forward again or switching windows closes it without running anything.
While the ring is open, clicks and the mouse wheel never reach the window
below.

| Default ring (clockwise from the top) | Does |
|---|---|
| Play or pause | `PlayPause` |
| Screenshot | snipping screenshot |
| Media (folder) | previous and next track, volume down and up, mute |
| Lock screen | locks the session |
| Editing (folder) | copy, paste, cut, undo, redo, clipboard history |
| Emoji | emoji panel |
| File Explorer | opens File Explorer |
| Apps (folder) | Calculator, Notepad, Terminal, Settings |

Click the tray icon to pause or resume Thumbring. Right-click it for the menu:
pause, open the settings file, open the log, start with Windows (normally or
as administrator) and quit.

## Configuration

The settings file is `%APPDATA%\thumbring\config.toml`. Thumbring creates
it on the first run with every default and a comment for each option (the
template is [`core/src/config/default.toml`](core/src/config/default.toml)),
and reloads it every time it is saved. An invalid or out-of-range value falls
back to its default and shows a warning instead of crashing; a file that
cannot be parsed keeps the previous settings. The log is at
`%LOCALAPPDATA%\thumbring\engine.log` (tray menu > Open log), and it also
shows the `.exe` name of every app that gets focus.

The first line, `version = 1`, is the format of the file. If a future version
changes the format, Thumbring uses this number to update older files. A file
without it counts as version 1.

```toml
version = 1

[gestures]
button = "back"             # "back", "forward" or "middle"
up = "Win+Tab"
down = "Win+D"
left = "Ctrl+Win+Right"
right = "Ctrl+Win+Left"
click = "native"
threshold_px = 30           # at 100% scaling; grows with the monitor DPI

[apps]
disable_in_fullscreen = true
excluded = ["mstsc.exe"]    # the button works as usual in these apps

# A profile changes only the gestures it lists.
[[profiles]]
name = "Browser"
apps = ["browser.exe"]      # the executable of your browser
gestures = { left = "Ctrl+Tab", right = "Ctrl+Shift+Tab" }
```

`button` under `[gestures]` is the button that runs the gestures, and
`button` under `[ring]` the one that opens the ring: `"back"`, `"forward"` or
`"middle"` (pressing the wheel). They are Back and Forward unless you change
them. One button cannot do both: if the two name the same button, both go
back to their defaults, with a warning. While the ring is off, the gestures
may use its button. A quick tap, like the `"native"` command, sends the click
of the button that was held.

Commands, for gestures and ring actions alike:

| Text | Does |
|---|---|
| `"Ctrl+Shift+T"` | keyboard shortcut (Ctrl, Win, Alt, Shift plus one key) |
| `"VolumeUp"`, `"PlayPause"`, `"NextTrack"` | media and volume keys |
| `'open:C:\Users\you\Downloads'`, `'open:https://...'` | opens an app, file, folder or URL; `%VARIABLES%` are expanded |
| `'open:"C:\Program Files\App\app.exe" --option'` | the same, with arguments |
| `"screenshot"`, `"emoji"`, `"explorer"`, `"lock"` | built-in actions |
| `"native"` | what the button does without Thumbring |
| `"none"` | nothing (turns a gesture off in a profile) |

The ring is configured in the same file, which comes with the default ring
written out. It has 8 slots, starting at the top, clockwise; each one holds an
action, a folder with up to 9 actions, or nothing (`type = "empty"`). Saved
changes apply the next time the ring opens. To turn the ring off and keep its
button as it always was, leave the list empty: `slots = []` under `[ring]`.

Icons: `calculator`, `calendar`, `clipboard`, `clipboard-list`,
`clipboard-paste`, `code`, `copy`, `folder-open`, `globe`, `headphones`,
`layout-grid`, `lock`, `mail`, `music`, `play`, `redo-2`, `scan`, `scissors`,
`settings`, `skip-back`, `skip-forward`, `smile`, `sticky-note`, `terminal`,
`undo-2`, `volume-1`, `volume-2`, `volume-x` and `x`. Without an icon (or with
an unknown one, which logs a warning), the item shows the first letter of its
name.

```toml
[animation]
style = "viscous"     # or "elastic"
speed = 1.0           # 0.5 to 2.0

[behavior]
open_with = "hold"    # or "tap"
hold_to_open_ms = 150
path_tolerance_ms = 50

[ring]
button = "forward"    # "forward", "back" or "middle"

[[ring.slots]]
type = "action"
name = "Screenshot"
icon = "scan"
command = "Win+Shift+S"

[[ring.slots]]
type = "folder"
name = "Media"
icon = "music"
items = [
  { name = "Mute", icon = "volume-x", command = "VolumeMute" },
]
```

## Known limitations

**Windows that run as administrator.** Windows does not let a program without
administrator rights watch or send input to windows that run as administrator,
such as Task Manager, an administrator terminal or some installers. While one
of them is in front, gestures and the ring do not work there, unless Thumbring
has administrator rights too:

1. Right-click the tray icon and check **Start with Windows as
   administrator**. Windows asks for permission once (the UAC prompt) and
   creates a task, "Thumbring (your user name)", in Task Scheduler.
2. From the next sign-in, that task starts Thumbring with administrator
   rights, without asking.

The apps that Thumbring opens (`open:` commands, the settings file, the log)
still run without administrator rights: File Explorer opens them, and if
Explorer does not answer, Thumbring opens nothing and says so. Unchecking
the option, or checking **Start with Windows**, deletes the task. After an
update, the installer starts Thumbring without administrator rights until the
next sign-in.

Think before you turn it on. A program that starts with administrator rights
is only as safe as its folder, and the `.msi` installs Thumbring in your user
folder, where any program you run can write. A malicious program could replace
`thumbring.exe` there and get administrator rights at your next sign-in,
without a UAC prompt. If that worries you, leave the option off, or unzip the
`.zip` into a folder that only administrators can change, such as
`C:\Program Files\Thumbring`, and turn the option on from the Thumbring in
that folder. For the same reason, while it has administrator rights,
Thumbring ignores mouse button presses that other programs simulate, such as
remote control tools or software that remaps mouse buttons: only the real
mouse buttons run gestures and open the ring.

**Games and anti-cheat.** Thumbring uses the same Windows features as macro
tools: a low-level mouse hook and simulated key presses. Some anti-cheat
systems block or flag programs that do this. Thumbring turns itself off while
an app is fullscreen, but a game in a borderless window may not count as
fullscreen. Before playing online, pause Thumbring from the tray or add the
game's `.exe` to `excluded` under `[apps]` in the settings file.

**Warnings from SmartScreen and antivirus programs.** The release files are
not code-signed yet: a certificate is expensive for a free project, and free
signing for open source projects has been requested. Until then, Windows
SmartScreen warns about a new program with no reputation ("Windows protected
your PC"), and an antivirus may distrust an unknown program that hooks the
mouse and simulates key presses, which is exactly what Thumbring does. To make
sure a download is the one GitHub built from this repository:

- compare the output of `Get-FileHash <file>` in PowerShell with the line for
  that file in `SHA256SUMS`, from the same release;
- or, with the [GitHub CLI](https://cli.github.com), run
  `gh attestation verify <file> --repo iglesiaskun/thumbring`.

You can also build Thumbring from the source code yourself (see below).

**Mouse software.** If your mouse's own software maps the side buttons to
something else, Thumbring no longer sees them as Back and Forward. Set them
back to Back and Forward (or "default") in that software.

## FAQ

**Does Thumbring collect data or use the internet?**
No. It makes no network connections. The log stays on your computer; it lists
the names of the apps you focus, which helps when writing profiles.

**Does it work with my mouse?**
Yes. By default Thumbring uses the two side buttons, Back and Forward
(sometimes called buttons 4 and 5). On a mouse without them, write
`button = "middle"` under `[gestures]` or under `[ring]` in the settings file,
and pressing the wheel runs the gestures or opens the ring.

**How do I change the gestures or the ring?**
Right-click the tray icon > Open settings file, edit it and save it. Thumbring
reloads it at once; the "Configuration" section above explains the options.

**Is there a settings window?**
Not yet. Version 0.1.0 is configured through the settings file, and a visual
editor is planned for 0.2.0.

**How do I go back to the default settings?**
Quit Thumbring, delete `%APPDATA%\thumbring\config.toml` and start Thumbring
again: it creates a new file with the defaults.

**Gestures do not work in Task Manager or in an administrator terminal. Why?**
Those windows run as administrator, and Windows keeps programs without those
rights away from them. Check **Start with Windows as administrator** in the
tray menu and sign in again; "Known limitations" above explains the
trade-off.

**Gestures stopped working. What now?**
Check the tray icon: a dimmed ring means Thumbring is paused, off in an
excluded app or off because an app is fullscreen; its tooltip says which.
Remember the limitation with windows that run as administrator. If none of
these explains it, the log (tray menu > Open log) usually does; please
[open an issue](https://github.com/iglesiaskun/thumbring/issues/new/choose) with the
relevant lines.

**Why "Thumbring"?**
The thumb works the side buttons, and the ring opens around the cursor.

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

Release build, which runs without a console:

```bash
cargo build --release -p thumbring
```

Then start `target\release\thumbring.exe`. `--dir <folder>` keeps the
settings file and the log in one folder, which is handy for testing without
touching your real settings. The release files are made by
[`packaging/package.ps1`](packaging/package.ps1).

## Project layout

```
core/       library (thumbring-core): pure, testable logic with no Windows
            APIs: ring state machine, gestures, commands, profiles and
            the settings file
engine/     Windows program (thumbring.exe): mouse hook, tray icon,
            command runner and the ring, drawn on the GPU
packaging/  release packaging: installer, license notices
```

## Contributing

Bug reports, ideas and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md)
explains how changes get in. Please report security problems privately, as
described in [SECURITY.md](SECURITY.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

The ring icons in [`engine/assets/icons`](engine/assets/icons) come from
[Lucide](https://lucide.dev) and are under the ISC license
([LICENSE-LUCIDE](engine/assets/icons/LICENSE-LUCIDE)).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
