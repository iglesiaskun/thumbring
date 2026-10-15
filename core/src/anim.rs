//! Ring animations, as pure functions of the time elapsed.
//!
//! Opening and closing the ring, the highlight, the center button, the label
//! and the folder, with the viscous and elastic blob trips. Nothing here keeps
//! a clock: the engine passes the milliseconds since each animation started
//! and draws the frame it gets back. Times are `f64` milliseconds.

use std::f64::consts::PI;

use crate::config::{Animation, BlobStyle};
use crate::layout::{Geometry, Vec2, hero_index};

/// Circles between the folder and the blob, forming its neck.
pub const NECK_CIRCLES: usize = 6;

// ---------------------------------------------------------------------------
// Curves
// ---------------------------------------------------------------------------

pub fn clamp01(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Cubic, slowing down at the end.
pub fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

/// Cubic, speeding up at the end.
pub fn ease_in(t: f64) -> f64 {
    t * t * t
}

/// Cubic, speeding up and then slowing down.
pub fn ease_in_out(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Goes a bit past 1 and comes back; `s` is how far (0 = no overshoot).
pub fn ease_out_back(t: f64, s: f64) -> f64 {
    1.0 + (s + 1.0) * (t - 1.0).powi(3) + s * (t - 1.0).powi(2)
}

/// Smooth step from 0 at `e0` to 1 at `e1`.
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = clamp01((x - e0) / (e1 - e0));
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------------------
// Durations
// ---------------------------------------------------------------------------

/// Turns the configured times into actual durations. The overall speed and the
/// reduced motion preference apply to animations only, never to the logic
/// timings (hold to open, path tolerance).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    pub speed: f64,
    pub reduced_motion: bool,
}

impl Timing {
    /// With "Animation effects" off in Windows, animations run at about a
    /// third of their time, unless the settings ask to ignore that.
    pub fn new(animation: &Animation, system_reduced_motion: bool) -> Self {
        Self {
            speed: animation.speed,
            reduced_motion: system_reduced_motion && !animation.ignore_system_reduced_motion,
        }
    }

    /// Actual duration of an animation configured as `ms`.
    pub fn dur(&self, ms: f64) -> f64 {
        let motion = if self.reduced_motion { 0.35 } else { 1.0 };
        ms * motion / self.speed
    }
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            speed: 1.0,
            reduced_motion: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Ring open and close
// ---------------------------------------------------------------------------

/// How to draw the whole ring at one moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingFrame {
    /// Multiplies the distance of every item from the center.
    pub spread: f64,
    /// Multiplies the size of every item.
    pub size: f64,
    pub alpha: f64,
    /// Still opening or closing.
    pub active: bool,
    /// The close animation is over: the window can go.
    pub closed: bool,
}

/// Ring frame `since_open` ms after opening, and `since_close` ms after
/// starting to close (if it is closing).
pub fn ring_frame(
    animation: &Animation,
    timing: &Timing,
    since_open: f64,
    since_close: Option<f64>,
) -> RingFrame {
    let open = ease_out(clamp01(
        since_open / timing.dur(f64::from(animation.ring_open_ms)),
    ));
    let mut frame = RingFrame {
        spread: lerp(0.15, 1.0, open),
        size: lerp(0.4, 1.0, open),
        alpha: clamp01(open * 1.6),
        active: open < 1.0,
        closed: false,
    };
    if let Some(since_close) = since_close {
        let pc = clamp01(since_close / timing.dur(f64::from(animation.ring_close_ms)));
        let e = ease_in_out(pc);
        frame.spread *= lerp(1.0, 0.15, e);
        frame.size *= lerp(1.0, 0.35, e);
        frame.alpha *= 1.0 - pc.powf(0.75);
        frame.active = pc < 1.0;
        frame.closed = pc >= 1.0;
    }
    frame
}

// ---------------------------------------------------------------------------
// Highlight, center and label fades
// ---------------------------------------------------------------------------

/// Which of the small fades this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeKind {
    /// Dark disc of the highlighted item: 70 ms, grows from 60% to 100%.
    Highlight,
    /// Red center button: 80 ms, linear.
    Center,
    /// Item label: shows up 60 ms later with a 50 ms fade; hides in 100 ms.
    Label,
}

