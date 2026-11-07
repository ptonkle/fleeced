//! The person: "be careful about sheep getting stuck on the edges btw".
//!
//! Each test here runs on the real map. A trap is a place a sheep could be
//! caught: every corner of the outer wall, both sides of where an inner wall
//! meets it, the Fold's outside corners beside its gap, both sides of each
//! wall end at a gap, the backs of trees and cairns toward the nearest wall,
//! and both long sides of each slab. Six sheep start tucked into each, and a scripted Sheepdog works them
//! three ways.
//!
//! Run with `--nocapture` to see how many got stuck and where.

mod common;

use common::*;
use tool_core::body::{Body, Input, Role, JUMP, SPRINT};
use tool_core::flock::{Flock, Kind, Sheep};
use tool_core::ground::Ground;
use tool_core::map::{table, Map, MapSize};
use tool_core::tune;
use tool_core::world::World;

const SEEDS: [u64; 4] = [1, 2, 3, 4];
/// A sheep under a dog's pressure this long has to have got somewhere.
const WINDOW: usize = 10;
/// That is, this far from where it stood.
const SOMEWHERE: f64 = 1.0;
const RUN_SECONDS: usize = 24;

#[derive(Debug, Clone)]
struct Trap {
    name: String,
    tip: (f64, f64),
    /// A step toward open ground from the tip, in the same field.
    out: (f64, f64),
}

fn p(point: (i32, i32)) -> (f64, f64) {
    (f(point.0), f(point.1))
}

fn unit(a: (f64, f64)) -> (f64, f64) {
    let l = (a.0 * a.0 + a.1 * a.1).sqrt().max(1e-9);
    (a.0 / l, a.1 / l)
}

fn toward(from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
    unit((to.0 - from.0, to.1 - from.1))
}

fn along(from: (f64, f64), way: (f64, f64), by: f64) -> (f64, f64) {
    (from.0 + way.0 * by, from.1 + way.1 * by)
}

