//! Measures map generation and collision, for MEASUREMENTS.md. Prints
//! numbers and asserts nothing.
//!
//!     cargo run --release --example map_probe -- [SEEDS]

use std::time::Instant;

use tool_core::fx::{self, Fx, ONE};
use tool_core::ground::Ground;
use tool_core::map::{proof, Map};

fn meters(fx: Fx) -> String {
    format!("{:.2}", fx as f64 / ONE as f64)
}

fn main() {
    let seeds: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1000);
    let mut times: Vec<f64> = Vec::new();
    let mut sink = 0usize;
    let mut shortest_tunnel = Fx::MAX;
    let mut closest_mouths = Fx::MAX;
    let mut steepest = 0;
    for seed in 0..seeds {
        let t = Instant::now();
        let map = Map::generate(seed);
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        sink += map.walls.len();
        for t in &map.tunnels {
            shortest_tunnel = shortest_tunnel.min(fx::length(t.a.0 - t.b.0, t.a.1 - t.b.1));
        }
        let mouths = map.mouths();
        for (i, a) in mouths.iter().enumerate() {
            for b in &mouths[i + 1..] {
                closest_mouths = closest_mouths.min(fx::length(a.0 - b.0, a.1 - b.1));
            }
        }
        // the steepest ground inside the walls, over a 2 m step
        for ix in (2..258).step_by(4) {
            for iz in (2..138).step_by(4) {
                let (x, z) = (fx::from_int(ix), fx::from_int(iz));
                if map.inside(x, z) {
                    let h = map.terrain_height(x, z);
                    let along = map.terrain_height(x + 2 * ONE, z) - h;
                    let across = map.terrain_height(x, z + 2 * ONE) - h;
                    steepest = steepest.max(fx::length(along, across) / 2);
                }
            }
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "generate: {seeds} seeds, mean {mean:.2} ms, p99 {:.2} ms, worst {:.2} ms ({sink})",
        times[times.len() * 99 / 100],
        times[times.len() - 1],
    );

    println!(
        "shortest tunnel {} m, closest mouths {} m, steepest ground {} m in 1 m",
        meters(shortest_tunnel),
        meters(closest_mouths),
        meters(steepest)
    );

    let map = Map::generate(1);
    println!(
        "map JSON: {} KB",
        serde_json::to_string(&map).map_or(0, |s| s.len()) / 1024
    );
    for count in [100, 400] {
        let t = Instant::now();
        let spots = map.sheep_starts(count);
        println!(
            "sheep_starts({count}): {:.2} ms, {} spots",
            t.elapsed().as_secs_f64() * 1000.0,
            spots.len()
        );
    }

    // bodies at every sheep start, so some are by walls and most in the open
    let spots = map.sheep_starts(400);
    let rounds = 2000;
    let t = Instant::now();
    let mut total = 0i64;
    for _ in 0..rounds {
        for &(x, z) in &spots {
            let (nx, nz) = map.push_out(x, z, ONE / 2);
            total += (nx + nz + map.height(x, z)) as i64;
        }
    }
    let calls = (rounds * spots.len()) as f64;
    println!(
        "push_out + height: {:.0} ns a body ({total})",
        t.elapsed().as_secs_f64() * 1e9 / calls
    );

    // the worst case: every body pressed against an inner wall
    let wall = map
        .walls
        .iter()
        .find(|w| matches!(w.kind, tool_core::map::walls::WallKind::Inner));
    if let Some(w) = wall {
        let mid = ((w.a.0 + w.b.0) / 2, (w.a.1 + w.b.1) / 2);
        let t = Instant::now();
        let mut total = 0i64;
        for _ in 0..400_000 {
            let (nx, nz) = map.push_out(mid.0 + ONE / 4, mid.1, ONE / 2);
            total += (nx + nz) as i64;
        }
        println!(
            "push_out against a wall: {:.0} ns ({total})",
            t.elapsed().as_secs_f64() * 1e9 / 400_000.0
        );
    }

    let mut narrowest = Fx::MAX;
    let mut widest = 0;
    let sample = seeds.min(300);
    for seed in 0..sample {
        let width = proof::route_width(&Map::generate(seed));
        narrowest = narrowest.min(width);
        widest = widest.max(width);
    }
    println!(
        "route width over {sample} seeds: narrowest {} m, widest {} m",
        meters(narrowest),
        meters(widest)
    );
}
