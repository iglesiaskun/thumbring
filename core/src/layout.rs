//! Ring geometry: where each item sits, where the ring opens on the screen and
//! what is under the cursor.
//!
//! Every size derives from `D`, the diameter of one item (52 px at 100%
//! scaling), and from the ring radius (1.8 D unless the appearance says
//! otherwise).

use std::f64::consts::PI;
use std::ops::{Add, Mul, Sub};

use crate::config::Appearance;
use crate::ring::{FOLDER_MAX_ITEMS, Ring, SLOT_COUNT, Slot};
use crate::ring_machine::Target;
use crate::screen::Rect;

/// Item diameter at 100% scaling (96 DPI), unless the appearance says
/// otherwise.
pub const ITEM_DIAMETER_PX: f64 = 52.0;

/// Ring radius, in item diameters, unless the appearance says otherwise.
pub const RING_RADIUS_PER_ITEM: f64 = 1.8;

/// Smallest item diameter at 100% scaling, used only on tiny monitors.
pub const MIN_ITEM_DIAMETER_PX: f64 = 30.0;

/// Smallest gap between two neighbouring items, in item diameters. Closer
/// than that, their shapes start to melt together and their hit areas touch.
pub const MIN_ITEM_GAP: f64 = 0.14;

/// A child shows up in the hit-test once it is this far into its appear
/// animation, so a click never lands on an item that is still too small to see.
pub const CHILD_HITTABLE_FROM: f64 = 0.6;

/// 2D vector, in pixels. Offsets are relative to the ring center, with `y`
/// growing downwards like the screen.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Unit vector at `angle` radians (0 = right, growing clockwise on screen).
    pub fn from_angle(angle: f64) -> Self {
        Self::new(angle.cos(), angle.sin())
    }

    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    /// Same direction, length 1. The zero vector stays zero.
    pub fn normalized(self) -> Self {
        let len = self.length();
        if len == 0.0 { self } else { self * (1.0 / len) }
    }

    pub fn lerp(self, other: Vec2, t: f64) -> Self {
        self + (other - self) * t
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f64) -> Vec2 {
        Vec2::new(self.x * k, self.y * k)
    }
}

/// Every size of the ring, derived from the item diameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    /// Item diameter (`D`).
    pub d: f64,
    /// Item radius.
    pub r: f64,
    /// Distance from the ring center to each item center: 1.8 D by default.
    pub ring_r: f64,
    /// Radius of the folder arc: twice the ring radius.
    pub folder_r: f64,
    /// Radius of the center (×) button: 0.7 D wide.
    pub center_r: f64,
    /// Radius of the cancel zone: 0.45 of the ring radius.
    pub cancel_r: f64,
    /// An item reacts a little beyond its edge: 1.12 of its radius.
    pub hit_r: f64,
    /// Angle between folder children: 22.5°, or more if that would bring
    /// them closer than [`MIN_ITEM_GAP`].
    pub child_spacing: f64,
}

impl Geometry {
    /// Items `d` wide, with the default ring radius.
    pub fn new(d: f64) -> Self {
        Self::with_radius(d, RING_RADIUS_PER_ITEM * d)
    }

    /// Items `d` wide, `ring_r` away from the ring center.
    pub fn with_radius(d: f64, ring_r: f64) -> Self {
        let r = d / 2.0;
        let folder_r = 2.0 * ring_r;
        // Distance between the centers of two children `angle` apart is
        // 2 folder_r sin(angle / 2). Past 40° the 9 children would go round
        // the whole circle, so the spacing stops there.
        let closest = (1.0 + MIN_ITEM_GAP) * d / (2.0 * folder_r);
        let needed = 2.0 * closest.min(1.0).asin();
        Self {
            d,
            r,
            ring_r,
            folder_r,
            center_r: 0.35 * d,
            cancel_r: 0.45 * ring_r,
            hit_r: 1.12 * r,
            child_spacing: needed.clamp(PI / 8.0, 2.0 * PI / FOLDER_MAX_ITEMS as f64),
        }
    }

