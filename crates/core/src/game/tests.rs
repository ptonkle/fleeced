//! A match played directly: bodies and sheep stood where a test wants them,
//! then real inputs through `Match::step`. Every number is worked out by hand
//! from the tune tables at 60 ticks a second.

use std::collections::VecDeque;
use std::sync::OnceLock;

use super::*;
use crate::fx::{from_int, QUARTER_TURN};
use crate::lobby::{Role as Pick, StartSeat};
use crate::online::{Peer, Session, DELAY, PLAYING};
use crate::rules::mounds::MoundLook;
use crate::rules::outcome::Outcome;
use crate::rules::setup::Settings;
use crate::rules::tune::Powerup::{self as Kind};
use crate::rules::tune::POWERUPS;

mod plates;
mod powerup_effects;

const DOG: usize = 0;
const WOLF: usize = 1;

/// One map for every test: generating one takes a moment.
fn map() -> Arc<Map> {
    static MAP: OnceLock<Arc<Map>> = OnceLock::new();
    MAP.get_or_init(|| Arc::new(Map::generate(1))).clone()
}

fn small() -> Settings {
    Settings {
        sheep: 20,
        eats_required: 5,
        ..Settings::default()
    }
}

fn start(picks: &[Pick], settings: Settings) -> MatchStart {
    let seat = |(k, role): (usize, &Pick)| StartSeat {
        seat: k as u32,
        role: *role,
        person: format!("p{k}"),
        name: format!("p{k}"),
    };
    MatchStart {
        seed: 1,
        seats: picks.iter().enumerate().map(seat).collect(),
        settings,
    }
}

fn game(picks: &[Pick], settings: Settings) -> Match {
    Match::new(&start(picks, settings), map())
}

/// A Sheepdog in seat 0 and a Wolf in seat 1.
fn two() -> Match {
    game(&[Pick::Sheepdog, Pick::Wolf], small())
}

fn tick(m: &mut Match, inputs: &[Input]) {
    let mut all = [None; SEATS];
    for (slot, input) in all.iter_mut().zip(inputs) {
        *slot = Some(*input);
    }
    m.step(&all);
}

fn idle(m: &mut Match, ticks: usize) {
    for _ in 0..ticks {
        tick(m, &[Input::default(); 8][..m.players.len()]);
    }
}

/// Looking down at the ground 1.5 m ahead, where these tests stand things:
/// the crosshair's line starts 1.15 m up and a target's middle is 0.3 m up,
/// and the arctangent of 0.85 / 1.5 is 29.5 degrees.
const LOOK_NEAR: i16 = -5370;

fn holding(buttons: u16) -> Input {
    Input {
        buttons,
        pitch: LOOK_NEAR,
        ..Input::default()
    }
}

const WALK: Input = Input {
    strafe: 0,
    forward: 127,
    yaw: 0,
    pitch: 0,
    buttons: 0,
};

/// Open ground, by the map's first tunnel mouth.
fn open(m: &Match) -> (Fx, Fx) {
    m.map().wolf_starts[0]
}

/// Open ground far from anything a test does near `open`.
fn far_off(m: &Match) -> (Fx, Fx) {
    *m.map().wolf_starts.last().unwrap()
}

/// Stands a body at rest here. Yaw 0 looks along -z.
fn stand(m: &mut Match, k: usize, (x, z): (Fx, Fx)) {
    let map = m.map.clone();
    let body = &mut m.world.bodies[k];
    (body.x, body.z, body.vx, body.vz) = (x, z, 0, 0);
    body.settle(&*map);
}

fn put_sheep(m: &mut Match, i: usize, (x, z): (Fx, Fx)) {
    let sheep = &mut m.world.flock.sheep[i];
    (sheep.x, sheep.z, sheep.vx, sheep.vz) = (x, z, 0, 0);
}

/// Every sheep off in a pile, out of the test's way.
fn clear_sheep(m: &mut Match) {
    let spot = far_off(m);
    for i in 0..m.world.flock.sheep.len() {
        put_sheep(m, i, spot);
    }
}

fn dial_ready(m: &mut Match, k: usize) {
    for _ in 0..1200 {
        m.players[k].eat.tick(false);
    }
}

/// The Wolf eats sheep `i` where it stands, 1.5 m ahead.
fn wolf_eats(m: &mut Match, i: usize) {
    let (x, z) = open(m);
    stand(m, WOLF, (x, z));
    put_sheep(m, i, (x, z - from_int(3) / 2));
    dial_ready(m, WOLF);
    idle(m, 1);
    put_sheep(m, i, (x, z - from_int(3) / 2));
    tick(m, &[Input::default(), holding(PRIMARY)]);
}

fn snapshot(m: &Match, seat: usize) -> Snapshot {
    m.snapshot_for(seat).unwrap()
}

#[test]
fn input_keys_become_one_input() {
    let bits = KEY_FORWARD | KEY_LEFT | SPRINT as u32 | PRIMARY as u32;
    let input = input_from_keys(bits, (300, -20));
    let expected = Input {
        strafe: -127,
        forward: 127,
        yaw: 300,
        pitch: -20,
        buttons: SPRINT | PRIMARY,
    };
    assert_eq!(input, expected);
    // opposite keys cancel
    let still = input_from_keys(KEY_LEFT | KEY_RIGHT | KEY_FORWARD | KEY_BACK, (0, 0));
    assert_eq!((still.strafe, still.forward), (0, 0));
}

#[test]
fn sheepdogs_start_before_the_fold_and_wolves_at_mouths() {
    let m = game(&[Pick::Wolf, Pick::Sheepdog, Pick::Wolf, Pick::Sheepdog], small());
    assert_eq!(m.players.iter().map(|p| p.seat).collect::<Vec<_>>(), [0, 1, 2, 3]);
    let map = m.map();
    let at = |k: usize| (m.world.bodies[k].x, m.world.bodies[k].z);
    assert_eq!(at(0), map.wolf_starts[0]);
    assert_eq!(at(2), map.wolf_starts[1]);
    assert_eq!(m.world.bodies[1].role, Role::Sheepdog);
    assert_eq!(
        at(1),
        map.push_out(
            map.sheepdog_starts[0].0,
            map.sheepdog_starts[0].1,
            Role::Sheepdog.breed().radius
        )
    );
    // the map is about 260 m long and a Sheepdog starts within 40 m of the
    // Fold's gap, a Wolf at the far mouths
    let (gap_x, gap_z) = map.fold.gap.center;
    assert!(fx::length(at(1).0 - gap_x, at(1).1 - gap_z) < from_int(40));
    assert!(fx::length(at(0).0 - gap_x, at(0).1 - gap_z) > from_int(100));
}

#[test]
fn a_match_starts_with_two_mounds_a_player_and_none_with_powerups_off() {
    assert_eq!(two().mounds.len(), 4);
    let off = Settings {
        powerups: false,
        ..small()
    };
    assert_eq!(game(&[Pick::Sheepdog, Pick::Wolf], off).mounds.len(), 0);
}

#[test]
fn a_new_mound_comes_up_after_two_minutes_with_two_players() {
    let mut m = two();
    // 120 s at 60 ticks a second is 7200 ticks
    idle(&mut m, 7199);
    assert_eq!(m.mounds.len(), 4);
    idle(&mut m, 1);
    assert_eq!(m.mounds.len(), 5);
}

