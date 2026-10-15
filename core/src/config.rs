//! Engine configuration, stored as TOML.
//!
//! Loading never panics: an invalid or out-of-range value falls back to its
//! default and produces a [`Warning`]. Only TOML that cannot be parsed returns
//! an error.

mod load;

use std::ops::RangeInclusive;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::apps::AppName;
use crate::color::Color;
use crate::command::{Chord, Command, Key, Modifiers};
use crate::gesture::{DEFAULT_THRESHOLD_PX, Direction, Outcome};
use crate::layout::{ITEM_DIAMETER_PX, RING_RADIUS_PER_ITEM};
use crate::ring::Ring;

pub use load::{ConfigError, Loaded, Warning};

/// Allowed range of every numeric value.
pub mod limits {
    use std::ops::RangeInclusive;

    pub const SPEED: RangeInclusive<f64> = 0.5..=2.0;
    /// Applies to every animation time.
    pub const ANIMATION_MS: RangeInclusive<u32> = 0..=1000;
    pub const VISCOSITY: RangeInclusive<f64> = 0.2..=2.0;
    pub const BREAK_AT: RangeInclusive<f64> = 0.5..=0.95;
    pub const WOBBLE: RangeInclusive<f64> = 0.0..=1.0;
    /// Below 60 ms it is hard to tap quickly without opening the ring.
    pub const HOLD_TO_OPEN_MS: RangeInclusive<u32> = 60..=1000;
    pub const PATH_TOLERANCE_MS: RangeInclusive<u32> = 0..=1000;
    /// Minimum gesture distance, in pixels at 100% scaling.
    pub const THRESHOLD_PX: RangeInclusive<u32> = 10..=200;
    /// Opacity of the ring items. Below 0.3 they get hard to see.
    pub const ITEM_OPACITY: RangeInclusive<f64> = 0.3..=1.0;
    /// Item diameter, in pixels at 100% scaling.
    pub const ITEM_SIZE_PX: RangeInclusive<f64> = 32.0..=96.0;
    /// Ring radius, in item diameters. At 1.5, two items 45° apart still keep
    /// [`crate::layout::MIN_ITEM_GAP`] between them.
    pub const RING_RADIUS_PER_ITEM: RangeInclusive<f64> = 1.5..=3.0;
}

/// Version of the file format, written as `version = 1` at the top of the
/// file. It goes up only when a change would make an older file mean
/// something else; the loader then converts older files as it reads them. A
/// file without it was written before the field existed and counts as
/// version 1.
pub const FORMAT_VERSION: u32 = 1;

/// File created on the first run: the default values, with comments that
/// explain each option. A test makes sure it loads without warnings and gives
/// exactly [`Config::default`].
pub const DEFAULT_FILE: &str = include_str!("config/default.toml");

/// Whole configuration file.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Config {
    pub gestures: Gestures,
    pub apps: Apps,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<Profile>,
    pub animation: Animation,
    pub behavior: Behavior,
    pub appearance: Appearance,
    pub ring: Ring,
}

impl Config {
    /// Reads the configuration from TOML text. See [`Loaded`].
    pub fn from_toml(text: &str) -> Result<Loaded, ConfigError> {
        // Older Notepad versions save UTF-8 with a BOM.
        load::from_toml(text.strip_prefix('\u{feff}').unwrap_or(text))
    }

    /// Writes the configuration as TOML, with the format version first.
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        #[derive(Serialize)]
        struct File<'a> {
            version: u32,
            #[serde(flatten)]
            config: &'a Config,
        }
        toml::to_string_pretty(&File {
            version: FORMAT_VERSION,
            config: self,
        })
    }
}

/// A mouse button that can run the gestures or open the ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    /// The side button that goes back (button 4).
    Back,
    /// The side button that goes forward (button 5).
    Forward,
    /// The wheel button.
    Middle,
}

/// `[gestures]` section: the button that runs the gestures and what each
/// gesture does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gestures {
    /// Never the same as the ring button while the ring has items.
    pub button: MouseButton,
    pub up: Command,
    pub down: Command,
    pub left: Command,
    pub right: Command,
    /// Quick tap, without dragging.
    pub click: Command,
    /// Minimum drag distance, in pixels at 100% scaling. The engine adjusts it
    /// to the monitor DPI.
    pub threshold_px: u32,
}

