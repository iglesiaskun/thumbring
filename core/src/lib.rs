//! Pure logic of the action ring and the mouse gestures.
//!
//! Nothing here talks to Windows, measures time or draws. The user of the
//! library (the engine) does the hit-testing, counts the milliseconds, sends
//! events and runs the effects it gets back. That keeps everything
//! deterministic and testable.
//!
//! - [`ring`]: ring structure (8 slots; each slot is empty, an action or a folder).
//! - [`ring_machine`]: interaction state machine, as a reducer.
//! - [`layout`]: ring geometry, placement on the screen and hit-testing.
//! - [`anim`]: ring animations as pure functions of the elapsed time.
//! - [`view`]: visual state of an open ring and the scene to draw each frame.
//! - [`gesture`]: gesture direction recognition (4 directions or a click).
//! - [`command`]: what a gesture or an action does (shortcut, open, built-in).
//! - [`apps`]: per-app profiles, exclusion list and when gestures are on.
//! - [`screen`]: rule that tells whether the focused window is fullscreen.
//! - [`config`]: TOML configuration file, with lenient validation.

pub mod anim;
pub mod apps;
pub mod command;
pub mod config;
pub mod gesture;
pub mod layout;
pub mod ring;
pub mod ring_machine;
pub mod screen;
pub mod view;
