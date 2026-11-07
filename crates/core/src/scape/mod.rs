//! The hillside's scenery, built once per match from the map. Everything
//! here takes the map's data and returns plain numbers: nothing in it moves
//! the rules, so it can change without touching a sheep. The look comes from
//! real photos: `notes/hillside-look.md`.

use crate::fx::{Fx, ONE};

pub mod backdrop;
pub mod dress;
pub mod field;
pub mod grass;
pub mod hills;
pub mod noise;
pub mod palette;
pub mod shapes;
pub mod sky;
pub mod view;
pub mod wall;
pub mod wind;
pub mod world;

/// Ticks a second, for the wind. The contract's 60.
pub(crate) const TICKS: i64 = 60;

/// A length in centimeters.
pub const fn cm(n: i32) -> Fx {
    n * ONE / 100
}