fn traps(map: &Map) -> Vec<Trap> {
    let mut list = Vec::new();
    let middle = (f(map.length()) * 6.0 / 13.0, f(map.width()) / 2.0);
    let n = map.outer.len();
    // the corners of the outer wall, past the Fold's own, which no free
    // sheep reaches: the two that end its front wall are the Fold's outside
    // corners, and they point at the last field
    let front = (p(map.fold.gap.center).0 - 30.0, p(map.fold.gap.center).1);
    for i in 0..n {
        let in_fold = (table::FOLD_FIRST_CORNER + 1..table::FOLD_LAST_CORNER).contains(&i);
        if in_fold {
            continue;
        }
        let tip = p(map.outer[i]);
        let ends = i == table::FOLD_FIRST_CORNER || i == table::FOLD_LAST_CORNER;
        let (name, inward) = if ends {
            (format!("Fold corner {i}"), toward(tip, front))
        } else {
            let (a, b) = (p(map.outer[(i + n - 1) % n]), p(map.outer[(i + 1) % n]));
            let (ua, ub) = (toward(tip, a), toward(tip, b));
            let split = unit((ua.0 + ub.0, ua.1 + ub.1));
            // a corner that bends the other way would point out of the map
            let aim = if map.inside(m(tip.0 + split.0 * 4.0), m(tip.1 + split.1 * 4.0)) {
                split
            } else {
                toward(tip, middle)
            };
            (format!("outer corner {i}"), aim)
        };
        list.push(Trap {
            name,
            tip,
            out: inward,
        });
    }
    // where each inner wall meets the outer wall, on both sides
    for (k, line) in map.dividers.iter().enumerate() {
        for (end, tip) in [("low", p(line[0])), ("high", p(line[line.len() - 1]))] {
            for (side, dx) in [("west", -1.0), ("east", 1.0)] {
                let inward = unit((dx, if tip.1 < f(map.width()) / 2.0 { 0.6 } else { -0.6 }));
                let name = format!("wall {k} {end} end, {side}");
                list.push(Trap {
                    name,
                    tip,
                    out: inward,
                });
            }
        }
    }
    // both faces of each wall end at a gap
    for gap in map.every_gap() {
        for (end, tip) in [("a", p(gap.a)), ("b", p(gap.b))] {
            let middle_of_gap = toward(tip, p(gap.center));
            for (side, dx) in [("west", -1.0), ("east", 1.0)] {
                if gap.wall as usize == map.dividers.len() && dx > 0.0 {
                    continue;
                }
                let name = format!("gap {} end {end}, {side}", gap.wall);
                let out = unit((dx - middle_of_gap.0 * 0.2, -middle_of_gap.1 * 0.2));
                list.push(Trap { name, tip, out });
            }
        }
    }
    // the far side of each tree and cairn, toward the nearest wall
    let wall_near = |at: (f64, f64)| {
        let mut best = (f64::MAX, (0.0, 0.0));
        for wall in &map.walls {
            let (a, b) = (p(wall.a), p(wall.b));
            for k in 0..=40 {
                let t = k as f64 / 40.0;
                let q = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                best = if dist(at, q) < best.0 {
                    (dist(at, q), q)
                } else {
                    best
                };
            }
        }
        best.1
    };
    let props = map.trees.iter().map(|t| ("tree", (t.x, t.z), t.trunk)).chain(
        map.cairns
            .iter()
            .map(|c| ("cairn", (c.x, c.z), c.tiers[0].radius)),
    );
    for (k, (kind, center, radius)) in props.enumerate() {
        let center = p(center);
        let to_wall = toward(center, wall_near(center));
        list.push(Trap {
            name: format!("back of {kind} {k}"),
            tip: along(center, to_wall, f(radius) + 0.5),
            // the dog comes over the prop's other side, so the sheep is
            // pressed onto its back only if it is drawn round; `out` is the
            // way the wall is not
            out: (-to_wall.0, -to_wall.1),
        });
    }
    for (k, s) in map.slabs.iter().enumerate() {
        let c = p(s.center());
        let way = toward(p(s.a), p(s.b));
        for (side, sign) in [("left", 1.0), ("right", -1.0)] {
            let away = (-way.1 * sign, way.0 * sign);
            list.push(Trap {
                name: format!("{side} of slab {k}"),
                tip: along(c, away, f(s.half) + 0.5),
                out: away,
            });
        }
    }
    list
}

/// Six sheep tucked in at the tip, as close as the map lets them.
fn tucked(map: &Map, trap: &Trap, seed: u64) -> Flock {
    let mut flock = Flock::on_map(map, 0, seed);
    let side = (-trap.out.1, trap.out.0);
    let start = along(trap.tip, trap.out, 2.5);
    let home = map.field(m(start.0), m(start.1));
    for k in 0..6 {
        let spot = along(
            along(trap.tip, trap.out, 1.2 + (k / 2) as f64 * 0.9),
            side,
            (k % 2) as f64 * 1.2 - 0.6,
        );
        let (x, z) = map.push_out(m(spot.0), m(spot.1), tune::SHEEP_RADIUS);
        if map.field(x, z) == home {
            flock.add(Sheep::new(Kind::Ewe, x, z));
        }
    }
    flock
}

fn walk_to(body: &Body, target: (f64, f64), stop: f64, sprint: bool) -> Input {
    let (dx, dz) = (target.0 - f(body.x), target.1 - f(body.z));
    if (dx * dx + dz * dz).sqrt() < stop {
        return Input::default();
    }
    let buttons = if sprint { SPRINT } else { 0 };
    Input {
        forward: 127,
        yaw: yaw_to(dx, dz),
        buttons,
        ..Input::default()
    }
}

/// The ways a Sheepdog works a trap.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Press {
    /// Stands square on the open side, 4.5 m out, and does not move.
    Pin,
    /// Keeps walking at the nearest sheep, right up to it.
    Harry,
    /// Goes in to the tip, between the sheep and the wall.
    Between,
}

