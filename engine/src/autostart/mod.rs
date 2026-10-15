//! Start with Windows, in one of two ways, both chosen in the tray menu:
//!
//! - normally, with a value under the `Run` registry key ([`run_key`]);
//! - as administrator, with a Task Scheduler task ([`task`]), so gestures and
//!   the ring also work in windows that run as administrator.
//!
//! The two exclude each other: turning one on turns the other off. Windows
//! itself is the source of truth, so the option lives only in the tray menu
//! and not in the settings file.

mod run_key;
mod task;

use windows::Win32::Foundation::HWND;

pub use crate::elevation::Failure;

/// A way to start with Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    Normal,
    Administrator,
}

/// Which ways are on. Both may be, if someone set them up by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Current {
    pub normal: bool,
    pub administrator: bool,
}

impl Current {
    pub fn has(self, way: Way) -> bool {
        match way {
            Way::Normal => self.normal,
            Way::Administrator => self.administrator,
        }
    }
}

pub fn current() -> Current {
    Current {
        normal: run_key::is_enabled(),
        administrator: task::is_enabled(),
    }
}

/// Turns `way` off if it is on, or on (and the other way off) if it is off.
/// Returns whether `way` is on now. May show the UAC prompt and wait for it,
/// so it must not run on the thread of a window; `owner` is the window the
/// prompt belongs to.
pub fn toggle(way: Way, owner: HWND) -> Result<bool, Failure> {
    let now = current();
    for step in plan(now, way) {
        match step {
            Step::AddRunValue => run_key::enable(&exe()?).map_err(Failure::Error)?,
            Step::RemoveRunValue => run_key::disable().map_err(Failure::Error)?,
            Step::CreateTask => task::create(&exe()?, owner)?,
            Step::DeleteTask => task::delete(owner)?,
        }
    }
    Ok(!now.has(way))
}

fn exe() -> Result<std::path::PathBuf, Failure> {
    std::env::current_exe().map_err(|e| Failure::Error(e.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    AddRunValue,
    RemoveRunValue,
    CreateTask,
    DeleteTask,
}

/// What a click on the menu item for `way` changes, in order. The task comes
/// first: it is the step that may wait for the UAC prompt, and a "No" there
/// stops everything before anything changed.
fn plan(now: Current, way: Way) -> Vec<Step> {
    match way {
        Way::Normal if now.normal => vec![Step::RemoveRunValue],
        Way::Normal if now.administrator => vec![Step::DeleteTask, Step::AddRunValue],
        Way::Normal => vec![Step::AddRunValue],
        Way::Administrator if now.administrator => vec![Step::DeleteTask],
        Way::Administrator if now.normal => vec![Step::CreateTask, Step::RemoveRunValue],
        Way::Administrator => vec![Step::CreateTask],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFF: Current = Current {
        normal: false,
        administrator: false,
    };
    const NORMAL: Current = Current {
        normal: true,
        administrator: false,
    };
    const ADMINISTRATOR: Current = Current {
        normal: false,
        administrator: true,
    };
    const BOTH: Current = Current {
        normal: true,
        administrator: true,
    };

    #[test]
    fn turning_one_way_on_turns_the_other_off() {
        assert_eq!(plan(OFF, Way::Normal), [Step::AddRunValue]);
        assert_eq!(plan(OFF, Way::Administrator), [Step::CreateTask]);
        assert_eq!(
            plan(NORMAL, Way::Administrator),
            [Step::CreateTask, Step::RemoveRunValue]
        );
        assert_eq!(
            plan(ADMINISTRATOR, Way::Normal),
            [Step::DeleteTask, Step::AddRunValue]
        );
    }

    #[test]
    fn turning_a_way_off_leaves_the_other_alone() {
        assert_eq!(plan(NORMAL, Way::Normal), [Step::RemoveRunValue]);
        assert_eq!(plan(ADMINISTRATOR, Way::Administrator), [Step::DeleteTask]);
        assert_eq!(plan(BOTH, Way::Normal), [Step::RemoveRunValue]);
        assert_eq!(plan(BOTH, Way::Administrator), [Step::DeleteTask]);
    }

    #[test]
    fn the_task_changes_before_the_registry() {
        // A "No" to the UAC prompt must leave everything as it was.
        for now in [OFF, NORMAL, ADMINISTRATOR, BOTH] {
            for way in [Way::Normal, Way::Administrator] {
                let steps = plan(now, way);
                let task = steps
                    .iter()
                    .position(|s| matches!(s, Step::CreateTask | Step::DeleteTask));
                if let Some(task) = task {
                    assert_eq!(task, 0, "{now:?} {way:?}: {steps:?}");
                }
            }
        }
    }
}
