//! The sheep. Herding is the whole game, so this follows how real flocks
//! work, not plain Boids:
//!
//! - Strömbom et al. 2014, "Solving the shepherding problem": repulsion from
//!   the dog, attraction to the local center of the neighbors, short-range
//!   repulsion between sheep, with attraction stronger than the dog's
//!   repulsion (rho_a > c > rho_s > h). That order is why a flock bunches up
//!   and moves off, where Boids with a predator bursts apart.
//! - King et al. 2012: a threatened sheep heads for the flock's center. So
//!   sight and spacing change with alarm: frightened sheep pack up and run
//!   for any flock in view, calm ones ease off to shoulder to shoulder and
//!   graze there (the person: "sheep should clump up more").
//! - Ginelli et al. 2015: a calm flock spreads slowly and packs back together.
//!   Here a calm sheep walks back once it strays from its local center.
//!
//! What makes it a drive and not an explosion:
//!
//! - Fear is only felt inside the flight zone, and its speed falls with
//!   distance, so the rear of a flock moves faster than the front and the
//!   flock closes up instead of fanning out.
//! - Alarm spreads by contagion at a share below one and only ever reaches a
//!   trot. A sheep ahead of the dog is carried by the velocity around it,
//!   and cohesion is added on top of that velocity, so the leading edge is
//!   held back by the flock behind it and never turns around.
//! - Walls take a sheep's speed as well as its step, so the velocity the
//!   neighbors copy is the one the sheep truly has.
//!
//! Edges (the person: "be careful about sheep getting stuck on the edges"):
//! a sheep feels a wall, tree or cairn before it touches one, by asking the
//! ground where a wider circle of itself would be pushed to. That one answer
//! covers corners, and costs nothing in the open. What the sheep meant to
//! spend going into an edge it spends going along it, at the same speed, on
//! the side it is already moving. So a sheep pressed on a wall runs along it,
//! one that must pass a sheepdog to get out of a corner keeps going past, and
//! a sheepdog coming between a sheep and a wall sends it down the wall. The
//! last resort is a committed run down the most open line.
//!
//! Calm sheep (the person: "the sheep should wander naturally a bit") each
//! keep their own slow round from the seed: a graze, a few steps along a
//! gentle curve, a stop and a turn, and again, at their own pace, so a flock
//! never moves in step. Each new walk leans toward the sheep's group the
//! farther out it stands, so a group drifts and stays a group. A lone sheep
//! walks further. No calm sheep goes through a gap by itself.
//!
//! Two grids, both flat Vecs and rebuilt from the sheep each tick. A fine one
//! finds the neighbors a sheep can touch. A coarse one keeps each cell's
//! count, mean position, mean velocity and mean alarm, so a sheep reads its
//! local center from nine cells however dense the flock is.

use std::hash::{Hash, Hasher};

use crate::fx::{self, Fx, ONE};
use crate::ground::{Ground, Rect, FOLD_FIELD};
use crate::map::{Map, Mover};
use crate::rng::Rng;
use crate::tune::{self, per_tick, Fright};

const NONE: u16 = u16::MAX;
/// Fine cells must be as wide as the widest touch: two rams at calm spacing,
/// 1.13 m. Any wider and a tight group reads too many neighbors.
const FINE_CELL: Fx = tune::milli(1250);
const COARSE_CELL: Fx = fx::from_int(8);
/// Fields a map may have; stats are kept per field in each coarse cell.
const FIELDS: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Ewe,
    Ram,
    Lamb,
}

/// Where a calm sheep is in its slow round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mood {
    /// Head down.
    Graze,
    /// A few unhurried steps along a gentle curve.
    Amble,
    /// Stopped, and turning to look somewhere else.
    Turn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sheep {
    pub x: Fx,
    pub z: Fx,
    pub vx: Fx,
    pub vz: Fx,
    /// Scales the contract's ewe.
    pub size: Fx,
    pub radius: Fx,
    /// 0 is calm and `ONE` is terrified.
    pub alarm: Fx,
    /// The velocity it is steering for, kept between its steering ticks.
    want_x: Fx,
    want_z: Fx,
    /// The way it faces, a unit vector: along its walk, and it turns slowly
    /// when it stops. The page's model reads this.
    pub face_x: Fx,
    pub face_z: Fx,
    /// How far it turns its facing each steering tick, in radians.
    bend: Fx,
    /// The index of a lamb's ewe, or `u16::MAX`.
    pub ewe: u16,
    /// Ticks left of its current `mood`.
    pub graze_ticks: u16,
    /// Ticks left of a bolt: it holds its line and does not steer.
    pub bolt: u16,
    /// Ticks left of a startle hop, for the page's model.
    pub hop: u8,
    /// An index into the fleece palette.
    pub fleece: u8,
    /// The walled field it was last in. It only sees sheep in the same one.
    pub field: u8,
    pub kind: Kind,
    pub mood: Mood,
    /// Its own pace: see `tune::TEMPO_MIN`.
    tempo: u8,
    /// In the fold and clear of its gap, for good.
    pub folded: bool,
    /// Eaten. Its slot stays, so links and indices hold.
    pub eaten: bool,
    pub belled: bool,
    /// Ticks in a row it has wanted to go and has hardly moved.
    stuck: u8,
}

impl Sheep {
    pub fn new(kind: Kind, x: Fx, z: Fx) -> Sheep {
        let size = match kind {
            Kind::Ewe => ONE,
            Kind::Ram => tune::RAM_SIZE,
            Kind::Lamb => tune::LAMB_SIZE,
        };
        Sheep {
            x,
            z,
            vx: 0,
            vz: 0,
            size,
            radius: fx::mul(tune::SHEEP_RADIUS, size),
            alarm: 0,
            want_x: 0,
            want_z: 0,
            face_x: 0,
            face_z: -ONE,
            bend: 0,
            ewe: NONE,
            graze_ticks: 0,
            bolt: 0,
            hop: 0,
            fleece: 0,
            field: 0,
            kind,
            mood: Mood::Graze,
            tempo: 100,
            folded: false,
            eaten: false,
            belled: false,
            stuck: 0,
        }
    }

    /// Head down and eating: calm, and not going anywhere.
    pub fn grazing(&self) -> bool {
        !self.eaten && self.alarm == 0 && self.mood == Mood::Graze
    }

    fn mover(&self) -> Mover {
        if self.folded {
            Mover::FoldedSheep
        } else {
            Mover::Sheep
        }
    }

    /// Height above the ground during a startle hop: a parabola over its
    /// ticks.
    pub fn hop_height(&self) -> Fx {
        let (t, whole) = (self.hop as i32, tune::HOP_TICKS as i32);
        tune::HOP_HEIGHT * 4 / (whole * whole) * t * (whole - t)
    }
}

/// Where a sheep in the fold heads for. `Ground::field` says what is in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fold {
    pub back_x: Fx,
    pub back_z: Fx,
}

/// A belled sheep, and whether a wolf has reached it yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Bell {
    sheep: u16,
    reached: bool,
}

/// A player, as the sheep see one. Wolves do not scare, but they do push.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Actor {
    pub x: Fx,
    pub z: Fx,
    pub radius: Fx,
    /// A sheepdog in the open, not crouched.
    pub scares: bool,
    /// On the ground, so it can shove sheep.
    pub solid: bool,
    /// The way it is going.
    pub vx: Fx,
    pub vz: Fx,
}

#[derive(Debug, Default, Clone, Copy)]
struct Cell {
    count: i64,
    x: i64,
    z: i64,
    vx: i64,
    vz: i64,
    alarm: i64,
}

/// What a sheep reads around itself, as offsets from it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Near {
    /// The local center, from the sheep.
    x: Fx,
    z: Fx,
    /// Mean velocity.
    vx: Fx,
    vz: Fx,
    alarm: Fx,
    /// How many sheep that makes, the one asking included.
    count: i32,
}

/// Rebuilt from the sheep at the start of every tick, so a copy of the world
/// needs none of it: a clone starts empty and two worlds compare without it.
#[derive(Debug, Default)]
struct Grid {
    min_x: Fx,
    min_z: Fx,
    fine_w: i32,
    fine_h: i32,
    coarse_w: i32,
    coarse_h: i32,
    head: Vec<u16>,
    next: Vec<u16>,
    cells: Vec<Cell>,
    used: Vec<u32>,
    /// How far from the back of the fold a folded sheep counts as there.
    settled: Fx,
}

impl Clone for Grid {
    fn clone(&self) -> Grid {
        Grid::default()
    }
}

impl PartialEq for Grid {
    fn eq(&self, _other: &Grid) -> bool {
        true
    }
}

impl Eq for Grid {}

fn cells_across(from: Fx, to: Fx, cell: Fx) -> i32 {
    ((to as i64 - from as i64) / cell as i64) as i32 + 1
}

impl Grid {
    fn fit(&mut self, bounds: &Rect, sheep: usize) {
        if !self.head.is_empty() {
            self.next.resize(sheep, NONE);
            return;
        }
        self.min_x = bounds.min_x;
        self.min_z = bounds.min_z;
        self.fine_w = cells_across(bounds.min_x, bounds.max_x, FINE_CELL);
        self.fine_h = cells_across(bounds.min_z, bounds.max_z, FINE_CELL);
        self.coarse_w = cells_across(bounds.min_x, bounds.max_x, COARSE_CELL);
        self.coarse_h = cells_across(bounds.min_z, bounds.max_z, COARSE_CELL);
        self.head = vec![NONE; (self.fine_w * self.fine_h) as usize];
        self.cells = vec![Cell::default(); (self.coarse_w * self.coarse_h * FIELDS) as usize];
        self.next = vec![NONE; sheep];
    }

    /// Cell coordinates. Anything outside the bounds lands in an edge cell,
    /// which keeps neighbors neighbors.
    fn coord(v: Fx, min: Fx, cell: Fx, cells: i32) -> i32 {
        (((v as i64 - min as i64) / cell as i64) as i32).clamp(0, cells - 1)
    }

    fn fine_at(&self, x: Fx, z: Fx) -> (i32, i32) {
        (
            Grid::coord(x, self.min_x, FINE_CELL, self.fine_w),
            Grid::coord(z, self.min_z, FINE_CELL, self.fine_h),
        )
    }

    fn coarse_at(&self, x: Fx, z: Fx) -> (i32, i32) {
        (
            Grid::coord(x, self.min_x, COARSE_CELL, self.coarse_w),
            Grid::coord(z, self.min_z, COARSE_CELL, self.coarse_h),
        )
    }

    fn coarse_index(&self, cx: i32, cz: i32, field: u8) -> usize {
        ((cz * self.coarse_w + cx) * FIELDS + (field as i32 & (FIELDS - 1))) as usize
    }

    fn build(&mut self, sheep: &[Sheep]) {
        self.head.fill(NONE);
        for &c in &self.used {
            self.cells[c as usize] = Cell::default();
        }
        self.used.clear();
        for (i, s) in sheep.iter().enumerate() {
            if s.eaten {
                continue;
            }
            let (cx, cz) = self.fine_at(s.x, s.z);
            let at = (cz * self.fine_w + cx) as usize;
            self.next[i] = self.head[at];
            self.head[at] = i as u16;
            if s.folded {
                // the flock outside does not count folded sheep as company
                continue;
            }
            let (cx, cz) = self.coarse_at(s.x, s.z);
            let at = self.coarse_index(cx, cz, s.field);
            let cell = &mut self.cells[at];
            if cell.count == 0 {
                self.used.push(at as u32);
            }
            cell.count += 1;
            cell.x += s.x as i64;
            cell.z += s.z as i64;
            cell.vx += s.vx as i64;
            cell.vz += s.vz as i64;
            cell.alarm += s.alarm as i64;
        }
        for &c in &self.used {
            let cell = &mut self.cells[c as usize];
            cell.x /= cell.count;
            cell.z /= cell.count;
            cell.vx /= cell.count;
            cell.vz /= cell.count;
            cell.alarm /= cell.count;
        }
    }

