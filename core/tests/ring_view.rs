//! Visual state of the ring driven by the state machine, the way the engine
//! uses it: events go through `reduce`, effects go to the view, and every
//! frame asks the view for the scene to draw.

use thumbring_core::anim::Timing;
use thumbring_core::config::{Animation, Behavior};
use thumbring_core::layout::{Geometry, ITEM_DIAMETER_PX, Vec2};
use thumbring_core::ring::{Action, Ring, Slot};
use thumbring_core::ring_machine::{Effect, Event, State, Target, reduce};
use thumbring_core::screen::Rect;
use thumbring_core::view::{IconKey, ItemKey, LabelDraw, RingView, Scene, label_position};

const CENTER: Vec2 = Vec2::new(960.0, 540.0);

fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn action(name: &str) -> Action {
    Action {
        name: name.into(),
        icon: None,
        command: None,
    }
}

fn folder(name: &str, count: usize) -> Slot {
    Slot::Folder {
        name: name.into(),
        icon: None,
        items: (0..count).map(|i| action(&format!("{name}{i}"))).collect(),
    }
}

/// Slot 0: action. Slot 2: folder with 5 actions. Slot 4: folder with 3. The
/// rest empty.
fn ring() -> Ring {
    let mut ring = Ring::empty();
    ring.slots[0] = Slot::Action(action("A"));
    ring.slots[2] = folder("F", 5);
    ring.slots[4] = folder("G", 3);
    ring
}

/// Plays the engine's role.
struct Engine {
    ring: Ring,
    geometry: Geometry,
    animation: Animation,
    timing: Timing,
    behavior: Behavior,
    state: State,
    view: Option<RingView>,
}

impl Engine {
    fn new() -> Self {
        Self {
            ring: ring(),
            geometry: Geometry::new(ITEM_DIAMETER_PX),
            animation: Animation::default(),
            timing: Timing::default(),
            behavior: Behavior {
                open_with: thumbring_core::config::OpenWith::Tap,
                ..Behavior::default()
            },
            state: State::default(),
            view: None,
        }
    }

    /// Opens the ring in click mode at time 0.
    fn opened() -> Self {
        let mut e = Self::new();
        e.send(Event::TriggerDown, 0.0);
        e.send(Event::TriggerUp, 0.0);
        e
    }

    fn send(&mut self, event: Event, now: f64) -> Vec<Effect> {
        let (state, effects) = reduce(self.state, event, &self.behavior);
        self.state = state;
        for effect in &effects {
            if *effect == Effect::OpenRing {
                self.view = Some(RingView::open(CENTER, now));
            }
            if let Some(view) = self.view.as_mut() {
                view.apply(effect, now, &self.timing);
            }
        }
        if let Some(view) = self.view.as_mut() {
            view.sync(&self.state, now, &self.timing);
        }
        effects
    }

    fn point(&mut self, target: Target, now: f64) {
        self.send(Event::Pointer(target), now);
    }

    fn scene(&mut self, now: f64) -> Option<Scene> {
        let view = self.view.as_mut()?;
        view.scene(
            &self.ring,
            &self.geometry,
            &self.animation,
            &self.timing,
            now,
        )
    }
}

fn icon(scene: &Scene, key: IconKey) -> thumbring_core::view::IconDraw {
    *scene
        .icons
        .iter()
        .find(|i| i.key == key)
        .unwrap_or_else(|| panic!("no icon {key:?}"))
}

// ---------------------------------------------------------------------------
// Opening
// ---------------------------------------------------------------------------

#[test]
fn an_open_ring_draws_its_items_and_nothing_for_empty_slots() {
    let mut e = Engine::opened();
    let s = e.scene(150.0).expect("open");
    // Three items, plus the bumps of the two folders.
    assert_eq!(s.white.len(), 5);
    assert!(s.dark.is_empty());
    assert!(s.blob.is_none() && s.hero.is_none());
    // Three item icons and the ×.
    assert_eq!(s.icons.len(), 4);
    assert_close(s.alpha, 1.0);
    assert!(s.label.is_none());
    assert!(!s.active, "nothing left to animate");
    // Items at 1.8 D from the center.
    let top = s.white[0];
    assert_close(top.center.x, CENTER.x);
    assert_close(CENTER.y - top.center.y, 1.8 * 52.0);
    assert_close(top.r, 26.0);
}

#[test]
fn at_the_first_frame_everything_is_small_and_invisible() {
    let mut e = Engine::opened();
    let s = e.scene(0.0).expect("open");
    assert_close(s.alpha, 0.0);
    assert!(s.active);
    let top = s.white[0];
    assert_close(CENTER.y - top.center.y, 1.8 * 52.0 * 0.15);
    assert_close(top.r, 26.0 * 0.4);
}

