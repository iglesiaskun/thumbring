//! Ring geometry, placement on the screen and hit-testing.

use std::f64::consts::PI;

use thumbring_core::layout::{Geometry, ITEM_DIAMETER_PX, OpenFolder, Vec2, hero_index};
use thumbring_core::ring::{Action, Ring, Slot};
use thumbring_core::ring_machine::Target;
use thumbring_core::screen::Rect;

const EPS: f64 = 1e-9;

fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn assert_near(a: Vec2, b: Vec2) {
    assert!((a - b).length() < 1e-6, "{a:?} != {b:?}");
}

fn geometry() -> Geometry {
    Geometry::new(ITEM_DIAMETER_PX)
}

fn action(name: &str) -> Action {
    Action {
        name: name.into(),
        icon: None,
        command: None,
    }
}

/// Slot 0: action. Slot 1: empty. Slot 2: folder with 5 actions. The rest empty.
fn ring() -> Ring {
    let mut ring = Ring::empty();
    ring.slots[0] = Slot::Action(action("A"));
    ring.slots[2] = Slot::Folder {
        name: "F".into(),
        icon: None,
        items: (0..5).map(|i| action(&format!("f{i}"))).collect(),
    };
    ring
}

// ---------------------------------------------------------------------------
// Sizes and positions
// ---------------------------------------------------------------------------

#[test]
fn sizes_follow_the_ring_proportions() {
    let g = geometry();
    assert_close(g.d, 52.0);
    assert_close(g.r, 26.0);
    assert_close(g.ring_r, 1.8 * 52.0);
    assert_close(g.folder_r, 2.0 * g.ring_r);
    // The center button is 0.7 D wide.
    assert_close(g.center_r * 2.0, 0.7 * 52.0);
    assert_close(g.cancel_r, 0.45 * g.ring_r);
    assert_close(g.child_spacing, PI / 8.0);
}

#[test]
fn slots_start_at_the_top_and_go_clockwise() {
    let g = geometry();
    let r = g.ring_r;
    assert_near(g.slot_offset(0), Vec2::new(0.0, -r));
    assert_near(g.slot_offset(2), Vec2::new(r, 0.0));
    assert_near(g.slot_offset(4), Vec2::new(0.0, r));
    assert_near(g.slot_offset(6), Vec2::new(-r, 0.0));
    // 45° apart: slot 1 is up and to the right.
    let s1 = g.slot_offset(1);
    assert_close(s1.x, -s1.y);
    assert!(s1.x > 0.0);
}

#[test]
fn folder_children_are_centered_on_the_folder_direction() {
    let g = geometry();
    let arc = g.folder_r;
    // Odd count: the middle child is right on the folder direction.
    assert_near(g.child_offset(2, 2, 5), Vec2::new(arc, 0.0));
    // 22.5° between children.
    assert_near(g.child_offset(2, 0, 5), Vec2::from_angle(-PI / 4.0) * arc);
    assert_near(g.child_offset(2, 4, 5), Vec2::from_angle(PI / 4.0) * arc);
    // Even count: symmetric around the folder direction.
    let sum = (0..4)
        .map(|j| g.child_offset(0, j, 4))
        .fold(Vec2::ZERO, |acc, v| acc + v);
    assert!(sum.x.abs() < 1e-6, "{sum:?}");
    // Nine children cover half a circle.
    assert_near(g.child_offset(0, 0, 9), Vec2::new(-arc, 0.0));
    assert_near(g.child_offset(0, 8, 9), Vec2::new(arc, 0.0));
}

#[test]
fn the_blob_goes_to_the_middle_child() {
    assert_eq!(hero_index(1), 0);
    assert_eq!(hero_index(2), 1);
    assert_eq!(hero_index(5), 2);
    assert_eq!(hero_index(6), 3);
    assert_eq!(hero_index(9), 4);
}

// ---------------------------------------------------------------------------
// Hit-test
// ---------------------------------------------------------------------------

#[test]
fn the_middle_is_the_cancel_zone() {
    let g = geometry();
    let ring = ring();
    assert_eq!(g.hit_test(&ring, Vec2::ZERO, None), Target::Center);
    let edge = g.cancel_r;
    assert_eq!(
        g.hit_test(&ring, Vec2::new(edge - EPS, 0.0), None),
        Target::Center
    );
    assert_eq!(
        g.hit_test(&ring, Vec2::new(edge + 1.0, 0.0), None),
        Target::Gap
    );
}