const PRESSES: [Press; 3] = [Press::Pin, Press::Harry, Press::Between];

/// Runs one trap. The count of sheep that stood still for `WINDOW` seconds
/// with the dog in reach all that time, and of sheep still within 3 m of the
/// tip when the dog has been going in there for the whole run.
fn run(map: &Map, trap: &Trap, press: Press, seed: u64) -> (usize, usize) {
    let flock = tucked(map, trap, seed);
    let count = flock.sheep.len();
    let start = along(trap.tip, trap.out, 9.0);
    let (x, z) = map.push_out(m(start.0), m(start.1), tune::SHEEPDOG.radius);
    let mut dog = Body::new(Role::Sheepdog, x, z);
    dog.settle(map);
    let mut world = World::new(vec![dog], flock);
    let post = along(trap.tip, trap.out, 4.5);
    let mut seconds: Vec<Vec<(f64, f64)>> = vec![Vec::new(); count];
    let mut pressed: Vec<Vec<bool>> = vec![Vec::new(); count];
    for tick in 0..RUN_SECONDS * SECOND as usize {
        let body = &world.bodies[0];
        let input = match press {
            Press::Pin => walk_to(body, post, 0.5, false),
            Press::Harry => {
                let here = (f(body.x), f(body.z));
                let nearest = world
                    .flock
                    .sheep
                    .iter()
                    .filter(|s| !s.folded)
                    .map(|s| (f(s.x), f(s.z)))
                    .min_by(|a, b| dist(*a, here).total_cmp(&dist(*b, here)));
                walk_to(body, nearest.unwrap_or(here), 0.5, true)
            }
            Press::Between => walk_to(body, trap.tip, 0.5, false),
        };
        world.step(map, &[input]);
        if tick % SECOND as usize == 0 {
            let at = (f(world.bodies[0].x), f(world.bodies[0].z));
            for (k, s) in world.flock.sheep.iter().enumerate() {
                seconds[k].push((f(s.x), f(s.z)));
                pressed[k].push(!s.folded && dist((f(s.x), f(s.z)), at) < f(tune::FLIGHT_RADIUS));
            }
        }
    }
    let mut stuck = 0;
    for k in 0..count {
        let held = (WINDOW..RUN_SECONDS).any(|t| {
            let span = t - WINDOW..=t;
            let under = span.clone().all(|i| pressed[k][i]);
            let moved = span
                .clone()
                .map(|i| dist(seconds[k][i], seconds[k][t - WINDOW]))
                .fold(0.0, f64::max);
            under && moved < SOMEWHERE
        });
        stuck += held as usize;
    }
    let left = world
        .flock
        .sheep
        .iter()
        .filter(|s| dist((f(s.x), f(s.z)), trap.tip) < 3.0)
        .count();
    (stuck, left)
}

#[test]
fn a_sheepdog_walking_at_a_slab_stops_and_a_jump_lands_on_it() {
    let map = Map::sized(3, MapSize::Medium);
    let s = &map.slabs[0];
    let c = p(s.center());
    let way = toward(p(s.a), p(s.b));
    let side = (-way.1, way.0);
    let start = along(c, side, f(s.half) + 4.0);
    let mut dog = Body::new(Role::Sheepdog, m(start.0), m(start.1));
    dog.settle(&map);
    let mut world = World::new(vec![dog], Flock::on_map(&map, 0, 1));
    let radius = f(s.half + tune::SHEEPDOG.radius);
    let walk = |world: &World| walk_to(&world.bodies[0], c, 0.0, false);
    for _ in 0..3 * SECOND as usize {
        let input = walk(&world);
        world.step(&map, &[input]);
    }
    let body = &world.bodies[0];
    let held = dist((f(body.x), f(body.z)), c);
    assert!(
        (held - radius).abs() < 0.1,
        "held at {held}, the side at {radius}"
    );
    assert_eq!(body.y, map.height(body.x, body.z), "on the ground");
    // round to the near end, then jump up it the long way: across, a body
    // in the air at a run is past before it comes down
    let end = along(p(s.b), way, f(s.half) + 1.2);
    let round = along(end, side, 2.5);
    for spot in [round, end] {
        for _ in 0..2 * SECOND as usize {
            let input = walk_to(&world.bodies[0], spot, 0.1, false);
            world.step(&map, &[input]);
        }
    }
    let mut stood = false;
    for tick in 0..2 * SECOND as usize {
        let mut input = walk_to(&world.bodies[0], p(s.a), 0.3, false);
        input.buttons |= if tick == 0 { JUMP } else { 0 };
        world.step(&map, &[input]);
        let b = &world.bodies[0];
        let on = map.height(b.x, b.z) > map.terrain_height(b.x, b.z);
        stood |= on && b.grounded && b.y == map.height(b.x, b.z);
    }
    assert!(stood, "never landed on it");
}