impl Gestures {
    /// Command for a gesture outcome. `None` when the gesture asks for nothing.
    pub fn command(&self, outcome: Outcome) -> Option<&Command> {
        match outcome {
            Outcome::Tap => Some(&self.click),
            Outcome::Swipe(Direction::Up) => Some(&self.up),
            Outcome::Swipe(Direction::Down) => Some(&self.down),
            Outcome::Swipe(Direction::Left) => Some(&self.left),
            Outcome::Swipe(Direction::Right) => Some(&self.right),
            Outcome::HeldStill | Outcome::Cancelled => None,
        }
    }
}

impl Default for Gestures {
    fn default() -> Self {
        let keys = |modifiers, key| Command::Keys(Chord::new(modifiers, key));
        let ctrl_win = Modifiers::CTRL.with(Modifiers::WIN);
        Self {
            button: MouseButton::Back,
            up: keys(Modifiers::WIN, Key::Tab),
            down: keys(Modifiers::WIN, Key::Char('D')),
            // Like pushing the screen aside: dragging right brings the
            // desktop on the left (the previous one) into view.
            left: keys(ctrl_win, Key::Right),
            right: keys(ctrl_win, Key::Left),
            click: Command::Native,
            threshold_px: DEFAULT_THRESHOLD_PX as u32,
        }
    }
}

/// Gestures of a profile. Only the ones set here change; the others come from
/// [`Gestures`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct GestureOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up: Option<Command>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down: Option<Command>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<Command>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<Command>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click: Option<Command>,
}

impl GestureOverrides {
    pub fn command(&self, outcome: Outcome) -> Option<&Command> {
        match outcome {
            Outcome::Tap => self.click.as_ref(),
            Outcome::Swipe(Direction::Up) => self.up.as_ref(),
            Outcome::Swipe(Direction::Down) => self.down.as_ref(),
            Outcome::Swipe(Direction::Left) => self.left.as_ref(),
            Outcome::Swipe(Direction::Right) => self.right.as_ref(),
            Outcome::HeldStill | Outcome::Cancelled => None,
        }
    }
}

/// `[apps]` section: where gestures stay off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Apps {
    /// Turns gestures off while the focused app is fullscreen.
    pub disable_in_fullscreen: bool,
    /// In these apps the button works as usual, as if the engine did not exist.
    pub excluded: Vec<AppName>,
}

impl Default for Apps {
    fn default() -> Self {
        Self {
            disable_in_fullscreen: true,
            excluded: Vec::new(),
        }
    }
}

/// One `[[profiles]]` entry: gestures of their own for some apps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Profile {
    /// Only for whoever reads the file and for the log.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Never empty. An app belongs to one profile at most.
    pub apps: Vec<AppName>,
    pub gestures: GestureOverrides,
}

/// Style of the blob that carries the highlight from the folder to the action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlobStyle {
    /// Thick neck that breaks at the end of the trip.
    #[default]
    Viscous,
    /// Lets go early, flies and wobbles.
    Elastic,
}

/// Starting values of the blob shape for each style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlobPreset {
    pub blob_ms_per_item: u32,
    pub viscosity: f64,
    pub break_at: f64,
    pub wobble: f64,
}

impl BlobStyle {
    pub fn preset(self) -> BlobPreset {
        match self {
            BlobStyle::Viscous => BlobPreset {
                blob_ms_per_item: 135,
                viscosity: 0.85,
                break_at: 0.95,
                wobble: 0.0,
            },
            BlobStyle::Elastic => BlobPreset {
                blob_ms_per_item: 135,
                viscosity: 1.2,
                break_at: 0.78,
                wobble: 0.4,
            },
        }
    }
}

/// `[animation]` section. It only changes the looks, never the behavior.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Animation {
    pub style: BlobStyle,
    /// Overall speed: multiplies every animation at once.
    pub speed: f64,
    pub ring_open_ms: u32,
    pub ring_close_ms: u32,
    /// Blob time per item diameter travelled.
    pub blob_ms_per_item: u32,
    pub viscosity: f64,
    /// Fraction of the trip at which the blob's neck breaks.
    pub break_at: f64,
    pub wobble: f64,
    pub folder_items_ms: u32,
    pub folder_close_ms: u32,
    /// If `true`, animates fully even with the Windows "Animation effects"
    /// turned off.
    pub ignore_system_reduced_motion: bool,
}