#[test]
fn a_wolf_eats_what_is_under_the_crosshair_once_the_dial_is_back() {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    let spot = (x, z - from_int(3) / 2);
    stand(&mut m, WOLF, (x, z));
    put_sheep(&mut m, 0, spot);
    tick(&mut m, &[Input::default(), holding(PRIMARY)]);
    assert_eq!(m.standing().eats, 0, "every Wolf's first eat waits out the dial");
    wolf_eats(&mut m, 0);
    assert_eq!(m.standing().eats, 1);
    assert!(m.world.flock.sheep[0].eaten);
    assert_eq!((m.carcasses[0].x, m.carcasses[0].z), spot);
    assert!(!m.players[WOLF].eat.ready());
}

#[test]
fn a_wolf_is_locked_in_place_for_a_second_by_an_eat() {
    let mut m = two();
    clear_sheep(&mut m);
    wolf_eats(&mut m, 0);
    let stood = m.world.bodies[WOLF].z;
    // the eat's own tick was the first of 60
    for _ in 0..59 {
        tick(&mut m, &[Input::default(), WALK]);
    }
    assert_eq!(m.world.bodies[WOLF].z, stood);
    tick(&mut m, &[Input::default(), WALK]);
    assert!(m.world.bodies[WOLF].z < stood);
}

#[test]
fn a_sheep_off_the_crosshair_or_past_reach_is_not_eaten() {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    stand(&mut m, WOLF, (x, z));
    dial_ready(&mut m, WOLF);
    // behind it, to its side, and 3.5 m ahead with a reach of 3 m
    let past_reach = (x, z - from_int(7) / 2);
    for spot in [(x, z + from_int(1)), (x + from_int(1), z), past_reach] {
        put_sheep(&mut m, 0, spot);
        tick(&mut m, &[Input::default(), holding(PRIMARY)]);
        idle(&mut m, 1);
    }
    assert_eq!(m.standing().eats, 0);
}

#[test]
fn eating_breaks_a_running_sheep_suit() {
    let mut m = two();
    clear_sheep(&mut m);
    m.players[WOLF].slot.take(Kind::SheepSuit);
    m.players[WOLF].slot.activate();
    wolf_eats(&mut m, 0);
    assert!(!m.players[WOLF].slot.running(Kind::SheepSuit));
}

#[test]
fn a_bark_takes_a_fresh_press_and_a_rest_between_barks() {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    let spot = (x, z - from_int(14));
    stand(&mut m, DOG, (x, z));
    let off = far_off(&m);
    stand(&mut m, WOLF, off);
    let e = holding(ACT);
    let bolted = |m: &mut Match, input: Input| {
        put_sheep(m, 0, spot);
        m.world.flock.sheep[0].bolt = 0;
        tick(m, &[input]);
        m.world.flock.sheep[0].bolt > 0
    };
    // 14 m is past the flight zone and inside the bark
    assert!(bolted(&mut m, e));
    // held down it does not bark again
    assert!(!bolted(&mut m, e));
    // 180 ticks of rest, one gone already and one above: this press is one
    // short of it, and the next one is not
    idle(&mut m, 176);
    assert!(!bolted(&mut m, e));
    idle(&mut m, 1);
    assert!(bolted(&mut m, e));
}

#[test]
fn e_held_through_the_rest_does_not_bark_again() {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    stand(&mut m, DOG, (x, z));
    let off = far_off(&m);
    stand(&mut m, WOLF, off);
    tick(&mut m, &[holding(ACT)]);
    // ticks 1 to 179 of the rest with E down, so the next is the first it could bark
    for _ in 0..179 {
        tick(&mut m, &[holding(ACT)]);
    }
    put_sheep(&mut m, 0, (x, z - from_int(14)));
    m.world.flock.sheep[0].bolt = 0;
    tick(&mut m, &[holding(ACT)]);
    assert_eq!(m.world.flock.sheep[0].bolt, 0);
}

#[test]
fn a_wolf_has_no_bark_and_a_stunned_sheepdog_cannot_bark() {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    let spot = (x, z - from_int(14));
    // with the mouth behind it, E has nothing to act on
    stand(&mut m, WOLF, (x, z - from_int(5)));
    put_sheep(&mut m, 0, (x, z - from_int(19)));
    tick(&mut m, &[Input::default(), holding(ACT)]);
    assert_eq!(m.world.flock.sheep[0].bolt, 0);
    idle(&mut m, 1);
    let off = far_off(&m);
    stand(&mut m, WOLF, off);
    stand(&mut m, DOG, (x, z));
    put_sheep(&mut m, 0, spot);
    m.players[DOG].stun.start(10);
    tick(&mut m, &[holding(ACT)]);
    assert_eq!(m.world.flock.sheep[0].bolt, 0);
}

/// A Sheepdog stood 2 m from mound 0, moved into the open.
fn at_a_mound() -> Match {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    stand(&mut m, DOG, (x, z));
    (m.mounds[0].x, m.mounds[0].z) = (x, z - from_int(2));
    m
}

#[test]
fn holding_e_on_a_mound_for_two_seconds_digs_it_and_holds_the_body_still() {
    let mut m = at_a_mound();
    let (x, z) = open(&m);
    let dig = Input {
        forward: 127,
        ..holding(ACT)
    };
    // 2 s is 120 ticks
    for _ in 0..119 {
        tick(&mut m, &[dig]);
    }
    assert_eq!(m.players[DOG].slot.held(), None);
    assert_eq!(m.mounds[0].mound.look(Role::Sheepdog), MoundLook::Fresh);
    tick(&mut m, &[dig]);
    let kind = m.players[DOG].slot.held().expect("the dig gives a powerup");
    assert_eq!(POWERUPS[kind as usize].role, Role::Sheepdog);
    assert_eq!(m.mounds[0].mound.look(Role::Wolf), MoundLook::DugUp);
    assert_eq!((m.world.bodies[DOG].x, m.world.bodies[DOG].z), (x, z));
}

#[test]
fn letting_go_of_e_starts_the_dig_over() {
    let mut m = at_a_mound();
    for _ in 0..119 {
        tick(&mut m, &[holding(ACT)]);
    }
    idle(&mut m, 1);
    for _ in 0..119 {
        tick(&mut m, &[holding(ACT)]);
    }
    assert_eq!(m.players[DOG].slot.held(), None);
}

#[test]
fn a_refused_dig_does_not_bark_and_a_dug_up_mound_lets_a_bark_through() {
    let mut m = at_a_mound();
    let (x, z) = open(&m);
    let spot = (x, z - from_int(14));
    m.players[DOG].slot.take(Kind::Zoomies);
    put_sheep(&mut m, 0, spot);
    tick(&mut m, &[holding(ACT)]);
    assert_eq!(m.world.flock.sheep[0].bolt, 0, "a full slot refuses the dig");
    assert_eq!(m.mounds[0].mound.claimed_by(), None);
    idle(&mut m, 1);
    m.mounds[0].mound.dig(WOLF as u8, Role::Wolf, true).unwrap();
    put_sheep(&mut m, 0, spot);
    tick(&mut m, &[holding(ACT)]);
    assert!(m.world.flock.sheep[0].bolt > 0);
}