/// A value that fades in and out between 0 and 1. Turning it around halfway
/// starts from where it was.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fade {
    pub kind: FadeKind,
    on: bool,
    start: f64,
    from: f64,
}

impl Fade {
    pub fn new(kind: FadeKind) -> Self {
        Self {
            kind,
            on: false,
            start: f64::NEG_INFINITY,
            from: 0.0,
        }
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Turns it on or off at `now`. Does nothing if it already is.
    pub fn set(&mut self, on: bool, now: f64, timing: &Timing) {
        if on == self.on {
            return;
        }
        self.from = self.value(now, timing);
        self.on = on;
        self.start = now;
    }

    pub fn value(&self, now: f64, timing: &Timing) -> f64 {
        let t = now - self.start;
        match self.kind {
            FadeKind::Highlight => {
                let p = clamp01(t / timing.dur(70.0));
                if self.on {
                    lerp(self.from.max(0.6), 1.0, ease_out(p))
                } else {
                    self.from * (1.0 - ease_in(p))
                }
            }
            FadeKind::Center => {
                let p = clamp01(t / timing.dur(80.0));
                lerp(self.from, if self.on { 1.0 } else { 0.0 }, p)
            }
            FadeKind::Label => {
                if self.on {
                    clamp01((t - timing.dur(60.0)) / timing.dur(50.0))
                } else {
                    self.from * (1.0 - clamp01(t / timing.dur(100.0)))
                }
            }
        }
    }

    /// Still changing at `now`.
    pub fn is_animating(&self, now: f64, timing: &Timing) -> bool {
        let t = now - self.start;
        let length = match self.kind {
            FadeKind::Highlight => timing.dur(70.0),
            FadeKind::Center => timing.dur(80.0),
            FadeKind::Label if self.on => timing.dur(110.0),
            FadeKind::Label => timing.dur(100.0),
        };
        t < length
    }
}

// ---------------------------------------------------------------------------
// Folder open: the blob and the children
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub center: Vec2,
    pub r: f64,
}

/// The blob that carries the highlight from the folder to a child. It can be
/// stretched along its direction of travel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blob {
    pub center: Vec2,
    pub r: f64,
    /// Direction of travel (unit vector).
    pub dir: Vec2,
    /// Scale along `dir`.
    pub stretch: f64,
    /// Scale across `dir`.
    pub squash: f64,
}

/// How to draw an open (or closing) folder at one moment. Positions are
/// offsets from the ring center.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderFrame {
    /// Appear progress of each child (0 to 1).
    pub children: Vec<f64>,
    /// Child that receives the blob.
    pub hero: usize,
    pub blob: Option<Blob>,
    pub neck: Vec<Circle>,
    /// How visible the hero icon is on top of the travelling blob.
    pub hero_on_blob: f64,
    /// Wobble of the hero after landing: (stretch, squash) along the trip.
    pub hero_shape: Option<(f64, f64)>,
    pub active: bool,
    /// The close animation is over.
    pub finished: bool,
}

