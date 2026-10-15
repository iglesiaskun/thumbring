//! Thumbring Settings, the settings editor: everything except the window.
//!
//! The window (`main.rs`) shows the interface in `ui/` and calls this code to
//! read the settings file and to save the interface's changes back to it.
//! Reading and validating use the engine's own code, from `thumbring-core`.

pub mod apps;
pub mod edit;
pub mod file;
pub mod icons;
pub mod session;
