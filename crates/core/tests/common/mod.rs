//! A scripted sheepdog and the fields it works, shared by the herding tests
//! and `examples/herd_probe.rs`. It plays through the real `Input`, so a test
//! exercises what a player does. It lives outside the core, so it may use
//! floats; a bot dog in the core would need an integer `yaw_toward`.
//!
//! The dog is a simplified Strömbom et al. 2014: it always stands behind the
//! sheep farthest from the goal, on the line away from the goal, and walks in
//! until that sheep is under pressure. Collecting and driving come out of the
//! one rule, the way a wearing collie sweeps side to side behind its flock.

#![allow(dead_code)]

use tool_core::body::{Body, Input, Role};
use tool_core::flock::{Flock, Fold, Sheep};
use tool_core::fx::{self, Fx};
use tool_core::ground::{Ground, Rect, WalledField};
use tool_core::map::Map;
use tool_core::tune;
use tool_core::world::World;

pub fn m(v: f64) -> Fx {
    (v * 65536.0) as Fx
}

pub fn f(v: Fx) -> f64 {
    v as f64 / 65536.0
}

pub fn rect(x0: f64, z0: f64, x1: f64, z1: f64) -> Rect {
    Rect {
        min_x: m(x0),
        min_z: m(z0),
        max_x: m(x1),
        max_z: m(z1),
    }
}

/// Field A west of a wall along x = 0..1, a 6 m gap at z = -3..3, field B
/// east of it, and a fold at the far end with a 6 m gate in its west wall.
pub struct Pasture {
    pub ground: WalledField,
    pub bounds: Rect,
    pub fold: Fold,
    /// The x of the wall with the gap, which a sheep crosses going east.
    pub wall_x: f64,
    pub gate_x: f64,
}

pub fn pasture() -> Pasture {
    let gap = 3.0;
    let walls = vec![
        // the outer wall
        rect(-61.0, -41.0, 61.0, -40.0),
        rect(-61.0, 40.0, 61.0, 41.0),
        rect(-61.0, -40.0, -60.0, 40.0),
        rect(60.0, -40.0, 61.0, 40.0),
        // the wall with the gap
        rect(0.0, -40.0, 1.0, -gap),
        rect(0.0, gap, 1.0, 40.0),
        // the fold: west wall with its gate, north and south walls
        rect(40.0, -40.0, 41.0, -gap),
        rect(40.0, gap, 41.0, 40.0),
    ];
    let bounds = rect(-61.0, -41.0, 61.0, 41.0);
    Pasture {
        ground: WalledField {
            walls,
            dividers: vec![m(0.5)],
            fold: Some(rect(41.0, -40.0, 60.0, 40.0)),
        },
        bounds,
        fold: Fold {
            back_x: m(60.0),
            back_z: 0,
        },
        wall_x: 0.5,
        gate_x: 40.5,
    }
}

/// `count` calm sheep in a loose patch.
pub fn loose_flock(p: &Pasture, count: usize, cx: f64, cz: f64, spacing: f64, seed: u64) -> Flock {
    let mut flock = Flock::new(p.bounds, Some(p.fold), seed);
    let side = (count as f64).sqrt().ceil() as usize;
    for k in 0..count {
        let (col, row) = ((k % side) as f64, (k / side) as f64);
        let x = cx + (col - side as f64 / 2.0) * spacing + ((k * 7 % 5) as f64 - 2.0) * 0.17;
        let z = cz + (row - side as f64 / 2.0) * spacing + ((k * 11 % 5) as f64 - 2.0) * 0.17;
        flock.add(Sheep::new(tool_core::flock::Kind::Ewe, m(x), m(z)));
    }
    flock
}

pub fn free_sheep(flock: &Flock) -> Vec<(f64, f64)> {
    flock
        .sheep
        .iter()
        .filter(|s| !s.eaten && !s.folded)
        .map(|s| (f(s.x), f(s.z)))
        .collect()
}

pub fn center(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len().max(1) as f64;
    (
        points.iter().map(|p| p.0).sum::<f64>() / n,
        points.iter().map(|p| p.1).sum::<f64>() / n,
    )
}

pub fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// How far the farthest sheep is from the center.
pub fn reach(points: &[(f64, f64)]) -> f64 {
    let c = center(points);
    points.iter().map(|&p| dist(p, c)).fold(0.0, f64::max)
}