#[test]
fn f_throws_away_the_powerup_held() {
    let mut m = two();
    m.players[DOG].slot.take(Kind::Zoomies);
    tick(&mut m, &[holding(DISCARD)]);
    assert!(m.players[DOG].slot.is_free());
}

/// A Sheepdog this many tenths of a meter behind the Wolf, ready to Lunge at it.
fn at_lunge_range(gap_tenths: i32) -> Match {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    stand(&mut m, DOG, (x, z));
    stand(&mut m, WOLF, (x, z - fx::ratio(gap_tenths, 10)));
    m
}

fn lunge(m: &mut Match) {
    tick(m, &[holding(PRIMARY)]);
    idle(m, tune::LUNGE_TICKS as usize - 1);
}

#[test]
fn a_lunge_kills_a_wolf_inside_its_reach() {
    let mut m = at_lunge_range(99);
    m.players[WOLF].slot.take(Kind::Bone);
    lunge(&mut m);
    assert!(m.world.bodies[WOLF].out);
    assert!(!m.players[WOLF].life.alive());
    assert_eq!(m.standing().wolves_alive, 0);
    assert!(m.players[WOLF].slot.is_free(), "a killed Wolf loses what it held");
    assert!(m.players[DOG].lunge.ready(), "a hit carries no wait");
    assert_eq!(m.outcome(), None, "with Sudden death off the match goes on");
}

#[test]
fn a_lunge_that_misses_scatters_sheep_and_waits_four_seconds() {
    let mut m = at_lunge_range(109);
    let (x, z) = open(&m);
    // the sheep scatter from where the Lunge ends, 9.2 m along -z
    put_sheep(&mut m, 0, (x + from_int(4), z - from_int(9)));
    tick(&mut m, &[holding(PRIMARY)]);
    idle(&mut m, tune::LUNGE_TICKS as usize - 2);
    m.world.flock.sheep[0].bolt = 0;
    assert_eq!(m.world.flock.sheep[0].bolt, 0);
    idle(&mut m, 1);
    assert!(!m.world.bodies[WOLF].out, "10.9 m is out of a 10.8 m reach");
    assert!(m.world.flock.sheep[0].bolt > 0);
    assert!(!m.players[DOG].lunge.ready());
    // 4 s is 240 ticks
    idle(&mut m, 239);
    assert!(!m.players[DOG].lunge.ready());
    idle(&mut m, 1);
    assert!(m.players[DOG].lunge.ready());
}

#[test]
fn a_lunge_carries_the_sheepdog_along_its_line() {
    let mut m = at_lunge_range(120);
    let (_, z) = open(&m);
    lunge(&mut m);
    // (10.8 - 1.6) m over 24 ticks, in whole Fx units: 24 * 25122 = 602928
    assert_eq!(m.world.bodies[DOG].z, z - 602_928);
}

#[test]
fn a_lunge_stops_at_every_wall_of_the_map_on_the_ground_and_from_a_jump() {
    // 0.38 m a tick against a wall 1 m thick. From 3 m up it still stays on
    // its own side: a Lunge clears nothing, the Fold's fence included.
    let walls = map().walls.clone();
    let mut m = two();
    clear_sheep(&mut m);
    let mut runs = 0;
    for wall in &walls {
        let (dx, dz) = (wall.b.0 - wall.a.0, wall.b.1 - wall.a.1);
        let long = fx::length(dx, dz);
        if long < from_int(8) {
            continue;
        }
        let (nx, nz) = (fx::div(-dz, long), fx::div(dx, long));
        let at = (wall.a.0 + dx / 2, wall.a.1 + dz / 2);
        for side in [fx::ONE, -fx::ONE] {
            let (ox, oz) = (fx::mul(nx, side), fx::mul(nz, side));
            let from = (at.0 + ox * 3, at.1 + oz * 3);
            let map = m.map.clone();
            if map.push_out(from.0, from.1, m.world.bodies[DOG].role.breed().radius) != from
                || !map.inside(from.0, from.1)
            {
                continue;
            }
            for up in [0, from_int(3)] {
                stand(&mut m, DOG, from);
                let body = &mut m.world.bodies[DOG];
                body.y += up;
                body.grounded = up == 0;
                m.players[DOG].lunge = Lunge::default();
                let aim = Input {
                    yaw: aim::yaw_toward(-ox, -oz),
                    ..holding(PRIMARY)
                };
                tick(&mut m, &[aim]);
                idle(&mut m, tune::LUNGE_TICKS as usize - 1);
                let b = m.world.bodies[DOG];
                let past = (b.x - at.0) as i64 * ox as i64 + (b.z - at.1) as i64 * oz as i64;
                assert!(past > 0, "{wall:?} from {up}");
                runs += 1;
            }
        }
    }
    assert!(runs > 8, "{runs}");
}

#[test]
fn a_sheepdog_in_a_lunge_shows_flat_out_along_its_line() {
    let mut m = at_lunge_range(120);
    tick(&mut m, &[holding(PRIMARY)]);
    // the player looks a quarter turn away while the Lunge carries on
    let aside = Input {
        yaw: 16384,
        ..Input::default()
    };
    tick(&mut m, &[aside]);
    let view = snapshot(&m, DOG);
    let row = &view.bodies[DOG];
    assert_eq!(row.action, Action::Lunging);
    assert_eq!(row.yaw, 0);
    // 25122 a tick, 60 ticks a second
    assert_eq!(row.speed, 1_507_320);
}

#[test]
fn a_killed_wolf_lies_where_it_died_for_four_seconds_seen_as_a_wolf_by_those_in_sight() {
    let mut m = at_lunge_range(24);
    let at = (m.world.bodies[WOLF].x, m.world.bodies[WOLF].z);
    m.players[WOLF].slot.take(Kind::SheepSuit);
    m.players[WOLF].slot.activate();
    tick(&mut m, &[holding(PRIMARY)]);
    let mut ticks = 1;
    while snapshot(&m, DOG).fallen.is_empty() {
        idle(&mut m, 1);
        ticks += 1;
        assert!(ticks < 24, "the Lunge should have hit");
    }
    // 2.4 m apart, touching at 1.6: 0.8 m at 0.383 m a tick is the 3rd tick
    assert_eq!(ticks, 3);
    let row = snapshot(&m, DOG).fallen[0].clone();
    assert_eq!((row.x, row.z, row.age), (at.0, at.1, 0));
    assert!(snapshot(&m, DOG).bodies.iter().all(|b| b.role != Role::Wolf));
    assert_eq!(snapshot(&m, WOLF).fallen.len(), 1, "it sees its own body");
    // 4 s is 240 ticks, and this is the first of them
    idle(&mut m, 239);
    assert_eq!(snapshot(&m, DOG).fallen[0].age, 239);
    idle(&mut m, 1);
    assert!(snapshot(&m, DOG).fallen.is_empty());
}

#[test]
fn a_body_past_a_sheepdog_s_sight_reaches_no_snapshot() {
    let mut m = at_lunge_range(24);
    let (x, z) = (m.world.bodies[WOLF].x, m.world.bodies[WOLF].z);
    m.fallen.push(Fallen::new(x, z, 0, 0));
    assert_eq!(snapshot(&m, DOG).fallen.len(), 1);
    // far past the 150 m a Sheepdog sees by day
    m.fallen[0].x = x + from_int(400);
    assert!(snapshot(&m, DOG).fallen.is_empty());
}

