//! Nameplates: who gets one over whom (`information::sees_plate`, FL-7).

use super::*;

/// Sheepdogs in seats 0 and 3, Wolves in 1 and 2, all in sight of each other.
fn four() -> Match {
    let mut m = game(&[Pick::Sheepdog, Pick::Wolf, Pick::Wolf, Pick::Sheepdog], small());
    let (x, z) = open(&m);
    for k in 0..4 {
        stand(&mut m, k, (x + from_int(2) * k as Fx, z));
    }
    m
}

fn plates(m: &Match, viewer: usize) -> Vec<u8> {
    snapshot(m, viewer).plates
}

#[test]
fn a_sheepdog_gets_a_plate_over_its_teammate_and_none_over_a_wolf() {
    assert_eq!(plates(&four(), 0), vec![3]);
}

#[test]
fn a_wolf_gets_plates_over_its_teammate_and_the_sheepdogs_but_not_itself() {
    // WOLVES_SEE_DOGS is on: the Sheepdogs' outlines, and so their plates
    assert_eq!(plates(&four(), 1), vec![0, 2, 3]);
}

#[test]
fn a_cloaked_wolf_has_no_plate_for_sheepdogs_and_keeps_its_teams() {
    let mut m = four();
    m.players[2].slot.take(Kind::InvisibilityCloak);
    m.players[2].slot.activate();
    assert_eq!(plates(&m, 0), vec![3]);
    assert_eq!(plates(&m, 1), vec![0, 2, 3]);
}

#[test]
fn a_dead_wolf_has_no_plate() {
    let mut m = four();
    m.world.bodies[2].out = true;
    assert_eq!(plates(&m, 1), vec![0, 3]);
}
