//! How the ring answers each event (trigger, clicks, wheel, Esc and focus),
//! plus the path tolerance, free cursor and "tap" mode rules.

use thumbring_core::config::{Behavior, OpenWith};
use thumbring_core::ring_machine::{CloseReason, Effect, Event, Phase, State, Target, reduce};

use Effect::*;
use Event::*;

const ACTION: Target = Target::Action { slot: 0 };
const ACTION_2: Target = Target::Action { slot: 6 };
const FOLDER: Target = Target::Folder { slot: 2 };
const FOLDER_2: Target = Target::Folder { slot: 4 };
const CHILD: Target = Target::Child { slot: 2, index: 1 };
/// No effect.
const NONE: [Effect; 0] = [];

/// Keeps the state between events, as the engine will.
struct Ring {
    state: State,
    cfg: Behavior,
}

impl Ring {
    fn new() -> Self {
        Self::with(Behavior::default())
    }

    fn with(cfg: Behavior) -> Self {
        Self {
            state: State::default(),
            cfg,
        }
    }

    fn send(&mut self, event: Event) -> Vec<Effect> {
        let (state, fx) = reduce(self.state, event, &self.cfg);
        self.state = state;
        fx
    }

    fn phase(&self) -> Phase {
        self.state.phase
    }
}

/// Ring open with the trigger still held.
fn open_held() -> Ring {
    let mut r = Ring::new();
    r.send(TriggerDown);
    r.send(HoldElapsed);
    r
}

/// Ring open in click mode (the trigger was released away from the center).
fn open_click() -> Ring {
    let mut r = open_held();
    r.send(TriggerUp);
    r
}

/// Click mode with the folder in slot 2 open.
fn folder_open() -> Ring {
    let mut r = open_click();
    assert_eq!(r.send(Pointer(FOLDER)), [OpenFolder { slot: 2 }]);
    r
}

// ---------------------------------------------------------------------------
// Interaction table
// ---------------------------------------------------------------------------

/// "Tap the trigger (< 150 ms)": passes the native click on.
#[test]
fn quick_tap_passes_the_native_click() {
    let mut r = Ring::new();
    assert_eq!(r.send(TriggerDown), [StartHold]);
    assert_eq!(r.phase(), Phase::Armed);

    assert_eq!(r.send(TriggerUp), [CancelHold, PassNative]);
    assert_eq!(r.state, State::default());
}

/// "Hold the trigger for 150 ms": opens the ring.
#[test]
fn holding_opens_the_ring() {
    let mut r = Ring::new();
    r.send(TriggerDown);
    assert_eq!(r.send(HoldElapsed), [OpenRing]);
    assert_eq!(r.phase(), Phase::OpenHeld);
    assert!(r.state.is_open());
}

/// A hold timer that fires after the release opens nothing.
#[test]
fn hold_timer_after_release_is_ignored() {
    let mut r = Ring::new();
    r.send(TriggerDown);
    r.send(TriggerUp);
    assert_eq!(r.send(HoldElapsed), NONE);
    assert_eq!(r.phase(), Phase::Idle);
}

/// "Release on the center": closes with no action.
#[test]
fn release_on_center_closes_without_action() {
    let mut r = open_held();
    r.send(Pointer(Target::Center));
    assert_eq!(r.send(TriggerUp), [CloseRing(CloseReason::Center)]);
    assert_eq!(r.state, State::default());
}

/// "Release over an action, folder or empty area": the ring stays open in click mode.
#[test]
fn release_elsewhere_keeps_the_ring_open_in_click_mode() {
    for target in [ACTION, FOLDER, Target::Gap, Target::Outside] {
        let mut r = open_held();
        r.send(Pointer(target));
        assert_eq!(r.send(TriggerUp), [StayOpen], "released over {target:?}");
        assert_eq!(r.phase(), Phase::OpenClick);
        assert!(!r.state.trigger_held);
    }

    // Over an action inside the folder.
    let mut r = open_held();
    r.send(Pointer(FOLDER));
    r.send(Pointer(CHILD));
    assert_eq!(r.send(TriggerUp), [StayOpen]);
    assert_eq!(r.phase(), Phase::OpenClick);

    // Nothing highlighted (no movement since it opened).
    let mut r = open_held();
    assert_eq!(r.send(TriggerUp), [StayOpen]);
}

