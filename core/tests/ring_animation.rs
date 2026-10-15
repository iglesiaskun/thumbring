//! Ring animations with their default values:
//! open 150 ms, close 170 ms, highlight 70 ms, center 80 ms, label 60 + 50 ms,
//! blob 135 ms per item diameter with the neck breaking at 95%, children 170 ms,
//! folder close 100 ms.

use thumbring_core::anim::{
    Fade, FadeKind, NECK_CIRCLES, Timing, clamp01, ease_in, ease_in_out, ease_out, ease_out_back,
    folder_frame, ring_frame, smoothstep,
};
use thumbring_core::config::{Animation, BlobStyle};
use thumbring_core::layout::{Geometry, ITEM_DIAMETER_PX, OpenFolder};
use thumbring_core::ring::{Action, Ring, Slot};
use thumbring_core::ring_machine::Target;

fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn timing() -> Timing {
    Timing::default()
}

// ---------------------------------------------------------------------------
// Curves and durations
// ---------------------------------------------------------------------------

#[test]
fn curves_start_at_0_and_end_at_1() {
    for curve in [ease_out, ease_in, ease_in_out] {
        assert_close(curve(0.0), 0.0);
        assert_close(curve(1.0), 1.0);
    }
    assert_close(ease_in_out(0.5), 0.5);
    assert!(ease_out(0.5) > 0.5, "slows down at the end");
    assert!(ease_in(0.5) < 0.5, "speeds up at the end");
    assert_close(smoothstep(0.65, 1.0, 0.5), 0.0);
    assert_close(smoothstep(0.65, 1.0, 1.2), 1.0);
    assert_close(clamp01(-3.0), 0.0);
}

#[test]
fn ease_out_back_overshoots_by_its_strength() {
    assert_close(ease_out_back(0.0, 0.0), 0.0);
    assert_close(ease_out_back(1.0, 1.0), 1.0);
    let peak = (1..100)
        .map(|i| ease_out_back(f64::from(i) / 100.0, 1.0))
        .fold(0.0, f64::max);
    assert!(peak > 1.0, "{peak}");
    // Without strength, it never goes past the end.
    let flat = (1..100)
        .map(|i| ease_out_back(f64::from(i) / 100.0, 0.0))
        .fold(0.0, f64::max);
    assert!(flat <= 1.0, "{flat}");
}

#[test]
fn speed_and_reduced_motion_change_animation_durations() {
    assert_close(timing().dur(150.0), 150.0);
    let fast = Animation {
        speed: 2.0,
        ..Animation::default()
    };
    assert_close(Timing::new(&fast, false).dur(150.0), 75.0);
    // "Animation effects" off in Windows: about a third of the time.
    let reduced = Timing::new(&Animation::default(), true);
    assert!(reduced.reduced_motion);
    assert_close(reduced.dur(150.0), 52.5);
    // Unless the settings ask to ignore it.
    let ignore = Animation {
        ignore_system_reduced_motion: true,
        ..Animation::default()
    };
    assert!(!Timing::new(&ignore, true).reduced_motion);
}

// ---------------------------------------------------------------------------
// Ring open and close
// ---------------------------------------------------------------------------

#[test]
fn opening_grows_from_the_center_in_150_ms() {
    let a = Animation::default();
    let start = ring_frame(&a, &timing(), 0.0, None);
    assert_close(start.spread, 0.15);
    assert_close(start.size, 0.4);
    assert_close(start.alpha, 0.0);
    assert!(start.active && !start.closed);

    let open = ring_frame(&a, &timing(), 150.0, None);
    assert_close(open.spread, 1.0);
    assert_close(open.size, 1.0);
    assert_close(open.alpha, 1.0);
    assert!(!open.active);
}

#[test]
fn opening_slows_down_at_the_end() {
    let half = ring_frame(&Animation::default(), &timing(), 75.0, None);
    // ease_out(0.5) = 0.875.
    assert_close(half.spread, 0.15 + 0.85 * 0.875);
}

#[test]
fn closing_shrinks_to_the_center_and_fades_in_170_ms() {
    let a = Animation::default();
    let start = ring_frame(&a, &timing(), 1000.0, Some(0.0));
    assert_close(start.spread, 1.0);
    assert_close(start.alpha, 1.0);
    assert!(start.active && !start.closed);

    let end = ring_frame(&a, &timing(), 1000.0, Some(170.0));
    assert_close(end.spread, 0.15);
    assert_close(end.size, 0.35);
    assert_close(end.alpha, 0.0);
    assert!(!end.active && end.closed);
}

#[test]
fn the_overall_speed_applies_to_open_and_close() {
    let a = Animation {
        speed: 2.0,
        ..Animation::default()
    };
    let t = Timing::new(&a, false);
    assert!(!ring_frame(&a, &t, 75.0, None).active);
    assert!(ring_frame(&a, &t, 1000.0, Some(85.0)).closed);
}

// ---------------------------------------------------------------------------
// Highlight, center and label
// ---------------------------------------------------------------------------