#[test]
fn the_folder_bump_points_outwards() {
    let mut e = Engine::opened();
    let s = e.scene(150.0).unwrap();
    let folder = e.geometry.slot_offset(2) + CENTER;
    let bump = s
        .white
        .iter()
        .find(|c| c.r < 10.0 && (c.center - folder).length() < 40.0)
        .expect("bump");
    // Right of the folder, which sits on the right of the ring.
    assert!(bump.center.x > folder.x);
    assert_close(bump.r, 26.0 * 0.22);
}

// ---------------------------------------------------------------------------
// Highlight, center and label
// ---------------------------------------------------------------------------

#[test]
fn hovering_an_action_darkens_it_and_shows_its_label_60_ms_later() {
    let mut e = Engine::opened();
    e.point(Target::Action { slot: 0 }, 200.0);
    let s = e.scene(200.0).unwrap();
    assert_eq!(s.dark.len(), 1);
    assert_close(s.dark[0].r, 26.0 * 0.6);
    assert!(s.label.is_none(), "label waits 60 ms");
    let s = e.scene(270.0).unwrap();
    assert_close(s.dark[0].r, 26.0);
    assert!(icon(&s, IconKey::Item(ItemKey::Slot(0))).on);
    let label = e.scene(285.0).unwrap().label.expect("label fading in");
    assert_eq!(label.target, Target::Action { slot: 0 });
    assert_close(label.opacity, 0.5);
    assert!(e.scene(285.0).unwrap().active);
}

#[test]
fn moving_away_hides_the_highlight_and_the_label() {
    let mut e = Engine::opened();
    e.point(Target::Action { slot: 0 }, 200.0);
    e.point(Target::Gap, 400.0);
    let s = e.scene(400.0).unwrap();
    assert_close(s.label.unwrap().opacity, 1.0);
    let s = e.scene(450.0).unwrap();
    assert_close(s.label.unwrap().opacity, 0.5);
    let s = e.scene(500.0).unwrap();
    assert!(s.label.is_none());
    assert!(s.dark.is_empty());
    assert!(!s.active);
}

#[test]
fn hovering_a_folder_opens_it_without_a_label_and_hides_its_bump() {
    let mut e = Engine::opened();
    e.point(Target::Folder { slot: 2 }, 200.0);
    let s = e.scene(300.0).unwrap();
    assert!(s.label.is_none(), "folders have no label");
    let folder = e.geometry.slot_offset(2) + CENTER;
    let bump_left = s
        .white
        .iter()
        .any(|c| c.r < 10.0 && (c.center - folder).length() < 40.0);
    assert!(!bump_left, "the bump goes away while the folder is dark");
}

#[test]
fn the_center_turns_red_in_80_ms() {
    let mut e = Engine::opened();
    e.point(Target::Center, 200.0);
    assert_close(e.scene(240.0).unwrap().center_red, 0.5);
    let s = e.scene(280.0).unwrap();
    assert_close(s.center_red, 1.0);
    assert!(icon(&s, IconKey::Center).on);
    assert_close(s.center.r, 0.35 * 52.0);
}

// ---------------------------------------------------------------------------
// Folders
// ---------------------------------------------------------------------------

/// The blob of a 5-child folder lands 35 ms + 135 ms × 1.8 after opening.
const LANDING: f64 = 35.0 + 135.0 * 1.8;

#[test]
fn opening_a_folder_sends_the_blob_and_then_shows_the_children() {
    let mut e = Engine::opened();
    e.point(Target::Folder { slot: 2 }, 200.0);
    let s = e.scene(200.0 + 120.0).unwrap();
    assert!(s.blob.is_some(), "the blob travels");
    assert!(!s.dark.is_empty(), "with its neck and the dark folder");
    // The blob carries the hero icon.
    let hero_icon = icon(&s, IconKey::Item(ItemKey::Child { slot: 2, index: 2 }));
    assert!(hero_icon.on && hero_icon.opacity > 0.0);

    let s = e.scene(200.0 + LANDING + 171.0).unwrap();
    assert!(s.blob.is_none());
    let hero = s.hero.expect("the hero is drawn on its own");
    assert_close(hero.r, 26.0);
    // Items, the bump of the other folder and the four siblings.
    assert_eq!(s.white.len(), 3 + 1 + 4);
    let sibling = icon(&s, IconKey::Item(ItemKey::Child { slot: 2, index: 0 }));
    assert_close(sibling.scale, 1.0);
    assert!(!s.active);
}

