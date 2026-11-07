//! Where a player looks, the one line the crosshair and the rules share.
//!
//! The play camera orbits a point above the animal, so the crosshair at the
//! middle of the screen lies on a line through that point. `Ray` is that line.
//! It comes from a body's place, yaw and pitch alone, so every browser finds
//! the same target. A camera pulled in by a hill or a tree moves along the
//! line, so what's under the crosshair never changes with it, and the page's
//! camera needs nothing of the rules but this file (`scene.rs`).

use crate::body::Body;
use crate::fx::{self, from_int, Fx};
use crate::render::geom::{atan2, cm, v3, V3};

/// How high above the animal's feet the camera turns.
pub const PIVOT_UP: Fx = cm(115);
/// How far back along the look the camera stands.
pub const BACK: Fx = from_int(5);
/// The look a match starts with: 5 m behind and 2 m above, a third lower than
/// it first stood (asin of 0.85 / 5 is 9.8 degrees).
pub const REST_PITCH: i16 = -1780;
/// How far a player can look up and down. Up is 40 degrees, with the camera
/// pulled in over the ground; down stops short of looking at the animal's back.
pub const LOOK_UP: i16 = 7300;
pub const LOOK_DOWN: i16 = -11000;
/// Angle units a pixel of mouse: 0.11 degrees.
pub const LOOK_PER_PIXEL: i32 = 20;
/// The height of the middle of a sheep, a mound or a carcass above the ground.
pub const TARGET_UP: Fx = cm(30);

/// The way a yaw and a pitch look, a unit vector. See `body.rs` on facing.
pub fn look_way(yaw: u16, pitch: i16) -> V3 {
    let (s, c) = (fx::sin(yaw as i32), fx::cos(yaw as i32));
    let flat = fx::cos(pitch as i32);
    v3(fx::mul(s, flat), fx::sin(pitch as i32), -fx::mul(c, flat)).norm()
}

/// A look moved by the mouse: the yaw wraps, the pitch stops at its ends.
pub fn turned(aim: (u16, i16), dx: i32, dy: i32) -> (u16, i16) {
    turned_at(aim, dx, dy, 1)
}

/// `turned`, seen through this much zoom: the look moves that many times
/// slower, so the mouse moves the view as far across the screen.
pub fn turned_at(aim: (u16, i16), dx: i32, dy: i32, zoom: i32) -> (u16, i16) {
    let units = |pixels: i32| pixels.saturating_mul(LOOK_PER_PIXEL) / zoom;
    let yaw = (aim.0 as i32).wrapping_add(units(dx)) as u16;
    let pitch = (aim.1 as i32).saturating_sub(units(dy));
    (yaw, pitch.clamp(LOOK_DOWN as i32, LOOK_UP as i32) as i16)
}

/// The yaw that faces along (dx, dz): see `body.rs` on facing.
pub fn yaw_toward(dx: Fx, dz: Fx) -> u16 {
    atan2(dx, -dz) as u16
}

/// The pitch that puts the crosshair on a target this far off across the
/// ground, with its foot this far above the looker's feet.
pub fn pitch_toward(far: Fx, up: Fx) -> i16 {
    let rise = up + TARGET_UP - PIVOT_UP;
    atan2(rise, far).clamp(LOOK_DOWN as i32, LOOK_UP as i32) as i16
}

/// How far apart two yaws are, the short way round.
pub fn yaw_apart(a: u16, b: u16) -> i32 {
    (a.wrapping_sub(b) as i16 as i32).abs()
}

/// A yaw moved toward `want` by at most `step`, the short way round.
pub fn turned_toward(yaw: u16, want: u16, step: i32) -> u16 {
    let off = want.wrapping_sub(yaw) as i16 as i32;
    yaw.wrapping_add(off.clamp(-step, step) as u16)
}

/// The crosshair's line in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ray {
    /// The point the camera turns around.
    pub pivot: V3,
    pub way: V3,
}

impl Ray {
    pub fn new(at: V3, yaw: u16, pitch: i16) -> Ray {
        Ray {
            pivot: at.add(v3(0, PIVOT_UP, 0)),
            way: look_way(yaw, pitch),
        }
    }

    pub fn of(body: &Body) -> Ray {
        Ray::new(v3(body.x, body.y, body.z), body.yaw, body.pitch)
    }

    /// The camera, `back` along the line.
    pub fn eye(&self, back: Fx) -> V3 {
        self.pivot.sub(self.way.scale(back))
    }