#[test]
fn no_sheep_stays_stuck_in_any_corner_of_the_real_map_under_a_sheepdogs_pressure() {
    let (mut traps_run, mut sheep_run) = (0, 0);
    let mut stuck_at: Vec<(u64, String, Press, usize)> = Vec::new();
    for size in MapSize::ALL {
        let before: usize = stuck_at.iter().map(|s| s.3).sum();
        for seed in SEEDS {
            let map = Map::sized(seed, size);
            for trap in traps(&map) {
                for press in PRESSES {
                    let (stuck, _) = run(&map, &trap, press, seed);
                    traps_run += 1;
                    sheep_run += tucked(&map, &trap, seed).sheep.len();
                    if stuck > 0 {
                        stuck_at.push((seed, format!("{size:?}, {}", trap.name), press, stuck));
                    }
                }
            }
        }
        let on_size = stuck_at.iter().map(|s| s.3).sum::<usize>() - before;
        println!("{size:?}: {on_size} stuck");
    }
    let total: usize = stuck_at.iter().map(|s| s.3).sum();
    println!("{traps_run} runs, {sheep_run} sheep, {total} stuck");
    for (seed, name, press, stuck) in &stuck_at {
        println!("  seed {seed}: {stuck} stuck at {name}, {press:?}");
    }
    assert_eq!(total, 0, "{stuck_at:?}");
}

#[test]
fn a_sheepdog_going_in_between_the_sheep_and_the_wall_clears_a_corner() {
    let (mut left_total, mut left_at) = (0, Vec::new());
    for seed in SEEDS {
        let map = Map::generate(seed);
        for trap in traps(&map) {
            if trap.name.starts_with("outer corner") || trap.name.contains("end,") {
                let (_, left) = run(&map, &trap, Press::Between, seed);
                left_total += left;
                if left > 0 {
                    left_at.push((seed, trap.name.clone(), left));
                }
            }
        }
    }
    println!("{left_total} sheep left within 3 m of a tip: {left_at:?}");
    assert_eq!(left_total, 0, "{left_at:?}");
}

/// Sheep in a block just west of a point.
fn block(map: &Map, count: usize, at: (f64, f64), seed: u64) -> Flock {
    let mut flock = Flock::on_map(map, 0, seed);
    let side = (count as f64).sqrt().ceil() as usize;
    for k in 0..count {
        let x = at.0 - ((k / side) as f64) * 1.3;
        let z = at.1 + ((k % side) as f64 - side as f64 / 2.0) * 1.3;
        let (x, z) = map.push_out(m(x), m(z), tune::SHEEP_RADIUS);
        flock.add(Sheep::new(Kind::Ewe, x, z));
    }
    flock
}

fn world_on(map: &Map, x: f64, z: f64, flock: Flock) -> World {
    let mut dog = Body::new(Role::Sheepdog, m(x), m(z));
    dog.settle(map);
    World::new(vec![dog], flock)
}