#[test]
fn the_hit_test_sees_the_children_the_view_shows() {
    let mut e = Engine::opened();
    e.point(Target::Folder { slot: 2 }, 200.0);
    let view = e.view.as_ref().unwrap();
    let (slot, before) = view
        .open_folder_children(&e.ring, &e.geometry, &e.animation, &e.timing, 200.0 + 100.0)
        .expect("open folder");
    assert_eq!(slot, 2);
    assert!(before.iter().all(|&c| c == 0.0));
    let (_, after) = view
        .open_folder_children(
            &e.ring,
            &e.geometry,
            &e.animation,
            &e.timing,
            200.0 + LANDING + 171.0,
        )
        .unwrap();
    assert!(after.iter().all(|&c| c == 1.0));
}

#[test]
fn opening_another_folder_closes_the_first_one() {
    let mut e = Engine::opened();
    e.send(Event::Click(Target::Folder { slot: 2 }), 200.0);
    e.send(Event::Click(Target::Folder { slot: 4 }), 900.0);
    let view = e.view.as_ref().unwrap();
    let (slot, _) = view
        .open_folder_children(&e.ring, &e.geometry, &e.animation, &e.timing, 950.0)
        .unwrap();
    assert_eq!(slot, 4);
    // Halfway through closing, the children of the first folder are smaller.
    let s = e.scene(950.0).unwrap();
    let old = icon(&s, IconKey::Item(ItemKey::Child { slot: 2, index: 0 }));
    assert_close(old.scale, 1.0 - 0.125);
    let s = e.scene(1001.0).unwrap();
    let old = icon(&s, IconKey::Item(ItemKey::Child { slot: 2, index: 0 }));
    assert_close(old.scale, 0.0);
}

#[test]
fn an_empty_folder_opens_without_a_blob() {
    let mut e = Engine::opened();
    e.ring.slots[6] = folder("E", 0);
    e.point(Target::Folder { slot: 6 }, 200.0);
    let s = e.scene(300.0).unwrap();
    assert!(s.blob.is_none());
}

// ---------------------------------------------------------------------------
// Closing
// ---------------------------------------------------------------------------

#[test]
fn closing_turns_the_dark_layer_grey_and_ends_in_170_ms() {
    let mut e = Engine::opened();
    e.point(Target::Action { slot: 0 }, 200.0);
    let effects = e.send(Event::Click(Target::Action { slot: 0 }), 500.0);
    assert!(effects.contains(&Effect::Execute(Target::Action { slot: 0 })));
    let s = e.scene(520.0).unwrap();
    assert_eq!(s.dark_color, [0.56, 0.56, 0.6]);
    // The action that ran keeps its disc while the ring closes.
    assert_eq!(s.dark.len(), 1);
    assert!(s.active);
    assert!(e.scene(670.0).is_none(), "nothing left to draw");
}

// ---------------------------------------------------------------------------
// Label placement
// ---------------------------------------------------------------------------

const SCREEN: Rect = Rect::new(0, 0, 1920, 1080);

fn label_at(item: Vec2) -> LabelDraw {
    LabelDraw {
        target: Target::Action { slot: 0 },
        item,
        item_r: 26.0,
        opacity: 1.0,
    }
}

#[test]
fn labels_go_outside_the_ring() {
    let size = (120.0, 30.0);
    // Item on the right: label to its right, vertically centered.
    let right = label_position(
        &label_at(CENTER + Vec2::new(94.0, 0.0)),
        CENTER,
        size,
        10.0,
        SCREEN,
    );
    assert_close(right.x, CENTER.x + 94.0 + 26.0 + 10.0);
    assert_close(right.y, CENTER.y - 15.0);
    // Item on the left: label to its left.
    let left = label_position(
        &label_at(CENTER - Vec2::new(94.0, 0.0)),
        CENTER,
        size,
        10.0,
        SCREEN,
    );
    assert_close(left.x, CENTER.x - 94.0 - 26.0 - 10.0 - 120.0);
    // Item at the top: label above it, horizontally centered.
    let top = label_position(
        &label_at(CENTER - Vec2::new(0.0, 94.0)),
        CENTER,
        size,
        10.0,
        SCREEN,
    );
    assert_close(top.x, CENTER.x - 60.0);
    assert_close(top.y, CENTER.y - 94.0 - 26.0 - 10.0 - 30.0);
}

#[test]
fn labels_stay_6_px_inside_the_screen() {
    let size = (300.0, 30.0);
    let near_edge = label_position(
        &label_at(Vec2::new(1800.0, 540.0)),
        Vec2::new(1700.0, 540.0),
        size,
        10.0,
        SCREEN,
    );
    assert_close(near_edge.x, 1920.0 - 300.0 - 6.0);
}