#[test]
fn items_react_a_little_beyond_their_edge() {
    let g = geometry();
    let ring = ring();
    let item = g.slot_offset(0);
    assert_eq!(g.hit_test(&ring, item, None), Target::Action { slot: 0 });
    // 1.12 of the radius still counts...
    let inside = item + Vec2::new(g.r * 1.12 - EPS, 0.0);
    assert_eq!(g.hit_test(&ring, inside, None), Target::Action { slot: 0 });
    // ...a bit more is already the gap.
    let outside = item + Vec2::new(g.r * 1.12 + 0.01, 0.0);
    assert_eq!(g.hit_test(&ring, outside, None), Target::Gap);
}

#[test]
fn folders_empty_slots_and_far_away() {
    let g = geometry();
    let ring = ring();
    assert_eq!(
        g.hit_test(&ring, g.slot_offset(2), None),
        Target::Folder { slot: 2 }
    );
    // Nothing is drawn in an empty slot: it is the gap between items.
    assert_eq!(g.hit_test(&ring, g.slot_offset(1), None), Target::Gap);
    let outer = g.outer_r();
    assert_eq!(
        g.hit_test(&ring, Vec2::new(0.0, outer - 0.5), None),
        Target::Gap
    );
    assert_eq!(
        g.hit_test(&ring, Vec2::new(0.0, outer + 0.5), None),
        Target::Outside
    );
}

#[test]
fn children_are_hit_only_once_they_are_visible_enough() {
    let g = geometry();
    let ring = ring();
    let child = g.child_offset(2, 3, 5);
    let mut appear = [1.0; 5];
    let open = |appear: &[f64; 5]| {
        let folder = OpenFolder {
            slot: 2,
            appear: &appear[..],
        };
        g.hit_test(&ring, child, Some(folder))
    };
    assert_eq!(open(&appear), Target::Child { slot: 2, index: 3 });
    appear[3] = 0.6;
    assert_eq!(open(&appear), Target::Child { slot: 2, index: 3 });
    appear[3] = 0.59;
    assert_eq!(open(&appear), Target::Gap);
    // A closed folder has no children to hit.
    assert_eq!(g.hit_test(&ring, child, None), Target::Gap);
}

// ---------------------------------------------------------------------------
// Placement and size on the screen
// ---------------------------------------------------------------------------

const FULL_HD: Rect = Rect::new(0, 0, 1920, 1080);

#[test]
fn the_ring_opens_on_the_cursor() {
    let g = geometry();
    let cursor = Vec2::new(960.0, 540.0);
    assert_near(g.place(cursor, FULL_HD), cursor);
}

#[test]
fn near_an_edge_the_ring_moves_in_just_enough_for_the_folder_arc() {
    let g = geometry();
    let m = g.screen_margin();
    assert_close(m, g.folder_r + g.r + 10.0);
    let placed = g.place(Vec2::new(5.0, 1075.0), FULL_HD);
    assert_near(placed, Vec2::new(m, 1080.0 - m));
}

#[test]
fn a_second_monitor_with_negative_coordinates_works_the_same() {
    let g = geometry();
    let m = g.screen_margin();
    let left = Rect::new(-1920, 0, 0, 1080);
    let placed = g.place(Vec2::new(-5.0, 300.0), left);
    assert_near(placed, Vec2::new(-m, 300.0));
}

#[test]
fn on_a_monitor_too_small_the_ring_opens_in_its_middle() {
    let g = geometry();
    let tiny = Rect::new(100, 50, 500, 350);
    assert_near(
        g.place(Vec2::new(110.0, 60.0), tiny),
        Vec2::new(300.0, 200.0),
    );
}

#[test]
fn item_size_follows_the_monitor_scale() {
    assert_close(Geometry::for_monitor(1.0, FULL_HD).d, 52.0);
    let scaled = Rect::new(0, 0, 2880, 1620);
    assert_close(Geometry::for_monitor(1.5, scaled).d, 78.0);
}

#[test]
fn a_small_monitor_shrinks_the_ring_down_to_a_minimum() {
    let small = Geometry::for_monitor(1.0, Rect::new(0, 0, 800, 400));
    assert_close(small.d, (400.0 - 24.0) / 8.4);
    let tiny = Geometry::for_monitor(1.0, Rect::new(0, 0, 200, 200));
    assert_close(tiny.d, 30.0);
}

#[test]
fn extra_margins_grow_with_the_item() {
    let normal = Geometry::new(52.0);
    let big = Geometry::new(104.0);
    assert_close(big.screen_margin(), 2.0 * normal.screen_margin());
    assert_close(big.outer_r(), 2.0 * normal.outer_r());
}
