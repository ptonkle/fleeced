//! One small test per powerup through `Match`: the six of the second half of
//! the table at the top of `game.rs`, and the Bell's sound.

use super::*;
use crate::render::geom::atan2;
use crate::tune::{GRAVITY, REFILL_TICKS, SHEEPDOG, STAMINA_FULL};

/// A Sheepdog in seat 0 and two Wolves.
fn pack() -> Match {
    let mut m = game(&[Pick::Sheepdog, Pick::Wolf, Pick::Wolf], small());
    side_by_side(&mut m);
    m
}

/// Everyone in sight of everyone, by the first mouth.
fn side_by_side(m: &mut Match) {
    let (x, z) = open(m);
    for k in 0..m.players.len() {
        stand(m, k, (x + from_int(2) * k as Fx, z));
    }
}

/// This powerup, used by this player.
fn use_it(m: &mut Match, k: usize, kind: Kind) {
    m.players[k].slot.take(kind);
    let mut inputs = [Input::default(); 8];
    inputs[k] = holding(SECONDARY);
    tick(m, &inputs[..m.players.len()]);
}

fn sees(m: &Match, viewer: usize, seat: u8) -> bool {
    snapshot(m, viewer).bodies.iter().any(|b| b.seat == seat)
}

#[test]
fn a_cloaked_wolf_leaves_a_sheepdogs_page_and_stays_on_its_teams() {
    let mut m = pack();
    let wolf = WOLF as u8;
    assert!(sees(&m, DOG, wolf));
    use_it(&mut m, WOLF, Kind::InvisibilityCloak);
    assert!(!sees(&m, DOG, wolf));
    assert!(sees(&m, 2, wolf), "the other Wolf");
    assert!(sees(&m, WOLF, wolf), "the Wolf itself");
    // 5 s is 300 ticks, the tick of use being the first
    let quiet = [Input::default(); 3];
    for _ in 0..298 {
        tick(&mut m, &quiet);
    }
    assert!(!sees(&m, DOG, wolf));
    tick(&mut m, &quiet);
    assert!(sees(&m, DOG, wolf));
}

#[test]
fn eating_brings_a_cloaked_wolf_back_at_once() {
    let mut m = two();
    clear_sheep(&mut m);
    side_by_side(&mut m);
    m.players[WOLF].slot.take(Kind::InvisibilityCloak);
    m.players[WOLF].slot.activate();
    assert!(!sees(&m, DOG, WOLF as u8));
    wolf_eats(&mut m, 0);
    assert!(sees(&m, DOG, WOLF as u8));
}

#[test]
fn a_sheep_suit_shows_to_everyone_until_the_wolf_eats() {
    let mut m = two();
    clear_sheep(&mut m);
    side_by_side(&mut m);
    let suited = |m: &Match, viewer: usize| {
        let s = snapshot(m, viewer);
        s.bodies.iter().find(|b| b.seat == WOLF as u8).unwrap().suited
    };
    assert!(!suited(&m, DOG));
    use_it(&mut m, WOLF, Kind::SheepSuit);
    assert!(suited(&m, DOG));
    assert!(suited(&m, WOLF));
    wolf_eats(&mut m, 0);
    assert!(!suited(&m, DOG));
}

#[test]
fn a_carrot_draws_sheep_and_stops_scaring_them_only_while_it_is_held_out() {
    let mut m = two();
    clear_sheep(&mut m);
    let (x, z) = open(&m);
    stand(&mut m, DOG, (x, z));
    let off = far_off(&m);
    stand(&mut m, WOLF, off);
    let spot = (x, z - from_int(10));
    put_sheep(&mut m, 0, spot);
    m.players[DOG].slot.take(Kind::CarrotOnAStick);
    let out = Input {
        buttons: SECONDARY,
        ..Input::default()
    };
    for _ in 0..120 {
        tick(&mut m, &[out, Input::default()]);
    }
    let sheep = m.world.flock.sheep[0];
    assert_eq!(sheep.alarm, 0, "calm");
    // a trot of 2.4 m/s for 2 s comes more than 2 m closer
    assert!(fx::length(sheep.x - x, sheep.z - z) < from_int(8));
    assert!(m.world.bodies[DOG].calm);
    idle(&mut m, 1);
    assert!(!m.world.bodies[DOG].calm, "put away");
}

#[test]
fn a_super_jump_goes_about_five_sheep_high_once() {
    let mut m = two();
    clear_sheep(&mut m);
    let spot = bare_ground(&m);
    stand(&mut m, DOG, spot);
    let floor = m.world.bodies[DOG].y;
    use_it(&mut m, DOG, Kind::SuperJump);
    assert!(m.players[DOG].slot.is_free());
    let mut top = 0;
    for _ in 0..90 {
        idle(&mut m, 1);
        top = top.max(m.world.bodies[DOG].y - floor);
    }
    // five ewes are 3.1 m, and a step at a time takes off about v / 120 = 0.09
    assert!(top > from_int(3) && top < fx::ratio(31, 10), "{top}");
    assert!(m.world.bodies[DOG].grounded);
}

