//! Bots tested through `Match`, on the real map, with bodies stood where a
//! test wants them. Distances are worked out by hand from the tune tables.

use std::sync::{Arc, OnceLock};

use super::{gives_itself_away, Action, BodyRow};
use crate::body::{Input, Role, CROUCH};
use crate::fx::{self, from_int};
use crate::game::{CarcassSpot, Match};
use crate::lobby::{MatchStart, Role as Pick, StartSeat};
use crate::map::Map;
use crate::online::{Rollback, SEATS};
use crate::rules::carcasses::Carcass;
use crate::rules::setup::Settings;
use crate::rules::tune::{self, Powerup as Kind};

const DOG: usize = 0;
const WOLF: usize = 1;

fn map() -> Arc<Map> {
    static MAP: OnceLock<Arc<Map>> = OnceLock::new();
    MAP.get_or_init(|| Arc::new(Map::generate(1))).clone()
}

fn bots(picks: &[Pick], sheep: i32) -> Match {
    let seat = |(k, role): (usize, &Pick)| StartSeat {
        seat: k as u32,
        role: *role,
        person: String::new(),
        name: String::new(),
    };
    let start = MatchStart {
        seed: 1,
        seats: picks.iter().enumerate().map(seat).collect(),
        settings: Settings {
            sheep,
            eats_required: 5,
            ..Settings::default()
        },
    };
    Match::new(&start, map())
}

fn play(m: &mut Match, ticks: usize) {
    for _ in 0..ticks {
        m.step(&[None; SEATS]);
    }
}

/// A bot Sheepdog this far west of a Wolf held by a person, on open ground in
/// the second field, with the sheep far off.
fn dog_near_held_wolf(gap: i32) -> Match {
    let mut m = bots(&[Pick::Sheepdog, Pick::Wolf], 10);
    for s in &mut m.world.flock.sheep {
        (s.x, s.z) = (from_int(190), from_int(150));
    }
    let (wolf, dog) = (&mut m.world.bodies[WOLF], from_int(160 - gap));
    (wolf.x, wolf.z) = (from_int(160), from_int(88));
    m.world.bodies[DOG].x = dog;
    m.world.bodies[DOG].z = from_int(88);
    m
}

/// A tick with the Wolf holding these buttons and the Sheepdog a bot.
fn tick_with_wolf_holding(m: &mut Match, buttons: u16) {
    let mut keys = [None; SEATS];
    keys[WOLF] = Some(Input {
        buttons,
        ..Input::default()
    });
    m.step(&keys);
}

#[test]
fn two_copies_of_a_bot_match_end_on_the_same_checksum() {
    let mut a = bots(&[Pick::Sheepdog, Pick::Wolf, Pick::Wolf, Pick::Sheepdog], 30);
    play(&mut a, 700);
    let mut b = a.clone();
    play(&mut a, 700);
    play(&mut b, 700);
    assert_eq!(a.sum(), b.sum());
    assert_eq!(a, b);
}

#[test]
fn what_a_bot_keeps_is_in_the_checksum() {
    // a replay that lost a bot's memory has to show as a different match
    let a = bots(&[Pick::Sheepdog, Pick::Wolf], 10);
    let mut b = a.clone();
    b.players[DOG].brain.barked = 1;
    assert_ne!(a.sum(), b.sum());
}

#[test]
fn a_bot_seat_walks_away_from_where_it_started() {
    // a walk is 8.4 m/s at the slowest, so 3 s of it is 25 m, less the turn
    let mut m = bots(&[Pick::Sheepdog, Pick::Wolf], 30);
    play(&mut m, 180);
    assert!(m.moved(DOG) > from_int(5), "{}", m.moved(DOG));
    assert!(m.moved(WOLF) > from_int(5), "{}", m.moved(WOLF));
}

#[test]
fn a_sheepdog_bot_runs_down_a_tall_wolf_in_the_open_and_lunges() {
    // 20 m at a sprint of 16 m/s is 1.25 s, and a decision comes every 0.4 s
    let mut m = dog_near_held_wolf(20);
    for _ in 0..240 {
        tick_with_wolf_holding(&mut m, 0);
    }
    assert!(m.world.bodies[WOLF].out, "dog at {}", m.world.bodies[DOG].x >> 16);
}

