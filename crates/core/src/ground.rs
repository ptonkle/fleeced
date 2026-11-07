//! The seam between the sim and a map (`notes/contract.md`). The sim asks a
//! map for the height of the land and to push a circle out of anything
//! solid; `map::Map` implements this, and neither side waits on the other.

use crate::fx::{self, Fx};
use crate::map::Mover;

/// What `Ground::field` answers in the Fold.
pub const FOLD_FIELD: u8 = 3;

pub trait Ground {
    fn height(&self, x: Fx, z: Fx) -> Fx;
    /// Push a circle out of walls, trees and cairns; returns the new x, z.
    fn push_out(&self, x: Fx, z: Fx, radius: Fx) -> (Fx, Fx);
    /// `push_out` for this kind of body: its feet decide what a jump clears,
    /// and the Fold's gap is shut to some and open to others. Land with
    /// nothing like that leaves this as it is.
    fn push_out_as(&self, x: Fx, z: Fx, radius: Fx, _mover: Mover) -> (Fx, Fx) {
        self.push_out(x, z, radius)
    }
    /// Which walled field a point is in, from 0 up, or `FOLD_FIELD` in the
    /// Fold. A dry-stone wall hides sheep from each other, so a flock's two
    /// halves on either side of a wall do not walk back toward each other.
    /// A map without fields leaves this as it is: everything in sight.
    fn field(&self, _x: Fx, _z: Fx) -> u8 {
        0
    }
}

/// Level, empty land for tests.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FlatField;

impl Ground for FlatField {
    fn height(&self, _x: Fx, _z: Fx) -> Fx {
        0
    }

    fn push_out(&self, x: Fx, z: Fx, _radius: Fx) -> (Fx, Fx) {
        (x, z)
    }
}

/// A box on the ground plane.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub min_x: Fx,
    pub min_z: Fx,
    pub max_x: Fx,
    pub max_z: Fx,
}

impl Rect {
    pub fn contains(&self, x: Fx, z: Fx) -> bool {
        x >= self.min_x && x <= self.max_x && z >= self.min_z && z <= self.max_z
    }

    /// Push a circle out of this box; `None` when it does not touch.
    fn eject(&self, x: Fx, z: Fx, radius: Fx) -> Option<(Fx, Fx)> {
        let near_x = x.clamp(self.min_x, self.max_x);
        let near_z = z.clamp(self.min_z, self.max_z);
        let (dx, dz) = (x - near_x, z - near_z);
        if dx == 0 && dz == 0 {
            // the center is inside: leave by the nearest side
            let sides = [
                (x - self.min_x, self.min_x - radius, z),
                (self.max_x - x, self.max_x + radius, z),
                (z - self.min_z, x, self.min_z - radius),
                (self.max_z - z, x, self.max_z + radius),
            ];
            let (_, a, b) = *sides.iter().min_by_key(|s| s.0).expect("four sides");
            return Some((a, b));
        }
        if dx.abs() >= radius || dz.abs() >= radius {
            return None;
        }
        let d = fx::length(dx, dz);
        if d >= radius {
            return None;
        }
        let scaled = |v: Fx| (v as i64 * radius as i64 / d as i64) as Fx;
        Some((near_x + scaled(dx), near_z + scaled(dz)))
    }
}

/// Level land with solid boxes on it: walls with gaps, for tests and probes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WalledField {
    pub walls: Vec<Rect>,
    /// The x of each wall that divides one field from the next, west to east.
    pub dividers: Vec<Fx>,
    /// The Fold, if there is one. A folded sheep stays in it.
    pub fold: Option<Rect>,
}

impl Ground for WalledField {
    fn height(&self, _x: Fx, _z: Fx) -> Fx {
        0
    }

    fn push_out(&self, x: Fx, z: Fx, radius: Fx) -> (Fx, Fx) {
        let (mut x, mut z) = (x, z);
        for wall in &self.walls {
            if let Some(out) = wall.eject(x, z, radius) {
                (x, z) = out;
            }
        }
        (x, z)
    }

