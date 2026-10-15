//! Visual state of an open ring, and the scene to draw at each frame.
//!
//! [`RingView`] remembers what is animating since when; [`RingView::scene`]
//! turns that into plain shapes, icons and a label for the engine to draw. No
//! clock, no Windows: the engine passes `now` in milliseconds.

use crate::anim::{Blob, Circle, Fade, FadeKind, Timing, clamp01, folder_frame, ring_frame};
use crate::config::Animation;
use crate::layout::{Geometry, Vec2, slot_angle};
use crate::ring::{Ring, SLOT_COUNT, Slot};
use crate::ring_machine::{Effect, State, Target};
use crate::screen::Rect;

/// Most white circles the shader takes.
pub const MAX_WHITE: usize = 32;
/// Most dark circles the shader takes.
pub const MAX_DARK: usize = 24;

/// Item that can be highlighted or carry an icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKey {
    Slot(u8),
    Child { slot: u8, index: u8 },
}

impl ItemKey {
    /// Item a target points at, if it is an item.
    pub fn of(target: Target) -> Option<ItemKey> {
        match target {
            Target::Action { slot } | Target::Folder { slot } => Some(ItemKey::Slot(slot)),
            Target::Child { slot, index } => Some(ItemKey::Child { slot, index }),
            Target::Center | Target::Gap | Target::Outside => None,
        }
    }
}