/// Default: "releasing over an icon runs it" is off.
#[test]
fn execute_on_release_is_off_by_default() {
    assert!(!Behavior::default().execute_on_release);
    assert_eq!(Behavior::default().open_with, OpenWith::Hold);
}

/// Quick mode: releasing over an action runs it; over the folder, it stays open.
#[test]
fn quick_mode_executes_on_release() {
    let quick = Behavior {
        execute_on_release: true,
        ..Behavior::default()
    };

    let mut r = Ring::with(quick.clone());
    r.send(TriggerDown);
    r.send(HoldElapsed);
    r.send(Pointer(ACTION));
    assert_eq!(
        r.send(TriggerUp),
        [Execute(ACTION), CloseRing(CloseReason::Execute)]
    );
    assert_eq!(r.phase(), Phase::Idle);

    let mut r = Ring::with(quick.clone());
    r.send(TriggerDown);
    r.send(HoldElapsed);
    r.send(Pointer(FOLDER));
    r.send(Pointer(CHILD));
    assert_eq!(
        r.send(TriggerUp),
        [Execute(CHILD), CloseRing(CloseReason::Execute)]
    );

    let mut r = Ring::with(quick);
    r.send(TriggerDown);
    r.send(HoldElapsed);
    r.send(Pointer(FOLDER));
    assert_eq!(r.send(TriggerUp), [StayOpen]);
}

/// "Click an action (holding or not)": runs it and closes. While holding,
/// the later trigger release is swallowed (the native click does not fire).
#[test]
fn click_on_action_while_holding_executes_and_closes() {
    let mut r = open_held();
    r.send(Pointer(ACTION));
    assert_eq!(
        r.send(Click(ACTION)),
        [Execute(ACTION), CloseRing(CloseReason::Execute)]
    );
    assert_eq!(r.phase(), Phase::Idle);
    assert!(r.state.trigger_held);

    assert_eq!(r.send(TriggerUp), [SwallowUp]);
    assert_eq!(r.state, State::default());
}

/// "Click an action (holding or not)": runs it and closes, in click mode.
#[test]
fn click_on_action_in_click_mode_executes_and_closes() {
    let mut r = open_click();
    assert_eq!(
        r.send(Click(ACTION)),
        [Execute(ACTION), CloseRing(CloseReason::Execute)]
    );
    assert_eq!(r.state, State::default());
}

/// An action inside the folder also runs and closes.
#[test]
fn click_on_folder_child_executes_and_closes() {
    let mut r = folder_open();
    r.send(Pointer(CHILD));
    assert_eq!(
        r.send(Click(CHILD)),
        [Execute(CHILD), CloseRing(CloseReason::Execute)]
    );
    assert_eq!(r.state, State::default());
}

/// "Click a folder": opens the folder and does not close the ring.
#[test]
fn click_on_folder_opens_it_without_closing() {
    let mut r = open_click();
    assert_eq!(r.send(Click(FOLDER)), [OpenFolder { slot: 2 }]);
    assert_eq!(r.phase(), Phase::OpenClick);
    assert_eq!(r.state.folder, Some(2));
    assert_eq!(r.state.hover, Some(FOLDER));

    // The same folder again: nothing changes.
    let before = r.state;
    assert_eq!(r.send(Click(FOLDER)), NONE);
    assert_eq!(r.state, before);

    // Another folder: switches right away, without waiting for the tolerance.
    assert_eq!(
        r.send(Click(FOLDER_2)),
        [CloseFolder { slot: 2 }, OpenFolder { slot: 4 }]
    );
    assert_eq!(r.state.folder, Some(4));
}

/// Click on a folder while another item was waiting for the tolerance.
#[test]
fn click_on_folder_cancels_pending_intent() {
    let mut r = folder_open();
    assert_eq!(r.send(Pointer(FOLDER_2)), [StartIntent(FOLDER_2)]);
    assert_eq!(
        r.send(Click(FOLDER_2)),
        [
            CancelIntent,
            CloseFolder { slot: 2 },
            OpenFolder { slot: 4 }
        ]
    );
    assert_eq!(r.state.pending, None);
}

/// "Click between the items": ignored.
#[test]
fn click_between_items_is_ignored() {
    let mut r = folder_open();
    let before = r.state;
    assert_eq!(r.send(Click(Target::Gap)), [IgnoreClick]);
    assert_eq!(r.state, before);
}