/// Frame of the folder in `slot` with `count` children, `since_open` ms after
/// it opened and `since_close` ms after it started to close (if it is closing).
pub fn folder_frame(
    geometry: &Geometry,
    animation: &Animation,
    timing: &Timing,
    slot: u8,
    count: usize,
    since_open: f64,
    since_close: Option<f64>,
) -> FolderFrame {
    let hero = hero_index(count);
    if count == 0 {
        // An empty folder has nowhere to send the blob.
        return FolderFrame {
            children: Vec::new(),
            hero,
            blob: None,
            neck: Vec::new(),
            hero_on_blob: 0.0,
            hero_shape: None,
            active: false,
            finished: since_close.is_some(),
        };
    }
    let from = geometry.slot_offset(slot);
    let to = geometry.child_offset(slot, hero, count);
    let length = (to - from).length();
    let trip = Trip {
        from,
        dir: (to - from).normalized(),
        length,
        r: geometry.r,
    };
    let wobble = animation.wobble;
    let delay = timing.dur(35.0);
    let travel = timing
        .dur((f64::from(animation.blob_ms_per_item) * length / geometry.d).clamp(100.0, 520.0));
    let siblings = timing.dur(f64::from(animation.folder_items_ms));
    let landing = timing.dur(150.0);
    let wobble_time = timing.dur(420.0);
    let t = since_open;

    let mut frame = FolderFrame {
        children: vec![0.0; count],
        hero,
        blob: None,
        neck: Vec::new(),
        hero_on_blob: 0.0,
        hero_shape: None,
        active: true,
        finished: false,
    };

    if t >= delay && t < delay + travel {
        let q = (t - delay) / travel;
        match animation.style {
            BlobStyle::Viscous => travel_viscous(&mut frame, &trip, animation, timing, q, travel),
            BlobStyle::Elastic => travel_elastic(&mut frame, &trip, animation, q),
        }
        frame.hero_on_blob = clamp01((q - 0.15) * 3.0);
    } else if t >= delay + travel {
        if animation.style == BlobStyle::Viscous {
            let after_break = t - delay - animation.break_at * travel;
            neck_remnants_viscous(&mut frame, &trip, animation, timing, after_break, to);
        }
        let tl = t - delay - travel;
        let w = wobble
            * 0.3
            * (-tl / timing.dur(110.0)).exp()
            * (2.0 * PI * tl / timing.dur(150.0)).sin();
        let shape = (1.0 - w, 1.0 + w * 0.8);
        let qf = clamp01(tl / landing);
        if qf < 1.0 {
            frame.blob = Some(Blob {
                center: to,
                r: geometry.r * (1.0 - ease_in(qf)),
                dir: trip.dir,
                stretch: shape.0,
                squash: shape.1,
            });
        }
        frame.hero_shape = Some(shape);
        let s = ease_out(clamp01(tl / siblings));
        for (index, appear) in frame.children.iter_mut().enumerate() {
            *appear = if index == hero { 1.0 } else { s };
        }
        let settle = if wobble > 0.0 { wobble_time } else { 0.0 };
        frame.active = tl < siblings.max(landing).max(settle);
    }

    if let Some(since_close) = since_close {
        let pc = clamp01(since_close / timing.dur(f64::from(animation.folder_close_ms)));
        let k = 1.0 - ease_in(pc);
        for appear in &mut frame.children {
            *appear *= k;
        }
        if let Some(blob) = &mut frame.blob {
            blob.r *= k;
        }
        for circle in &mut frame.neck {
            circle.r *= k;
        }
        frame.hero_on_blob *= k;
        frame.active = pc < 1.0;
        frame.finished = pc >= 1.0;
    }
    frame
}

/// Straight path of the blob, from the folder to the hero child.
struct Trip {
    from: Vec2,
    dir: Vec2,
    length: f64,
    /// Item radius.
    r: f64,
}

impl Trip {
    /// Point at `fraction` of the way.
    fn at(&self, fraction: f64) -> Vec2 {
        self.from + self.dir * (self.length * fraction)
    }
}

/// Neck thickness and how far the viscous blob goes before the neck breaks.
fn viscous_shape(viscosity: f64) -> (f64, f64) {
    let thick = 0.55 + 0.375 * viscosity.clamp(0.0, 2.0);
    let break_fraction = (0.8 + 0.08 * viscosity).clamp(0.8, 0.95);
    (thick, break_fraction)
}

/// Default style: the blob leaves big (35% to 85% of an item) and goes almost
/// all the way still attached by a thick neck, which only thins out at the
/// end; it breaks right at the target and settles without flying.
fn travel_viscous(
    frame: &mut FolderFrame,
    trip: &Trip,
    animation: &Animation,
    timing: &Timing,
    q: f64,
    travel: f64,
) {
    let viscosity = animation.viscosity;
    let break_at = animation.break_at;
    let (thick, break_fraction) = viscous_shape(viscosity);
    if q < break_at {
        let a = q / break_at;
        let center = trip.at(break_fraction * ease_in_out(a));
        let r = trip.r * lerp(0.35, 0.85, ease_out(a));
        let stretch = 1.0 + 0.12 * viscosity * (PI * a).sin();
        frame.blob = Some(Blob {
            center,
            r,
            dir: trip.dir,
            stretch,
            squash: 1.0 / stretch.sqrt(),
        });
        let pinch = 0.22 + 0.73 * smoothstep(0.65, 1.0, a);
        for i in 1..=NECK_CIRCLES {
            let s = i as f64 / (NECK_CIRCLES as f64 + 1.0);
            frame.neck.push(Circle {
                center: trip.from.lerp(center, s),
                r: lerp(trip.r * 0.62, r * 0.8, s) * thick * (1.0 - pinch * (PI * s).sin()),
            });
        }
    } else {
        let u = (q - break_at) / (1.0 - break_at);
        let fraction =
            break_fraction + (1.0 - break_fraction) * ease_out_back(u, 1.6 * animation.wobble);
        let center = trip.at(fraction);
        frame.blob = Some(Blob {
            center,
            r: trip.r * lerp(0.85, 1.0, ease_out(u)),
            dir: trip.dir,
            stretch: 1.0,
            squash: 1.0,
        });
        neck_remnants_viscous(
            frame,
            trip,
            animation,
            timing,
            (q - break_at) * travel,
            center,
        );
    }
}