#[test]
fn only_the_killer_gets_the_banner_and_the_flash_and_the_other_sheepdog_a_line() {
    let mut m = game(&[Pick::Sheepdog, Pick::Wolf, Pick::Sheepdog], small());
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    stand(&mut m, DOG, (x, z));
    stand(&mut m, WOLF, (x, z - fx::ratio(24, 10)));
    tick(&mut m, &[holding(PRIMARY)]);
    idle(&mut m, 2);
    let (killer, other, wolf) = (snapshot(&m, DOG), snapshot(&m, 2), snapshot(&m, WOLF));
    assert!(killer.me.killed);
    assert_eq!(killer.me.kill_flash, fx::ONE);
    assert!(!other.me.killed);
    assert_eq!(other.me.kill_flash, 0);
    assert!(other.view.wolf_killed);
    assert!(!wolf.me.killed);
    assert!(!wolf.view.wolf_killed);
    // 2 s is 120 ticks, and this is the first of them
    idle(&mut m, 119);
    assert!(snapshot(&m, DOG).me.killed);
    idle(&mut m, 1);
    assert!(!snapshot(&m, DOG).me.killed);
    assert!(!snapshot(&m, 2).view.wolf_killed);
}

#[test]
fn a_body_is_part_of_the_checksum() {
    let m = at_lunge_range(24);
    let mut with = m.clone();
    with.fallen.push(Fallen::new(0, 0, 0, 0));
    assert_ne!(Rollback::sum(&m), Rollback::sum(&with));
}

#[test]
fn a_killed_wolf_returns_after_ten_seconds_away_from_the_sheepdogs_with_its_dial_stopped() {
    let mut m = at_lunge_range(107);
    lunge(&mut m);
    let dial = m.players[WOLF].eat.fraction();
    // 10 s is 600 ticks, the Lunge's last tick being the first of them
    idle(&mut m, 598);
    assert!(m.world.bodies[WOLF].out);
    assert_eq!(m.players[WOLF].eat.fraction(), dial);
    idle(&mut m, 1);
    let (wolf, dog) = (m.world.bodies[WOLF], m.world.bodies[DOG]);
    assert!(!wolf.out);
    assert!(m.players[WOLF].life.alive());
    assert!(fx::length(wolf.x - dog.x, wolf.z - dog.z) >= from_int(40));
    let near_mouth = |p: &(Fx, Fx)| fx::length(wolf.x - p.0, wolf.z - p.1) < from_int(2);
    assert!(m.map().mouths().iter().any(near_mouth));
    idle(&mut m, 1);
    assert!(m.players[WOLF].eat.fraction() > dial, "the dial runs again");
}

#[test]
fn e_on_a_mouth_takes_a_wolf_to_the_far_mouth_and_the_tunnel_then_waits() {
    let mut m = two();
    clear_sheep(&mut m);
    let mouths = m.map().mouths();
    let (x, z) = mouths[0];
    stand(&mut m, WOLF, (x, z + from_int(2)));
    tick(&mut m, &[Input::default(), holding(ACT)]);
    let body = m.world.bodies[WOLF];
    assert!(fx::length(body.x - mouths[1].0, body.z - mouths[1].1) < from_int(1));
    idle(&mut m, 1);
    // the far mouth takes it back only after 30 s, or 1800 ticks
    stand(&mut m, WOLF, (mouths[1].0, mouths[1].1 + from_int(2)));
    idle(&mut m, 1797);
    tick(&mut m, &[Input::default(), holding(ACT)]);
    let far = (mouths[1].0, mouths[1].1 + from_int(2));
    assert_eq!((m.world.bodies[WOLF].x, m.world.bodies[WOLF].z), far);
    idle(&mut m, 1);
    tick(&mut m, &[Input::default(), holding(ACT)]);
    let body = m.world.bodies[WOLF];
    assert!(fx::length(body.x - x, body.z - z) < from_int(1));
}

#[test]
fn a_sheepdog_at_a_mouth_barks_and_stays() {
    let mut m = two();
    clear_sheep(&mut m);
    let mouths = m.map().mouths();
    let spot = (mouths[0].0, mouths[0].1 + from_int(2));
    stand(&mut m, DOG, spot);
    tick(&mut m, &[holding(ACT)]);
    assert_eq!((m.world.bodies[DOG].x, m.world.bodies[DOG].z), spot);
    assert!(!m.players[DOG].bark.bark(), "E barked");
}

/// A point on each side of an inner wall, and the wall's middle.
fn across_a_wall(m: &Match) -> ((Fx, Fx), (Fx, Fx)) {
    let map = m.map();
    let inner = |w: &&crate::map::walls::Wall| w.kind == crate::map::walls::WallKind::Inner;
    let wall = map.walls.iter().find(inner).unwrap();
    let (mx, mz) = ((wall.a.0 + wall.b.0) / 2, (wall.a.1 + wall.b.1) / 2);
    ((mx - from_int(4), mz), (mx + from_int(4), mz))
}

#[test]
fn a_sheepdog_finds_a_carcass_with_the_crosshair_and_a_clear_line_of_sight() {
    let mut m = two();
    clear_sheep(&mut m);
    let (near, beyond) = across_a_wall(&m);
    m.carcasses.push(CarcassSpot {
        x: beyond.0,
        z: beyond.1,
        carcass: Carcass::default(),
    });
    stand(&mut m, DOG, near);
    let look = Input {
        yaw: QUARTER_TURN as u16,
        pitch: LOOK_NEAR,
        ..Input::default()
    };
    // looking at it, with the wall between
    tick(&mut m, &[look]);
    assert_eq!(m.standing().carcasses_found, 0);
    // looking away, with nothing between
    stand(&mut m, DOG, (beyond.0 - from_int(2), beyond.1));
    tick(&mut m, &[Input::default()]);
    assert_eq!(m.standing().carcasses_found, 0);
    // looking at it, with nothing between
    tick(&mut m, &[look]);
    assert_eq!(m.standing().carcasses_found, 1);
    tick(&mut m, &[look]);
    assert_eq!(m.standing().carcasses_found, 1, "once a carcass");
}

#[test]
fn eating_the_required_sheep_ends_the_match_for_the_wolves_and_stops_it() {
    let two_eats = Settings {
        eats_required: 2,
        ..small()
    };
    let mut m = game(&[Pick::Sheepdog, Pick::Wolf], two_eats);
    clear_sheep(&mut m);
    wolf_eats(&mut m, 0);
    assert_eq!(m.outcome(), None);
    idle(&mut m, 60);
    wolf_eats(&mut m, 1);
    assert_eq!(m.outcome(), Some(Outcome::WolvesAte));
    let ended = m.world.tick;
    idle(&mut m, 10);
    assert_eq!(m.world.tick, ended, "a finished match stands still");
}

#[test]
fn filling_the_fold_ends_the_match_for_the_sheepdogs() {
    // one Sheepdog wants 60, and 10 - 8 + 1 = 3 caps it
    let few = Settings {
        sheep: 10,
        eats_required: 8,
        ..Settings::default()
    };
    let mut m = game(&[Pick::Sheepdog, Pick::Wolf], few);
    assert_eq!(m.goals.fold_target, 3);
    let back = m.map().fold.back;
    let inside = m.map().fold.gap.center;
    for i in 0..10 {
        // on a line from the back toward the gap, a meter apart near the back
        let share = fx::ratio(i, 40);
        let x = back.0 + fx::mul(inside.0 - back.0, share);
        let z = back.1 + fx::mul(inside.1 - back.1, share) + from_int(i - 5);
        put_sheep(&mut m, i as usize, (x, z));
    }
    idle(&mut m, 60);
    assert_eq!(m.outcome(), Some(Outcome::FoldFilled));
    assert!(m.standing().in_fold >= 3);
}