#[test]
fn a_scripted_dog_drives_a_flock_the_length_of_the_real_map_into_the_fold() {
    // from the lowest field: about 245 m, a trot of about 102 s at 2.4 m/s,
    // and the time to get behind them and sweep at each gap
    for seed in [1, 2, 3, 4, 5, 6] {
        let map = Map::generate(seed);
        let first = map.gaps.iter().find(|g| g.wall == 0).expect("a gap");
        let start = (f(first.center.0) - 30.0, f(first.center.1));
        let flock = block(&map, 30, start, seed);
        let mut world = world_on(&map, start.0 - 14.0, start.1, flock);
        let mut dog = map_shepherd(&map, 0);
        let mut done = None;
        for t in 0..240 * SECOND {
            let input = dog.input(&map, &world.bodies[0], &world.flock);
            world.step(&map, &[input]);
            if world.flock.folded == 30 {
                done = Some(t / SECOND);
                break;
            }
        }
        println!(
            "seed {seed}: all folded at {done:?} s, {} folded",
            world.flock.folded
        );
        assert_eq!(world.flock.folded, 30, "seed {seed}");
    }
}

#[test]
fn a_flock_does_not_pile_up_on_the_wall_ends_of_a_gap() {
    // 60 sheep driven at each gap of the first wall, starting off to one
    // side so the flock meets the wall end and not the middle of the gap.
    // A sheep held there would stand under pressure and go nowhere.
    for seed in [1, 2, 3, 4] {
        let map = Map::generate(seed);
        for gap in map.gaps.iter().filter(|g| g.wall == 0) {
            for off in [-12.0, 12.0] {
                let start = (f(gap.center.0) - 22.0, f(gap.center.1) + off);
                let flock = block(&map, 60, start, seed);
                let mut world = world_on(&map, start.0 - 14.0, start.1, flock);
                let mut dog = Shepherd {
                    goals: vec![Goal {
                        x: f(gap.center.0) + 3.0,
                        z: f(gap.center.1),
                        field: 0,
                    }],
                    stage: 0,
                    doors: vec![],
                };
                let ends = [p(gap.a), p(gap.b)];
                let mut held = vec![0; 60];
                let mut worst = 0;
                for _ in 0..50 * SECOND {
                    let input = dog.input(&map, &world.bodies[0], &world.flock);
                    world.step(&map, &[input]);
                    for (k, s) in world.flock.sheep.iter().enumerate() {
                        let at = (f(s.x), f(s.z));
                        // pressed and going nowhere; a sheep resting at the
                        // tail of a flock already through is not a pile-up
                        let slow = s.alarm >= tune::ALERT && len(s.vx, s.vz) < 0.3;
                        let by_end = ends.iter().any(|&e| dist(at, e) < 2.5);
                        held[k] = if by_end && slow { held[k] + 1 } else { 0 };
                        worst = worst.max(held[k]);
                    }
                }
                let through = world
                    .flock
                    .sheep
                    .iter()
                    .filter(|s| map.field(s.x, s.z) > 0)
                    .count();
                println!(
                    "seed {seed} gap z {:.0} off {off}: {through} of 60 through, longest hold {:.1} s",
                    f(gap.center.1),
                    worst as f64 / SECOND as f64
                );
                assert!(through >= 57, "seed {seed} gap {off}: {through}");
                assert!(worst < 3 * SECOND, "held {} ticks", worst);
            }
        }
    }
}

#[test]
fn calm_sheep_drift_off_every_edge_of_the_real_map_by_themselves() {
    // set down touching each trap, with no dog anywhere: in a minute of
    // grazing each has left the stone
    for seed in [1, 2] {
        let map = Map::generate(seed);
        for trap in traps(&map) {
            let flock = tucked(&map, &trap, seed);
            let mut world = World::new(Vec::new(), flock);
            for _ in 0..60 * SECOND {
                world.step(&map, &[]);
            }
            for s in &world.flock.sheep {
                let wider = s.radius + tune::EDGE_SENSE / 4;
                assert_eq!(
                    map.push_out(s.x, s.z, wider),
                    (s.x, s.z),
                    "{} seed {seed}",
                    trap.name
                );
            }
        }
    }
}