/// How far behind the rearmost sheep the dog walks: inside the flight zone,
/// short of its core.
const PRESSURE: f64 = 4.5;
/// How wide the dog runs round a sheep: outside the flight zone.
const FLANK: f64 = 10.0;

/// Where the flock should go next. A sheep is through once it and a point
/// 2 m behind it are past `field`; `NO_FIELD` is a goal on open land.
#[derive(Debug, Clone, Copy)]
pub struct Goal {
    pub x: f64,
    pub z: f64,
    pub field: u8,
}

pub const NO_FIELD: u8 = u8::MAX;

/// A gap the dog itself can use, in the wall after field `wall`.
#[derive(Debug, Clone, Copy)]
pub struct Door {
    pub wall: u8,
    pub x: f64,
    pub z: f64,
}

pub struct Shepherd {
    /// The dog works the sheep not yet through a goal and moves to the next
    /// goal when there are none, as a handler finishes a gate before leaving
    /// it.
    pub goals: Vec<Goal>,
    pub stage: usize,
    pub doors: Vec<Door>,
}

fn at<G: Ground>(ground: &G, p: (f64, f64)) -> u8 {
    ground.field(m(p.0), m(p.1))
}

fn through<G: Ground>(ground: &G, sheep: (f64, f64), goal: &Goal) -> bool {
    goal.field != NO_FIELD && at(ground, (sheep.0 - 2.0, sheep.1)) > goal.field
}

impl Shepherd {
    /// A dog working toward a point on open land.
    pub fn toward(x: f64, z: f64) -> Shepherd {
        Shepherd {
            goals: vec![Goal {
                x,
                z,
                field: NO_FIELD,
            }],
            stage: 0,
            doors: vec![],
        }
    }

    /// Where the dog wants to stand now, and the sheep that puts it there.
    fn post<G: Ground>(
        &mut self,
        ground: &G,
        dog: (f64, f64),
        sheep: &[(f64, f64)],
    ) -> ((f64, f64), (f64, f64)) {
        let left = |goal: &Goal| -> Vec<(f64, f64)> {
            sheep
                .iter()
                .copied()
                .filter(|&s| !through(ground, s, goal))
                .collect()
        };
        while self.stage + 1 < self.goals.len() && left(&self.goals[self.stage]).is_empty() {
            self.stage += 1;
        }
        let goal_at = self.goals[self.stage];
        let flock = left(&goal_at);
        if flock.is_empty() {
            return (dog, dog);
        }
        let goal = (goal_at.x, goal_at.z);
        let far = flock
            .iter()
            .copied()
            .max_by(|a, b| dist(*a, goal).total_cmp(&dist(*b, goal)))
            .unwrap_or(goal);
        let d = dist(far, goal).max(0.1);
        let post = (
            far.0 + (far.0 - goal.0) / d * PRESSURE,
            far.1 + (far.1 - goal.1) / d * PRESSURE,
        );
        (post, far)
    }

    /// A post beyond the sheep from the dog would drive it the wrong way,
    /// so the dog runs wide round it first, as a collie casts out behind its
    /// sheep: to the point abreast of the sheep on the dog's own side.
    fn flank(dog: (f64, f64), far: (f64, f64), post: (f64, f64)) -> Option<(f64, f64)> {
        let (ax, az) = (dog.0 - far.0, dog.1 - far.1);
        let (bx, bz) = (post.0 - far.0, post.1 - far.1);
        let length = (bx * bx + bz * bz).sqrt();
        if length < 0.1 || ax * bx + az * bz >= 0.0 {
            return None;
        }
        let (nx, nz) = (-bz / length, bx / length);
        let side = if ax * nx + az * nz >= 0.0 { 1.0 } else { -1.0 };
        Some((far.0 + nx * side * FLANK, far.1 + nz * side * FLANK))
    }

    /// Where to walk to reach `post`: through a door if it lies across a wall.
    fn waypoint<G: Ground>(&self, ground: &G, dog: (f64, f64), post: (f64, f64)) -> (f64, f64) {
        let (from, to) = (at(ground, dog), at(ground, post));
        if from == to {
            return post;
        }
        let wall = from.min(to);
        let door = self
            .doors
            .iter()
            .filter(|d| d.wall == wall)
            .min_by(|a, b| dist(dog, (a.x, a.z)).total_cmp(&dist(dog, (b.x, b.z))));
        let Some(door) = door else {
            return post;
        };
        let way = if to > from { 3.0 } else { -3.0 };
        if (dog.1 - door.z).abs() > 1.5 {
            (door.x - way, door.z)
        } else {
            (door.x + way, door.z)
        }
    }