#[test]
fn a_crouched_wolf_keeping_pace_shows_nothing_at_any_distance() {
    let row = |crouched, sprinting, action| BodyRow {
        seat: 1,
        role: Role::Wolf,
        x: 0,
        y: 0,
        z: 0,
        yaw: 0,
        pitch: 0,
        crouched,
        sprinting,
        grounded: true,
        speed: 2 * fx::ONE,
        action,
        winded: false,
        outline: false,
        suited: false,
        cloaked: false,
        carrot: false,
        frenzy: false,
    };
    let hiding = row(true, false, Action::Moving);
    assert!(!gives_itself_away(&hiding, fx::ONE, false));
    // the same Wolf stood tall shows within 25 m, not past it
    let tall = row(false, false, Action::Moving);
    assert!(gives_itself_away(&tall, from_int(25), false));
    assert!(!gives_itself_away(&tall, from_int(26), false));
    // at a meal it shows within 40 m
    let eating = row(true, false, Action::Snapping);
    assert!(gives_itself_away(&eating, from_int(40), false));
    assert!(!gives_itself_away(&eating, from_int(41), false));
    // a sprint shows within 60 m
    assert!(gives_itself_away(
        &row(true, true, Action::Moving),
        from_int(60),
        false
    ));
    // at 2 m/s it keeps pace through a bark, and stood still it doesn't
    assert!(!gives_itself_away(&hiding, from_int(5), true));
    let still = BodyRow { speed: 0, ..hiding };
    assert!(gives_itself_away(&still, from_int(12), true));
    assert!(!gives_itself_away(&still, from_int(12), false));
}

#[test]
fn a_wolf_in_the_sheep_suit_passes_standing_and_running() {
    let row = |sprinting| BodyRow {
        seat: 1,
        role: Role::Wolf,
        x: 0,
        y: 0,
        z: 0,
        yaw: 0,
        pitch: 0,
        crouched: false,
        sprinting,
        grounded: true,
        speed: 2 * fx::ONE,
        action: Action::Moving,
        winded: false,
        outline: false,
        suited: true,
        cloaked: false,
        carrot: false,
        frenzy: false,
    };
    // unsuited, these show within 25 m and 60 m; suited, not even at 1 m
    assert!(!gives_itself_away(&row(false), fx::ONE, false));
    assert!(!gives_itself_away(&row(true), fx::ONE, false));
}

#[test]
fn a_sheepdog_bot_loses_a_wolf_that_cloaks_mid_chase() {
    // 20 m off and stood tall, the Wolf is spotted at the first decision
    let mut m = dog_near_held_wolf(20);
    for _ in 0..tune::BOT_THINK_TICKS * 2 {
        tick_with_wolf_holding(&mut m, 0);
    }
    assert!(m.players[DOG].brain.quarry_left > 0);
    let slot = &mut m.players[WOLF].slot;
    assert!(slot.take(Kind::InvisibilityCloak));
    assert!(slot.activate().is_some());
    // the cloak lasts 5 s; 4 s of it, stood still, and the Sheepdog never reaches it
    for _ in 0..240 {
        tick_with_wolf_holding(&mut m, 0);
    }
    assert!(!m.world.bodies[WOLF].out);
    assert_eq!(m.players[DOG].brain.quarry_left, 0);
}

#[test]
fn a_sheepdog_bot_walks_past_a_crouched_wolf_it_has_no_reason_to_doubt() {
    let mut m = dog_near_held_wolf(5);
    m.world.bodies[WOLF].crouched = true;
    for _ in 0..120 {
        tick_with_wolf_holding(&mut m, CROUCH);
    }
    assert!(!m.world.bodies[WOLF].out);
    assert_eq!(m.players[DOG].brain.quarry_left, 0);
}

/// A bot Wolf on open ground with a sheep a step away, its dial run out, and
/// a Sheepdog held by a person this far west of it.
fn hungry_wolf_with_dog_at(gap: i32) -> Match {
    let mut m = bots(&[Pick::Sheepdog, Pick::Wolf], 10);
    let (x, z) = (from_int(160), from_int(88));
    for s in &mut m.world.flock.sheep {
        (s.x, s.z) = (x + 2 * fx::ONE, z);
    }
    let wolf = &mut m.world.bodies[WOLF];
    (wolf.x, wolf.z) = (x, z);
    (m.world.bodies[DOG].x, m.world.bodies[DOG].z) = (x - from_int(gap), z);
    for _ in 0..tune::EAT_COOLDOWN_SECONDS * 60 {
        m.players[WOLF].eat.tick(false);
    }
    m
}

fn tick_with_held_sheepdog(m: &mut Match) {
    let mut keys = [None; SEATS];
    keys[DOG] = Some(Input::default());
    m.step(&keys);
}

#[test]
fn a_wolf_bot_eats_with_no_sheepdog_near_and_holds_off_with_one_close() {
    let mut alone = hungry_wolf_with_dog_at(50);
    let mut watched = hungry_wolf_with_dog_at(15);
    for _ in 0..120 {
        tick_with_held_sheepdog(&mut alone);
        tick_with_held_sheepdog(&mut watched);
    }
    assert_eq!(alone.standing().eats, 1);
    assert_eq!(watched.standing().eats, 0);
}

