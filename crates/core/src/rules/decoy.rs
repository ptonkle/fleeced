//! The Decoy sheep: a fake sheep a Sheepdog puts down, which grazes where it
//! stands until a Wolf bites it. A Sheepdog keeps one at a time.
//!
//! To a Wolf it must read as any other sheep (FL-7), so it goes by an id past
//! the real flock's, one per player, and a Wolf's page gets every sheep's id
//! through one shuffle drawn at match start (`labels`). A Wolf can't tell a
//! decoy's id from a real one's, and the rows come in id order, so where it
//! sits in the list says nothing either.

use super::tune;
use crate::flock::Kind;
use crate::fx::{self, Fx};
use crate::rng::Rng;

/// A Decoy sheep on the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Decoy {
    /// The Sheepdog's index in `Match::players`.
    pub owner: u8,
    pub x: Fx,
    pub z: Fx,
    pub face_x: Fx,
    pub face_z: Fx,
    /// Its look, copied from a real sheep near it.
    pub size: Fx,
    pub fleece: u8,
    pub kind: Kind,
}

/// Where a Decoy sheep goes: `DECOY_AHEAD` in front of a body at (x, z)
/// facing `yaw`, which at 0 looks along -z.
pub fn spot(x: Fx, z: Fx, yaw: u16) -> (Fx, Fx) {
    let (s, c) = (fx::sin(yaw as i32), fx::cos(yaw as i32));
    (
        x + fx::mul(s, tune::DECOY_AHEAD),
        z - fx::mul(c, tune::DECOY_AHEAD),
    )
}

/// The id this player's Decoy sheep goes by: past the flock's, one a player.
pub fn id(flock: usize, owner: usize) -> u16 {
    (flock + owner) as u16
}

/// The player whose Decoy sheep goes by this id, or `None` for a real sheep.
pub fn owner_of(id: u16, flock: usize) -> Option<usize> {
    (id as usize).checked_sub(flock)
}

/// Every id from 0 to `n`, shuffled with `rng`: what a Wolf's page calls each
/// sheep and decoy. Each id gets a label of its own.
pub fn labels(n: usize, rng: &mut Rng) -> Vec<u16> {
    let mut out: Vec<u16> = (0..n as u16).collect();
    for i in (1..n).rev() {
        let j = rng.below(i as u32 + 1) as usize;
        out.swap(i, j);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decoy_stands_two_meters_ahead() {
        // yaw 0 looks along -z, and a quarter turn along +x
        assert_eq!(spot(0, 0, 0), (0, -fx::from_int(2)));
        let (x, z) = spot(0, 0, (fx::TURN / 4) as u16);
        assert_eq!((fx::round(x), fx::round(z)), (2, 0));
    }

    #[test]
    fn a_decoys_id_is_past_the_flock_and_names_its_owner() {
        assert_eq!(id(20, 3), 23);
        assert_eq!(owner_of(23, 20), Some(3));
        assert_eq!(owner_of(19, 20), None);
    }

    #[test]
    fn every_id_gets_a_label_of_its_own() {
        let mut seen = labels(28, &mut Rng::new(5));
        seen.sort_unstable();
        assert_eq!(seen, (0..28).collect::<Vec<u16>>());
    }
}