#[test]
fn under_sudden_death_the_last_wolf_dying_ends_the_match() {
    let sudden = Settings {
        sudden_death: true,
        ..small()
    };
    let mut m = game(&[Pick::Sheepdog, Pick::Wolf, Pick::Wolf], sudden);
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    let gap = (x, z - fx::ratio(24, 10));
    stand(&mut m, DOG, (x, z));
    stand(&mut m, 1, gap);
    let off = far_off(&m);
    stand(&mut m, 2, off);
    lunge(&mut m);
    assert!(m.players[1].life.spectating());
    assert_eq!(m.outcome(), None, "one Wolf is left");
    stand(&mut m, DOG, (x, z));
    stand(&mut m, 2, gap);
    lunge(&mut m);
    assert_eq!(m.outcome(), Some(Outcome::LastWolfDead));
}

#[test]
fn the_clock_running_out_ends_the_match_for_the_sheepdogs() {
    let short = Settings {
        match_minutes: 1,
        sheep: 10,
        eats_required: 1,
        ..Settings::default()
    };
    let mut m = game(&[Pick::Sheepdog, Pick::Wolf], short);
    assert_eq!(m.seconds_left(), 60);
    // 1 min is 3600 ticks
    idle(&mut m, 3599);
    assert_eq!(m.seconds_left(), 1);
    assert_eq!(m.outcome(), None);
    idle(&mut m, 1);
    assert_eq!(m.outcome(), Some(Outcome::TimeRanOut));
    assert_eq!(m.outcome().unwrap().winner(), Role::Sheepdog);
}

#[test]
fn super_appetite_brings_the_dial_back_at_double_speed() {
    let mut m = two();
    m.players[WOLF].slot.take(Kind::SuperAppetite);
    tick(&mut m, &[Input::default(), holding(SECONDARY)]);
    // that tick and 299 more at double speed are 600 of 1200, a half
    idle(&mut m, 299);
    assert_eq!(m.players[WOLF].eat.fraction(), fx::HALF);
}

#[test]
fn a_howl_darkens_each_team_and_leaves_their_sight_alone() {
    let mut m = two();
    m.players[WOLF].slot.take(Kind::Howl);
    tick(&mut m, &[Input::default(), holding(SECONDARY)]);
    assert!(m.players[WOLF].slot.is_free());
    // the change takes 3 s, or 180 ticks, and that tick was the first
    idle(&mut m, 179);
    assert_eq!(snapshot(&m, DOG).view.sight_range, from_int(150));
    assert_eq!(snapshot(&m, WOLF).view.sight_range, from_int(150));
    assert_eq!(snapshot(&m, DOG).view.light, crate::fx::ratio(50, 100));
    assert_eq!(snapshot(&m, WOLF).view.light, crate::fx::ratio(80, 100));
}

#[test]
fn a_stampede_sends_the_sheep_within_fifteen_meters_bolting() {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    stand(&mut m, WOLF, (x, z));
    put_sheep(&mut m, 0, (x, z - from_int(14)));
    put_sheep(&mut m, 1, (x, z - from_int(16)));
    m.players[WOLF].slot.take(Kind::Stampede);
    tick(&mut m, &[Input::default(), holding(SECONDARY)]);
    assert!(m.world.flock.sheep[0].bolt > 0);
    assert_eq!(m.world.flock.sheep[1].bolt, 0);
}

/// Open ground with no tree or cairn within 12 m.
fn bare_ground(m: &Match) -> (Fx, Fx) {
    let map = m.map();
    let trees = map.trees.iter().map(|t| (t.x, t.z));
    let cairns = map.cairns.iter().map(|c| (c.x, c.z));
    let props: Vec<(Fx, Fx)> = trees.chain(cairns).collect();
    let bare = |p: &(Fx, Fx)| {
        props
            .iter()
            .all(|q| fx::length(p.0 - q.0, p.1 - q.1) > from_int(12))
    };
    map.mouths()
        .into_iter()
        .find(bare)
        .expect("a bare mouth on seed 1")
}

#[test]
fn a_bone_is_refused_away_from_a_tree_or_cairn() {
    let mut m = two();
    let spot = bare_ground(&m);
    stand(&mut m, WOLF, spot);
    m.players[WOLF].slot.take(Kind::Bone);
    tick(&mut m, &[Input::default(), holding(SECONDARY)]);
    assert_eq!(m.players[WOLF].slot.held(), Some(Kind::Bone));
    assert_eq!(m.mounds.len(), 4);
}

/// A Wolf plants a Bone 3 m along +x from the first tree.
fn with_a_decoy() -> Match {
    let mut m = two();
    let tree = m.map().trees[0];
    stand(&mut m, WOLF, (tree.x + tree.trunk + from_int(3), tree.z));
    m.players[WOLF].slot.take(Kind::Bone);
    plant(&mut m, 180);
    m
}

/// The Wolf holds right click this many ticks.
fn plant(m: &mut Match, ticks: usize) {
    for _ in 0..ticks {
        tick(m, &[Input::default(), holding(SECONDARY)]);
    }
}

/// A Wolf with a Bone beside the first tree, with no one near.
fn wolf_at_a_tree() -> Match {
    let mut m = two();
    clear_sheep(&mut m);
    let tree = m.map().trees[0];
    stand(&mut m, WOLF, (tree.x + tree.trunk + from_int(3), tree.z));
    m.players[WOLF].slot.take(Kind::Bone);
    m
}

#[test]
fn a_bone_takes_three_seconds_of_holding_with_the_bar_and_the_wolf_held_still() {
    let mut m = wolf_at_a_tree();
    let (x, z) = (m.world.bodies[WOLF].x, m.world.bodies[WOLF].z);
    // 3 s is 180 ticks
    plant(&mut m, 90);
    assert_eq!(snapshot(&m, WOLF).me.dig, fx::ONE / 2);
    let row = snapshot(&m, WOLF);
    let wolf = row.bodies.iter().find(|b| b.role == Role::Wolf).unwrap();
    assert_eq!(wolf.action, Action::Digging);
    plant(&mut m, 89);
    assert_eq!(m.mounds.len(), 4);
    assert_eq!(m.players[WOLF].slot.held(), Some(Kind::Bone));
    plant(&mut m, 1);
    assert_eq!(m.mounds.len(), 5);
    assert!(m.players[WOLF].slot.is_free());
    assert_eq!(snapshot(&m, WOLF).me.dig, 0);
    assert_eq!((m.world.bodies[WOLF].x, m.world.bodies[WOLF].z), (x, z));
}

#[test]
fn letting_go_of_a_bone_starts_it_over_and_keeps_it() {
    let mut m = wolf_at_a_tree();
    plant(&mut m, 179);
    tick(&mut m, &[Input::default(), Input::default()]);
    assert_eq!(snapshot(&m, WOLF).me.dig, 0);
    assert_eq!(m.players[WOLF].slot.held(), Some(Kind::Bone));
    plant(&mut m, 179);
    assert_eq!(m.mounds.len(), 4);
}