    /// The cells in reach of this point, weighted by how many sheep each
    /// holds and how near it is, so a cell fades out rather than popping as
    /// a sheep crosses a border.
    fn near(&self, x: Fx, z: Fx, field: u8, reach: Fx) -> Near {
        let (cx, cz) = self.coarse_at(x, z);
        let (mut weight, mut sum_x, mut sum_z, mut sum_vx, mut sum_vz, mut sum_alarm) = (0i64, 0, 0, 0, 0, 0);
        let mut count = 0;
        for row in (cz - 1).max(0)..=(cz + 1).min(self.coarse_h - 1) {
            for col in (cx - 1).max(0)..=(cx + 1).min(self.coarse_w - 1) {
                let cell = &self.cells[self.coarse_index(col, row, field)];
                if cell.count == 0 {
                    continue;
                }
                let (dx, dz) = (cell.x - x as i64, cell.z - z as i64);
                let d = approx_len(dx as Fx, dz as Fx);
                if d >= reach {
                    continue;
                }
                let w = cell.count * (reach - d) as i64;
                count += cell.count;
                weight += w;
                sum_x += w * dx;
                sum_z += w * dz;
                sum_vx += w * cell.vx;
                sum_vz += w * cell.vz;
                sum_alarm += w * cell.alarm;
            }
        }
        if weight == 0 {
            return Near::default();
        }
        Near {
            x: (sum_x / weight) as Fx,
            z: (sum_z / weight) as Fx,
            vx: (sum_vx / weight) as Fx,
            vz: (sum_vz / weight) as Fx,
            alarm: (sum_alarm / weight) as Fx,
            count: count as i32,
        }
    }
}

/// The length of (x, z), for steering, where the exact root would cost too
/// much a few thousand times a tick. Three chords of sqrt(1 + t^2): exact on
/// the axes and the diagonal, and never more than 1.3% long.
pub fn approx_len(x: Fx, z: Fx) -> Fx {
    let (a, b) = ((x as i64).abs(), (z as i64).abs());
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    let scaled = if lo * 4 <= hi {
        4096 * hi + 504 * lo
    } else if lo * 2 <= hi {
        3865 * hi + 1429 * lo
    } else {
        3366 * hi + 2426 * lo
    };
    (scaled >> 12) as Fx
}

/// `(x, z)` scaled to this length, or nothing if it has none.
fn with_length(x: Fx, z: Fx, length: Fx) -> (Fx, Fx) {
    let d = approx_len(x, z);
    if d == 0 {
        return (0, 0);
    }
    let scaled = |v: Fx| (v as i64 * length as i64 / d as i64) as Fx;
    (scaled(x), scaled(z))
}

/// `(x, z)` cut down to this length if it is longer.
pub fn at_most(x: Fx, z: Fx, length: Fx) -> (Fx, Fx) {
    if approx_len(x, z) > length {
        with_length(x, z, length)
    } else {
        (x, z)
    }
}

/// Dot product, in `Fx`.
fn dot(ax: Fx, az: Fx, bx: Fx, bz: Fx) -> Fx {
    ((ax as i64 * bx as i64 + az as i64 * bz as i64) >> fx::SHIFT) as Fx
}

/// How frightening a sheepdog at this distance is: nothing at the rim of the
/// flight zone, all of it inside the core.
pub fn threat(d: Fx) -> Fx {
    if d >= tune::FLIGHT_RADIUS {
        0
    } else if d <= tune::FLIGHT_CORE {
        ONE
    } else {
        fx::div(tune::FLIGHT_RADIUS - d, tune::FLIGHT_RADIUS - tune::FLIGHT_CORE)
    }
}