impl Animation {
    /// Defaults of a blob style.
    pub fn for_style(style: BlobStyle) -> Self {
        let p = style.preset();
        Self {
            style,
            speed: 1.0,
            ring_open_ms: 150,
            ring_close_ms: 170,
            blob_ms_per_item: p.blob_ms_per_item,
            viscosity: p.viscosity,
            break_at: p.break_at,
            wobble: p.wobble,
            folder_items_ms: 170,
            folder_close_ms: 100,
            ignore_system_reduced_motion: false,
        }
    }

    /// Actual duration of an animation after the overall speed.
    pub fn scaled(&self, ms: u32) -> Duration {
        Duration::from_secs_f64(f64::from(ms) / 1000.0 / self.speed)
    }
}

impl Default for Animation {
    fn default() -> Self {
        Self::for_style(BlobStyle::default())
    }
}

/// How the ring opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenWith {
    /// Holding the trigger opens it; a quick tap keeps the native function.
    #[default]
    Hold,
    /// One tap opens and another closes; the trigger loses its native function.
    Tap,
}

/// `[behavior]` section. Logic timings: the animation speed does not touch them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Behavior {
    pub open_with: OpenWith,
    /// Quick mode: releasing the trigger over an action runs it.
    pub execute_on_release: bool,
    pub hold_to_open_ms: u32,
    /// With a folder open, how long the cursor must rest on another ring item
    /// before it gets the highlight.
    pub path_tolerance_ms: u32,
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            open_with: OpenWith::Hold,
            execute_on_release: false,
            hold_to_open_ms: 150,
            path_tolerance_ms: 50,
        }
    }
}

/// Colors of the label next to the item under the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelStyle {
    /// Dark text on a light background.
    #[default]
    Light,
    /// Light text on a dark background.
    Dark,
}

/// `[appearance]` section: colors, transparency and size of the ring. Like
/// the animation, it only changes the looks. The defaults are the ring as it
/// always looked: white items with a black highlight.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Appearance {
    /// The circles of the ring and the center button.
    pub item_color: Color,
    /// The highlight and the blob are always solid; the center button is a
    /// little fainter than the items.
    pub item_opacity: f64,
    /// Icons on the items without the highlight.
    pub icon_color: Color,
    /// The item under the cursor, the open folder and the blob.
    pub highlight_color: Color,
    pub highlight_icon_color: Color,
    /// The center button, when releasing there would close the ring.
    pub cancel_color: Color,
    /// Item diameter, in pixels at 100% scaling. The center button follows
    /// it.
    pub item_size_px: f64,
    /// Distance from the ring center to the center of each item, in pixels at
    /// 100% scaling. The folder arc is twice as far.
    pub ring_radius_px: f64,
    /// A soft shadow around the items, so they stand out on light windows.
    pub shadow: bool,
    pub label: LabelStyle,
}

impl Appearance {
    /// The ring radius when the file does not set one: it follows the item
    /// size, to a tenth of a pixel.
    pub fn default_ring_radius_px(item_size_px: f64) -> f64 {
        tenths(item_size_px * RING_RADIUS_PER_ITEM)
    }

    /// The ring radii allowed with this item size: close enough to reach,
    /// far enough for the items never to touch.
    pub fn ring_radius_px_range(item_size_px: f64) -> RangeInclusive<f64> {
        let (min, max) = limits::RING_RADIUS_PER_ITEM.into_inner();
        // Rounded inwards to a tenth, so the limits read well in a warning.
        // The tiny margin keeps 52.3 × 3 (156.899…) at 156.9.
        let low = (item_size_px * min * 10.0 - 1e-9).ceil() / 10.0;
        let high = (item_size_px * max * 10.0 + 1e-9).floor() / 10.0;
        low..=high
    }
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            item_color: Color::WHITE,
            item_opacity: 0.96,
            icon_color: Color::rgb(0x2A, 0x2B, 0x31),
            highlight_color: Color::rgb(0x12, 0x12, 0x14),
            highlight_icon_color: Color::WHITE,
            cancel_color: Color::rgb(0xE6, 0x40, 0x54),
            item_size_px: ITEM_DIAMETER_PX,
            ring_radius_px: Self::default_ring_radius_px(ITEM_DIAMETER_PX),
            shadow: true,
            label: LabelStyle::Light,
        }
    }
}

fn tenths(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