    /// How far along the line, from the camera at its full distance, the line
    /// passes within `radius` of `at`, if it does.
    pub fn hits(&self, at: V3, radius: Fx) -> Option<Fx> {
        let to = at.sub(self.eye(BACK));
        let along = to.dot(self.way);
        let off = to.sub(self.way.scale(along));
        let miss = fx::length(fx::length(off.x, off.y), off.z);
        (along >= 0 && miss <= radius).then_some(along)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::ONE;

    #[test]
    fn a_match_starts_about_five_meters_back_and_two_up() {
        let eye = Ray::new(v3(0, 0, 0), 0, REST_PITCH).eye(BACK);
        // the look is along -z, so behind is +z
        assert!((eye.z - from_int(5)).abs() < cm(30), "{}", eye.z);
        assert!((eye.y - from_int(2)).abs() < cm(10), "{}", eye.y);
    }

    #[test]
    fn a_yaw_toward_a_point_is_the_way_a_body_faces() {
        // yaw 0 faces -z, a quarter turn +x, and so round
        assert_eq!(yaw_toward(0, -ONE), 0);
        assert_eq!(yaw_toward(ONE, 0), 16384);
        assert_eq!(yaw_toward(0, ONE), 32768);
        assert_eq!(yaw_toward(-ONE, 0), 49152);
    }

    #[test]
    fn a_pitch_toward_a_target_puts_the_crosshair_on_it() {
        // the line starts 1.15 m up and a target's middle is 0.3 m up, so
        // 0.85 m off across the ground is 45 degrees down
        let pitch = pitch_toward(cm(85), 0);
        assert!((pitch as i32 + 8192).abs() < 64, "{pitch}");
        // straight underfoot stops at the look's end
        assert_eq!(pitch_toward(0, 0), LOOK_DOWN);
    }

    #[test]
    fn a_turn_takes_the_short_way_round_and_stops_at_its_step() {
        assert_eq!(turned_toward(65000, 500, 1000), 464);
        assert_eq!(turned_toward(500, 65000, 1000), 65036);
        assert_eq!(turned_toward(100, 300, 1000), 300);
        assert_eq!(yaw_apart(65000, 500), 1036);
    }

    #[test]
    fn the_mouse_turns_the_look_and_the_pitch_stops_at_its_ends() {
        assert_eq!(turned((100, 0), 5, 0), (100 + 5 * LOOK_PER_PIXEL as u16, 0));
        assert_eq!(turned((0, 0), -1, 0).0 as i32, 65536 - LOOK_PER_PIXEL);
        assert_eq!(turned((0, 0), 0, 1).1, -(LOOK_PER_PIXEL as i16));
        assert_eq!(turned((0, 0), 0, 100_000).1, LOOK_DOWN);
        assert_eq!(turned((0, 0), 0, -100_000).1, LOOK_UP);
    }

    #[test]
    fn through_three_times_the_zoom_the_mouse_turns_the_look_a_third_as_far() {
        assert_eq!(turned_at((100, 0), 30, 0, 3), (300, 0));
        assert_eq!(turned_at((100, 0), 0, 30, 3), (100, -200));
    }

    #[test]
    fn the_crosshair_finds_what_the_pitch_points_at_and_passes_over_the_rest() {
        // looking down at 45 degrees from 1.15 m up, the line meets the ground
        // 1.15 m ahead of the feet, and the feet are at the origin
        let ray = Ray::new(v3(0, 0, 0), 0, -8192);
        assert!(ray.hits(v3(0, cm(30), -cm(85)), cm(10)).is_some());
        assert_eq!(ray.hits(v3(0, 0, -from_int(9)), ONE), None);
        // level, the line goes 0.85 m over a sheep's middle
        let level = Ray::new(v3(0, 0, 0), 0, 0);
        assert_eq!(level.hits(v3(0, cm(30), -from_int(2)), cm(80)), None);
    }

    #[test]
    fn a_camera_pulled_in_stays_on_the_crosshairs_line() {
        let ray = Ray::new(v3(from_int(3), 0, from_int(4)), 9000, -2000);
        let target = ray.eye(-from_int(9));
        for back in [BACK, from_int(3), ONE] {
            let to = target.sub(ray.eye(back)).norm();
            assert!(
                (to.x - ray.way.x).abs() < 64 && (to.y - ray.way.y).abs() < 64,
                "{back}"
            );
        }
    }
}