/// How fast a sheep leaves under this threat.
pub fn flight_speed(threat: Fx) -> Fx {
    tune::WALK_OFF + fx::mul(tune::FLEE - tune::WALK_OFF, threat)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flock {
    pub sheep: Vec<Sheep>,
    pub fold: Option<Fold>,
    /// How many sheep are folded. Only ever rises.
    pub folded: u16,
    bounds: Rect,
    /// Carrots held out this tick. Cleared after each step.
    lures: Vec<(Fx, Fx)>,
    bells: Vec<Bell>,
    rng: Rng,
    grid: Grid,
}

impl Flock {
    /// No sheep yet. `bounds` is the land they can reach; the grid covers it.
    pub fn new(bounds: Rect, fold: Option<Fold>, seed: u64) -> Flock {
        Flock {
            sheep: Vec::new(),
            fold,
            folded: 0,
            bounds,
            lures: Vec::new(),
            bells: Vec::new(),
            rng: Rng::new(seed),
            grid: Grid::default(),
        }
    }

    /// A flock on this map: `count` sheep on the map's own start spots.
    pub fn on_map(map: &Map, count: usize, seed: u64) -> Flock {
        let edge = fx::from_int(4);
        let bounds = Rect {
            min_x: -edge,
            min_z: -edge,
            max_x: map.length() + edge,
            max_z: map.width() + edge,
        };
        let (back_x, back_z) = map.fold.back;
        let mut flock = Flock::new(bounds, Some(Fold { back_x, back_z }), seed);
        flock.populate(map, &map.sheep_starts(count));
        flock
    }

    /// Add a sheep, with a pace and a first graze of its own from the seed,
    /// and return its index.
    pub fn add(&mut self, mut sheep: Sheep) -> usize {
        let angle = self.rng.next_u32() as i32;
        (sheep.face_x, sheep.face_z) = (fx::sin(angle), fx::cos(angle));
        sheep.tempo = self.rng.between(tune::TEMPO_MIN, tune::TEMPO_MAX) as u8;
        sheep.graze_ticks = self.rng.below(tune::GRAZE_MAX_TICKS as u32) as u16;
        self.sheep.push(sheep);
        self.sheep.len() - 1
    }

    /// A sheep to each spot. A lamb starts by the nearest ewe without one,
    /// so a few spots go unused.
    pub fn populate<G: Ground + ?Sized>(&mut self, ground: &G, spots: &[(Fx, Fx)]) {
        let mut mothers = vec![false; spots.len()];
        for &(x, z) in spots {
            let ewe = if self.rng.chance(1, tune::LAMB_ONE_IN) {
                self.free_ewe(x, z, &mothers)
            } else {
                None
            };
            let kind = match ewe {
                Some(_) => Kind::Lamb,
                None if self.rng.chance(1, tune::RAM_ONE_IN) => Kind::Ram,
                None => Kind::Ewe,
            };
            let mut s = Sheep::new(kind, x, z);
            s.size = match kind {
                Kind::Ewe => ONE + self.rng.between(-tune::EWE_SPREAD, tune::EWE_SPREAD),
                Kind::Ram => tune::RAM_SIZE,
                Kind::Lamb => tune::LAMB_SIZE + self.rng.between(-tune::LAMB_SPREAD, tune::LAMB_SPREAD),
            };
            s.radius = fx::mul(tune::SHEEP_RADIUS, s.size);
            s.fleece = self.fleece();
            if let Some(mother) = ewe {
                mothers[mother] = true;
                let her = &self.sheep[mother];
                s.ewe = mother as u16;
                s.fleece = her.fleece;
                let (near_x, near_z) = (her.x + tune::LAMB_LEASH, her.z);
                (s.x, s.z) = ground.push_out(near_x, near_z, s.radius);
            }
            self.add(s);
        }
    }

    /// The nearest ewe to this spot with no lamb yet.
    fn free_ewe(&self, x: Fx, z: Fx, mothers: &[bool]) -> Option<usize> {
        self.sheep
            .iter()
            .enumerate()
            .filter(|&(i, s)| s.kind == Kind::Ewe && !mothers[i])
            .map(|(i, s)| (approx_len(s.x - x, s.z - z), i))
            .filter(|&(d, _)| d < tune::LAMB_FINDS_EWE)
            .min()
            .map(|(_, i)| i)
    }

    fn fleece(&mut self) -> u8 {
        let total: u32 = tune::FLEECE_WEIGHTS.iter().sum();
        let mut roll = self.rng.below(total);
        for (k, &w) in tune::FLEECE_WEIGHTS.iter().enumerate() {
            if roll < w {
                return k as u8;
            }
            roll -= w;
        }
        0
    }

    /// What a checksum needs of the flock. The grid and the carrots held out
    /// this tick are built afresh each tick and left out.
    pub fn hash_state<H: Hasher>(&self, h: &mut H) {
        self.sheep.hash(h);
        self.folded.hash(h);
        self.bells.hash(h);
        self.rng.hash(h);
    }

    /// How many sheep are left, in or out of the fold.
    pub fn alive(&self) -> usize {
        self.sheep.iter().filter(|s| !s.eaten).count()
    }

    /// A sheepdog's bark.
    pub fn bark(&mut self, x: Fx, z: Fx) {
        self.fright(x, z, &tune::BARK);
    }

    /// A missed lunge: the sheep around it scatter and regroup.
    pub fn scatter(&mut self, x: Fx, z: Fx) {
        self.fright(x, z, &tune::SCATTER);
    }

    /// The Stampede power-up: everything near runs for seconds.
    pub fn stampede(&mut self, x: Fx, z: Fx) {
        self.fright(x, z, &tune::STAMPEDE);
    }

    /// Every sheep in reach runs from the point. Sheep near it run hardest,
    /// so a fright pushes a flock along instead of bursting it.
    fn fright(&mut self, x: Fx, z: Fx, fright: &Fright) {
        for i in 0..self.sheep.len() {
            let s = self.sheep[i];
            if s.eaten || s.folded {
                continue;
            }
            let (dx, dz) = (s.x - x, s.z - z);
            if dx.abs() >= fright.radius || dz.abs() >= fright.radius {
                continue;
            }
            let d = approx_len(dx, dz);
            if d >= fright.radius {
                continue;
            }
            let share = fx::div(d, fright.radius);
            let speed = fright.near_speed - fx::mul(fright.near_speed - fright.far_speed, share);
            let (ax, az) = self.away(i, dx, dz);
            let s = &mut self.sheep[i];
            (s.want_x, s.want_z) = with_length(ax, az, speed);
            s.bolt = s.bolt.max(fright.ticks);
            s.alarm = ONE;
        }
    }

    /// A direction away from a point this far off, with sheep on the very
    /// spot sent off by their index.
    fn away(&self, i: usize, dx: Fx, dz: Fx) -> (Fx, Fx) {
        if dx == 0 && dz == 0 {
            let angle = (i as i32).wrapping_mul(40503);
            (fx::sin(angle), fx::cos(angle))
        } else {
            (dx, dz)
        }
    }

    /// A wolf has eaten here. The sheep around it hop away a step and carry
    /// on: a clue for sharp eyes, with no alarm and so no stampede.
    pub fn flinch(&mut self, x: Fx, z: Fx) {
        for i in 0..self.sheep.len() {
            let s = self.sheep[i];
            if s.eaten || s.folded {
                continue;
            }
            let (dx, dz) = (s.x - x, s.z - z);
            if approx_len(dx, dz) >= tune::FLINCH_RADIUS {
                continue;
            }
            let (ax, az) = self.away(i, dx, dz);
            let (kick_x, kick_z) = with_length(ax, az, tune::FLINCH_KICK);
            let s = &mut self.sheep[i];
            s.vx += kick_x;
            s.vz += kick_z;
            s.hop = tune::HOP_TICKS;
        }
    }

    /// A carrot is held out here this tick.
    pub fn lure(&mut self, x: Fx, z: Fx) {
        self.lures.push((x, z));
    }

    /// A wolf ate this sheep. Its lamb goes on with the flock, and the
    /// sheep around it flinch.
    pub fn eat(&mut self, i: usize) {
        if self.sheep[i].eaten {
            return;
        }
        self.sheep[i].eaten = true;
        self.set_bell(i, false);
        for s in &mut self.sheep {
            if s.ewe == i as u16 {
                s.ewe = NONE;
            }
        }
        self.flinch(self.sheep[i].x, self.sheep[i].z);
    }

    /// Hang the Bell on a sheep, or take it off. A bell hung again can be
    /// reached again.
    pub fn set_bell(&mut self, i: usize, on: bool) {
        self.sheep[i].belled = on;
        self.bells.retain(|b| b.sheep != i as u16);
        if on {
            self.bells.push(Bell {
                sheep: i as u16,
                reached: false,
            });
        }
    }

    /// The first bell a wolf here has come within `tune::BELL_RADIUS` of,
    /// if any. Each bell answers once; the rules run the ring. With few
    /// bells, a distance check per bell is all this needs.
    pub fn reach_bell(&mut self, x: Fx, z: Fx) -> Option<u16> {
        let r = tune::BELL_RADIUS;
        for bell in &mut self.bells {
            let s = &self.sheep[bell.sheep as usize];
            let (dx, dz) = (s.x - x, s.z - z);
            if !bell.reached && dx.abs() < r && dz.abs() < r && approx_len(dx, dz) < r {
                bell.reached = true;
                return Some(bell.sheep);
            }
        }
        None
    }

    /// One tick. `tick` staggers the steering.
    pub fn step<G: Ground + ?Sized>(&mut self, ground: &G, actors: &[Actor], tick: u32) {
        self.grid.fit(&self.bounds, self.sheep.len());
        self.grid.build(&self.sheep);
        self.grid.settled = fx::mul(tune::FOLD_PACKING, fx::sqrt(fx::from_int(self.folded as i32)));
        for i in 0..self.sheep.len() {
            if (i as u32).wrapping_add(tick).is_multiple_of(tune::STEER_EVERY) && !self.sheep[i].eaten {
                self.steer(i, ground, actors);
            }
        }
        self.shoved_by(ground, actors);
        self.advance(ground);
        self.lures.clear();
    }

    fn steer<G: Ground + ?Sized>(&mut self, i: usize, ground: &G, actors: &[Actor]) {
        let mut s = self.sheep[i];
        s.field = ground.field(s.x, s.z);
        self.fold_in(&mut s, ground);
        if s.bolt > 0 {
            s.bolt = s.bolt.saturating_sub(tune::STEER_EVERY as u16);
            s.alarm = ONE;
            keep_off_edges(&mut s, ground, &Near::default(), 0);
        } else {
            s.graze_ticks = s.graze_ticks.saturating_sub(tune::STEER_EVERY as u16);
            let (want, near) = if s.folded {
                (self.steer_folded(&mut s), Near::default())
            } else {
                let sight = tune::SIGHT_CALM + fx::mul(tune::SIGHT_ALARMED - tune::SIGHT_CALM, s.alarm);
                let near = self.grid.near(s.x, s.z, s.field, sight);
                (self.steer_free(i, &mut s, &near, actors), near)
            };
            let spacing = tune::SPACING_CALM - fx::mul(tune::SPACING_CALM - tune::SPACING_TIGHT, s.alarm);
            let (shove_x, shove_z) = self.shove(i, &s, spacing);
            (s.want_x, s.want_z) = (want.0 + shove_x, want.1 + shove_z);
            let mut push = tune::EDGE_PUSH;
            if s.alarm == 0 {
                // a calm sheep shuffles aside at its own pace, and no faster
                let pace = approx_len(want.0, want.1).max(tune::GRAZE_DRIFT);
                (s.want_x, s.want_z) = at_most(s.want_x, s.want_z, pace);
                push = tune::GRAZE_DRIFT;
            }
            keep_off_edges(&mut s, ground, &near, push);
            stay_in_field(&mut s, ground);
            self.run_if_stuck(i, &mut s, ground, actors);
        }
        let speed = approx_len(s.vx, s.vz);
        if speed > tune::FACE_SPEED && (s.alarm > 0 || s.mood != Mood::Amble) {
            (s.face_x, s.face_z) = with_length(s.vx, s.vz, ONE);
        }
        self.sheep[i] = s;
    }

    /// In the fold, and clear of the gap's line, which would otherwise pop a
    /// folded sheep inward: it stays for good.
    fn fold_in<G: Ground + ?Sized>(&mut self, s: &mut Sheep, ground: &G) {
        if s.folded || s.field != FOLD_FIELD {
            return;
        }
        if ground.push_out_as(s.x, s.z, s.radius, Mover::FoldedSheep) == (s.x, s.z) {
            s.folded = true;
            s.bolt = 0;
            self.folded += 1;
        }
    }

    /// A frightened sheep that wants to go and is not going, for a moment,
    /// does what a cornered ewe does and makes a run for it.
    fn run_if_stuck<G: Ground + ?Sized>(&mut self, i: usize, s: &mut Sheep, ground: &G, actors: &[Actor]) {
        let want = approx_len(s.want_x, s.want_z);
        let held = !s.folded && s.alarm >= tune::ALERT && want >= tune::STUCK_WANT;
        if !held || approx_len(s.vx, s.vz) >= want / 4 {
            s.stuck = 0;
            return;
        }
        s.stuck = s.stuck.saturating_add(tune::STEER_EVERY as u8);
        if s.stuck >= tune::STUCK_TICKS {
            self.break_out(i, s, ground, actors);
        }
    }

    /// The run: it walks each line round itself as it would really go, walls
    /// and all, and takes the most open, leaning away from the fear and
    /// toward its flock. Even a line past the sheepdog beats staying put.
    fn break_out<G: Ground + ?Sized>(&mut self, i: usize, s: &mut Sheep, ground: &G, actors: &[Actor]) {
        let (fear_x, fear_z, _) = self.fear(i, s, actors);
        let (fear_x, fear_z) = with_length(fear_x, fear_z, ONE);
        let near = self.grid.near(s.x, s.z, s.field, tune::SIGHT_ALARMED);
        let (flock_x, flock_z) = with_length(near.x, near.z, ONE);
        let (mut best, mut run_x, mut run_z) = (Fx::MIN, 0, 0);
        for k in 0..tune::BREAK_LINES {
            let angle = fx::TURN * k / tune::BREAK_LINES;
            let (dx, dz) = (fx::sin(angle), fx::cos(angle));
            let (step_x, step_z) = (fx::mul(dx, tune::BREAK_LOOK), fx::mul(dz, tune::BREAK_LOOK));
            let (mut x, mut z) = (s.x, s.z);
            for _ in 0..tune::BREAK_STEPS {
                (x, z) = ground.push_out_as(x + step_x, z + step_z, s.radius, s.mover());
            }
            let (mx, mz) = (x - s.x, z - s.z);
            let lean = dot(dx, dz, fear_x, fear_z) + dot(dx, dz, flock_x, flock_z);
            let score = approx_len(mx, mz) + fx::mul(lean, tune::BREAK_LEAN);
            if score > best {
                (best, run_x, run_z) = (score, mx, mz);
            }
        }
        (s.want_x, s.want_z) = with_length(run_x, run_z, tune::FLEE);
        s.bolt = tune::BREAK_OUT_TICKS;
        s.stuck = 0;
    }

    fn steer_free(&mut self, i: usize, s: &mut Sheep, near: &Near, actors: &[Actor]) -> (Fx, Fx) {
        let (fear_x, fear_z, fright) = self.fear(i, s, actors);
        let fade = ONE / tune::ALARM_FADE_TICKS * tune::STEER_EVERY as Fx;
        let heard = fx::mul(near.alarm, tune::CONTAGION);
        let felt = if fright > 0 {
            tune::ALERT + fx::mul(ONE - tune::ALERT, fright)
        } else {
            0
        };
        s.alarm = (s.alarm - fade).max(felt).max(heard).max(0);

        if let Some(pull) = self.pull_of_lure(s).or_else(|| self.pull_of_ewe(s)) {
            break_off(s);
            let cap = flight_speed(fright).max(tune::FLOCK_TROT);
            return at_most(pull.0 + fear_x, pull.1 + fear_z, cap);
        }
        if s.alarm == 0 {
            return self.graze(s, near);
        }
        break_off(s);

        // the flock's own motion, then a pull to its center on top of that:
        // a front sheep slows and a rear sheep hurries, and neither turns
        // back. The pull is about a match for fear, as Strömbom's c is for
        // rho_s, and saturates near the center so a flock parts around
        // nothing.
        let follow = fx::mul(tune::ALIGNMENT, (s.alarm * 3).min(ONE));
        let pull = fx::mul(approx_len(near.x, near.z), tune::COHESION_GAIN).min(tune::COHESION_MAX);
        let (pull_x, pull_z) = with_length(near.x, near.z, fx::mul(pull, s.alarm));
        let x = fear_x + fx::mul(near.vx, follow) + pull_x;
        let z = fear_z + fx::mul(near.vz, follow) + pull_z;
        let cap = tune::GRAZE_DRIFT + fx::mul(tune::FLOCK_TROT - tune::GRAZE_DRIFT, s.alarm);
        let cap = if fright > 0 {
            cap.max(flight_speed(fright))
        } else {
            cap
        };
        let (x, z) = at_most(x, z, cap);
        // a little wander keeps a moving flock from looking like a parade
        let swing = fx::mul(approx_len(x, z), tune::WANDER);
        let (wander_x, wander_z) = (self.rng.between(-swing, swing), self.rng.between(-swing, swing));
        (x + wander_x, z + wander_z)
    }

    /// The push of every scary actor in reach, and the worst threat. Sheep
    /// past the fold's gate never get here.
    fn fear(&self, i: usize, s: &Sheep, actors: &[Actor]) -> (Fx, Fx, Fx) {
        let (mut x, mut z, mut worst) = (0, 0, 0);
        let r = tune::FLIGHT_RADIUS;
        for a in actors {
            let (dx, dz) = (s.x - a.x, s.z - a.z);
            if !a.scares || dx.abs() >= r || dz.abs() >= r {
                continue;
            }
            let d = approx_len(dx, dz);
            let t = threat(d);
            if t == 0 {
                continue;
            }
            let (ax, az) = self.away(i, dx, dz);
            let (ax, az) = with_length(ax, az, t);
            x += ax;
            z += az;
            worst = worst.max(t);
        }
        if worst == 0 {
            return (0, 0, 0);
        }
        let (x, z) = with_length(x, z, flight_speed(worst));
        (x, z, worst)
    }

    /// The carrot's pull, when one is in reach.
    fn pull_of_lure(&self, s: &Sheep) -> Option<(Fx, Fx)> {
        let mut best: Option<(Fx, Fx, Fx)> = None;
        for &(x, z) in &self.lures {
            let (dx, dz) = (x - s.x, z - s.z);
            let d = approx_len(dx, dz);
            if d < tune::LURE_RADIUS && best.is_none_or(|b| d < b.2) {
                best = Some((dx, dz, d));
            }
        }
        let (dx, dz, d) = best?;
        if d <= tune::LURE_CLOSE {
            return Some((0, 0));
        }
        Some(with_length(dx, dz, tune::FLOCK_TROT))
    }

    /// A lamb's pull to its ewe.
    fn pull_of_ewe(&self, s: &Sheep) -> Option<(Fx, Fx)> {
        if s.ewe == NONE {
            return None;
        }
        let ewe = &self.sheep[s.ewe as usize];
        let (dx, dz) = (ewe.x - s.x, ewe.z - s.z);
        let d = approx_len(dx, dz);
        if d <= tune::LAMB_LEASH {
            return None;
        }
        let fastest = if s.alarm == 0 {
            tune::FLOCK_TROT
        } else {
            tune::FLEE
        };
        let speed = fx::mul(d - tune::LAMB_LEASH, tune::LAMB_GAIN).min(fastest);
        Some(with_length(dx, dz, speed))
    }

    /// A calm sheep's slow round. One walking away from a group it has
    /// strayed from stops early, and so turns back.
    fn graze(&mut self, s: &mut Sheep, near: &Near) -> (Fx, Fx) {
        let leaving = dot(s.face_x, s.face_z, near.x, near.z) < 0;
        if s.mood == Mood::Amble && leaving && approx_len(near.x, near.z) > tune::COMFORT {
            s.graze_ticks = 0;
        }
        if s.graze_ticks == 0 {
            self.next_mood(s, near);
        }
        match s.mood {
            Mood::Graze => (0, 0),
            Mood::Turn => {
                turn(s);
                (0, 0)
            }
            Mood::Amble => {
                turn(s);
                (
                    fx::mul(s.face_x, tune::GRAZE_DRIFT),
                    fx::mul(s.face_z, tune::GRAZE_DRIFT),
                )
            }
        }
    }

    /// On round: from a graze to a walk the way it faces, from a walk to a
    /// turn toward its next walk, and from the turn to a graze.
    fn next_mood(&mut self, s: &mut Sheep, near: &Near) {
        let tempo = s.tempo as i32;
        match s.mood {
            Mood::Graze => {
                let mut ticks = self.rng.between(tune::AMBLE_MIN_TICKS, tune::AMBLE_MAX_TICKS) * 100 / tempo;
                if near.count <= 1 {
                    ticks = ticks * tune::LONE_WALK_PERCENT / 100;
                }
                s.mood = Mood::Amble;
                s.bend = self.rng.between(-tune::AMBLE_BEND, tune::AMBLE_BEND);
                s.graze_ticks = ticks as u16;
            }
            Mood::Amble => {
                let (aim_x, aim_z) = self.next_aim(near);
                let across = fx::mul(s.face_x, aim_z) - fx::mul(s.face_z, aim_x);
                let off = ONE - dot(s.face_x, s.face_z, aim_x, aim_z);
                s.mood = Mood::Turn;
                s.bend = if across >= 0 {
                    tune::TURN_BEND
                } else {
                    -tune::TURN_BEND
                };
                s.graze_ticks = (fx::floor(fx::mul(off, fx::from_int(tune::HALF_TURN_TICKS))) / 2) as u16;
            }
            Mood::Turn => {
                s.mood = Mood::Graze;
                let ticks = self.rng.between(tune::GRAZE_MIN_TICKS, tune::GRAZE_MAX_TICKS) * tempo / 100;
                s.graze_ticks = ticks as u16;
            }
        }
    }

    /// Which way a sheep means to walk next: anywhere, leaning in toward the
    /// middle of its group the farther out it stands.
    fn next_aim(&mut self, near: &Near) -> (Fx, Fx) {
        let angle = self.rng.next_u32() as i32;
        let out = fx::div(approx_len(near.x, near.z), tune::COMFORT).min(ONE);
        let (in_x, in_z) = with_length(near.x, near.z, out * tune::GROUP_LEAN);
        with_length(fx::sin(angle) + in_x, fx::cos(angle) + in_z, ONE)
    }

    /// A folded sheep walks to the back until it meets the sheep already
    /// there, then keeps its round, leaning to the back. Nothing outside
    /// reaches it now.
    fn steer_folded(&mut self, s: &mut Sheep) -> (Fx, Fx) {
        let Some(fold) = self.fold else {
            return (0, 0);
        };
        s.alarm = 0;
        s.ewe = NONE;
        let (dx, dz) = (fold.back_x - s.x, fold.back_z - s.z);
        if approx_len(dx, dz) > self.grid.settled {
            return with_length(dx, dz, tune::FOLD_AMBLE);
        }
        let near = Near {
            x: dx,
            z: dz,
            count: self.folded as i32,
            ..Near::default()
        };
        self.graze(s, &near)
    }

    /// Sliding out of the neighbors it overlaps at this spacing.
    fn shove(&self, i: usize, s: &Sheep, spacing: Fx) -> (Fx, Fx) {
        let (cx, cz) = self.grid.fine_at(s.x, s.z);
        let (mut x, mut z) = (0, 0);
        for row in (cz - 1).max(0)..=(cz + 1).min(self.grid.fine_h - 1) {
            for col in (cx - 1).max(0)..=(cx + 1).min(self.grid.fine_w - 1) {
                let mut j = self.grid.head[(row * self.grid.fine_w + col) as usize];
                while j != NONE {
                    let o = &self.sheep[j as usize];
                    let at = j as usize;
                    j = self.grid.next[at];
                    if at == i {
                        continue;
                    }
                    let reach = fx::mul(s.radius + o.radius, spacing);
                    let (dx, dz) = (s.x - o.x, s.z - o.z);
                    if dx.abs() >= reach || dz.abs() >= reach {
                        continue;
                    }
                    let d = approx_len(dx, dz);
                    if d >= reach {
                        continue;
                    }
                    let (ax, az) = self.away(i, dx, dz);
                    let (ax, az) = with_length(ax, az, fx::div(reach - d, reach));
                    x += ax;
                    z += az;
                }
            }
        }
        at_most(fx::mul(x, tune::SHOVE), fx::mul(z, tune::SHOVE), tune::SHOVE_MAX)
    }

    /// Each sheep a player is touching, and how deep: `(index, dx, dz, d,
    /// reach)` to `seen`. Fleece gives, so that is nearer than their radii
    /// added. A folded sheep is out of reach.
    fn touching(&self, a: &Actor, mut seen: impl FnMut(usize, Fx, Fx, Fx, Fx)) {
        let (cx, cz) = self.grid.fine_at(a.x, a.z);
        for row in (cz - 1).max(0)..=(cz + 1).min(self.grid.fine_h - 1) {
            for col in (cx - 1).max(0)..=(cx + 1).min(self.grid.fine_w - 1) {
                let mut j = self.grid.head[(row * self.grid.fine_w + col) as usize];
                while j != NONE {
                    let at = j as usize;
                    j = self.grid.next[at];
                    let s = &self.sheep[at];
                    let reach = fx::mul(a.radius + s.radius, tune::BODY_TOUCH);
                    let (dx, dz) = (s.x - a.x, s.z - a.z);
                    if s.folded || dx.abs() >= reach || dz.abs() >= reach {
                        continue;
                    }
                    let d = approx_len(dx, dz);
                    if d < reach {
                        seen(at, dx, dz, d, reach);
                    }
                }
            }
        }
    }

    /// Players part the sheep they touch: a player's step is slowed and not
    /// stopped, and a sheep moves no faster than `tune::PUSH` for it. The
    /// sheep around close in behind by themselves. None of this is fear, so
    /// no alarm comes of it (FL-6).
    fn shoved_by<G: Ground + ?Sized>(&mut self, ground: &G, actors: &[Actor]) {
        let limit = per_tick(tune::PUSH);
        let mut touched: Vec<(usize, Fx, Fx, Fx, Fx)> = Vec::new();
        for a in actors.iter().filter(|a| a.solid) {
            touched.clear();
            self.touching(a, |at, dx, dz, d, reach| touched.push((at, dx, dz, d, reach)));
            for &(at, dx, dz, d, reach) in &touched {
                let s = self.sheep[at];
                let (ax, az) = self.parting(a, at, dx, dz);
                let (px, pz) = with_length(ax, az, (reach - d).min(limit));
                (self.sheep[at].x, self.sheep[at].z) =
                    ground.push_out_as(s.x + px, s.z + pz, s.radius, s.mover());
            }
        }
    }

    /// Which way a touched sheep gives. Away from the player, and if it is
    /// ahead of the player, aside as well: to the side it is already on, or
    /// by its index if it stands dead ahead.
    fn parting(&self, a: &Actor, at: usize, dx: Fx, dz: Fx) -> (Fx, Fx) {
        let (ax, az) = self.away(at, dx, dz);
        let (ax, az) = with_length(ax, az, ONE);
        let (hx, hz) = with_length(a.vx, a.vz, ONE);
        let ahead = dot(ax, az, hx, hz);
        if ahead <= 0 {
            return (ax, az);
        }
        let (mut sx, mut sz) = (ax - fx::mul(hx, ahead), az - fx::mul(hz, ahead));
        if approx_len(sx, sz) < tune::milli(50) {
            let side = if at & 1 == 0 { ONE } else { -ONE };
            (sx, sz) = (fx::mul(-hz, side), fx::mul(hx, side));
        }
        let (sx, sz) = with_length(sx, sz, tune::PART);
        (ax + sx, az + sz)
    }

    /// The share of a player's step that the sheep it is pressing into take
    /// from it. Standing still it presses into none, so sheep never move a
    /// player. The world reads this after the flock's step, when the sheep
    /// have had their chance to give way.
    pub fn crowding(&self, a: &Actor) -> Fx {
        if !a.solid {
            return 0;
        }
        let mut pressed = 0;
        self.touching(a, |_, dx, dz, _, _| {
            if dot(dx, dz, a.vx, a.vz) > 0 {
                pressed += 1;
            }
        });
        (tune::CROWD_DRAG * pressed).min(tune::CROWD_MOST)
    }

    fn advance<G: Ground + ?Sized>(&mut self, ground: &G) {
        for s in &mut self.sheep {
            if s.eaten {
                continue;
            }
            s.vx += (s.want_x - s.vx) / tune::INERTIA_TICKS;
            s.vz += (s.want_z - s.vz) / tune::INERTIA_TICKS;
            s.hop = s.hop.saturating_sub(1);
            let (dx, dz) = (per_tick(s.vx), per_tick(s.vz));
            if dx == 0 && dz == 0 {
                continue;
            }
            let (wanted_x, wanted_z) = (s.x + dx, s.z + dz);
            let (x, z) = ground.push_out_as(wanted_x, wanted_z, s.radius, s.mover());
            if (x, z) != (wanted_x, wanted_z) {
                // whatever took some of the step takes the same speed, so
                // sheep copy what a neighbor really does; it adds none
                let (mx, mz) = (
                    (x - s.x) * tune::TICKS_PER_SECOND,
                    (z - s.z) * tune::TICKS_PER_SECOND,
                );
                (s.vx, s.vz) = at_most(mx, mz, approx_len(s.vx, s.vz));
            }
            (s.x, s.z) = (x, z);
        }
    }
}

/// Something else has its attention: whatever it was at is over, and it
/// starts again from a graze.
fn break_off(s: &mut Sheep) {
    s.mood = Mood::Turn;
    s.graze_ticks = 0;
}

/// One steering tick of a sheep's facing, by its `bend`.
fn turn(s: &mut Sheep) {
    let (x, z) = (s.face_x, s.face_z);
    (s.face_x, s.face_z) = with_length(x - fx::mul(z, s.bend), z + fx::mul(x, s.bend), ONE);
}

/// Feel for walls, trees and cairns before touching them. The ground says
/// where a circle wider by the sense would be pushed to: nowhere in the open,
/// and straight out of a wall or a corner near one. Of whatever the sheep
/// means to spend going into the edge, the nearer it is the more goes along
/// the edge instead, so it loses no speed. It goes toward its flock if it
/// has one, which funnels a flock into a gap, and otherwise on the side the
/// sheep is already moving, so a run along a wall carries on past whatever
/// waits there. `push` eases it off the edge as well; a bolt, which holds its
/// speed, takes none.
fn keep_off_edges<G: Ground + ?Sized>(s: &mut Sheep, ground: &G, near: &Near, push: Fx) {
    let sense = s.radius + tune::EDGE_SENSE;
    let (x, z) = ground.push_out_as(s.x, s.z, sense, s.mover());
    let (ex, ez) = (x - s.x, z - s.z);
    if ex == 0 && ez == 0 {
        return;
    }
    let close = fx::div(approx_len(ex, ez), tune::EDGE_SENSE - tune::EDGE_CLEAR).min(ONE);
    let (nx, nz) = with_length(ex, ez, ONE);
    let into = -dot(s.want_x, s.want_z, nx, nz);
    if into > 0 {
        let way_x = s.want_x + s.vx + near.x * tune::EDGE_TO_FLOCK;
        let way_z = s.want_z + s.vz + near.z * tune::EDGE_TO_FLOCK;
        let (tx, tz) = if dot(way_x, way_z, -nz, nx) >= 0 {
            (-nz, nx)
        } else {
            (nz, -nx)
        };
        let moved = fx::mul(into, close);
        s.want_x += fx::mul(nx + tx, moved);
        s.want_z += fx::mul(nz + tz, moved);
    }
    let off = fx::mul(close, push);
    s.want_x += fx::mul(nx, off);
    s.want_z += fx::mul(nz, off);
}

/// A calm sheep does not go through a gap by itself: at the line between two
/// fields it stops and turns. A lamb goes where its ewe goes.
fn stay_in_field<G: Ground + ?Sized>(s: &mut Sheep, ground: &G) {
    if s.alarm > 0 || s.ewe != NONE || s.folded || (s.want_x == 0 && s.want_z == 0) {
        return;
    }
    let (ahead_x, ahead_z) = with_length(s.want_x, s.want_z, tune::FIELD_LOOK);
    if ground.field(s.x + ahead_x, s.z + ahead_z) != s.field {
        (s.want_x, s.want_z) = (0, 0);
        if s.mood == Mood::Amble {
            s.graze_ticks = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{from_int, ratio};
    use crate::ground::{FlatField, WalledField};
    use crate::tune::{milli, seconds};

    const WIDE: Rect = Rect {
        min_x: from_int(-100),
        min_z: from_int(-100),
        max_x: from_int(100),
        max_z: from_int(100),
    };

    /// Sheep with their heads down for a long while, so a test sees only
    /// what it is about.
    fn flock(sheep: &[(i32, i32)]) -> Flock {
        let mut f = Flock::new(WIDE, None, 1);
        for &(x, z) in sheep {
            let i = f.add(Sheep::new(Kind::Ewe, from_int(x), from_int(z)));
            f.sheep[i].graze_ticks = 60_000;
        }
        f
    }

    /// One calm sheep at the start of its round, at the usual pace.
    fn walker(x: i32, z: i32) -> Flock {
        let mut f = flock(&[(x, z)]);
        f.sheep[0].graze_ticks = 0;
        f.sheep[0].tempo = 100;
        f
    }

    fn dog_at(x: i32, z: i32) -> Actor {
        Actor {
            x: from_int(x),
            z: from_int(z),
            radius: tune::SHEEPDOG.radius,
            scares: true,
            solid: true,
            ..Actor::default()
        }
    }

    fn run<G: Ground>(f: &mut Flock, ground: &G, actors: &[Actor], ticks: i32) {
        for t in 0..ticks {
            f.step(ground, actors, t as u32);
        }
    }

    fn apart(a: &Sheep, b: &Sheep) -> Fx {
        fx::length(a.x - b.x, a.z - b.z)
    }

    #[test]
    fn approx_len_is_exact_on_the_axes_and_a_little_long_between() {
        assert_eq!(approx_len(from_int(5), 0), from_int(5));
        assert_eq!(approx_len(0, from_int(-7)), from_int(7));
        // (3, 3) is 4.2426, and 3366 + 2426 over 4096 is 1.41406 times 3
        assert_eq!(
            approx_len(from_int(3), from_int(3)),
            ((from_int(3) as i64 * 5792) >> 12) as Fx
        );
        // (4, 3) is 5, and (3366 * 4 + 2426 * 3) / 4096 = 5.064, 1.3% long
        let long = approx_len(from_int(4), from_int(3));
        assert!(long > from_int(5) && long < milli(5070), "{long}");
    }

    #[test]
    fn threat_runs_from_nothing_at_the_rim_to_all_of_it_in_the_core() {
        assert_eq!(threat(from_int(12)), 0);
        assert_eq!(threat(from_int(9)), 0);
        assert_eq!(threat(from_int(6)), fx::HALF);
        assert_eq!(threat(from_int(3)), ONE);
        assert_eq!(threat(from_int(1)), ONE);
    }

    #[test]
    fn flight_speed_runs_from_walking_off_to_the_flee_speed() {
        assert_eq!(flight_speed(0), tune::WALK_OFF);
        assert_eq!(flight_speed(ONE), tune::FLEE);
        // halfway between 2 and 10 m/s
        assert_eq!(flight_speed(fx::HALF), milli(6000));
    }

    #[test]
    fn the_local_center_reads_the_mean_of_the_sheep_around() {
        // cells start at -100 and run 8 m, so -3 and 3 share one cell, whose
        // mean is 3 m east of the first, and 6 lies in the next
        let mut f = flock(&[(-3, 2), (3, 2), (6, 2)]);
        f.grid.fit(&f.bounds, 3);
        f.grid.build(&f.sheep);
        let near = f.grid.near(from_int(-3), from_int(2), 0, from_int(7));
        // that cell counts 2 * (7 - 3) = 8, and the next, 9 m off, is too far
        assert_eq!((near.x, near.z), (from_int(3), 0));
        // nearer cells count for more: from x = 0 the first cell's mean is on
        // the spot and weighs 2 * 7 = 14, the sheep at 6 weighs 1 * (7 - 6),
        // so the center is 6 * 1 / 15 = 0.4 m east
        let near = f.grid.near(0, from_int(2), 0, from_int(7));
        assert_eq!(near.x, milli(400));
        // nothing within a meter of a spot 10 m off
        let empty = f.grid.near(from_int(14), from_int(2), 0, from_int(1));
        assert_eq!(empty, Near::default());
    }

    #[test]
    fn sheep_on_either_side_of_a_wall_do_not_count_as_company() {
        let mut f = flock(&[(2, 2), (6, 2)]);
        f.sheep[1].field = 1;
        f.grid.fit(&f.bounds, 2);
        f.grid.build(&f.sheep);
        let near = f.grid.near(from_int(2), from_int(2), 0, from_int(7));
        assert_eq!((near.x, near.z), (0, 0), "only itself on its side");
        let other = f.grid.near(from_int(2), from_int(2), 1, from_int(7));
        assert_eq!((other.x, other.z), (from_int(4), 0));
    }

    #[test]
    fn a_sheep_outside_the_bounds_still_meets_its_neighbors() {
        // half a meter apart and 50 m past the edge of a small grid
        let bounds = Rect {
            min_x: 0,
            min_z: 0,
            max_x: from_int(10),
            max_z: from_int(10),
        };
        let mut f = Flock::new(bounds, None, 1);
        f.add(Sheep::new(Kind::Ewe, from_int(60), 0));
        f.add(Sheep::new(Kind::Ewe, from_int(60) + fx::HALF, 0));
        run(&mut f, &FlatField, &[], 90);
        assert!(
            f.sheep[1].x - f.sheep[0].x > ratio(9, 10),
            "{}",
            f.sheep[1].x - f.sheep[0].x
        );
    }

    #[test]
    fn a_sheep_walks_out_of_a_flight_zone_and_stops_past_its_rim() {
        // 6 m from the dog: half the threat, so 6 m/s to the rim at 9 m,
        // and a coast of about 3 m, twice its old one, as the alarm fades. Twelve ticks of
        // following a share of 1/8 reach 1 - (7/8)^12 = 0.8 of it: 4.8 m/s
        let mut f = flock(&[(0, 0)]);
        run(&mut f, &FlatField, &[dog_at(-6, 0)], 12);
        let s = f.sheep[0];
        assert!(s.vx > milli(4000) && s.vx < milli(6100), "{}", s.vx);
        // 0.6 + 0.4 * 0.5 at first, fading a little as it leaves
        assert!(s.alarm > milli(750) && s.alarm <= milli(800), "{}", s.alarm);
        run(&mut f, &FlatField, &[dog_at(-6, 0)], seconds(8));
        let s = f.sheep[0];
        assert!(s.x > from_int(3) && s.x < from_int(7), "{}", s.x);
        assert!(s.z.abs() < from_int(1), "{}", s.z);
        assert_eq!(s.alarm, 0);
    }

    #[test]
    fn a_sheepdog_that_does_not_scare_leaves_sheep_calm() {
        let mut f = flock(&[(0, 0)]);
        let crouched = Actor {
            scares: false,
            ..dog_at(-3, 0)
        };
        run(&mut f, &FlatField, &[crouched], 60);
        assert_eq!(f.sheep[0].alarm, 0);
    }

    #[test]
    fn a_dog_at_the_rim_scares_nothing_and_just_inside_it_alerts() {
        let mut f = flock(&[(0, 0)]);
        run(&mut f, &FlatField, &[dog_at(-9, 0)], 3);
        assert_eq!(f.sheep[0].alarm, 0);
        run(&mut f, &FlatField, &[dog_at(-8, 0)], 3);
        // a sixth of the threat, on top of the floor of 0.6
        assert!(f.sheep[0].alarm >= tune::ALERT, "{}", f.sheep[0].alarm);
    }

    #[test]
    fn two_sheepdogs_on_either_side_hold_a_sheep_between_them() {
        let mut f = flock(&[(0, 0)]);
        run(&mut f, &FlatField, &[dog_at(-5, 0), dog_at(5, 0)], 30);
        assert!(f.sheep[0].x.abs() < milli(300), "{}", f.sheep[0].x);
        assert!(f.sheep[0].alarm > milli(600));
    }

    #[test]
    fn alarm_spreads_at_less_than_its_own_strength_and_fades() {
        // one coarse cell: the mean of 1 and 0 is a half, before anything
        // moves
        let mut f = flock(&[(-3, 2), (-1, 2)]);
        f.sheep[0].alarm = ONE;
        f.grid.fit(&f.bounds, 2);
        f.grid.build(&f.sheep);
        let near = f.grid.near(from_int(-1), from_int(2), 0, tune::SIGHT_CALM);
        assert_eq!(near.alarm, fx::HALF);
        // the second sheep steers on tick 2, after the first has faded one
        // step of 65536 / 180 * 3 = 1092: half of 64444, times 0.8, is 25777
        run(&mut f, &FlatField, &[], 3);
        assert!((f.sheep[1].alarm - 25777).abs() < 4, "{}", f.sheep[1].alarm);
        run(&mut f, &FlatField, &[], tune::ALARM_FADE_TICKS + seconds(6));
        assert_eq!((f.sheep[0].alarm, f.sheep[1].alarm), (0, 0));
    }

    #[test]
    fn a_stampede_runs_sheep_harder_than_a_bark_or_their_own_flight() {
        // at 3 m of its 15 m reach: 11 - 2 * 0.2 = 10.6 m/s, past a flee of 10
        let mut f = flock(&[(3, 0)]);
        f.stampede(0, 0);
        assert!(
            (f.sheep[0].want_x - milli(10_600)).abs() < 8,
            "{}",
            f.sheep[0].want_x
        );
        // and at 12 m, 11 - 2 * 0.8 = 9.4 m/s, past a bark at that range, 6.1
        let mut f = flock(&[(12, 0)]);
        f.stampede(0, 0);
        assert!(
            (f.sheep[0].want_x - milli(9400)).abs() < 8,
            "{}",
            f.sheep[0].want_x
        );
    }

    #[test]
    fn a_bark_reaches_past_the_flight_zone_and_sends_a_nearer_sheep_harder() {
        let mut f = flock(&[(4, 0), (12, 0), (17, 0)]);
        f.bark(0, 0);
        // at a quarter of the reach: 10 - 5.2 * 0.25 = 8.7 m/s;
        // at three quarters: 10 - 5.2 * 0.75 = 6.1 m/s
        assert!(
            (f.sheep[0].want_x - milli(8700)).abs() < 8,
            "{}",
            f.sheep[0].want_x
        );
        assert!(
            (f.sheep[1].want_x - milli(6100)).abs() < 8,
            "{}",
            f.sheep[1].want_x
        );
        assert_eq!(f.sheep[2].bolt, 0, "past its 16 m reach");
        assert_eq!(f.sheep[0].bolt, tune::BARK.ticks);
    }

    #[test]
    fn a_missed_lunge_scatters_the_sheep_near_it_and_no_farther_than_its_reach() {
        let mut f = flock(&[(-3, 0), (3, 0), (10, 0)]);
        f.scatter(0, 0);
        assert_eq!(f.sheep[2].bolt, 0, "past its 8 m reach");
        run(&mut f, &FlatField, &[], 30);
        assert!(f.sheep[0].x < from_int(-4), "{}", f.sheep[0].x);
        assert!(f.sheep[1].x > from_int(4), "{}", f.sheep[1].x);
    }

    #[test]
    fn a_stampede_reaches_wider_than_a_scatter_and_runs_for_seconds() {
        let mut f = flock(&[(12, 0)]);
        f.scatter(0, 0);
        assert_eq!(f.sheep[0].bolt, 0);
        f.stampede(0, 0);
        // 5.5 - 1.0 * 0.8 = 4.7 m/s for 3.5 s is 16.4 m, less about half a
        // meter getting up to speed
        run(&mut f, &FlatField, &[], tune::STAMPEDE.ticks as i32);
        assert!(f.sheep[0].x > from_int(12 + 15), "{}", f.sheep[0].x);
        assert!(f.sheep[0].vx > milli(4000), "still running: {}", f.sheep[0].vx);
        run(&mut f, &FlatField, &[], tune::ALARM_FADE_TICKS + seconds(3));
        assert_eq!(f.sheep[0].alarm, 0, "and calm by then");
    }

    #[test]
    fn a_bolting_sheep_holds_its_line_and_ignores_the_flock() {
        // a sheep beside the point bolts away from a group it would
        // otherwise stick with
        let mut f = flock(&[(1, 0), (-1, 0), (-1, 1), (-1, -1)]);
        f.scatter(0, 0);
        let want = f.sheep[0].want_x;
        run(&mut f, &FlatField, &[], 30);
        assert_eq!(f.sheep[0].want_x, want);
        assert!(f.sheep[0].bolt > 0);
    }

    #[test]
    fn a_flinch_is_a_hop_and_a_step_and_leaves_no_alarm() {
        let mut f = flock(&[(2, 0), (20, 0)]);
        f.flinch(0, 0);
        assert_eq!(f.sheep[0].hop, tune::HOP_TICKS);
        assert_eq!(f.sheep[1].hop, 0, "past its 5 m reach");
        // 1.5 m/s fading by an eighth a tick covers 1.5 * 8 / 60 = 0.2 m
        run(&mut f, &FlatField, &[], 60);
        let s = f.sheep[0];
        assert!((s.x - from_int(2) - milli(200)).abs() < milli(30), "{}", s.x);
        assert_eq!((s.alarm, s.bolt, s.hop), (0, 0, 0));
    }

    #[test]
    fn a_hop_rises_to_its_height_at_the_middle_and_lands() {
        let mut s = Sheep::new(Kind::Ewe, 0, 0);
        assert_eq!(s.hop_height(), 0);
        s.hop = tune::HOP_TICKS / 2;
        assert!(
            (s.hop_height() - tune::HOP_HEIGHT).abs() < milli(5),
            "{}",
            s.hop_height()
        );
    }

    #[test]
    fn eating_a_sheep_takes_it_out_frees_its_lamb_and_makes_neighbors_flinch() {
        let mut f = flock(&[(0, 0), (2, 0), (30, 30)]);
        let lamb = f.add(Sheep::new(Kind::Lamb, from_int(30), from_int(31)));
        f.sheep[lamb].ewe = 0;
        f.set_bell(0, true);
        f.eat(0);
        assert!(f.sheep[0].eaten);
        assert_eq!(f.alive(), 3);
        assert_eq!(f.sheep[lamb].ewe, NONE);
        assert_eq!(f.sheep[1].hop, tune::HOP_TICKS);
        assert_eq!(f.sheep[2].hop, 0);
        assert!(f.bells.is_empty());
        // and nothing bumps into it any more
        f.eat(0);
        run(&mut f, &FlatField, &[], 10);
        assert_eq!(f.alive(), 3);
    }

    #[test]
    fn players_shift_a_sheep_at_a_walking_pace_and_no_faster() {
        // a wolf overlapping a ewe by 0.37 m (0.42 + 0.45 - 0.5)
        let mut f = flock(&[(0, 0)]);
        let wolf = Actor {
            x: -milli(500),
            z: 0,
            radius: tune::WOLF.radius,
            scares: false,
            solid: true,
            ..Actor::default()
        };
        run(&mut f, &FlatField, &[wolf], 1);
        // 4.5 m/s is 294912 units a second, 4915 a tick
        assert_eq!(f.sheep[0].x, 4915);
        run(&mut f, &FlatField, &[wolf], 30);
        // and it stops once they only touch, at 0.8 of 0.87 m: 0.696 m
        let touching = wolf.x + milli(696);
        assert!((f.sheep[0].x - touching).abs() < 20, "{}", f.sheep[0].x);
    }

    #[test]
    fn a_player_in_the_air_pushes_nothing() {
        let mut f = flock(&[(0, 0)]);
        let wolf = Actor {
            x: milli(300),
            z: 0,
            radius: tune::WOLF.radius,
            scares: false,
            solid: false,
            ..Actor::default()
        };
        run(&mut f, &FlatField, &[wolf], 10);
        assert_eq!((f.sheep[0].x, f.sheep[0].z), (0, 0));
    }

    /// A wolf at this spot walking this way.
    fn wolf_going(x: Fx, z: Fx, vx: Fx, vz: Fx) -> Actor {
        Actor {
            x,
            z,
            radius: tune::WOLF.radius,
            solid: true,
            vx,
            vz,
            ..Actor::default()
        }
    }

    #[test]
    fn a_sheep_touched_ahead_of_a_walking_wolf_goes_aside_as_well_as_away() {
        // a sheep ahead of a wolf going along +z and off to one side goes
        // that side by more than it goes ahead
        let mut f = flock(&[(0, 0)]);
        f.sheep[0].x = milli(300);
        f.sheep[0].z = milli(400);
        run(&mut f, &FlatField, &[wolf_going(0, 0, 0, from_int(8))], 1);
        let (x, z) = (f.sheep[0].x - milli(300), f.sheep[0].z - milli(400));
        assert!(x > 0 && z > 0, "{x} {z}");
        assert!(x > z, "aside {x} ahead {z}");
    }

    #[test]
    fn a_sheep_dead_ahead_goes_to_a_side_by_its_index() {
        let mut f = flock(&[(0, 0), (0, 0)]);
        f.sheep[0].z = milli(400);
        f.sheep[1].z = milli(400);
        run(&mut f, &FlatField, &[wolf_going(0, 0, 0, from_int(8))], 1);
        // a side of +z is +x or -x, and the first sheep's is -x
        assert!(
            f.sheep[0].x < 0 && f.sheep[1].x > 0,
            "{} {}",
            f.sheep[0].x,
            f.sheep[1].x
        );
    }

    #[test]
    fn sheep_ahead_take_a_share_of_a_step_and_never_all_of_it() {
        // one sheep ahead takes a quarter, two take half, and six can take
        // no more than three fifths
        let spots = [
            (milli(100), milli(500)),
            (-milli(100), milli(500)),
            (milli(300), milli(400)),
            (-milli(300), milli(400)),
            (milli(450), milli(200)),
            (-milli(450), milli(200)),
        ];
        let wolf = wolf_going(0, 0, 0, from_int(8));
        let mut taken = Vec::new();
        for n in [1, 2, 6] {
            let mut f = flock(&vec![(0, 0); n]);
            for (s, &(x, z)) in f.sheep.iter_mut().zip(&spots) {
                (s.x, s.z) = (x, z);
            }
            f.grid.fit(&WIDE, n);
            f.grid.build(&f.sheep);
            taken.push(f.crowding(&wolf));
        }
        assert_eq!(taken, [milli(250), milli(500), milli(600)]);
    }

    #[test]
    fn a_sheep_behind_a_body_or_beside_a_standing_one_takes_none_of_its_step() {
        let mut f = flock(&[(0, 0)]);
        f.sheep[0].z = -milli(400);
        f.grid.fit(&WIDE, 1);
        f.grid.build(&f.sheep);
        assert_eq!(f.crowding(&wolf_going(0, 0, 0, from_int(8))), 0);
        assert_eq!(f.crowding(&wolf_going(0, 0, 0, 0)), 0);
        let above = Actor {
            solid: false,
            ..wolf_going(0, -milli(800), 0, from_int(8))
        };
        assert_eq!(f.crowding(&above), 0);
        assert_eq!(
            f.crowding(&wolf_going(0, -milli(800), 0, from_int(8))),
            tune::CROWD_DRAG
        );
    }

    #[test]
    fn being_crowded_by_a_body_is_not_fear() {
        // FL-6: a wolf walking into a sheep moves it and frightens nothing
        let mut f = flock(&[(0, 0), (1, 0), (0, 1)]);
        for t in 0..60 {
            let z = from_int(-1) + milli(100) * t;
            let wolf = wolf_going(milli(300), z, 0, from_int(6));
            f.step(&FlatField, &[wolf], t as u32);
            assert!(f.sheep.iter().all(|s| s.alarm == 0 && s.bolt == 0));
        }
        assert_ne!((f.sheep[0].x, f.sheep[0].z), (0, 0));
    }

    #[test]
    fn two_sheep_overlap_softly_and_part_at_a_gentle_pace() {
        // two ewes 0.2 m apart: at calm spacing they want 1.15 * 0.9 = 1.035 m
        let mut f = flock(&[(0, 0), (0, 0)]);
        f.sheep[1].x = milli(200);
        let mut last = apart(&f.sheep[0], &f.sheep[1]);
        for t in 0..300 {
            f.step(&FlatField, &[], t);
            let d = apart(&f.sheep[0], &f.sheep[1]);
            // calm, so never faster than 0.8 m/s each way, 0.0267 m a tick
            assert!(d - last <= milli(27), "{d} {last}");
            last = d;
        }
        // and each carries on a little at 0.8 m/s as it stops
        assert!(last > milli(950) && last < milli(1200), "{last}");
    }

    #[test]
    fn sheep_on_one_spot_are_parted_by_their_index() {
        // a bark on the very spot gives no way to run but the index
        let mut f = flock(&[(5, 5), (5, 5)]);
        f.bark(from_int(5), from_int(5));
        run(&mut f, &FlatField, &[], 30);
        assert!(apart(&f.sheep[0], &f.sheep[1]) > milli(500));
    }

    #[test]
    fn a_lamb_trots_to_its_ewe_and_stays_by_her() {
        // 6 m off and 1.2 m of leash leaves 4.8 m, 1 s at a trot of 4.8 m/s
        let mut f = flock(&[(0, 0)]);
        let lamb = f.add(Sheep::new(Kind::Lamb, from_int(6), 0));
        f.sheep[lamb].ewe = 0;
        run(&mut f, &FlatField, &[], seconds(1));
        assert!(f.sheep[lamb].x < from_int(5), "{}", f.sheep[lamb].x);
        run(&mut f, &FlatField, &[], seconds(2));
        assert!(apart(&f.sheep[lamb], &f.sheep[0]) < milli(1400));
        // the ewe is elsewhere, and the lamb goes to her
        f.sheep[0].x = from_int(-20);
        run(&mut f, &FlatField, &[], seconds(10));
        assert!(apart(&f.sheep[lamb], &f.sheep[0]) < milli(1800));
    }

    #[test]
    fn a_lamb_keeps_up_with_a_ewe_at_a_trot() {
        // at 4.8 m/s it needs (d - 1.2) * 3 = 4.8 m/s: 2.8 m behind her
        let mut f = flock(&[(0, 0)]);
        let lamb = f.add(Sheep::new(Kind::Lamb, from_int(-1), 0));
        f.sheep[lamb].ewe = 0;
        for t in 0..seconds(3) {
            f.sheep[0].x += per_tick(tune::FLOCK_TROT);
            f.step(&FlatField, &[], t as u32);
        }
        let gap = apart(&f.sheep[0], &f.sheep[lamb]);
        assert!(gap > milli(2500) && gap < milli(3100), "{gap}");
    }

    #[test]
    fn the_carrot_draws_sheep_in_reach_only_while_it_is_held_out() {
        // 10 m to go, 8.5 to the crowd around it, 1.8 s at a trot, and it
        // coasts 4.8 * 8 / 60 = 0.64 m past that as it stops, to about 0.86
        let mut f = flock(&[(10, 0), (40, 0)]);
        for t in 0..seconds(5) {
            f.lure(0, 0);
            f.step(&FlatField, &[], t as u32);
        }
        assert!(
            f.sheep[0].x < milli(1500) && f.sheep[0].x > milli(800),
            "{}",
            f.sheep[0].x
        );
        assert_eq!(f.sheep[1].x, from_int(40), "out of reach");
        // put away, it stops drawing them
        f.sheep[0].x = from_int(10);
        f.sheep[0].vx = 0;
        run(&mut f, &FlatField, &[], seconds(2));
        assert!(f.sheep[0].x > from_int(9), "{}", f.sheep[0].x);
    }

    #[test]
    fn a_lone_calm_sheep_keeps_its_round_at_no_more_than_the_drift() {
        let mut f = walker(0, 0);
        let mut seen = [0; 3];
        for t in 0..seconds(120) {
            f.step(&FlatField, &[], t as u32);
            let s = f.sheep[0];
            assert!(fx::length(s.vx, s.vz) <= tune::GRAZE_DRIFT + 64);
            seen[s.mood as usize] += 1;
        }
        // grazes of 3 to 8 s, walks of 3.2 to 8 s alone, and short turns
        let [graze, amble, turn] = seen;
        assert!(graze > seconds(30) && graze < seconds(90), "{graze}");
        assert!(amble > seconds(30) && amble < seconds(90), "{amble}");
        assert!(turn > seconds(2) && turn < seconds(30), "{turn}");
    }

    #[test]
    fn a_walk_bends_gently_and_a_stopped_sheep_turns_on_the_spot() {
        let mut s = Sheep::new(Kind::Ewe, 0, 0);
        (s.face_x, s.face_z, s.bend) = (ONE, 0, tune::AMBLE_BEND);
        for _ in 0..80 {
            turn(&mut s);
        }
        // 80 steps of 0.014 radians is 1.12 radians, about 64 degrees
        assert!((fx::mul(s.face_x, s.face_x) + fx::mul(s.face_z, s.face_z) - ONE).abs() < milli(30));
        assert!((s.face_x - milli(436)).abs() < milli(25), "{}", s.face_x);
        assert!((s.face_z - milli(900)).abs() < milli(25), "{}", s.face_z);
        // a half turn takes the two seconds the table says, 40 steering ticks
        (s.face_x, s.face_z, s.bend) = (ONE, 0, tune::TURN_BEND);
        for _ in 0..tune::HALF_TURN_TICKS / tune::STEER_EVERY as i32 {
            turn(&mut s);
        }
        assert!(s.face_x < -milli(980), "{}", s.face_x);
    }

    #[test]
    fn each_sheep_keeps_its_own_time() {
        // sixty sheep all let loose on one tick, far enough apart to ignore
        // each other: a few seconds on, no large share is doing the same thing
        let mut f = Flock::new(WIDE, None, 8);
        for k in 0..60 {
            f.add(Sheep::new(
                Kind::Ewe,
                from_int(k * 3 - 90),
                from_int((k % 4) * 30),
            ));
        }
        let tempos: Vec<u8> = f.sheep.iter().map(|s| s.tempo).collect();
        assert!(
            tempos.iter().any(|&t| t < 85) && tempos.iter().any(|&t| t > 125),
            "{tempos:?}"
        );
        let mut most = 0;
        for t in 0..seconds(120) {
            f.step(&FlatField, &[], t as u32);
            if t > seconds(10) {
                let walking = f.sheep.iter().filter(|s| s.mood == Mood::Amble).count();
                most = most.max(walking);
            }
        }
        // about half are walking at any moment
        assert!(most <= 45, "{most} of 60 walking at once");
    }

    #[test]
    fn a_walk_leans_in_the_farther_out_a_sheep_stands() {
        // at the middle of its group: anywhere. At 3 m out or more: within
        // 30 degrees of the middle, which lies due east here
        let mut f = flock(&[]);
        let middle = Near::default();
        let far_out = Near {
            x: from_int(4),
            ..Near::default()
        };
        let (mut east, mut west) = (0, 0);
        for _ in 0..400 {
            east += (f.next_aim(&middle).0 > 0) as i32;
            west += (f.next_aim(&far_out).0 < milli(850)) as i32;
        }
        assert!((150..=250).contains(&east), "{east}");
        assert_eq!(west, 0);
    }

    #[test]
    fn a_sheep_walking_away_from_its_group_stops_and_turns_back() {
        let mut f = walker(0, 0);
        f.sheep[0].mood = Mood::Amble;
        f.sheep[0].graze_ticks = 600;
        (f.sheep[0].face_x, f.sheep[0].face_z) = (ONE, 0);
        let mut s = f.sheep[0];
        let group_to_the_west = Near {
            x: -from_int(4),
            count: 9,
            ..Near::default()
        };
        f.graze(&mut s, &group_to_the_west);
        assert_eq!(s.mood, Mood::Turn);
        assert!(s.graze_ticks > 0, "a turn to make");
    }

    #[test]
    fn a_lone_sheep_walks_further_than_one_in_company() {
        let mut lone = walker(0, 0).sheep[0];
        let mut company = lone;
        let (mut a, mut b) = (walker(0, 0), walker(0, 0));
        a.next_mood(&mut lone, &Near::default());
        b.next_mood(
            &mut company,
            &Near {
                count: 5,
                ..Near::default()
            },
        );
        assert_eq!(
            lone.graze_ticks as i32,
            company.graze_ticks as i32 * tune::LONE_WALK_PERCENT / 100
        );
    }

    #[test]
    fn a_frightened_sheep_drops_its_round_and_goes_back_to_it_calm() {
        let mut f = walker(0, 0);
        f.sheep[0].mood = Mood::Amble;
        run(&mut f, &FlatField, &[dog_at(-4, 0)], 3);
        assert_eq!((f.sheep[0].mood, f.sheep[0].graze_ticks), (Mood::Turn, 0));
        run(&mut f, &FlatField, &[], seconds(8));
        assert_eq!(f.sheep[0].alarm, 0);
        assert_ne!(f.sheep[0].graze_ticks, 0);
    }

    fn fold_at_the_east() -> Fold {
        Fold {
            back_x: from_int(20),
            back_z: 0,
        }
    }

    /// Open land, with the fold as a box from x = 10 to 20.
    fn fold_field() -> WalledField {
        WalledField {
            fold: Some(Rect {
                min_x: from_int(10),
                min_z: from_int(-5),
                max_x: from_int(20),
                max_z: from_int(5),
            }),
            ..WalledField::default()
        }
    }

    #[test]
    fn a_sheep_clear_of_the_gap_counts_once_and_walks_to_the_back() {
        let mut f = Flock::new(WIDE, Some(fold_at_the_east()), 1);
        let ground = fold_field();
        // on the line, it could still step back out
        f.add(Sheep::new(Kind::Ewe, from_int(10) + milli(100), 0));
        run(&mut f, &ground, &[], 3);
        assert_eq!((f.folded, f.sheep[0].folded), (0, false));
        // a whole sheep in, it stays
        f.sheep[0].x = from_int(11);
        run(&mut f, &ground, &[], 3);
        assert_eq!((f.folded, f.sheep[0].folded), (1, true));
        // 9 m to the back at 0.9 m/s is 10 s; one sheep stops a meter short
        run(&mut f, &ground, &[], seconds(12));
        assert_eq!(f.folded, 1);
        assert!(f.sheep[0].x > from_int(17), "{}", f.sheep[0].x);
    }

    #[test]
    fn a_folded_sheep_ignores_dogs_and_barks_and_never_leaves() {
        let mut f = Flock::new(WIDE, Some(fold_at_the_east()), 1);
        let ground = fold_field();
        f.add(Sheep::new(Kind::Ewe, from_int(11), 0));
        f.sheep[0].folded = true;
        f.folded = 1;
        f.bark(from_int(12), 0);
        assert_eq!(f.sheep[0].bolt, 0);
        run(&mut f, &ground, &[dog_at(13, 0)], seconds(3));
        assert_eq!(f.sheep[0].alarm, 0);
        // however hard it is sent at the open gap
        f.sheep[0].x = from_int(10) + f.sheep[0].radius + milli(10);
        f.sheep[0].want_x = -tune::FLEE;
        f.sheep[0].vx = -tune::FLEE;
        run(&mut f, &ground, &[], 1);
        assert_eq!(f.sheep[0].x, from_int(10) + f.sheep[0].radius);
    }

    #[test]
    fn a_folded_lamb_lets_go_of_its_ewe() {
        let mut f = Flock::new(WIDE, Some(fold_at_the_east()), 1);
        f.add(Sheep::new(Kind::Ewe, from_int(-20), 0));
        let lamb = f.add(Sheep::new(Kind::Lamb, from_int(12), 0));
        f.sheep[lamb].ewe = 0;
        f.sheep[lamb].folded = true;
        f.folded = 1;
        run(&mut f, &fold_field(), &[], seconds(5));
        assert!(
            f.sheep[lamb].x > from_int(12),
            "walking to the back: {}",
            f.sheep[lamb].x
        );
        assert_eq!(f.sheep[lamb].ewe, NONE);
    }

    #[test]
    fn the_back_of_the_fold_fills_from_the_back_as_sheep_arrive() {
        // nine sheep settle within 3 m of the back (1 m times the root of
        // nine), and their spacing adds a little
        let mut f = Flock::new(WIDE, Some(fold_at_the_east()), 1);
        for k in 0..9 {
            f.add(Sheep::new(Kind::Ewe, from_int(11), from_int(k - 4)));
        }
        run(&mut f, &fold_field(), &[], seconds(40));
        assert_eq!(f.folded, 9);
        for s in &f.sheep {
            assert!(s.x > from_int(14), "{}", s.x);
        }
    }

    fn walled() -> WalledField {
        // a wall at x = 0..1 with a gap at z = -3..3, the west is field 0
        WalledField {
            walls: vec![
                Rect {
                    min_x: 0,
                    min_z: from_int(-40),
                    max_x: from_int(1),
                    max_z: from_int(-3),
                },
                Rect {
                    min_x: 0,
                    min_z: from_int(3),
                    max_x: from_int(1),
                    max_z: from_int(40),
                },
            ],
            dividers: vec![fx::HALF],
            fold: None,
        }
    }

    #[test]
    fn a_calm_sheep_stops_at_the_line_of_a_gap_and_turns() {
        // walking east from just short of the line at x = 0.5
        let mut f = walker(0, 0);
        f.sheep[0].x = -milli(100);
        f.sheep[0].mood = Mood::Amble;
        f.sheep[0].graze_ticks = 600;
        (f.sheep[0].face_x, f.sheep[0].face_z) = (ONE, 0);
        for t in 0..seconds(60) {
            f.step(&walled(), &[], t as u32);
            assert!(f.sheep[0].x < fx::HALF, "{}", f.sheep[0].x);
        }
    }

    #[test]
    fn a_frightened_sheep_will_go_through_a_gap() {
        // walked off by a dog at the rim, too slowly to coast over the line
        let ground = walled();
        let mut f = flock(&[(-2, 0)]);
        for t in 0..seconds(4) {
            let dog = Actor {
                x: f.sheep[0].x - from_int(8),
                ..dog_at(0, 0)
            };
            f.step(&ground, &[dog], t as u32);
        }
        assert!(f.sheep[0].x > from_int(1), "{}", f.sheep[0].x);
    }

    /// A wall along the south at z = -10 and one along the east at x = 10,
    /// meeting in a corner, each 1 m thick.
    fn corner() -> WalledField {
        WalledField {
            walls: vec![
                Rect {
                    min_x: from_int(-40),
                    min_z: from_int(-11),
                    max_x: from_int(11),
                    max_z: from_int(-10),
                },
                Rect {
                    min_x: from_int(10),
                    min_z: from_int(-11),
                    max_x: from_int(11),
                    max_z: from_int(40),
                },
            ],
            ..WalledField::default()
        }
    }

    #[test]
    fn a_sheep_steers_off_a_wall_before_it_touches() {
        // drawn straight at the east wall from 4 m out at a trot, by a carrot
        // beyond it: it turns along the wall, and its side stays off the stone
        let mut f = flock(&[(6, 20)]);
        for t in 0..seconds(3) {
            f.lure(from_int(18), from_int(20));
            f.step(&corner(), &[], t as u32);
            let s = f.sheep[0];
            assert!(s.x + s.radius < from_int(10), "{}", s.x);
        }
        assert!(f.sheep[0].z != from_int(20));
    }

    #[test]
    fn what_a_sheep_meant_to_spend_on_a_wall_it_spends_along_it() {
        // touching a wall to the east and wanting to go 2 m/s south east: the
        // 1.41 m/s into the wall goes onto the 1.41 m/s south
        let ground = corner();
        let mut s = Sheep::new(Kind::Ewe, from_int(10) - tune::SHEEP_RADIUS, from_int(20));
        let d = fx::mul(from_int(2), milli(707));
        (s.want_x, s.want_z) = (d, -d);
        keep_off_edges(&mut s, &ground, &Near::default(), 0);
        assert!(s.want_x.abs() < milli(5), "{}", s.want_x);
        assert!((s.want_z + 2 * d).abs() < milli(5), "{}", s.want_z);
    }

    #[test]
    fn a_sheep_sent_straight_at_a_wall_keeps_to_the_way_it_was_going() {
        // nothing either way in what it wants, so what it is doing decides
        let ground = corner();
        let mut s = Sheep::new(Kind::Ewe, from_int(10) - tune::SHEEP_RADIUS, from_int(20));
        s.want_x = from_int(3);
        s.vz = -ONE;
        keep_off_edges(&mut s, &ground, &Near::default(), 0);
        assert!(s.want_z < -milli(2900), "{}", s.want_z);
        s.want_x = from_int(3);
        s.want_z = 0;
        s.vz = ONE;
        keep_off_edges(&mut s, &ground, &Near::default(), 0);
        assert!(s.want_z > milli(2900), "{}", s.want_z);
    }

    #[test]
    fn a_sheep_with_a_flock_along_the_wall_slides_toward_it() {
        // moving a little south, but its flock is 4 m north along the wall
        let ground = corner();
        let mut s = Sheep::new(Kind::Ewe, from_int(10) - tune::SHEEP_RADIUS, from_int(20));
        s.want_x = from_int(3);
        s.vz = -ONE;
        let flock_north = Near {
            z: from_int(4),
            count: 12,
            ..Near::default()
        };
        keep_off_edges(&mut s, &ground, &flock_north, 0);
        assert!(s.want_z > milli(2900), "{}", s.want_z);
    }

    #[test]
    fn a_fleeing_sheep_pressed_on_a_wall_keeps_moving_along_it() {
        // the dog stands square behind it and follows it down the wall: all
        // the fear is into the stone, and none of it may be lost. The wall
        // ends in the corner 30 m south, so this stops at 2 s, before that
        let ground = corner();
        let mut f = flock(&[(9, 20)]);
        let mut dog = dog_at(4, 20);
        for t in 0..seconds(2) {
            dog.z = f.sheep[0].z;
            f.step(&ground, &[dog], t as u32);
        }
        let s = f.sheep[0];
        assert!((s.z - from_int(20)).abs() > from_int(15), "{}", s.z);
        assert!(fx::length(s.vx, s.vz) > from_int(2));
    }

    #[test]
    fn a_sheep_in_full_flight_stays_on_its_side_of_every_wall_of_the_map() {
        // a Stampede's 11 m/s, 0.18 m a tick, straight at the stone, for a
        // second, at seven spots along each side of every wall, the Fold's
        // fence with them. Past either end of a wall there is no side to be on
        for seed in 1..=3 {
            let map = Map::generate(seed);
            let mut runs = 0;
            for wall in &map.walls {
                let (dx, dz) = (wall.b.0 - wall.a.0, wall.b.1 - wall.a.1);
                let long = fx::length(dx, dz);
                if long < from_int(8) {
                    continue;
                }
                let (nx, nz) = (fx::div(-dz, long), fx::div(dx, long));
                for spot in 1..8 {
                    let at = (wall.a.0 + dx / 8 * spot, wall.a.1 + dz / 8 * spot);
                    for side in [ONE, -ONE] {
                        let (ox, oz) = (fx::mul(nx, side), fx::mul(nz, side));
                        let start = (at.0 + ox * 4, at.1 + oz * 4);
                        let clear = map.push_out(start.0, start.1, tune::SHEEP_RADIUS) == start;
                        if !clear || !map.inside(start.0, start.1) || map.in_fold(start.0, start.1) {
                            continue;
                        }
                        runs += 1;
                        let mut f = Flock::on_map(&map, 0, 1);
                        let i = f.add(Sheep::new(Kind::Ewe, start.0, start.1));
                        let flat_out = (-fx::mul(ox, milli(11_000)), -fx::mul(oz, milli(11_000)));
                        let s = &mut f.sheep[i];
                        (s.want_x, s.want_z) = flat_out;
                        (s.vx, s.vz) = flat_out;
                        s.bolt = 60;
                        run(&mut f, &map, &[], 60);
                        let s = f.sheep[i];
                        let (off_x, off_z) = (s.x - wall.a.0, s.z - wall.a.1);
                        let along = (off_x as i64 * nz as i64 - off_z as i64 * nx as i64) >> fx::SHIFT;
                        let past = (s.x - at.0) as i64 * ox as i64 + (s.z - at.1) as i64 * oz as i64;
                        let beside = (0..long as i64).contains(&along);
                        assert!(past > 0 || !beside, "seed {seed} {wall:?} spot {spot}");
                    }
                }
            }
            assert!(runs > 100, "{runs}");
        }
    }

    #[test]
    fn a_calm_sheep_drifts_off_an_edge_by_itself() {
        let ground = corner();
        let mut f = flock(&[(9, 20)]);
        f.sheep[0].x = from_int(10) - tune::SHEEP_RADIUS;
        run(&mut f, &ground, &[], seconds(10));
        let clear = from_int(10) - tune::SHEEP_RADIUS - f.sheep[0].x;
        assert!(clear > tune::EDGE_SENSE / 2, "{clear}");
        assert_eq!(f.sheep[0].z, from_int(20), "straight off it");
    }

    #[test]
    fn a_sheepdog_coming_between_a_sheep_and_the_wall_sends_it_down_the_other() {
        // a sheep tucked into the corner, and the dog on the east wall north
        // of it: south is stone, so its way out is west along the south wall,
        // at a walk and not as a last-resort run
        let ground = corner();
        let mut f = flock(&[(9, -9)]);
        for t in 0..seconds(4) {
            f.step(&ground, &[dog_at(9, -5)], t as u32);
            assert_eq!(f.sheep[0].bolt, 0);
        }
        let s = f.sheep[0];
        assert!(s.x < from_int(3), "{}", s.x);
        assert!(s.z < from_int(-5), "{}", s.z);
    }

    #[test]
    fn a_cornered_sheep_runs_out_past_a_dog_that_pins_it() {
        // a dead end 2 m wide and 3 m deep, with the dog at its mouth: away
        // is stone, and so is either side, so the only way out is past it
        let wall = |min_x, min_z, max_x, max_z| Rect {
            min_x: from_int(min_x),
            min_z: from_int(min_z),
            max_x: from_int(max_x),
            max_z: from_int(max_z),
        };
        let ground = WalledField {
            walls: vec![wall(0, 1, 4, 2), wall(0, -2, 4, -1), wall(3, -2, 4, 2)],
            ..WalledField::default()
        };
        let mut f = flock(&[(2, 0)]);
        let mut out = false;
        for t in 0..seconds(8) {
            f.step(&ground, &[dog_at(-3, 0)], t as u32);
            out |= f.sheep[0].x < 0;
        }
        assert!(out, "{}", f.sheep[0].x);
    }

    #[test]
    fn a_shoved_sheep_is_not_pushed_through_a_wall() {
        // the shove alone: a still sheep takes no step for a wall to stop
        let ground = corner();
        let mut f = flock(&[(9, 20)]);
        f.sheep[0].x = from_int(10) - f.sheep[0].radius;
        let wolf = Actor {
            x: from_int(9),
            z: from_int(20),
            radius: tune::WOLF.radius,
            scares: false,
            solid: true,
            ..Actor::default()
        };
        f.grid.fit(&f.bounds, 1);
        f.grid.build(&f.sheep);
        f.shoved_by(&ground, &[wolf]);
        assert!(
            f.sheep[0].x + f.sheep[0].radius <= from_int(10) + 8,
            "{}",
            f.sheep[0].x
        );
    }

    #[test]
    fn a_bell_answers_once_when_a_wolf_comes_in_reach() {
        let mut f = flock(&[(0, 0), (50, 0)]);
        f.set_bell(0, true);
        assert_eq!(f.reach_bell(from_int(21), 0), None);
        assert_eq!(f.reach_bell(from_int(19), 0), Some(0));
        assert_eq!(f.reach_bell(from_int(19), 0), None, "once");
        // hung again, it can be reached again
        f.set_bell(0, false);
        f.set_bell(0, true);
        assert_eq!(f.reach_bell(from_int(19), 0), Some(0));
    }

    #[test]
    fn the_start_mixes_sizes_lambs_and_fleeces_within_the_contract() {
        let map = Map::generate(5);
        let f = Flock::on_map(&map, 400, 5);
        assert_eq!(f.sheep.len(), 400);
        assert_eq!(f.fold.map(|b| (b.back_x, b.back_z)), Some(map.fold.back));
        let lambs = f.sheep.iter().filter(|s| s.kind == Kind::Lamb).count();
        let rams = f.sheep.iter().filter(|s| s.kind == Kind::Ram).count();
        assert!((20..=120).contains(&lambs), "{lambs}");
        assert!((5..=50).contains(&rams), "{rams}");
        for s in &f.sheep {
            assert!((s.fleece as usize) < tune::FLEECE_WEIGHTS.len());
            match s.kind {
                Kind::Ewe => assert!((s.size - ONE).abs() <= tune::EWE_SPREAD),
                Kind::Ram => assert_eq!(s.size, tune::RAM_SIZE),
                Kind::Lamb => {
                    let ewe = &f.sheep[s.ewe as usize];
                    assert_eq!(ewe.kind, Kind::Ewe);
                    assert!((s.size - tune::LAMB_SIZE).abs() <= tune::LAMB_SPREAD);
                    assert_eq!(s.fleece, ewe.fleece);
                    assert!(apart(s, ewe) < from_int(2));
                }
            }
        }
        assert!(f.sheep.iter().any(|s| s.size < milli(900)));
        assert!(f
            .sheep
            .iter()
            .any(|s| s.size > milli(1100) && s.kind == Kind::Ewe));
    }

    #[test]
    fn a_ewe_has_one_lamb_at_most() {
        let f = Flock::on_map(&Map::generate(9), 200, 9);
        let mut mothers: Vec<u16> = f.sheep.iter().filter(|s| s.ewe != NONE).map(|s| s.ewe).collect();
        let lambs = mothers.len();
        mothers.sort();
        mothers.dedup();
        assert_eq!(mothers.len(), lambs);
        assert!(lambs > 0);
    }

    #[test]
    fn a_clone_of_the_flock_leaves_the_grid_behind_and_runs_on_the_same() {
        let mut f = flock(&[(0, 0), (1, 0), (2, 1)]);
        run(&mut f, &FlatField, &[dog_at(-4, 0)], 30);
        assert!(!f.grid.head.is_empty());
        let mut copy = f.clone();
        assert!(copy.grid.head.is_empty());
        assert_eq!(copy, f);
        for t in 30..90 {
            f.step(&FlatField, &[dog_at(-4, 0)], t);
            copy.step(&FlatField, &[dog_at(-4, 0)], t);
        }
        assert_eq!(copy, f);
    }
}
