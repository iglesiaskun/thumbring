//! `--preview-ring`: shows the ring from the settings file, in its
//! appearance, on the monitor under the cursor and plays a fixed script, to
//! look at the drawing and the animations without touching the mouse hook.
//!
//! The script: the ring opens, its biggest folder opens with the blob, the
//! second of its actions gets the highlight and the label, and after a few
//! seconds the ring closes. With the default ring, that is "Paste" in the
//! "Editing" folder. A ring without folders highlights its first action; a
//! ring that is off shows the default one.
//!
//! - `--preview-ring <ms>` freezes the script at that moment and shows it for
//!   three seconds in the middle of the monitor, to look at one frame.
//! - `--preview-ring <ms> --save <file.png>` draws that frame without showing
//!   it and saves the ring area as a PNG with transparency.
//! - `--dir <folder>` reads the settings from that folder, as the engine does.

use std::error::Error;
use std::path::Path;
use std::time::Instant;

use thumbring_core::anim::Timing;
use thumbring_core::config::{Animation, Behavior, Config};
use thumbring_core::layout::{Geometry, Vec2};
use thumbring_core::ring::{Ring, Slot};
use thumbring_core::ring_machine::{Effect, Event, State, Target, reduce};
use thumbring_core::view::{RingView, Scene};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
};

use super::overlay::Overlay;
use super::png;
use super::render::{LabelText, label_text};
use crate::dpi;

/// How long a frozen frame stays on the screen.
const FROZEN_MS: f64 = 3000.0;

/// When each scripted event happens, in ms after the start.
fn script(ring: &Ring) -> Vec<(f64, Event)> {
    let mut events = vec![
        (0.0, Event::TriggerDown),
        (0.0, Event::HoldElapsed),
        (10.0, Event::TriggerUp),
    ];
    // Of the folders with the most actions, the first one.
    let folder = ring
        .slots
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(slot, content)| match content {
            Slot::Folder { items, .. } if !items.is_empty() => Some((slot as u8, items.len())),
            _ => None,
        })
        .max_by_key(|&(_, count)| count);
    let action = (0u8..)
        .zip(&ring.slots)
        .find(|(_, content)| matches!(content, Slot::Action(_)))
        .map(|(slot, _)| slot);
    match (folder, action) {
        (Some((slot, count)), _) => {
            let index = u8::from(count > 1);
            events.push((500.0, Event::Pointer(Target::Folder { slot })));
            events.push((1500.0, Event::Pointer(Target::Child { slot, index })));
        }
        (None, Some(slot)) => events.push((500.0, Event::Pointer(Target::Action { slot }))),
        (None, None) => {}
    }
    events.push((6000.0, Event::Esc));
    events
}

/// The scripted ring: machine state, visual state and what is left to play.
struct Player {
    ring: Ring,
    geometry: Geometry,
    animation: Animation,
    timing: Timing,
    behavior: Behavior,
    center: Vec2,
    state: State,
    view: Option<RingView>,
    pending: std::iter::Peekable<std::vec::IntoIter<(f64, Event)>>,
}

impl Player {
    /// Plays every event due by `now`. Each one takes effect at its scheduled
    /// time, so a frozen frame is the same on every run.
    fn advance(&mut self, now: f64) {
        while let Some((at, event)) = self.pending.next_if(|(at, _)| *at <= now) {
            let (next, effects) = reduce(self.state, event, &self.behavior);
            self.state = next;
            for effect in &effects {
                if *effect == Effect::OpenRing {
                    self.view = Some(RingView::open(self.center, at));
                }
                if let Some(view) = self.view.as_mut() {
                    view.apply(effect, at, &self.timing);
                }
            }
            if let Some(view) = self.view.as_mut() {
                view.sync(&self.state, at, &self.timing);
            }
        }
    }

    /// Scene and label text at `now`. `None` before the ring opens or after it
    /// closed.
    fn frame(&mut self, now: f64) -> Option<(Scene, Option<LabelText>)> {
        let view = self.view.as_mut()?;
        let scene = view.scene(
            &self.ring,
            &self.geometry,
            &self.animation,
            &self.timing,
            now,
        )?;
        let label = scene
            .label
            .as_ref()
            .and_then(|l| label_text(&self.ring, l.target));
        Some((scene, label))
    }
}