    /// The input for this tick, for the dog at `body`.
    pub fn input<G: Ground>(&mut self, ground: &G, body: &Body, flock: &Flock) -> Input {
        let sheep = free_sheep(flock);
        if sheep.is_empty() {
            return Input::default();
        }
        let dog = (f(body.x), f(body.z));
        let (post, far) = self.post(ground, dog, &sheep);
        let wide = Self::flank(dog, far, post);
        let post = self.waypoint(ground, dog, wide.unwrap_or(post));
        let (dx, dz) = (post.0 - dog.0, post.1 - dog.1);
        let d = (dx * dx + dz * dz).sqrt();
        if d < 0.75 {
            return Input::default();
        }
        if wide.is_some() {
            let buttons = tool_core::body::SPRINT;
            return Input {
                forward: 127,
                yaw: yaw_to(dx, dz),
                buttons,
                ..Input::default()
            };
        }
        // Str\u{f6}mbom's stop: do not walk into the flock
        let goal = self.goals[self.stage];
        let working: Vec<(f64, f64)> = sheep
            .iter()
            .copied()
            .filter(|&s| !through(ground, s, &goal))
            .collect();
        let closing = working.iter().any(|&s| {
            let (sx, sz) = (s.0 - dog.0, s.1 - dog.1);
            dist(s, dog) < 3.5 && (sx * dx + sz * dz) > 0.0
        });
        if closing {
            return Input::default();
        }
        // run to a post that is far away; walk the flock
        let nearest = working.iter().map(|&s| dist(s, dog)).fold(f64::MAX, f64::min);
        let buttons = if d > 8.0 && nearest > 12.0 {
            tool_core::body::SPRINT
        } else {
            0
        };
        Input {
            forward: 127,
            yaw: yaw_to(dx, dz),
            buttons,
            ..Input::default()
        }
    }
}

/// The yaw that faces along (dx, dz): yaw 0 faces -z and a quarter turn +x.
pub fn yaw_to(dx: f64, dz: f64) -> u16 {
    let yaw = (dx.atan2(-dz) / std::f64::consts::TAU * 65536.0) as i64;
    yaw.rem_euclid(65536) as u16
}

pub fn shepherd(p: &Pasture) -> Shepherd {
    Shepherd {
        goals: vec![
            Goal {
                x: p.wall_x + 2.0,
                z: 0.0,
                field: 0,
            },
            Goal {
                x: p.gate_x + 4.0,
                z: 0.0,
                field: 1,
            },
        ],
        stage: 0,
        doors: vec![
            Door {
                wall: 0,
                x: p.wall_x,
                z: 0.0,
            },
            Door {
                wall: 1,
                x: p.gate_x,
                z: 0.0,
            },
        ],
    }
}

/// A dog that drives a flock from this field on through each wall's gap
/// nearest the last and into the Fold.
pub fn map_shepherd(map: &Map, from_field: u8) -> Shepherd {
    let doors: Vec<Door> = map
        .every_gap()
        .map(|g| Door {
            wall: g.wall,
            x: f(g.center.0),
            z: f(g.center.1),
        })
        .collect();
    let mut goals = Vec::new();
    let mut last = (0.0, f(map.fold.gap.center.1));
    for wall in from_field..=map.dividers.len() as u8 {
        let door = doors
            .iter()
            .filter(|d| d.wall == wall)
            .min_by(|a, b| dist(last, (a.x, a.z)).total_cmp(&dist(last, (b.x, b.z))))
            .copied()
            .expect("every wall has a gap");
        goals.push(Goal {
            x: door.x + 3.0,
            z: door.z,
            field: wall,
        });
        last = (door.x, door.z);
    }
    Shepherd {
        goals,
        stage: 0,
        doors,
    }
}

/// A world with one sheepdog at this spot and this flock.
pub fn world_with_dog(x: f64, z: f64, flock: Flock) -> World {
    World::new(vec![Body::new(Role::Sheepdog, m(x), m(z))], flock)
}

pub const SECOND: u32 = tune::TICKS_PER_SECOND as u32;

pub fn len(x: Fx, z: Fx) -> f64 {
    f(fx::length(x, z))
}