    fn push_out_as(&self, x: Fx, z: Fx, radius: Fx, mover: Mover) -> (Fx, Fx) {
        let (x, z) = self.push_out(x, z, radius);
        match (mover, self.fold) {
            (Mover::FoldedSheep, Some(fold)) => (
                x.clamp(fold.min_x + radius, fold.max_x - radius),
                z.clamp(fold.min_z + radius, fold.max_z - radius),
            ),
            _ => (x, z),
        }
    }

    fn field(&self, x: Fx, z: Fx) -> u8 {
        if self.fold.is_some_and(|fold| fold.contains(x, z)) {
            return FOLD_FIELD;
        }
        self.dividers.iter().filter(|&&d| x >= d).count() as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::from_int;

    fn wall() -> WalledField {
        // a wall 1 m thick across x = 10..11
        WalledField {
            walls: vec![Rect {
                min_x: from_int(10),
                min_z: from_int(-20),
                max_x: from_int(11),
                max_z: from_int(20),
            }],
            dividers: vec![from_int(10)],
            fold: Some(Rect {
                min_x: from_int(30),
                min_z: from_int(-5),
                max_x: from_int(40),
                max_z: from_int(5),
            }),
        }
    }

    #[test]
    fn a_flat_field_is_level_and_stops_nothing() {
        assert_eq!(FlatField.height(from_int(7), from_int(-3)), 0);
        assert_eq!(
            FlatField.push_out(from_int(7), from_int(-3), from_int(1)),
            (from_int(7), from_int(-3))
        );
    }

    #[test]
    fn a_circle_touching_a_wall_is_pushed_to_its_face() {
        // center 0.5 m from the face, radius 1: out to 1 m from it
        let (x, z) = wall().push_out(fx::ratio(19, 2), from_int(3), from_int(1));
        assert_eq!((x, z), (from_int(9), from_int(3)));
    }

    #[test]
    fn a_center_inside_a_wall_leaves_by_the_nearest_side() {
        // 10.25 is 0.25 m in from the near face and 0.75 m from the far one
        let (x, _) = wall().push_out(fx::ratio(41, 4), 0, fx::HALF);
        assert_eq!(x, fx::ratio(19, 2));
    }

    #[test]
    fn a_circle_clear_of_a_wall_does_not_move() {
        assert_eq!(wall().push_out(from_int(8), 0, from_int(1)), (from_int(8), 0));
        // past the corner at (10, 20) by 0.8 m each way is 1.13 m from it:
        // close on each axis, clear on the diagonal
        let (x, z) = (from_int(10) - fx::ratio(4, 5), from_int(20) + fx::ratio(4, 5));
        assert_eq!(wall().push_out(x, z, from_int(1)), (x, z));
    }

    #[test]
    fn a_wall_that_divides_fields_counts_them_from_the_west() {
        assert_eq!(wall().field(from_int(9), 0), 0);
        assert_eq!(wall().field(from_int(12), 0), 1);
        assert_eq!(wall().field(from_int(35), 0), FOLD_FIELD);
        assert_eq!(FlatField.field(from_int(12), 0), 0);
    }

    #[test]
    fn a_folded_sheep_stays_in_and_any_other_body_does_not_care() {
        let land = wall();
        let r = fx::HALF;
        let (x, z) = (from_int(29), from_int(9));
        assert_eq!(land.push_out_as(x, z, r, Mover::Sheep), (x, z));
        // the box less the radius: 30.5, 4.5
        let held = (from_int(30) + r, from_int(5) - r);
        assert_eq!(land.push_out_as(x, z, r, Mover::FoldedSheep), held);
        assert_eq!(FlatField.push_out_as(x, z, r, Mover::FoldedSheep), (x, z));
    }

    #[test]
    fn a_corner_pushes_along_the_diagonal() {
        // 0.3 and 0.4 past the corner at (10, 20) is 0.5 away (3-4-5), and a
        // radius of 1 doubles it
        let (x, z) = wall().push_out(
            from_int(10) - fx::ratio(3, 10),
            from_int(20) + fx::ratio(4, 10),
            from_int(1),
        );
        assert!((x - (from_int(10) - fx::ratio(6, 10))).abs() <= 8, "{x}");
        assert!((z - (from_int(20) + fx::ratio(8, 10))).abs() <= 8, "{z}");
    }
}