pub fn run(
    freeze_at: Option<f64>,
    save: Option<&Path>,
    config: &Config,
) -> Result<(), Box<dyn Error>> {
    let pointer = super::cursor();
    let monitor = dpi::monitor_at(pointer)?;
    let scale = dpi::scale_at(pointer);
    let cursor = Vec2::new(f64::from(pointer.x), f64::from(pointer.y));
    let geometry = Geometry::for_monitor(scale, &config.appearance, monitor);
    let animation = config.animation.clone();
    let ring = if config.ring.is_empty() {
        Ring::default()
    } else {
        config.ring.clone()
    };
    let origin = Vec2::new(f64::from(monitor.left), f64::from(monitor.top));
    // A frozen frame always opens in the same place, so captures line up.
    let anchor = match freeze_at {
        Some(_) => Vec2::new(
            f64::from(monitor.left + monitor.right) / 2.0,
            f64::from(monitor.top + monitor.bottom) / 2.0,
        ),
        None => cursor,
    };
    let mut player = Player {
        pending: script(&ring).into_iter().peekable(),
        ring,
        center: geometry.place(anchor, monitor) - origin,
        geometry,
        timing: Timing::new(&animation, false),
        animation,
        behavior: Behavior::default(),
        state: State::default(),
        view: None,
    };
    let mut overlay = Overlay::open(monitor, scale)?;
    overlay.renderer.set_appearance(&config.appearance)?;

    if let (Some(at), Some(path)) = (freeze_at, save) {
        player.advance(at);
        let frame = player.frame(at);
        let (scene, label) = match &frame {
            Some((scene, label)) => (Some(scene), label.as_ref()),
            None => (None, None),
        };
        overlay
            .renderer
            .render(scene, &player.ring, player.geometry.d, label)?;
        let (width, height, pixels) = overlay.renderer.read_pixels()?;
        let area = crop_area(player.center, &player.geometry, width, height);
        let rgba = png::bgra_premultiplied_to_rgba(&crop(&pixels, width, area));
        std::fs::write(path, png::encode_rgba(area.2, area.3, &rgba))?;
        return Ok(());
    }

    let start = Instant::now();
    loop {
        pump_messages();
        // The script opens the ring at 0 ms, so every pass draws a frame.
        overlay.renderer.wait_for_frame();
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let now = match freeze_at {
            Some(at) if elapsed > at + FROZEN_MS => return Ok(()),
            Some(at) => elapsed.min(at),
            None => elapsed,
        };
        player.advance(now);
        if player.view.is_none() {
            continue;
        }
        let frame = player.frame(now);
        let (scene, label) = match &frame {
            Some((scene, label)) => (Some(scene), label.as_ref()),
            None => (None, None),
        };
        overlay
            .renderer
            .draw(scene, &player.ring, player.geometry.d, label)?;
        if frame.is_none() {
            return Ok(());
        }
    }
}

/// Square around the ring big enough for the open folder arc and its labels:
/// left, top, width, height.
fn crop_area(center: Vec2, geometry: &Geometry, width: u32, height: u32) -> (u32, u32, u32, u32) {
    let half = (geometry.screen_margin() + geometry.d * 2.0).ceil();
    let left = (center.x - half).max(0.0) as u32;
    let top = (center.y - half).max(0.0) as u32;
    let right = ((center.x + half) as u32).min(width);
    let bottom = ((center.y + half) as u32).min(height);
    (left, top, right - left, bottom - top)
}

fn crop(pixels: &[u8], width: u32, (left, top, w, h): (u32, u32, u32, u32)) -> Vec<u8> {
    let stride = width as usize * 4;
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in top..top + h {
        let start = y as usize * stride + left as usize * 4;
        out.extend_from_slice(&pixels[start..start + w as usize * 4]);
    }
    out
}

fn pump_messages() {
    let mut msg = MSG::default();
    // SAFETY: standard message pump with a local variable.
    unsafe {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