#[test]
fn a_super_jump_waits_for_the_ground() {
    let mut m = two();
    let spot = bare_ground(&m);
    stand(&mut m, DOG, spot);
    let body = &mut m.world.bodies[DOG];
    (body.grounded, body.vy) = (false, -GRAVITY);
    body.y += from_int(9);
    use_it(&mut m, DOG, Kind::SuperJump);
    assert_eq!(m.players[DOG].slot.held(), Some(Kind::SuperJump));
    assert!(m.world.bodies[DOG].vy < 0);
}

#[test]
fn zoomies_fill_the_bar_and_sprint_at_twenty_eight_meters_a_second() {
    let mut m = two();
    clear_sheep(&mut m);
    let spot = bare_ground(&m);
    stand(&mut m, DOG, spot);
    let body = &mut m.world.bodies[DOG];
    (body.stamina, body.winded) = (0, true);
    use_it(&mut m, DOG, Kind::Zoomies);
    let run = Input {
        forward: 127,
        buttons: SPRINT,
        ..Input::default()
    };
    // 80 m/s^2 is 4/3 m/s a tick, so 28 m/s comes on the 21st tick
    for _ in 0..24 {
        tick(&mut m, &[run, Input::default()]);
    }
    let body = m.world.bodies[DOG];
    assert_eq!(body.stamina, STAMINA_FULL - REFILL_TICKS, "topped up each tick");
    assert_eq!(fx::round(fx::length(body.vx, body.vz)), 28);
    // back to 16 m/s once the 600 ticks are out
    idle(&mut m, 586);
    let (x, z) = spot;
    stand(&mut m, DOG, (x, z));
    for _ in 0..20 {
        tick(&mut m, &[run, Input::default()]);
    }
    let body = m.world.bodies[DOG];
    assert_eq!(fx::length(body.vx, body.vz) >> 13, SHEEPDOG.sprint >> 13);
}

#[test]
fn dognoculars_add_no_sight_at_night_now_it_is_dark_and_not_short() {
    let mut m = two();
    m.night.howl();
    // the light goes in 3 s
    idle(&mut m, 200);
    use_it(&mut m, DOG, Kind::Dognoculars);
    assert!(m.scoped(DOG));
    assert_eq!(snapshot(&m, DOG).view.sight_range, from_int(150));
}

#[test]
fn dognoculars_see_no_farther_than_a_clear_day_so_the_flock_stays_uncounted() {
    let mut m = two();
    use_it(&mut m, DOG, Kind::Dognoculars);
    assert_eq!(snapshot(&m, DOG).view.sight_range, from_int(150));
}

/// Sixty meters of clear line from a mouth, and the look along it.
fn long_view(m: &Match) -> ((Fx, Fx), (Fx, Fx), Input) {
    let span = from_int(60);
    let ways = [(0, -span), (0, span), (span, 0), (-span, 0)];
    let mut clear = m
        .map()
        .wolf_starts
        .iter()
        .flat_map(|&(x, z)| ways.iter().map(move |&(dx, dz)| ((x, z), (x + dx, z + dz))));
    let (from, to) = clear
        .find(|&(from, to)| m.clear_line(from, to) && m.map().push_out(to.0, to.1, fx::ONE) == to)
        .expect("a mouth with a clear view");
    let drop = m.map().height(to.0, to.1) + aim::TARGET_UP - m.map().height(from.0, from.1) - aim::PIVOT_UP;
    let look = Input {
        yaw: atan2(to.0 - from.0, from.1 - to.1) as u16,
        pitch: atan2(drop, span) as i16,
        ..Input::default()
    };
    (from, to, look)
}

#[test]
fn a_carcass_sixty_meters_off_is_found_through_dognoculars_alone() {
    let mut m = two();
    clear_sheep(&mut m);
    let (from, to, look) = long_view(&m);
    stand(&mut m, DOG, from);
    let off = far_off(&m);
    stand(&mut m, WOLF, off);
    m.carcasses.push(CarcassSpot {
        x: to.0,
        z: to.1,
        carcass: Carcass::default(),
    });
    tick(&mut m, &[look, Input::default()]);
    assert_eq!(m.standing().carcasses_found, 0);
    stand(&mut m, DOG, from);
    m.players[DOG].slot.take(Kind::Dognoculars);
    let scoped = Input {
        buttons: SECONDARY,
        ..look
    };
    tick(&mut m, &[scoped, Input::default()]);
    assert_eq!(m.standing().carcasses_found, 1);
}

#[test]
fn a_bell_makes_its_noise_once_at_its_sheep_when_it_goes_off() {
    let mut m = belled();
    idle(&mut m, 1);
    let bells = |m: &Match| m.heard().iter().filter(|h| h.name == BELL_NOISE).count();
    assert_eq!(bells(&m), 0);
    let (x, z) = open(&m);
    stand(&mut m, WOLF, (x, z + from_int(19)));
    idle(&mut m, 30);
    assert_eq!(bells(&m), 1);
    let heard = m.heard().iter().find(|h| h.name == BELL_NOISE).unwrap();
    assert_eq!(
        (heard.who, heard.x, heard.z),
        (0, m.world.flock.sheep[0].x, m.world.flock.sheep[0].z)
    );
}
