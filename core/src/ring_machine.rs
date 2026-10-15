//! Ring state machine, as a reducer:
//! `(state, event, config) -> (state, effects)`.
//!
//! The [`reduce`] function does not measure time, hit-test or draw. The
//! engine:
//!
//! 1. finds out what is under the cursor and sends [`Event::Pointer`] when it
//!    changes;
//! 2. sends the trigger, click and keyboard events;
//! 3. runs every [`Effect`] it gets back, including starting and stopping the
//!    timers (when a timer fires, it becomes an event: [`Event::HoldElapsed`]
//!    or [`Event::IntentElapsed`]).

use crate::config::{Behavior, OpenWith};

/// Interaction phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    /// Ring closed.
    #[default]
    Idle,
    /// Trigger pressed, waiting for the hold time.
    Armed,
    /// Ring open with the trigger still held.
    OpenHeld,
    /// Ring open, waiting for a click.
    OpenClick,
}

/// What is under the cursor, according to the engine's hit-test.
///
/// An empty slot counts as [`Target::Gap`]: nothing is drawn there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Cancel zone, in the middle of the ring.
    Center,
    /// Ring slot that holds an action.
    Action { slot: u8 },
    /// Ring slot that holds a folder.
    Folder { slot: u8 },
    /// Action inside the open folder.
    Child { slot: u8, index: u8 },
    /// Between the items, still inside the ring area.
    Gap,
    /// Outside the ring.
    Outside,
}

impl Target {
    /// Slot of an item of the ring itself (action or folder).
    fn ring_slot(self) -> Option<u8> {
        match self {
            Target::Action { slot } | Target::Folder { slot } => Some(slot),
            _ => None,
        }
    }

    /// An action that can run: from the ring or from inside a folder.
    fn is_runnable(self) -> bool {
        matches!(self, Target::Action { .. } | Target::Child { .. })
    }
}

/// Full interaction state. It is `Copy`: every call to [`reduce`] returns a
/// new state instead of changing the old one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    pub phase: Phase,
    /// Highlighted item.
    pub hover: Option<Target>,
    /// Slot of the open folder.
    pub folder: Option<u8>,
    /// Ring item waiting for the path tolerance before it gets the highlight.
    pub pending: Option<Target>,
    pub trigger_held: bool,
}

impl State {
    pub fn is_open(&self) -> bool {
        matches!(self.phase, Phase::OpenHeld | Phase::OpenClick)
    }
}

/// Machine input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    TriggerDown,
    TriggerUp,
    /// The [`Effect::StartHold`] timer fired.
    HoldElapsed,
    /// The cursor now points at another target.
    Pointer(Target),
    /// The [`Effect::StartIntent`] timer fired for this slot.
    IntentElapsed {
        slot: u8,
    },
    /// Click (left button) with the ring open.
    Click(Target),
    Esc,
    FocusLost,
}

/// Why the ring closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    Center,
    Execute,
    Outside,
    Esc,
    FocusLost,
    /// The trigger was pressed again.
    Trigger,
}

/// Machine output: what the engine must do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    OpenRing,
    CloseRing(CloseReason),
    /// Start the `behavior.hold_to_open_ms` timer.
    StartHold,
    CancelHold,
    /// Quick tap: pass the trigger's native click on.
    PassNative,
    /// Released without running anything: the ring stays open, in click mode.
    StayOpen,
    /// Swallow the trigger release, because the ring handled the press.
    SwallowUp,
    /// Start the `behavior.path_tolerance_ms` timer for this item.
    StartIntent(Target),
    CancelIntent,
    OpenFolder {
        slot: u8,
    },
    CloseFolder {
        slot: u8,
    },
    Execute(Target),
    /// Click outside the ring: discard it, without passing it to the app below.
    SwallowClick,
    /// Click between the items: does nothing.
    IgnoreClick,
}

/// Closes the ring. `fx` are the effects that come before closing.
fn close(s: State, reason: CloseReason, mut fx: Vec<Effect>) -> (State, Vec<Effect>) {
    if s.pending.is_some() {
        fx.push(Effect::CancelIntent);
    }
    fx.push(Effect::CloseRing(reason));
    let closed = State {
        trigger_held: s.trigger_held,
        ..State::default()
    };
    (closed, fx)
}