#[test]
fn a_move_key_breaks_off_a_bone_and_keeps_it() {
    let mut m = wolf_at_a_tree();
    plant(&mut m, 179);
    let step = Input {
        forward: 127,
        ..holding(SECONDARY)
    };
    let was = m.world.bodies[WOLF].z;
    tick(&mut m, &[Input::default(), step]);
    assert_eq!(snapshot(&m, WOLF).me.dig, 0);
    assert_ne!(m.world.bodies[WOLF].z, was, "the Wolf walks off at once");
    assert_eq!(m.players[WOLF].slot.held(), Some(Kind::Bone));
    assert_eq!(m.mounds.len(), 4);
}

#[test]
fn a_wolf_planting_a_bone_is_held_as_at_a_dig() {
    let mut m = wolf_at_a_tree();
    plant(&mut m, 5);
    tick(&mut m, &[Input::default(), holding(SECONDARY | JUMP)]);
    assert!(m.world.bodies[WOLF].grounded, "no jump");
    assert!(snapshot(&m, WOLF).me.dig > 0);
}

#[test]
fn e_at_a_mouth_goes_through_only_once_a_bone_is_let_go() {
    let mut m = wolf_at_a_tree();
    let mouth = m.map().mouths()[0];
    // the first tree moved beside the mouth, so both are in reach
    let mut moved = (*m.map).clone();
    moved.trees[0].x = mouth.0 + from_int(3);
    moved.trees[0].z = mouth.1;
    m.map = Arc::new(moved);
    stand(&mut m, WOLF, (mouth.0, mouth.1 + from_int(3) / 2));
    let at_it = |buttons| Input {
        pitch: LOOK_NEAR,
        ..holding(buttons)
    };
    let was = m.world.bodies[WOLF].z;
    tick(&mut m, &[Input::default(), at_it(SECONDARY | ACT)]);
    assert!(snapshot(&m, WOLF).me.dig > 0, "planting");
    assert_eq!(m.world.bodies[WOLF].z, was);
    tick(&mut m, &[Input::default(), Input::default()]);
    tick(&mut m, &[Input::default(), at_it(ACT)]);
    assert_ne!(
        m.world.bodies[WOLF].z, was,
        "the same E goes through with no Bone going in"
    );
}

#[test]
fn a_bot_wolf_with_a_bone_by_a_tree_plants_it() {
    let mut m = wolf_at_a_tree();
    let off = far_off(&m);
    stand(&mut m, DOG, off);
    // a turn to decide, within 24 ticks, then 3 s
    for _ in 0..210 {
        let mut all = [None; SEATS];
        all[DOG] = Some(Input::default());
        m.step(&all);
        if m.mounds.len() > 4 {
            break;
        }
    }
    assert_eq!(m.mounds.len(), 5);
    assert!(m.players[WOLF].slot.is_free());
}

#[test]
fn a_sheepdog_holding_a_bell_sees_the_sheep_it_would_go_on_and_a_wolf_sees_nothing() {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    stand(&mut m, DOG, (x, z));
    let off = far_off(&m);
    stand(&mut m, WOLF, off);
    put_sheep(&mut m, 3, (x, z - from_int(4)));
    let further = Input {
        pitch: -2800,
        ..Input::default()
    };
    tick(&mut m, &[further]);
    assert_eq!(snapshot(&m, DOG).me.belling, None, "no Bell held");
    m.players[DOG].slot.take(Kind::Bell);
    tick(&mut m, &[further]);
    assert_eq!(snapshot(&m, DOG).me.belling, Some(3));
    assert_eq!(snapshot(&m, WOLF).me.belling, None);
    m.bells.push(Bell::hang(3));
    assert_eq!(snapshot(&m, DOG).me.belling, None, "it wears one");
}

#[test]
fn a_bone_plants_a_decoy_at_the_base_of_a_tree_that_only_wolves_can_tell() {
    let m = with_a_decoy();
    assert!(m.players[WOLF].slot.is_free());
    assert_eq!(m.mounds.len(), 5);
    let tree = m.map().trees[0];
    let decoy = m.mounds[4];
    assert_eq!(decoy.z, tree.z);
    assert!((decoy.x - (tree.x + tree.trunk + tune::BONE_BASE_GAP)).abs() < 16);
    assert_eq!(snapshot(&m, DOG).view.mounds[4], MoundLook::Fresh);
    assert_eq!(snapshot(&m, WOLF).view.mounds[4], MoundLook::Decoy);
}

#[test]
fn a_sheepdog_who_digs_a_decoy_does_nothing_else_for_ten_seconds() {
    let mut m = with_a_decoy();
    clear_sheep(&mut m);
    let decoy = m.mounds[4];
    stand(&mut m, DOG, (decoy.x, decoy.z + from_int(2)));
    let off = far_off(&m);
    stand(&mut m, WOLF, off);
    for _ in 0..120 {
        tick(&mut m, &[holding(ACT)]);
    }
    assert!(snapshot(&m, DOG).me.stunned);
    assert!(m.players[DOG].slot.is_free(), "a decoy gives no powerup");
    let at = m.world.bodies[DOG].z;
    // 10 s is 600 ticks, the dig's last tick being the first
    for _ in 0..599 {
        tick(&mut m, &[WALK]);
    }
    assert_eq!(m.world.bodies[DOG].z, at);
    assert!(!snapshot(&m, DOG).me.stunned);
    tick(&mut m, &[WALK]);
    assert!(m.world.bodies[DOG].z < at);
}

#[test]
fn a_bell_needs_a_sheep_under_the_crosshair() {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    stand(&mut m, DOG, (x, z));
    let off = far_off(&m);
    stand(&mut m, WOLF, off);
    m.players[DOG].slot.take(Kind::Bell);
    tick(&mut m, &[holding(SECONDARY)]);
    assert_eq!(m.players[DOG].slot.held(), Some(Kind::Bell));
    idle(&mut m, 1);
    put_sheep(&mut m, 3, (x, z - from_int(4)));
    // 1.1 m down over 4 m ahead
    let further = Input {
        pitch: -2800,
        ..holding(SECONDARY)
    };
    tick(&mut m, &[further]);
    assert!(m.players[DOG].slot.is_free());
    assert_eq!(snapshot(&m, DOG).view.worn_bells, [3]);
    assert!(snapshot(&m, WOLF).view.worn_bells.is_empty());
}

/// A Bell on sheep 0, with the Wolf far off.
fn belled() -> Match {
    let mut m = two();
    clear_sheep(&mut m);
    let spot = open(&m);
    let off = far_off(&m);
    stand(&mut m, WOLF, off);
    put_sheep(&mut m, 0, spot);
    m.bells.push(Bell::hang(0));
    m
}

#[test]
fn a_bell_rings_for_everyone_when_a_wolf_comes_near_and_finishes_if_its_sheep_is_eaten() {
    let mut m = belled();
    idle(&mut m, 1);
    assert!(snapshot(&m, WOLF).view.ringing_bells.is_empty());
    let (x, z) = open(&m);
    stand(&mut m, WOLF, (x, z + from_int(19)));
    idle(&mut m, 1);
    assert_eq!(snapshot(&m, DOG).view.ringing_bells, [0]);
    assert_eq!(snapshot(&m, WOLF).view.ringing_bells, [0]);
    m.world.flock.eat(0);
    // 10 s is 600 ticks, the tick that set it off being the first
    idle(&mut m, 599);
    assert_eq!(snapshot(&m, DOG).view.ringing_bells, [0], "the ring finishes");
    idle(&mut m, 1);
    assert!(snapshot(&m, DOG).view.ringing_bells.is_empty());
    assert!(m.bells.is_empty());
}

