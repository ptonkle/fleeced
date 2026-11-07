//! The worlds a step is timed on, shared by `bench.rs` and `wasm_bench.rs`:
//! the real map and eight players, in the cases that cost the most.
//!
//! - calm: the start of a match, sheep spread out and grazing.
//! - driven: the whole flock tight in the open with sheepdogs all round it,
//!   every sheep alarmed and among neighbors.
//! - cornered: the whole flock packed into the map's sharpest corner with
//!   sheepdogs across the mouth, every sheep at an edge.

use tool_core::body::{Body, Input, Role, SPRINT};
use tool_core::flock::{Flock, Kind, Sheep};
use tool_core::fx::{self, from_int, Fx};
use tool_core::ground::Ground;
use tool_core::map::Map;
use tool_core::world::World;

pub const PLAYERS: usize = 8;
#[allow(dead_code)]
pub const SCENES: [&str; 3] = ["calm", "driven", "cornered"];

/// A block of `count` sheep, 0.9 m apart, from this corner along this way.
fn block(map: &Map, count: usize, at: (Fx, Fx), way: (Fx, Fx), seed: u64) -> Flock {
    let mut flock = Flock::on_map(map, 0, seed);
    let spacing = fx::ratio(9, 10);
    let mut side = 1;
    while side * side < count {
        side += 1;
    }
    for k in 0..count {
        let (a, b) = ((k / side) as i32, (k % side) as i32 - side as i32 / 2);
        let (ahead, aside) = (spacing * a, spacing * b);
        let x = at.0 + fx::mul(ahead, way.0) - fx::mul(aside, way.1);
        let z = at.1 + fx::mul(ahead, way.1) + fx::mul(aside, way.0);
        let (x, z) = map.push_out(x, z, fx::ratio(45, 100));
        flock.add(Sheep::new(Kind::Ewe, x, z));
    }
    flock
}

fn on_ground(map: &Map, role: Role, x: Fx, z: Fx) -> Body {
    let (x, z) = map.push_out(x, z, fx::ratio(4, 10));
    let mut body = Body::new(role, x, z);
    body.settle(map);
    body
}

/// A ring of sheepdogs round a point.
fn ring(map: &Map, x: Fx, z: Fx, radius: Fx) -> Vec<Body> {
    (0..PLAYERS as i32)
        .map(|k| {
            let angle = fx::TURN * k / PLAYERS as i32;
            let (dx, dz) = (fx::mul(radius, fx::sin(angle)), fx::mul(radius, fx::cos(angle)));
            on_ground(map, Role::Sheepdog, x + dx, z + dz)
        })
        .collect()
}

pub fn scene(map: &Map, which: usize, sheep: usize) -> World {
    match which {
        0 => {
            let flock = Flock::on_map(map, sheep, 1);
            let dogs = map
                .sheepdog_starts
                .iter()
                .take(4)
                .map(|&(x, z)| (Role::Sheepdog, x, z));
            let wolves = map.wolf_starts.iter().take(4).map(|&(x, z)| (Role::Wolf, x, z));
            let bodies = dogs
                .chain(wolves)
                .map(|(r, x, z)| on_ground(map, r, x, z))
                .collect();
            World::new(bodies, flock)
        }
        1 => {
            let (x, z) = (from_int(130), from_int(70));
            let flock = block(map, sheep, (x, z), (fx::ONE, 0), 1);
            let radius = from_int((sheep as f64).sqrt() as i32 + 5);
            let (cx, cz) = (x + from_int((sheep as f64).sqrt() as i32) / 3, z);
            World::new(ring(map, cx, cz, radius), flock)
        }
        _ => {
            let (x, z) = map.outer[0];
            let way = (fx::ratio(7, 10), fx::ratio(7, 10));
            let flock = block(map, sheep, (x + from_int(2), z + from_int(2)), way, 1);
            let reach = from_int((sheep as f64).sqrt() as i32 + 8);
            let bodies = (0..PLAYERS as i32)
                .map(|k| {
                    let (dx, dz) = (reach + from_int(k * 2), reach - from_int(k * 2));
                    on_ground(map, Role::Sheepdog, x + dx, z + dz)
                })
                .collect();
            World::new(bodies, flock)
        }
    }
}

/// A busy script: every body walks a small circle of its own and some sprint
/// or jump now and then, in integers so a native and a wasm run are the same.
pub fn inputs(tick: u32, out: &mut [Input; PLAYERS]) {
    for (k, input) in out.iter_mut().enumerate() {
        let k = k as u32;
        let yaw = tick.wrapping_mul(400).wrapping_add(k.wrapping_mul(9000)) as u16;
        let buttons = if (tick + k * 37) % 300 < 40 { SPRINT } else { 0 };
        *input = Input {
            forward: 127,
            yaw,
            buttons,
            ..Input::default()
        };
    }
}

/// How many bytes one copy of the world takes, heap and all.
pub fn bytes(world: &World) -> usize {
    use std::mem::size_of_val;
    size_of_val(world) + size_of_val(&world.bodies[..]) + size_of_val(&world.flock.sheep[..])
}