/// After the break, the half of the neck near the folder goes back into it and
/// the other half is absorbed by the blob, in 70 ms.
fn neck_remnants_viscous(
    frame: &mut FolderFrame,
    trip: &Trip,
    animation: &Animation,
    timing: &Timing,
    after_break: f64,
    blob: Vec2,
) {
    let sn = ease_out(clamp01(after_break / timing.dur(70.0)));
    if sn >= 1.0 {
        return;
    }
    let (thick, break_fraction) = viscous_shape(animation.viscosity);
    let broke_at = trip.at(break_fraction);
    for i in 1..=NECK_CIRCLES {
        let s = i as f64 / (NECK_CIRCLES as f64 + 1.0);
        let r0 =
            lerp(trip.r * 0.62, trip.r * 0.85 * 0.8, s) * thick * (1.0 - 0.95 * (PI * s).sin());
        let start = trip.from.lerp(broke_at, s);
        let target = if s < 0.5 { trip.from } else { blob };
        frame.neck.push(Circle {
            center: start.lerp(target, sn),
            r: r0 * (1.0 - sn),
        });
    }
}

/// Alternative style: the blob pulls slowly out of the folder, small and
/// stretched; the neck breaks early, and the blob flies to its place like a
/// rubber band, overshooting by the wobble.
fn travel_elastic(frame: &mut FolderFrame, trip: &Trip, animation: &Animation, q: f64) {
    let viscosity = animation.viscosity;
    let break_at = animation.break_at;
    let break_fraction = 0.45 + 0.35 * viscosity.clamp(0.0, 2.0) / 2.0;
    let neck_base = trip.r * (0.3 + 0.25 * viscosity).min(0.7);
    if q < break_at {
        let a = q / break_at;
        let center = trip.at(break_fraction * a.powf(1.4));
        let r = trip.r * lerp(0.3, 0.62, a);
        let stretch = 1.0 + 0.28 * viscosity * a;
        frame.blob = Some(Blob {
            center,
            r,
            dir: trip.dir,
            stretch,
            squash: 1.0 / stretch.sqrt(),
        });
        let pinch = 0.9 * a * a;
        for i in 1..=NECK_CIRCLES {
            let s = i as f64 / (NECK_CIRCLES as f64 + 1.0);
            frame.neck.push(Circle {
                center: trip.from.lerp(center, s),
                r: lerp(neck_base, r * 0.8, s) * (1.0 - pinch * (PI * s).sin()),
            });
        }
    } else {
        let u = (q - break_at) / (1.0 - break_at);
        let fraction =
            break_fraction + (1.0 - break_fraction) * ease_out_back(u, 1.6 * animation.wobble);
        let center = trip.at(fraction);
        let stretch = lerp(1.0 + 0.28 * viscosity, 1.0, ease_out(u));
        frame.blob = Some(Blob {
            center,
            r: trip.r * lerp(0.62, 1.0, ease_out(u)),
            dir: trip.dir,
            stretch,
            squash: 1.0 / stretch.sqrt(),
        });
        let sn = ease_out((u / 0.4).min(1.0));
        let broke_at = trip.at(break_fraction);
        for i in 1..=NECK_CIRCLES {
            let s = i as f64 / (NECK_CIRCLES as f64 + 1.0);
            let r0 = lerp(neck_base, trip.r * 0.62 * 0.8, s) * (1.0 - 0.9 * (PI * s).sin());
            let start = trip.from.lerp(broke_at, s);
            let target = if s < 0.5 { trip.from } else { center };
            frame.neck.push(Circle {
                center: start.lerp(target, sn),
                r: r0 * (1.0 - sn),
            });
        }
    }
}