/// "Click outside the ring": closes; the click is discarded.
#[test]
fn click_outside_closes_and_is_discarded() {
    let mut r = open_click();
    assert_eq!(
        r.send(Click(Target::Outside)),
        [SwallowClick, CloseRing(CloseReason::Outside)]
    );
    assert_eq!(r.state, State::default());
}

/// A click on the center button (×) closes with no action.
#[test]
fn click_on_center_closes_without_action() {
    let mut r = open_click();
    assert_eq!(
        r.send(Click(Target::Center)),
        [CloseRing(CloseReason::Center)]
    );
}

/// "Esc": closes.
#[test]
fn esc_closes() {
    let mut r = open_click();
    assert_eq!(r.send(Esc), [CloseRing(CloseReason::Esc)]);
    assert_eq!(r.state, State::default());

    let mut r = open_held();
    assert_eq!(r.send(Esc), [CloseRing(CloseReason::Esc)]);
    assert_eq!(r.send(TriggerUp), [SwallowUp]);
}

/// "Trigger again": closes, and the release of that second press is swallowed.
#[test]
fn trigger_again_closes() {
    let mut r = open_click();
    assert_eq!(r.send(TriggerDown), [CloseRing(CloseReason::Trigger)]);
    assert_eq!(r.phase(), Phase::Idle);
    assert_eq!(r.send(TriggerUp), [SwallowUp]);
    assert_eq!(r.state, State::default());
}

/// "Window loses focus": closes.
#[test]
fn focus_lost_closes() {
    let mut r = folder_open();
    assert_eq!(r.send(FocusLost), [CloseRing(CloseReason::FocusLost)]);
    assert_eq!(r.state, State::default());
}

/// Esc before the ring opens cancels the wait and does not pass the native click on.
#[test]
fn esc_while_armed_cancels_the_hold() {
    let mut r = Ring::new();
    r.send(TriggerDown);
    assert_eq!(r.send(Esc), [CancelHold]);
    assert_eq!(r.phase(), Phase::Idle);
    assert_eq!(r.send(TriggerUp), [SwallowUp]);
}

// ---------------------------------------------------------------------------
// Free cursor and path tolerance (50 ms)
// ---------------------------------------------------------------------------

/// "Free cursor": with no folder open, the highlight follows the item under the cursor.
#[test]
fn highlight_follows_the_cursor() {
    let mut r = open_held();
    assert_eq!(r.send(Pointer(ACTION)), NONE);
    assert_eq!(r.state.hover, Some(ACTION));
    assert_eq!(r.send(Pointer(ACTION_2)), NONE);
    assert_eq!(r.state.hover, Some(ACTION_2));
    assert_eq!(r.send(Pointer(Target::Gap)), NONE);
    assert_eq!(r.state.hover, None);

    // A folder with no other one open: opens right away.
    assert_eq!(r.send(Pointer(FOLDER)), [OpenFolder { slot: 2 }]);
    assert_eq!(r.state.hover, Some(FOLDER));
}

/// With the folder open, another item only gets the highlight after the tolerance.
#[test]
fn with_folder_open_other_items_wait_for_tolerance() {
    let mut r = folder_open();
    assert_eq!(r.send(Pointer(ACTION)), [StartIntent(ACTION)]);
    assert_eq!(r.state.hover, None);
    assert_eq!(r.state.folder, Some(2));
    assert_eq!(r.state.pending, Some(ACTION));

    assert_eq!(r.send(IntentElapsed { slot: 0 }), [CloseFolder { slot: 2 }]);
    assert_eq!(r.state.hover, Some(ACTION));
    assert_eq!(r.state.folder, None);
    assert_eq!(r.state.pending, None);
}

/// Resting on another folder for the tolerance switches folders.
#[test]
fn tolerance_on_another_folder_switches_folders() {
    let mut r = folder_open();
    assert_eq!(r.send(Pointer(FOLDER_2)), [StartIntent(FOLDER_2)]);
    assert_eq!(
        r.send(IntentElapsed { slot: 4 }),
        [CloseFolder { slot: 2 }, OpenFolder { slot: 4 }]
    );
    assert_eq!(r.state.folder, Some(4));
    assert_eq!(r.state.hover, Some(FOLDER_2));
}