    /// Geometry for a monitor with this scale (1.0 = 100%), with the item size
    /// and ring radius of the appearance. The ring keeps that size unless the
    /// monitor is too small for the open folder arc; then it shrinks as a
    /// whole, keeping its proportions.
    pub fn for_monitor(scale: f64, appearance: &Appearance, monitor: Rect) -> Self {
        let width = f64::from(monitor.right - monitor.left);
        let height = f64::from(monitor.bottom - monitor.top);
        let per_item = appearance.ring_radius_px / appearance.item_size_px;
        // The open ring spans about 4 ring radii and 1.2 D from edge to edge
        // of the folder arc: 8.4 D with the default radius.
        let fits = (width.min(height) - 24.0 * scale) / (4.0 * per_item + 1.2);
        let d = (appearance.item_size_px * scale)
            .min(fits)
            .max(MIN_ITEM_DIAMETER_PX * scale);
        Self::with_radius(d, per_item * d)
    }

    /// Scale of the extra pixels the layout adds at 100% (6 px around the
    /// arc, 10 px of screen margin).
    fn px(&self, at_100: f64) -> f64 {
        at_100 * self.d / ITEM_DIAMETER_PX
    }

    /// Beyond this distance from the center, the cursor is outside the ring.
    pub fn outer_r(&self) -> f64 {
        self.folder_r + self.r + self.px(6.0)
    }

    /// How far the ring center stays from the monitor edges, so the folder arc
    /// always fits on the screen.
    pub fn screen_margin(&self) -> f64 {
        self.folder_r + self.r + self.px(10.0)
    }

    /// Offset of a ring item from the center.
    pub fn slot_offset(&self, slot: u8) -> Vec2 {
        Vec2::from_angle(slot_angle(slot)) * self.ring_r
    }

    /// Offset of child `index` of a folder in `slot` with `count` children. The
    /// arc is centered on the folder direction.
    pub fn child_offset(&self, slot: u8, index: usize, count: usize) -> Vec2 {
        let shift = index as f64 - (count as f64 - 1.0) / 2.0;
        let angle = slot_angle(slot) + shift * self.child_spacing;
        Vec2::from_angle(angle) * self.folder_r
    }

    /// Ring center on the screen: on the cursor, pushed away from the monitor
    /// edges just enough for the open folder arc. On a monitor too small even
    /// for that, the ring opens in the middle of it.
    pub fn place(&self, cursor: Vec2, monitor: Rect) -> Vec2 {
        let m = self.screen_margin();
        let axis = |pos: f64, low: i32, high: i32| {
            let (low, high) = (f64::from(low), f64::from(high));
            if high - low > 2.0 * m {
                pos.clamp(low + m, high - m)
            } else {
                (low + high) / 2.0
            }
        };
        Vec2::new(
            axis(cursor.x, monitor.left, monitor.right),
            axis(cursor.y, monitor.top, monitor.bottom),
        )
    }

    /// What is under the cursor. `offset` is the cursor minus the ring center.
    ///
    /// The open folder children come first, then the cancel zone, then the
    /// ring items. An empty slot counts as the gap between items.
    pub fn hit_test(&self, ring: &Ring, offset: Vec2, open: Option<OpenFolder<'_>>) -> Target {
        if let Some(folder) = open {
            let count = folder.appear.len();
            for (index, &appear) in folder.appear.iter().enumerate() {
                if appear < CHILD_HITTABLE_FROM {
                    continue;
                }
                let child = self.child_offset(folder.slot, index, count);
                if (offset - child).length() <= self.hit_r {
                    return Target::Child {
                        slot: folder.slot,
                        index: index as u8,
                    };
                }
            }
        }
        let distance = offset.length();
        if distance <= self.cancel_r {
            return Target::Center;
        }
        for slot in 0..SLOT_COUNT as u8 {
            let item = &ring.slots[usize::from(slot)];
            if *item == Slot::Empty {
                continue;
            }
            if (offset - self.slot_offset(slot)).length() <= self.hit_r {
                return match item {
                    Slot::Folder { .. } => Target::Folder { slot },
                    _ => Target::Action { slot },
                };
            }
        }
        if distance <= self.outer_r() {
            Target::Gap
        } else {
            Target::Outside
        }
    }
}

/// The open folder, as the hit-test needs it.
#[derive(Debug, Clone, Copy)]
pub struct OpenFolder<'a> {
    pub slot: u8,
    /// Appear progress of each child (0 to 1), from the folder animation.
    pub appear: &'a [f64],
}

/// Angle of a ring slot: slot 0 at the top, then clockwise, 45° apart.
pub fn slot_angle(slot: u8) -> f64 {
    -PI / 2.0 + f64::from(slot) * PI / 4.0
}

/// Child that receives the blob when a folder with `count` children opens.
pub fn hero_index(count: usize) -> usize {
    count / 2
}
