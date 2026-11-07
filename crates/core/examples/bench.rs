//! One `World::step` on the real map, native, with the worst single tick,
//! a 12-tick replay from a snapshot, and what a snapshot costs.
//!
//!     cargo run --release -p tool_core --example bench
//!
//! `scripts/bench_wasm.mjs` runs the same worlds under wasm, which is where
//! the contract's budget applies.

mod scene;

use scene::{bytes, inputs, scene, PLAYERS, SCENES};
use std::time::Instant;
use tool_core::body::Input;
use tool_core::map::Map;

const WARM: u32 = 300;
const TICKS: u32 = 3000;

fn main() {
    let map = Map::generate(1);
    println!(
        "size of a World {} bytes, a Sheep {}",
        std::mem::size_of::<tool_core::world::World>(),
        std::mem::size_of::<tool_core::flock::Sheep>()
    );
    println!(
        "{:9} {:>6} {:>9} {:>9} {:>11} {:>9} {:>9}",
        "scene", "sheep", "mean ns", "worst ns", "replay 12", "copy ns", "bytes"
    );
    for sheep in [100, 400] {
        for (which, name) in SCENES.iter().enumerate() {
            let mut world = scene(&map, which, sheep);
            let mut keys = [Input::default(); PLAYERS];
            for t in 0..WARM {
                inputs(t, &mut keys);
                world.step(&map, &keys);
            }
            let (mut total, mut worst) = (0u128, 0u128);
            for t in WARM..WARM + TICKS {
                inputs(t, &mut keys);
                let start = Instant::now();
                world.step(&map, &keys);
                let spent = start.elapsed().as_nanos();
                total += spent;
                worst = worst.max(spent);
            }
            let mut copy_total = 0u128;
            let mut replay_worst = 0u128;
            for r in 0..200 {
                let start = Instant::now();
                let mut copy = world.clone();
                copy_total += start.elapsed().as_nanos();
                let start = Instant::now();
                for t in 0..12 {
                    inputs(WARM + TICKS + r + t, &mut keys);
                    copy.step(&map, &keys);
                }
                replay_worst = replay_worst.max(start.elapsed().as_nanos());
                world = copy;
            }
            println!(
                "{name:9} {sheep:>6} {:>9} {worst:>9} {:>8} us {:>9} {:>9}",
                total / TICKS as u128,
                replay_worst / 1000,
                copy_total / 200,
                bytes(&world),
            );
        }
    }
}