/// Where an icon goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconKey {
    Item(ItemKey),
    /// The × in the center.
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconDraw {
    pub key: IconKey,
    pub center: Vec2,
    /// 1 = full item size.
    pub scale: f64,
    pub opacity: f64,
    /// Drawn white, over a dark disc.
    pub on: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelDraw {
    pub target: Target,
    /// Center of the labelled item.
    pub item: Vec2,
    /// Current radius of the labelled item.
    pub item_r: f64,
    pub opacity: f64,
}

/// Everything to draw in one frame, in window coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub ring_center: Vec2,
    /// Multiplies everything.
    pub alpha: f64,
    /// White items, blended together with a smooth union.
    pub white: Vec<Circle>,
    /// Dark highlights and the blob neck, blended together.
    pub dark: Vec<Circle>,
    /// The travelling blob (dark).
    pub blob: Option<Blob>,
    /// The child that receives the blob, wobbling (white).
    pub hero: Option<Blob>,
    pub center: Circle,
    /// 0 = translucent white, 1 = red.
    pub center_red: f64,
    /// Smooth union strength of the white and dark layers.
    pub k_white: f64,
    pub k_dark: f64,
    /// Color of the dark layer: near black, or grey while closing.
    pub dark_color: [f32; 3],
    pub icons: Vec<IconDraw>,
    pub label: Option<LabelDraw>,
    /// Something is still moving: draw another frame.
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FolderEntry {
    slot: u8,
    opened_at: f64,
    closing_at: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LabelState {
    key: Option<ItemKey>,
    target: Option<Target>,
    last: Option<Target>,
    start: f64,
    from: f64,
}

impl LabelState {
    /// Shows up 60 ms after its item gets the highlight, with a 50 ms fade;
    /// hides in 100 ms.
    fn opacity(&self, now: f64, timing: &Timing) -> f64 {
        let t = now - self.start;
        if self.key.is_some() {
            clamp01((t - timing.dur(60.0)) / timing.dur(50.0))
        } else {
            self.from * (1.0 - clamp01(t / timing.dur(100.0)))
        }
    }
}

/// Visual state of the ring, from opening until the close animation ends.
#[derive(Debug, Clone, PartialEq)]
pub struct RingView {
    /// Ring center, in window coordinates.
    pub center: Vec2,
    opened_at: f64,
    closing_at: Option<f64>,
    folders: Vec<FolderEntry>,
    highlights: Vec<(ItemKey, Fade)>,
    center_fade: Fade,
    label: LabelState,
    pressed: Option<ItemKey>,
}

impl RingView {
    pub fn open(center: Vec2, now: f64) -> Self {
        Self {
            center,
            opened_at: now,
            closing_at: None,
            folders: Vec::new(),
            highlights: Vec::new(),
            center_fade: Fade::new(FadeKind::Center),
            label: LabelState {
                key: None,
                target: None,
                last: None,
                start: now,
                from: 0.0,
            },
            pressed: None,
        }
    }

    pub fn is_closing(&self) -> bool {
        self.closing_at.is_some()
    }

    /// Applies a machine effect. [`Effect::OpenRing`] is the caller's job
    /// (create the view where the ring opens); effects that are only timers
    /// or input handling are ignored here.
    pub fn apply(&mut self, effect: &Effect, now: f64, timing: &Timing) {
        match *effect {
            Effect::CloseRing(_) => {
                if self.closing_at.is_none() {
                    self.closing_at = Some(now);
                    // The item that ran keeps its dark disc while the ring closes.
                    if let Some(key) = self.pressed {
                        self.set_highlight(key, true, now, timing);
                    }
                }
            }
            Effect::OpenFolder { slot } => {
                for folder in &mut self.folders {
                    folder.closing_at.get_or_insert(now);
                }
                self.folders.push(FolderEntry {
                    slot,
                    opened_at: now,
                    closing_at: None,
                });
            }
            Effect::CloseFolder { slot } => {
                for folder in self.folders.iter_mut().filter(|f| f.slot == slot) {
                    folder.closing_at.get_or_insert(now);
                }
            }
            Effect::Execute(target) => self.pressed = ItemKey::of(target),
            _ => {}
        }
    }

    /// Follows the machine state: highlights, red center and label. Call it
    /// after every event, except while the ring is closing.
    pub fn sync(&mut self, state: &State, now: f64, timing: &Timing) {
        if self.is_closing() {
            return;
        }
        let mut wanted: Vec<ItemKey> = Vec::new();
        if let Some(
            hover @ (Target::Action { .. } | Target::Folder { .. } | Target::Child { .. }),
        ) = state.hover
        {
            wanted.extend(ItemKey::of(hover));
        }
        if let Some(folder) = state.folder {
            wanted.push(ItemKey::Slot(folder));
        }
        let off: Vec<ItemKey> = self
            .highlights
            .iter()
            .filter(|(key, fade)| fade.is_on() && !wanted.contains(key))
            .map(|(key, _)| *key)
            .collect();
        for key in off {
            self.set_highlight(key, false, now, timing);
        }
        for key in wanted {
            self.set_highlight(key, true, now, timing);
        }

        let on_center = state.hover == Some(Target::Center);
        self.center_fade.set(on_center, now, timing);

        // Folders have no label: open, their children use that space.
        let key = match state.hover {
            Some(t @ (Target::Action { .. } | Target::Child { .. })) => ItemKey::of(t),
            _ => None,
        };
        if key != self.label.key {
            let from = self.label.opacity(now, timing);
            self.label = LabelState {
                key,
                target: state.hover,
                last: state.hover.or(self.label.target).or(self.label.last),
                start: now,
                from,
            };
        }
    }

    fn set_highlight(&mut self, key: ItemKey, on: bool, now: f64, timing: &Timing) {
        match self.highlights.iter_mut().find(|(k, _)| *k == key) {
            Some((_, fade)) => fade.set(on, now, timing),
            None if on => {
                let mut fade = Fade::new(FadeKind::Highlight);
                fade.set(true, now, timing);
                self.highlights.push((key, fade));
            }
            None => {}
        }
    }

    fn highlight(&self, key: ItemKey, now: f64, timing: &Timing) -> (f64, bool) {
        self.highlights
            .iter()
            .find(|(k, _)| *k == key)
            .map_or((0.0, false), |(_, fade)| {
                (fade.value(now, timing), fade.is_animating(now, timing))
            })
    }

    /// Appear progress of the children of the open folder, for the hit-test.
    pub fn open_folder_children(
        &self,
        ring: &Ring,
        geometry: &Geometry,
        animation: &Animation,
        timing: &Timing,
        now: f64,
    ) -> Option<(u8, Vec<f64>)> {
        let folder = self.folders.iter().rev().find(|f| f.closing_at.is_none())?;
        let count = folder_len(ring, folder.slot);
        let frame = folder_frame(
            geometry,
            animation,
            timing,
            folder.slot,
            count,
            now - folder.opened_at,
            None,
        );
        Some((folder.slot, frame.children))
    }

    /// What to draw at `now`. `None` once the close animation is over.
    pub fn scene(
        &mut self,
        ring: &Ring,
        geometry: &Geometry,
        animation: &Animation,
        timing: &Timing,
        now: f64,
    ) -> Option<Scene> {
        let frame = ring_frame(
            animation,
            timing,
            now - self.opened_at,
            self.closing_at.map(|c| now - c),
        );
        if frame.closed {
            return None;
        }
        let mut active = frame.active;
        let center = self.center;
        let place = |offset: Vec2| center + offset * frame.spread;
        let item_r = geometry.r * frame.size;

        let mut white: Vec<Circle> = Vec::new();
        let mut dark: Vec<Circle> = Vec::new();
        let mut icons: Vec<IconDraw> = Vec::new();
        let push = |list: &mut Vec<Circle>, max: usize, center: Vec2, r: f64| {
            if list.len() < max && r > 0.2 {
                list.push(Circle { center, r });
            }
        };

        for slot in 0..SLOT_COUNT as u8 {
            let item = &ring.slots[usize::from(slot)];
            if *item == Slot::Empty {
                continue;
            }
            let p = place(geometry.slot_offset(slot));
            let (dv, animating) = self.highlight(ItemKey::Slot(slot), now, timing);
            active |= animating;
            push(&mut white, MAX_WHITE, p, item_r);
            if matches!(item, Slot::Folder { .. }) {
                // The folder bump: gone while the folder is dark (open or highlighted).
                let distance = (geometry.ring_r + geometry.r * 0.95) * frame.spread;
                let bump = center + Vec2::from_angle(slot_angle(slot)) * distance;
                push(
                    &mut white,
                    MAX_WHITE,
                    bump,
                    geometry.r * 0.22 * frame.size * (1.0 - dv),
                );
            }
            if dv > 0.0 {
                push(&mut dark, MAX_DARK, p, item_r * dv);
            }
            icons.push(IconDraw {
                key: IconKey::Item(ItemKey::Slot(slot)),
                center: p,
                scale: frame.size,
                opacity: frame.alpha,
                on: dv > 0.45,
            });
        }

        // The most recent open folder gets the blob and the wobbling hero;
        // folders that are closing use plain circles.
        let lead = self
            .folders
            .iter()
            .rposition(|f| f.closing_at.is_none())
            .or(self.folders.len().checked_sub(1));
        let mut blob = None;
        let mut hero = None;
        let mut finished = Vec::new();
        for (position, folder) in self.folders.iter().enumerate() {
            let count = folder_len(ring, folder.slot);
            let ff = folder_frame(
                geometry,
                animation,
                timing,
                folder.slot,
                count,
                now - folder.opened_at,
                folder.closing_at.map(|c| now - c),
            );
            let is_lead = Some(position) == lead;
            active |= ff.active;
            for (index, &appear) in ff.children.iter().enumerate() {
                let key = ItemKey::Child {
                    slot: folder.slot,
                    index: index as u8,
                };
                let p = place(geometry.child_offset(folder.slot, index, count));
                let (dv, animating) = self.highlight(key, now, timing);
                active |= animating;
                if appear > 0.0 {
                    match ff.hero_shape {
                        Some((stretch, squash)) if is_lead && index == ff.hero => {
                            let dir = (geometry.child_offset(folder.slot, index, count)
                                - geometry.slot_offset(folder.slot))
                            .normalized();
                            hero = Some(Blob {
                                center: p,
                                r: item_r * appear,
                                dir,
                                stretch,
                                squash,
                            });
                        }
                        _ => push(&mut white, MAX_WHITE, p, item_r * appear),
                    }
                    if dv > 0.0 {
                        push(&mut dark, MAX_DARK, p, item_r * appear * dv);
                    }
                }
                let mut icon = IconDraw {
                    key: IconKey::Item(key),
                    center: p,
                    scale: frame.size * appear,
                    opacity: frame.alpha * clamp01(appear * 1.5),
                    on: dv > 0.45,
                };
                // While the blob travels, it carries the hero icon.
                if let Some(b) = ff.blob.filter(|_| index == ff.hero && appear == 0.0) {
                    if ff.hero_on_blob > 0.0 {
                        icon = IconDraw {
                            key: icon.key,
                            center: place(b.center),
                            scale: frame.size * (b.r / geometry.r),
                            opacity: frame.alpha * ff.hero_on_blob,
                            on: true,
                        };
                    }
                }
                match icons.iter_mut().find(|i| i.key == icon.key) {
                    Some(existing) if appear > 0.0 || icon.on => *existing = icon,
                    Some(_) => {}
                    None => icons.push(icon),
                }
            }
            for circle in &ff.neck {
                push(
                    &mut dark,
                    MAX_DARK,
                    place(circle.center),
                    circle.r * frame.size,
                );
            }
            if let Some(b) = ff.blob {
                let placed = Blob {
                    center: place(b.center),
                    r: b.r * frame.size,
                    ..b
                };
                if is_lead {
                    blob = Some(placed);
                } else {
                    push(&mut dark, MAX_DARK, placed.center, placed.r);
                }
            }
            if ff.finished {
                finished.push(position);
            }
        }
        for position in finished.into_iter().rev() {
            self.folders.remove(position);
        }
        self.highlights
            .retain(|(_, fade)| fade.is_on() || fade.is_animating(now, timing));

        let center_red = self.center_fade.value(now, timing);
        active |= self.center_fade.is_animating(now, timing);
        icons.push(IconDraw {
            key: IconKey::Center,
            center,
            scale: frame.size,
            opacity: frame.alpha,
            on: center_red > 0.5,
        });

        let label_opacity = self.label.opacity(now, timing);
        active |= if self.label.key.is_some() {
            label_opacity < 1.0
        } else {
            label_opacity > 0.0
        };
        let target = if self.label.key.is_some() {
            self.label.target
        } else {
            self.label.last
        };
        let label = target
            .filter(|_| label_opacity * frame.alpha > 0.01)
            .and_then(|target| {
                let offset = match target {
                    Target::Child { slot, index } => {
                        geometry.child_offset(slot, usize::from(index), folder_len(ring, slot))
                    }
                    Target::Action { slot } | Target::Folder { slot } => geometry.slot_offset(slot),
                    _ => return None,
                };
                Some(LabelDraw {
                    target,
                    item: place(offset),
                    item_r,
                    opacity: label_opacity * frame.alpha,
                })
            });

        let closing = self.is_closing();
        Some(Scene {
            ring_center: center,
            alpha: frame.alpha,
            white,
            dark,
            blob,
            hero,
            center: Circle {
                center,
                r: geometry.center_r * frame.size,
            },
            center_red,
            k_white: geometry.r * 0.22,
            k_dark: (geometry.r * (0.22 + 0.18 * animation.viscosity)).max(0.5),
            dark_color: if closing {
                [0.56, 0.56, 0.6]
            } else {
                [0.07, 0.07, 0.08]
            },
            icons,
            label,
            active,
        })
    }
}

fn folder_len(ring: &Ring, slot: u8) -> usize {
    match ring.slots.get(usize::from(slot)) {
        Some(Slot::Folder { items, .. }) => items.len(),
        _ => 0,
    }
}

/// Top-left corner of a label of `size` (width, height) next to its item: to
/// the side for items on the left and right of the ring, above or below for
/// the others, never closer than 6 px to the edges of `bounds`.
pub fn label_position(
    label: &LabelDraw,
    ring_center: Vec2,
    size: (f64, f64),
    gap: f64,
    bounds: Rect,
) -> Vec2 {
    let (w, h) = size;
    let p = label.item;
    let dir = (p - ring_center).normalized();
    let rr = label.item_r;
    let (x, y) = if dir.x.abs() > 0.38 {
        let x = if dir.x > 0.0 {
            p.x + rr + gap
        } else {
            p.x - rr - gap - w
        };
        (x, p.y - h / 2.0)
    } else {
        let y = if dir.y > 0.0 {
            p.y + rr + gap
        } else {
            p.y - rr - gap - h
        };
        (p.x - w / 2.0, y)
    };
    let clamp = |v: f64, low: i32, high: i32, size: f64| {
        let (low, high) = (f64::from(low) + 6.0, f64::from(high) - size - 6.0);
        if high < low { low } else { v.clamp(low, high) }
    };
    Vec2::new(
        clamp(x, bounds.left, bounds.right, w),
        clamp(y, bounds.top, bounds.bottom, h),
    )
}
