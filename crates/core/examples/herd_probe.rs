//! Watch a scripted sheepdog work a flock, a row of numbers a few times a
//! minute and a map when asked. For tuning `tune.rs` by eye.
//!
//!     cargo run --release -p tool_core --example herd_probe -- [sheep] [seconds] [map] [seed]
//!
//! Each row: seconds; dog x, z; flock center x, z; the farthest free sheep
//! from the center; how many have crossed the gap wall and the fold's gate;
//! how many went back west over the wall.

#[path = "../tests/common/mod.rs"]
mod common;

use common::*;

fn map(p: &Pasture, world: &tool_core::world::World) {
    let (cols, rows) = (122usize, 41usize);
    let mut g = vec![vec![' '; cols]; rows];
    let at = |x: f64, z: f64| {
        (
            ((x + 61.0) as usize).min(cols - 1),
            (((z + 41.0) / 2.0) as usize).min(rows - 1),
        )
    };
    for w in &p.ground.walls {
        let (x0, z0, x1, z1) = (f(w.min_x), f(w.min_z), f(w.max_x), f(w.max_z));
        let mut x = x0;
        while x <= x1 {
            let mut z = z0;
            while z <= z1 {
                let (c, r) = at(x, z);
                g[r][c] = '#';
                z += 1.0;
            }
            x += 1.0;
        }
    }
    for s in &world.flock.sheep {
        let (c, r) = at(f(s.x), f(s.z));
        g[r][c] = match g[r][c] {
            'o' | '8' => '8',
            _ if s.folded => 'f',
            _ => 'o',
        };
    }
    let b = &world.bodies[0];
    let (c, r) = at(f(b.x), f(b.z));
    g[r][c] = 'D';
    let (_, mid) = at(f(b.x), f(b.z));
    let (from, to) = (mid.saturating_sub(10), (mid + 11).min(rows));
    for row in &g[from..to] {
        println!("{}", row.iter().collect::<String>());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let count: usize = args.get(1).map_or(30, |a| a.parse().expect("sheep"));
    let seconds: u32 = args.get(2).map_or(180, |a| a.parse().expect("seconds"));
    let show_map: u32 = args.get(3).map_or(0, |a| a.parse().expect("map every n seconds"));
    let seed: u64 = args.get(4).map_or(7, |a| a.parse().expect("seed"));

    let p = pasture();
    let flock = loose_flock(&p, count, -25.0, 8.0, 1.8, seed);
    let mut world = world_with_dog(-50.0, 0.0, flock);
    let mut dog = shepherd(&p);
    let mut east = vec![false; count];
    let mut back = 0;
    let mut most_far: f64 = 0.0;
    for t in 0..seconds * SECOND {
        let input = dog.input(&p.ground, &world.bodies[0], &world.flock);
        world.step(&p.ground, &[input]);
        for (k, s) in world.flock.sheep.iter().enumerate() {
            let is_east = f(s.x) > p.wall_x;
            if east[k] && !is_east {
                back += 1;
            }
            east[k] = is_east;
        }
        let free = free_sheep(&world.flock);
        if !free.is_empty() {
            most_far = most_far.max(reach(&free));
        }
        if t % (5 * SECOND) == 0 {
            let c = center(&free);
            let d = &world.bodies[0];
            println!(
                "{:4}s dog {:6.1} {:6.1}  center {:6.1} {:6.1}  far {:5.1}  east {:3}  folded {:3}  back {:3}  stage {}",
                t / SECOND,
                f(d.x),
                f(d.z),
                c.0,
                c.1,
                reach(&free),
                east.iter().filter(|e| **e).count(),
                world.flock.folded,
                back,
                dog.stage,
            );
        }
        if show_map > 0 && t % (show_map * SECOND) == 0 {
            map(&p, &world);
        }
        if world.flock.folded as usize == count {
            println!("all {count} folded at {}s; back over the wall {back}", t / SECOND);
            return;
        }
    }
    println!(
        "{} of {count} folded; back over the wall {back}",
        world.flock.folded
    );
}