#[test]
fn a_wolf_bot_runs_from_a_sheepdog_on_top_of_it() {
    let mut m = hungry_wolf_with_dog_at(3);
    for _ in 0..30 {
        tick_with_held_sheepdog(&mut m);
    }
    assert!(m.world.bodies[WOLF].sprinting);
    assert!(m.world.bodies[WOLF].x > from_int(160), "away, to the east");
}

#[test]
fn a_bot_on_the_far_side_of_a_wall_heads_for_the_cheaper_gap() {
    // on seed 1 the first wall's gaps are near (108, 46) and (107, 122). From
    // (60, 88) to (160, 88) the upper one is 58 + 63 m and the lower 64 + 67
    let m = bots(&[Pick::Sheepdog, Pick::Wolf], 10);
    let stop = m.route((from_int(60), from_int(88)), (from_int(160), from_int(88)));
    assert!((stop.1 - from_int(122)).abs() < from_int(3), "{}", stop.1 >> 16);
    assert!(stop.0 < from_int(107), "stops short of it: {}", stop.0 >> 16);
}

#[test]
fn sheepdog_bots_get_sheep_into_the_fold() {
    // the 16 sheep of the field by the Fold are 40 to 110 m from it at a
    // trot of 4.8 m/s, so some are home well inside 240 s
    let mut m = bots(&[Pick::Sheepdog, Pick::Wolf], 30);
    for _ in 0..240 * 60 {
        tick_with_wolf_holding(&mut m, 0);
    }
    assert!(m.standing().in_fold >= 3, "{}", m.standing().in_fold);
}

#[test]
fn a_sheepdog_bot_looks_at_a_carcass_near_it() {
    let mut m = dog_near_held_wolf(50);
    let (x, z) = (from_int(125), from_int(88));
    m.carcasses.push(CarcassSpot {
        x,
        z,
        carcass: Carcass::default(),
    });
    for _ in 0..60 {
        tick_with_wolf_holding(&mut m, 0);
    }
    assert_eq!(m.standing().carcasses_found, 1);
}

#[test]
fn a_bot_digs_a_mound_near_it_and_spends_what_it_finds() {
    let mut m = dog_near_held_wolf(50);
    let spot = m.mounds[0];
    (m.world.bodies[DOG].x, m.world.bodies[DOG].z) = (spot.x + from_int(8), spot.z);
    // on the ground: dropped from up the hill it would overshoot in the air,
    // where a body brakes at a third of the rate
    m.world.bodies[DOG].settle(&*map());
    // 8 m at a sprint is 1 s, then a 2 s dig, then a decision
    for _ in 0..240 {
        tick_with_wolf_holding(&mut m, 0);
    }
    assert!(m.mounds.iter().any(|s| s.mound.claimed_by() == Some(DOG as u8)));
}

#[test]
fn bots_play_on_a_small_map_and_stay_inside_its_walls() {
    use crate::map::MapSize;
    let picks = [Pick::Sheepdog, Pick::Wolf, Pick::Sheepdog, Pick::Wolf];
    let seat = |(k, role): (usize, &Pick)| StartSeat {
        seat: k as u32,
        role: *role,
        person: String::new(),
        name: String::new(),
    };
    let start = MatchStart {
        seed: 4,
        seats: picks.iter().enumerate().map(seat).collect(),
        settings: Settings {
            sheep: 80,
            eats_required: 9,
            map_size: MapSize::Small,
            ..Settings::default()
        },
    };
    let map = Arc::new(Map::sized(4, MapSize::Small));
    let mut m = Match::new(&start, map.clone());
    let at = |m: &Match| m.world.bodies[WOLF].x;
    let first = at(&m);
    play(&mut m, 60 * 20);
    assert_ne!(at(&m), first, "the Wolf bot went nowhere");
    for b in &m.world.bodies {
        assert!(map.inside(b.x, b.z));
    }
    for s in &m.world.flock.sheep {
        assert!(map.inside(s.x, s.z));
    }
}

#[test]
fn a_wolf_bot_saves_a_frenzy_for_a_sheepdog_within_fifteen_meters() {
    let mut m = dog_near_held_wolf(20);
    m.players[WOLF].slot.take(Kind::Frenzy);
    // the Sheepdog held by a person, standing still
    let still = |m: &mut Match| {
        let mut keys = [None; SEATS];
        keys[DOG] = Some(Input::default());
        m.step(&keys);
    };
    // two decisions at 20 m: still held
    for _ in 0..50 {
        still(&mut m);
    }
    assert_eq!(m.players[WOLF].slot.held(), Some(Kind::Frenzy));
    let wolf = m.world.bodies[WOLF];
    m.world.bodies[DOG].x = wolf.x - from_int(10);
    m.world.bodies[DOG].z = wolf.z;
    for _ in 0..50 {
        still(&mut m);
    }
    assert!(m.players[WOLF].slot.running(Kind::Frenzy));
}