/// Passing over an item on the way to a folder action does not close the folder.
#[test]
fn passing_over_an_item_on_the_way_cancels_the_intent() {
    let mut r = folder_open();
    r.send(Pointer(ACTION));
    assert_eq!(r.send(Pointer(CHILD)), [CancelIntent]);
    assert_eq!(r.state.hover, Some(CHILD));
    assert_eq!(r.state.folder, Some(2));

    // The timer that had already fired arrives late: ignored.
    let before = r.state;
    assert_eq!(r.send(IntentElapsed { slot: 0 }), NONE);
    assert_eq!(r.state, before);
}

/// Staying on the same item does not restart the wait.
#[test]
fn staying_on_the_pending_item_does_not_restart_the_timer() {
    let mut r = folder_open();
    r.send(Pointer(ACTION));
    assert_eq!(r.send(Pointer(ACTION)), NONE);
    assert_eq!(r.state.pending, Some(ACTION));
}

/// "The center closes the folder right away."
#[test]
fn center_closes_the_folder_immediately() {
    let mut r = folder_open();
    assert_eq!(r.send(Pointer(Target::Center)), [CloseFolder { slot: 2 }]);
    assert_eq!(r.state.folder, None);
    assert_eq!(r.state.hover, Some(Target::Center));

    // It also cancels a wait in progress.
    let mut r = folder_open();
    r.send(Pointer(ACTION));
    assert_eq!(
        r.send(Pointer(Target::Center)),
        [CancelIntent, CloseFolder { slot: 2 }]
    );
}

/// Between the items or outside, the folder stays open.
#[test]
fn gap_and_outside_keep_the_folder_open() {
    for target in [Target::Gap, Target::Outside] {
        let mut r = folder_open();
        assert_eq!(r.send(Pointer(target)), NONE);
        assert_eq!(r.state.folder, Some(2));
        assert_eq!(r.state.hover, None);
    }
}

/// Going back to the open folder itself waits for nothing.
#[test]
fn returning_to_the_open_folder_is_immediate() {
    let mut r = folder_open();
    r.send(Pointer(CHILD));
    assert_eq!(r.send(Pointer(FOLDER)), NONE);
    assert_eq!(r.state.hover, Some(FOLDER));
}

/// Closing the ring with a wait in progress stops the timer.
#[test]
fn closing_cancels_the_pending_intent() {
    let mut r = folder_open();
    r.send(Pointer(ACTION));
    assert_eq!(r.send(Esc), [CancelIntent, CloseRing(CloseReason::Esc)]);
}

// ---------------------------------------------------------------------------
// "Tap" mode and events with the ring closed
// ---------------------------------------------------------------------------

/// "Tap" mode: a tap opens and closes; the trigger loses its native function.
#[test]
fn tap_mode_opens_and_closes_with_taps() {
    let mut r = Ring::with(Behavior {
        open_with: OpenWith::Tap,
        ..Behavior::default()
    });
    let mut all = Vec::new();

    let fx = r.send(TriggerDown);
    assert_eq!(fx, [OpenRing]);
    assert_eq!(r.phase(), Phase::OpenClick);
    all.extend(fx);

    let fx = r.send(TriggerUp);
    assert_eq!(fx, [SwallowUp]);
    assert_eq!(r.phase(), Phase::OpenClick);
    all.extend(fx);

    let fx = r.send(TriggerDown);
    assert_eq!(fx, [CloseRing(CloseReason::Trigger)]);
    all.extend(fx);

    let fx = r.send(TriggerUp);
    assert_eq!(fx, [SwallowUp]);
    all.extend(fx);

    assert!(!all.contains(&PassNative));
    assert_eq!(r.state, State::default());
}

/// With the ring closed, cursor, click and late timers do nothing.
#[test]
fn events_with_the_ring_closed_do_nothing() {
    let mut r = Ring::new();
    for event in [
        Pointer(ACTION),
        Click(ACTION),
        Click(Target::Outside),
        IntentElapsed { slot: 0 },
        HoldElapsed,
        Esc,
        FocusLost,
        TriggerUp,
    ] {
        assert_eq!(r.send(event), NONE, "{event:?}");
        assert_eq!(r.state, State::default(), "{event:?}");
    }
}
