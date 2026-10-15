//! Gesture recognition: hold a button, drag and release.
//!
//! Coordinates are screen coordinates, with `y` growing downwards. The
//! distance threshold must already be adjusted to the monitor DPI here.

/// Distance threshold at 100% scaling (96 DPI).
pub const DEFAULT_THRESHOLD_PX: f64 = 30.0;

/// Point on the screen, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Result of classifying a movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    /// Did not go past the threshold.
    Click,
    Swipe(Direction),
}

/// Classifies the movement `(dx, dy)`.
///
/// Below the threshold (Euclidean distance) it is a [`Gesture::Click`]. Above
/// it, the dominant axis wins; on the exact diagonal (45°), the vertical one.
pub fn classify(dx: f64, dy: f64, threshold: f64) -> Gesture {
    if dx.hypot(dy) < threshold {
        return Gesture::Click;
    }
    let direction = if dx.abs() > dy.abs() {
        if dx > 0.0 {
            Direction::Right
        } else {
            Direction::Left
        }
    } else if dy > 0.0 {
        Direction::Down
    } else {
        Direction::Up
    };
    Gesture::Swipe(direction)
}

/// How a gesture ended, from pressing the button to releasing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Released quickly without going past the threshold: pass the native click on.
    Tap,
    /// Went past the threshold and released away from the origin.
    Swipe(Direction),
    /// Held longer than a tap without going past the threshold: nothing happens.
    HeldStill,
    /// Went past the threshold but came back near the origin: the user gave up.
    Cancelled,
}

/// Follows a gesture in progress.
#[derive(Debug, Clone, Copy)]
pub struct Tracker {
    origin: Point,
    threshold: f64,
    exceeded: bool,
}

impl Tracker {
    pub fn new(origin: Point, threshold: f64) -> Self {
        Self {
            origin,
            threshold,
            exceeded: false,
        }
    }

    pub fn origin(&self) -> Point {
        self.origin
    }

    /// Records a cursor movement while the button is held.
    pub fn moved(&mut self, pos: Point) {
        if !self.exceeded {
            self.exceeded = self.distance_to(pos) >= self.threshold;
        }
    }

    /// Ends the gesture. `held_ms` is the time between press and release;
    /// below `tap_ms`, without going past the threshold, it counts as a tap.
    pub fn release(mut self, pos: Point, held_ms: u32, tap_ms: u32) -> Outcome {
        self.moved(pos);
        if !self.exceeded {
            return if held_ms < tap_ms {
                Outcome::Tap
            } else {
                Outcome::HeldStill
            };
        }
        let (dx, dy) = self.delta_to(pos);
        match classify(dx, dy, self.threshold) {
            Gesture::Swipe(direction) => Outcome::Swipe(direction),
            Gesture::Click => Outcome::Cancelled,
        }
    }

    fn delta_to(&self, pos: Point) -> (f64, f64) {
        (
            f64::from(pos.x - self.origin.x),
            f64::from(pos.y - self.origin.y),
        )
    }

    fn distance_to(&self, pos: Point) -> f64 {
        let (dx, dy) = self.delta_to(pos);
        dx.hypot(dy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: f64 = DEFAULT_THRESHOLD_PX;

    #[test]
    fn classifies_the_four_directions() {
        assert_eq!(classify(100.0, 5.0, T), Gesture::Swipe(Direction::Right));
        assert_eq!(classify(-100.0, -5.0, T), Gesture::Swipe(Direction::Left));
        assert_eq!(classify(5.0, -100.0, T), Gesture::Swipe(Direction::Up));
        assert_eq!(classify(-5.0, 100.0, T), Gesture::Swipe(Direction::Down));
    }

    #[test]
    fn below_threshold_is_a_click() {
        assert_eq!(classify(0.0, 0.0, T), Gesture::Click);
        assert_eq!(classify(20.0, 20.0, T), Gesture::Click); // ~28.3 px
        assert_eq!(classify(29.9, 0.0, T), Gesture::Click);
    }

    #[test]
    fn threshold_is_inclusive_and_uses_euclidean_distance() {
        assert_eq!(classify(30.0, 0.0, T), Gesture::Swipe(Direction::Right));
        // Neither 18 nor 24 reaches the threshold on its own axis, but the distance is 30.
        assert_eq!(classify(18.0, -24.0, T), Gesture::Swipe(Direction::Up));
    }

    #[test]
    fn exact_diagonal_goes_vertical() {
        assert_eq!(classify(50.0, 50.0, T), Gesture::Swipe(Direction::Down));
        assert_eq!(classify(-50.0, -50.0, T), Gesture::Swipe(Direction::Up));
    }

    #[test]
    fn threshold_scales_with_dpi() {
        // At 150% the threshold becomes 45 px: 40 px is still a click.
        assert_eq!(classify(40.0, 0.0, T * 1.5), Gesture::Click);
        assert_eq!(classify(40.0, 0.0, T), Gesture::Swipe(Direction::Right));
    }

    #[test]
    fn quick_release_without_moving_is_a_tap() {
        let o = Point::new(500, 500);
        let mut g = Tracker::new(o, T);
        g.moved(Point::new(505, 498));
        assert_eq!(g.release(Point::new(506, 497), 80, 150), Outcome::Tap);
    }

    #[test]
    fn long_hold_without_moving_does_nothing() {
        let o = Point::new(500, 500);
        let g = Tracker::new(o, T);
        assert_eq!(g.release(o, 150, 150), Outcome::HeldStill);
        assert_eq!(g.release(o, 900, 150), Outcome::HeldStill);
    }

    #[test]
    fn a_quick_flick_is_still_a_gesture() {
        let g = Tracker::new(Point::new(0, 0), T);
        assert_eq!(
            g.release(Point::new(-120, 10), 60, 150),
            Outcome::Swipe(Direction::Left)
        );
    }

    #[test]
    fn slow_drag_is_a_gesture() {
        let mut g = Tracker::new(Point::new(0, 0), T);
        for y in 1..=20 {
            g.moved(Point::new(2, -y * 5));
        }
        assert_eq!(
            g.release(Point::new(2, -100), 700, 150),
            Outcome::Swipe(Direction::Up)
        );
    }

    #[test]
    fn going_out_and_back_cancels() {
        let mut g = Tracker::new(Point::new(0, 0), T);
        g.moved(Point::new(80, 0));
        assert_eq!(g.release(Point::new(3, 2), 400, 150), Outcome::Cancelled);
    }

    #[test]
    fn going_out_and_back_quickly_does_not_become_a_tap() {
        let mut g = Tracker::new(Point::new(0, 0), T);
        g.moved(Point::new(0, 60));
        assert_eq!(g.release(Point::new(0, 0), 90, 150), Outcome::Cancelled);
    }
}