/// Applies an event to the state and returns the new state and the effects.
pub fn reduce(s: State, event: Event, cfg: &Behavior) -> (State, Vec<Effect>) {
    use Effect::*;

    match event {
        Event::TriggerDown => {
            let t = State {
                trigger_held: true,
                ..s
            };
            match s.phase {
                Phase::Idle => match cfg.open_with {
                    OpenWith::Tap => (
                        State {
                            phase: Phase::OpenClick,
                            ..t
                        },
                        vec![OpenRing],
                    ),
                    OpenWith::Hold => (
                        State {
                            phase: Phase::Armed,
                            ..t
                        },
                        vec![StartHold],
                    ),
                },
                Phase::OpenClick => close(t, CloseReason::Trigger, vec![]),
                Phase::Armed | Phase::OpenHeld => (t, vec![]),
            }
        }

        Event::HoldElapsed => match s.phase {
            Phase::Armed => (
                State {
                    phase: Phase::OpenHeld,
                    ..s
                },
                vec![OpenRing],
            ),
            _ => (s, vec![]),
        },

        Event::TriggerUp => {
            let t = State {
                trigger_held: false,
                ..s
            };
            match s.phase {
                Phase::Armed => (
                    State {
                        phase: Phase::Idle,
                        ..t
                    },
                    vec![CancelHold, PassNative],
                ),
                // Only quick mode acts on the release: over an action it runs
                // it, over the center it closes. Otherwise the ring stays
                // open, even over the center, which closes with a click.
                Phase::OpenHeld => match s.hover {
                    Some(Target::Center) if cfg.execute_on_release => {
                        close(t, CloseReason::Center, vec![])
                    }
                    Some(h) if h.is_runnable() && cfg.execute_on_release => {
                        close(t, CloseReason::Execute, vec![Execute(h)])
                    }
                    _ => (
                        State {
                            phase: Phase::OpenClick,
                            ..t
                        },
                        vec![StayOpen],
                    ),
                },
                Phase::Idle | Phase::OpenClick => {
                    let fx = if s.trigger_held {
                        vec![SwallowUp]
                    } else {
                        vec![]
                    };
                    (t, fx)
                }
            }
        }

        Event::Pointer(target) => {
            if !s.is_open() {
                return (s, vec![]);
            }
            let mut n = s;
            let mut fx = vec![];

            // Left the item that was waiting for the tolerance: give up on it.
            if let Some(p) = n.pending {
                if target.ring_slot() != p.ring_slot() {
                    n.pending = None;
                    fx.push(CancelIntent);
                }
            }

            match target {
                Target::Child { .. } => n.hover = Some(target),
                Target::Center => {
                    n.hover = Some(target);
                    // The center closes the folder right away, with no tolerance.
                    if let Some(f) = n.folder.take() {
                        fx.push(CloseFolder { slot: f });
                    }
                }
                Target::Action { slot } | Target::Folder { slot } => {
                    if n.folder.is_some_and(|f| f != slot) {
                        // A folder is open: another item only gets the
                        // highlight if the cursor rests on it for the tolerance.
                        if n.pending.is_none() {
                            n.pending = Some(target);
                            fx.push(StartIntent(target));
                        }
                        n.hover = None;
                    } else {
                        n.hover = Some(target);
                        if matches!(target, Target::Folder { .. }) && n.folder != Some(slot) {
                            n.folder = Some(slot);
                            fx.push(OpenFolder { slot });
                        }
                    }
                }
                // Between the items or outside: the folder stays open.
                Target::Gap | Target::Outside => n.hover = None,
            }
            (n, fx)
        }

        Event::IntentElapsed { slot } => {
            let p = match s.pending {
                Some(p) if s.is_open() && p.ring_slot() == Some(slot) => p,
                _ => return (s, vec![]),
            };
            let mut fx = vec![];
            if let Some(f) = s.folder {
                fx.push(CloseFolder { slot: f });
            }
            let mut n = State {
                pending: None,
                hover: Some(p),
                folder: None,
                ..s
            };
            if let Target::Folder { slot } = p {
                n.folder = Some(slot);
                fx.push(OpenFolder { slot });
            }
            (n, fx)
        }

        Event::Click(target) => {
            if !s.is_open() {
                return (s, vec![]);
            }
            match target {
                Target::Action { .. } | Target::Child { .. } => {
                    close(s, CloseReason::Execute, vec![Execute(target)])
                }
                Target::Folder { slot } => {
                    if s.folder == Some(slot) {
                        return (s, vec![]);
                    }
                    let mut fx = vec![];
                    if s.pending.is_some() {
                        fx.push(CancelIntent);
                    }
                    if let Some(f) = s.folder {
                        fx.push(CloseFolder { slot: f });
                    }
                    fx.push(OpenFolder { slot });
                    let n = State {
                        folder: Some(slot),
                        hover: Some(target),
                        pending: None,
                        ..s
                    };
                    (n, fx)
                }
                Target::Center => close(s, CloseReason::Center, vec![]),
                Target::Outside => close(s, CloseReason::Outside, vec![SwallowClick]),
                Target::Gap => (s, vec![IgnoreClick]),
            }
        }

        Event::Esc | Event::FocusLost => {
            let reason = if event == Event::Esc {
                CloseReason::Esc
            } else {
                CloseReason::FocusLost
            };
            match s.phase {
                Phase::OpenHeld | Phase::OpenClick => close(s, reason, vec![]),
                Phase::Armed => (
                    State {
                        phase: Phase::Idle,
                        ..s
                    },
                    vec![CancelHold],
                ),
                Phase::Idle => (s, vec![]),
            }
        }
    }
}