#[test]
fn the_highlight_jumps_to_60_percent_and_grows_in_70_ms() {
    let t = timing();
    let mut f = Fade::new(FadeKind::Highlight);
    assert_close(f.value(0.0, &t), 0.0);
    f.set(true, 100.0, &t);
    assert_close(f.value(100.0, &t), 0.6);
    assert_close(f.value(170.0, &t), 1.0);
    assert!(f.is_animating(169.0, &t));
    assert!(!f.is_animating(171.0, &t));
}

#[test]
fn turning_a_fade_around_starts_from_where_it_was() {
    let t = timing();
    let mut f = Fade::new(FadeKind::Highlight);
    f.set(true, 0.0, &t);
    let halfway = f.value(35.0, &t);
    assert!(halfway > 0.6 && halfway < 1.0, "{halfway}");
    f.set(false, 35.0, &t);
    assert_close(f.value(35.0, &t), halfway);
    assert_close(f.value(105.0, &t), 0.0);
}

#[test]
fn setting_the_same_state_again_changes_nothing() {
    let t = timing();
    let mut f = Fade::new(FadeKind::Highlight);
    f.set(true, 0.0, &t);
    f.set(true, 50.0, &t);
    assert_close(f.value(70.0, &t), 1.0);
    assert!(f.is_on());
}

#[test]
fn the_center_turns_red_linearly_in_80_ms() {
    let t = timing();
    let mut f = Fade::new(FadeKind::Center);
    f.set(true, 0.0, &t);
    assert_close(f.value(40.0, &t), 0.5);
    assert_close(f.value(80.0, &t), 1.0);
    f.set(false, 100.0, &t);
    assert_close(f.value(140.0, &t), 0.5);
}

#[test]
fn the_label_shows_up_60_ms_later_with_a_50_ms_fade_and_hides_in_100_ms() {
    let t = timing();
    let mut f = Fade::new(FadeKind::Label);
    f.set(true, 0.0, &t);
    assert_close(f.value(59.0, &t), 0.0);
    assert_close(f.value(85.0, &t), 0.5);
    assert_close(f.value(110.0, &t), 1.0);
    f.set(false, 200.0, &t);
    assert_close(f.value(250.0, &t), 0.5);
    assert_close(f.value(300.0, &t), 0.0);
}

// ---------------------------------------------------------------------------
// Folder: the viscous blob (default)
// ---------------------------------------------------------------------------

const SLOT: u8 = 2;
const COUNT: usize = 5;
const DELAY: f64 = 35.0;

fn geometry() -> Geometry {
    Geometry::new(ITEM_DIAMETER_PX)
}

/// The middle child of an odd folder sits right on the folder direction, 1.8 D
/// away from the folder, so the trip takes 135 ms × 1.8.
fn travel() -> f64 {
    135.0 * 1.8
}

fn frame(
    a: &Animation,
    since_open: f64,
    since_close: Option<f64>,
) -> thumbring_core::anim::FolderFrame {
    folder_frame(
        &geometry(),
        a,
        &timing(),
        SLOT,
        COUNT,
        since_open,
        since_close,
    )
}

#[test]
fn nothing_moves_during_the_first_35_ms() {
    let f = frame(&Animation::default(), 20.0, None);
    assert!(f.blob.is_none());
    assert!(f.neck.is_empty());
    assert!(f.children.iter().all(|&c| c == 0.0));
    assert!(f.active);
    assert_eq!(f.hero, 2);
}

#[test]
fn the_blob_travels_from_the_folder_to_the_middle_child() {
    let a = Animation::default();
    let g = geometry();
    let mut last_x = g.slot_offset(SLOT).x;
    for step in 1..10 {
        let t = DELAY + travel() * f64::from(step) / 10.0;
        let blob = frame(&a, t, None).blob.expect("blob during the trip");
        // Straight line from the folder (right) to the middle child.
        assert!(blob.center.y.abs() < 1e-6, "{blob:?}");
        assert!(blob.center.x >= last_x, "goes forward: {blob:?}");
        assert!(blob.center.x <= g.child_offset(SLOT, 2, COUNT).x + 1e-6);
        last_x = blob.center.x;
    }
}

#[test]
fn before_the_break_a_thick_neck_ties_the_blob_to_the_folder() {
    let a = Animation::default();
    let f = frame(&a, DELAY + travel() * 0.5, None);
    assert_eq!(f.neck.len(), NECK_CIRCLES);
    let blob = f.blob.unwrap();
    // Leaves big: between 35% and 85% of an item.
    assert!(blob.r > 0.35 * 26.0 && blob.r < 0.85 * 26.0, "{blob:?}");
    assert!(f.hero_on_blob > 0.0);
}

#[test]
fn the_neck_breaks_at_95_percent_and_its_remnants_go_away_in_70_ms() {
    let a = Animation::default();
    assert_close(a.break_at, 0.95);
    let broke = DELAY + travel() * 0.95;
    let just_after = frame(&a, broke + 5.0, None);
    assert_eq!(just_after.neck.len(), NECK_CIRCLES);
    let near_end = frame(&a, broke + 69.0, None);
    assert!(
        near_end.neck.iter().all(|c| c.r < 1.0),
        "{:?}",
        near_end.neck
    );
    assert!(frame(&a, broke + 71.0, None).neck.is_empty());
}