#[test]
fn a_silent_bell_comes_off_with_its_sheep() {
    let mut m = belled();
    m.world.flock.eat(0);
    idle(&mut m, 1);
    assert!(m.bells.is_empty());
}

#[test]
fn sniff_snoff_shows_a_sheepdog_a_scent_near_each_wolf_and_redraws_it_every_three_seconds() {
    let mut m = two();
    m.players[DOG].slot.take(Kind::SniffSnoff);
    tick(&mut m, &[holding(SECONDARY)]);
    let offset = m.players[WOLF].scent;
    assert_eq!(snapshot(&m, DOG).scents.len(), 1);
    assert!(snapshot(&m, WOLF).scents.is_empty());
    let wolf = m.world.bodies[WOLF];
    assert_eq!(
        snapshot(&m, DOG).scents[0],
        [wolf.x + offset.0, wolf.z + offset.1]
    );
    let drift = fx::length(offset.0, offset.1);
    assert!(drift > fx::ratio(29, 10) && drift < fx::ratio(101, 10), "{drift}");
    // the clock's 3 s are 180 ticks and that was the first
    idle(&mut m, 178);
    assert_eq!(m.players[WOLF].scent, offset);
    idle(&mut m, 1);
    assert_ne!(m.players[WOLF].scent, offset);
}

#[test]
fn a_sheepdogs_page_is_never_handed_the_sheep_alive() {
    let mut m = two();
    clear_sheep(&mut m);
    wolf_eats(&mut m, 0);
    let json = serde_json::to_string(&snapshot(&m, DOG)).unwrap();
    assert!(!json.contains("alive\":1"), "only `me` says alive");
    assert_eq!(snapshot(&m, WOLF).view.eats, 1);
}

#[test]
fn a_sheep_eaten_out_of_sight_changes_nothing_on_a_sheepdogs_page() {
    let mut m = two();
    clear_sheep(&mut m);
    // the far end of the map from a Sheepdog at the Fold is past 150 m of
    // sight, so what's left there isn't on its page
    let (back, low) = (m.map().fold.back, open(&m));
    stand(&mut m, DOG, back);
    stand(&mut m, WOLF, low);
    assert!(fx::length(back.0 - low.0, back.1 - low.1) > from_int(150));
    put_sheep(&mut m, 0, low);
    let before = snapshot(&m, DOG);
    let mut eaten = m.clone();
    eaten.world.flock.eat(0);
    eaten.eats = 1;
    eaten.carcasses.push(CarcassSpot {
        x: low.0,
        z: low.1,
        carcass: Carcass::default(),
    });
    // the eats are public; nothing else on the page moves
    let mut after = snapshot(&eaten, DOG);
    assert_eq!(after.view.eats, before.view.eats + 1);
    after.view.eats = before.view.eats;
    assert_eq!(after, before);
    assert_ne!(snapshot(&eaten, WOLF), snapshot(&m, WOLF));
}

#[test]
fn a_wolf_past_a_sheepdogs_sight_is_off_its_page_and_a_sheepdog_is_on_every_wolfs() {
    let mut m = two();
    let (back, low) = (m.map().fold.back, open(&m));
    stand(&mut m, DOG, back);
    stand(&mut m, WOLF, low);
    let seats = |s: Snapshot| s.bodies.iter().map(|b| b.seat).collect::<Vec<_>>();
    assert_eq!(seats(snapshot(&m, DOG)), [0]);
    assert_eq!(seats(snapshot(&m, WOLF)), [0, 1]);
}

#[test]
fn a_wolf_is_shown_the_sheep_it_would_eat_and_a_sheepdog_gets_no_such_target() {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    stand(&mut m, WOLF, (x, z));
    put_sheep(&mut m, 2, (x, z - from_int(2)));
    assert_eq!(snapshot(&m, WOLF).me.prey, None, "looking level, over its back");
    m.world.bodies[WOLF].pitch = LOOK_NEAR;
    assert_eq!(snapshot(&m, WOLF).me.prey, Some(2));
    stand(&mut m, DOG, (x, z));
    m.world.bodies[DOG].pitch = LOOK_NEAR;
    assert_eq!(snapshot(&m, DOG).me.prey, None);
}

#[test]
fn a_wolf_pressing_on_the_fold_is_told_so() {
    let mut m = two();
    let (gap_x, gap_z) = m.map().fold.gap.center;
    let spot = (gap_x - from_int(1), gap_z);
    stand(&mut m, WOLF, spot);
    stand(&mut m, DOG, spot);
    m.world.bodies[WOLF].yaw = QUARTER_TURN as u16;
    m.world.bodies[DOG].yaw = QUARTER_TURN as u16;
    assert!(snapshot(&m, WOLF).me.at_fold);
    assert!(!snapshot(&m, DOG).me.at_fold);
    m.world.bodies[WOLF].yaw = (3 * QUARTER_TURN) as u16;
    assert!(!snapshot(&m, WOLF).me.at_fold);
}

#[test]
fn a_seat_with_no_one_in_it_has_no_page() {
    assert_eq!(two().snapshot_for(5), None);
}

/// Each seat's input for a tick: moving, looking, and every button in turn,
/// so a replay has something to get wrong.
fn script(seat: usize, tick: u32) -> u64 {
    if tick < DELAY {
        return 0;
    }
    let t = tick * (seat as u32 + 3);
    let tap = |every: u32, offset: u32, button: u16| if t % every == offset { button } else { 0 };
    Input {
        strafe: if t % 7 < 3 { 127 } else { -60 },
        forward: if t % 5 < 4 { 127 } else { 0 },
        yaw: tick.wrapping_mul(311).wrapping_add(seat as u32 * 9000) as u16,
        pitch: 0,
        buttons: tap(40, 3, ACT) | tap(50, 7, PRIMARY) | tap(90, 11, SECONDARY) | tap(30, 2, SPRINT),
    }
    .pack()
}

fn keys_at(seats: usize, tick: u32) -> [u64; SEATS] {
    let mut keys = [Match::NOBODY; SEATS];
    for (seat, k) in keys.iter_mut().enumerate().take(seats) {
        *k = script(seat, tick);
    }
    keys
}

const THREE: [Pick; 3] = [Pick::Sheepdog, Pick::Wolf, Pick::Wolf];

fn truth(ticks: u32) -> Match {
    let mut m = game(&THREE, small());
    for t in 0..ticks {
        Rollback::step(&mut m, keys_at(3, t));
    }
    m
}

#[test]
fn an_input_survives_the_wire_and_idle_is_zero() {
    let input = Input {
        strafe: -1,
        forward: 2,
        yaw: 3,
        pitch: -4,
        buttons: 5,
    };
    // byte by byte: 0xff, 0x02, 3 as 16 bits, -4 as 16 bits, 5 as 16 bits
    assert_eq!(input.pack(), 0x0005_fffc_0003_02ff);
    assert_eq!(Input::unpack(input.pack()), input);
    assert_eq!(Input::default().pack(), 0);
    assert_ne!(Input::unpack(Match::NOBODY).pack(), 0);
}