#[test]
fn landing_reveals_the_hero_then_the_siblings_in_170_ms() {
    let a = Animation::default();
    let landed = DELAY + travel();
    let f = frame(&a, landed - 1.0, None);
    assert!(f.children.iter().all(|&c| c == 0.0), "still travelling");

    let f = frame(&a, landed, None);
    assert_close(f.children[2], 1.0);
    assert_close(f.children[0], 0.0);
    // The dark blob is still full size over the hero...
    assert_close(f.blob.unwrap().r, 26.0);

    let f = frame(&a, landed + 170.0, None);
    assert!(f.children.iter().all(|&c| (c - 1.0).abs() < 1e-9));
    // ...and has shrunk away after 150 ms.
    assert!(f.blob.is_none());
    assert!(
        !frame(&a, landed + 171.0, None).active,
        "settled without wobble"
    );
}

#[test]
fn the_trip_time_follows_the_distance_within_limits() {
    let a = Animation::default();
    let g = geometry();
    // Even count: the hero is off the folder direction, a bit farther away.
    let count = 4;
    let hero = g.child_offset(SLOT, 2, count);
    let distance = (hero - g.slot_offset(SLOT)).length();
    let trip = 135.0 * distance / g.d;
    let landed = DELAY + trip;
    let t = timing();
    let before = folder_frame(&g, &a, &t, SLOT, count, landed - 0.5, None);
    let after = folder_frame(&g, &a, &t, SLOT, count, landed + 0.5, None);
    assert_close(before.children[2], 0.0);
    assert_close(after.children[2], 1.0);
    // Very slow blob settings are capped at 520 ms.
    let slow = Animation {
        blob_ms_per_item: 1000,
        ..Animation::default()
    };
    let capped = folder_frame(&g, &slow, &t, SLOT, COUNT, DELAY + 520.0 + 0.5, None);
    assert_close(capped.children[2], 1.0);
}

#[test]
fn closing_the_folder_shrinks_the_children_in_100_ms() {
    let a = Animation::default();
    let half = frame(&a, 2000.0, Some(50.0));
    assert!(
        half.children
            .iter()
            .all(|&c| (c - (1.0 - 0.125)).abs() < 1e-9)
    );
    assert!(half.active);
    let done = frame(&a, 2000.0, Some(100.0));
    assert!(done.children.iter().all(|&c| c == 0.0));
    assert!(done.finished && !done.active);
}

#[test]
fn hit_test_follows_the_children_animation() {
    let a = Animation::default();
    let g = geometry();
    let mut ring = Ring::empty();
    ring.slots[SLOT as usize] = Slot::Folder {
        name: "F".into(),
        icon: None,
        items: (0..COUNT)
            .map(|i| Action {
                name: format!("f{i}"),
                icon: None,
                command: None,
            })
            .collect(),
    };
    let landed = DELAY + travel();
    let hit = |since_open: f64, index: usize| {
        let f = frame(&a, since_open, None);
        let open = OpenFolder {
            slot: SLOT,
            appear: &f.children,
        };
        g.hit_test(&ring, g.child_offset(SLOT, index, COUNT), Some(open))
    };
    // Right after landing only the hero can be clicked...
    assert_eq!(
        hit(landed, 2),
        Target::Child {
            slot: SLOT,
            index: 2
        }
    );
    assert_eq!(hit(landed, 0), Target::Gap);
    // ...and the siblings once they have appeared.
    assert_eq!(
        hit(landed + 170.0, 0),
        Target::Child {
            slot: SLOT,
            index: 0
        }
    );
}

// ---------------------------------------------------------------------------
// Folder: the elastic blob (alternative style)
// ---------------------------------------------------------------------------

#[test]
fn the_elastic_blob_breaks_early_and_wobbles_after_landing() {
    let a = Animation::for_style(BlobStyle::Elastic);
    assert_close(a.break_at, 0.78);
    let before = frame(&a, DELAY + travel() * 0.5, None);
    assert_eq!(before.neck.len(), NECK_CIRCLES);
    // Starts small: 30% to 62% of an item before the break.
    let r = before.blob.unwrap().r;
    assert!(r > 0.3 * 26.0 && r < 0.62 * 26.0, "{r}");

    let landed = DELAY + travel();
    let wobbling = (1..40).any(|i| {
        let (stretch, _) = frame(&a, landed + f64::from(i) * 10.0, None)
            .hero_shape
            .unwrap();
        (stretch - 1.0).abs() > 0.01
    });
    assert!(wobbling, "the hero wobbles after landing");
    // It keeps animating while it wobbles, up to 420 ms after landing.
    assert!(frame(&a, landed + 400.0, None).active);
    assert!(!frame(&a, landed + 421.0, None).active);
}