#[test]
fn the_same_start_and_inputs_give_the_same_match_and_other_inputs_do_not() {
    let (a, b) = (truth(240), truth(240));
    assert_eq!(a, b);
    assert_eq!(a.sum(), b.sum());
    let mut other = game(&THREE, small());
    for t in 0..240 {
        let mut keys = keys_at(3, t);
        keys[0] = 0;
        Rollback::step(&mut other, keys);
    }
    assert_ne!(other.sum(), a.sum());
}

#[test]
fn a_checksum_sees_a_rule_only_change() {
    // nothing here moves a body or a sheep
    let m = two();
    let mut changed = m.clone();
    changed.players[WOLF].eat.tick(false);
    assert_ne!(changed.sum(), m.sum());
    let mut changed = m.clone();
    changed.mounds[0].mound.dig(0, Role::Wolf, true).unwrap();
    assert_ne!(changed.sum(), m.sum());
}

#[test]
fn a_twelve_tick_rollback_replay_lands_on_the_same_checksum() {
    let mut live = truth(60);
    let saved = live.clone();
    for t in 60..72 {
        Rollback::step(&mut live, keys_at(3, t));
    }
    let mut replay = saved.clone();
    for t in 60..72 {
        Rollback::step(&mut replay, keys_at(3, t));
    }
    assert_eq!(replay.sum(), live.sum());
    assert_eq!(replay, live);
}

/// Three browsers on a star with `lag` frames on each leg. Index 0 hosts.
struct Star {
    browsers: Vec<Session<Match>>,
    games: Vec<Match>,
    /// (arrives at frame, from peer, to browser, text)
    wire: VecDeque<(usize, Peer, usize, String)>,
    lag: usize,
    frame: usize,
}

impl Star {
    fn new(lag: usize) -> Star {
        const HOST: Peer = 100;
        let mut active = [false; SEATS];
        active[..3].fill(true);
        let mut browsers = vec![Session::new(true, "b")];
        for j in 1..3 {
            browsers.push(Session::new(false, "b"));
            browsers[0].connected(j as Peer);
            browsers[j].connected(HOST);
            for (_, text) in browsers[0].take_outbox() {
                browsers[j].message(HOST, &text);
            }
            for (_, text) in browsers[j].take_outbox() {
                browsers[0].message(j as Peer, &text);
            }
            browsers[j].begin(j, active, &[(HOST, 0)]);
        }
        browsers[0].begin(0, active, &[(1, 1), (2, 2)]);
        Star {
            browsers,
            games: vec![game(&THREE, small()); 3],
            wire: VecDeque::new(),
            lag,
            frame: 0,
        }
    }

    fn post(&mut self, k: usize) {
        for (to, text) in self.browsers[k].take_outbox() {
            let arrive = self.frame + self.lag;
            let (from, to) = if k == 0 {
                (100, to as usize)
            } else {
                (k as Peer, 0)
            };
            self.wire.push_back((arrive, from, to, text));
        }
    }

    fn deliver(&mut self) {
        let frame = self.frame;
        let (due, later): (VecDeque<_>, VecDeque<_>) = self.wire.drain(..).partition(|m| m.0 <= frame);
        self.wire = later;
        for (_, from, to, text) in due {
            if to < self.browsers.len() {
                self.browsers[to].message(from, &text);
            }
        }
    }

    fn play(&mut self, ticks: u32) {
        while self.browsers.iter().any(|s| s.tick < ticks) {
            for k in 0..self.browsers.len() {
                let keys = script(k, self.browsers[k].tick + DELAY);
                self.browsers[k].local(keys);
                self.post(k);
            }
            self.deliver();
            for k in 0..self.browsers.len() {
                if self.browsers[k].tick < ticks {
                    let (s, g) = (&mut self.browsers[k], &mut self.games[k]);
                    s.step(g);
                }
            }
            self.frame += 1;
            assert!(self.frame < 5_000, "the copies stalled");
        }
        self.drain();
    }

    fn drain(&mut self) {
        loop {
            for k in 0..self.browsers.len() {
                self.post(k);
            }
            if self.wire.is_empty() {
                break;
            }
            self.frame += 1;
            self.deliver();
        }
        for k in 0..self.browsers.len() {
            let (s, g) = (&mut self.browsers[k], &mut self.games[k]);
            s.settle(g);
        }
    }
}

#[test]
fn copies_over_a_lagged_wire_end_on_the_checksum_of_a_copy_fed_on_time() {
    let ticks = 180;
    let sum = truth(ticks).sum();
    for lag in [0, 3, 12] {
        let mut star = Star::new(lag);
        star.play(ticks);
        for k in 0..3 {
            assert_eq!(star.browsers[k].tick, ticks, "lag {lag}: browser {k}");
            assert_eq!(star.games[k].sum(), sum, "lag {lag}: browser {k}");
            assert_eq!(star.browsers[k].status, PLAYING, "lag {lag}: browser {k}");
        }
        if lag == 12 {
            assert!(star.browsers[0].rollbacks > 0, "a lagged wire rolls back");
        }
    }
}

#[test]
fn a_seat_left_mid_match_falls_to_the_bot_seam_at_the_tick_every_copy_agrees_on() {
    let mut star = Star::new(2);
    star.play(60);
    assert!(!star.games[0].players[2].bot);
    star.browsers.pop();
    star.games.pop();
    star.wire.retain(|m| m.2 != 2);
    star.browsers[0].left(2);
    star.drain();
    star.play(150);
    let (host, joiner) = (&star.games[0], &star.games[1]);
    assert!(host.players[2].bot);
    assert!(joiner.players[2].bot);
    assert_eq!(host.sum(), joiner.sum());
    // the bot in that seat plays on, the same on both
    star.play(300);
    assert_eq!(star.games[0].sum(), star.games[1].sum());
    assert_eq!(star.browsers[0].status, PLAYING);
}

#[test]
fn an_eat_is_heard_where_it_happened_and_by_who() {
    let mut m = two();
    clear_sheep(&mut m);
    wolf_eats(&mut m, 0);
    let (x, z) = open(&m);
    let ate = m.heard().iter().find(|h| h.name == "wolf_eat").unwrap();
    assert_eq!((ate.who, ate.x, ate.z), (WOLF as u16, x, z));
}

#[test]
fn a_noise_list_stays_short_however_long_the_match() {
    let mut m = two();
    for _ in 0..(HEARD_KEPT * 2) {
        m.noise("dig", DOG);
    }
    assert_eq!(m.heard().len(), HEARD_KEPT);
}

#[test]
fn a_match_starts_looking_at_the_middle_of_the_map() {
    let m = two();
    let (yaw, pitch) = m.first_look(DOG);
    assert_eq!(pitch, aim::REST_PITCH);
    let body = &m.world.bodies[DOG];
    let (mx, mz) = (crate::map::table::LENGTH / 2, crate::map::table::WIDTH / 2);
    let way = aim::look_way(yaw, 0);
    let to = v3(mx - body.x, 0, mz - body.z).norm();
    assert!(way.dot(to) > fx::ONE * 99 / 100, "{}", way.dot(to));
}
