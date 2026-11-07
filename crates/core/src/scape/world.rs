//! The hillside as the renderer's world bank: every mesh built once from the
//! map, one instance list kept with them, and the rows a frame draws. Stones
//! are kept by square of the map, so a far square draws eight-sided stones in
//! one row and a square nobody's looking at draws nothing.

use crate::fx::{self, from_int, mul, Fx, ONE, TURN};
use crate::map::props::{Cairn, Slab};
use crate::map::Map;
use crate::mesh::{self, Shape, Vertex, OWN};
use crate::render::geom::{v3, Affine, V3};
use crate::render::stand;
use crate::render::{
    Batch, Instance, MeshRange, Outline, BYTES_PER_VALUE, DRAW_VALUES, FRAME_SOURCE, RIGID, WORLD_BANK,
    WORLD_SOURCE,
};

use super::cm;
use super::dress::{self, Kind};
use super::field::Field;
use super::grass::{self, Bake, Clear};
use super::hills;
use super::noise::{lattice, lerp};
use super::palette::{self as pal, mix};
use super::shapes;
use super::wall::{self, Course, Stone, ROUND_SHAPES};
use super::wind::Wind;

type P = (Fx, Fx);

/// The meshes, as the world bank holds them.
#[derive(Debug, Clone, Default)]
pub struct Bank {
    pub vertices: Vec<i32>,
    pub indices: Vec<u32>,
    pub ranges: Vec<MeshRange>,
}

impl Bank {
    fn parts(&mut self, vs: &[Vertex], ix: &[u32]) -> usize {
        let base = (self.vertices.len() / mesh::VERTEX_VALUES) as u32;
        mesh::write_vertices(vs, 1, &mut self.vertices);
        self.ranges.push(MeshRange {
            first: self.indices.len() as u32,
            count: ix.len() as u32,
            pass: RIGID,
        });
        self.indices.extend(ix.iter().map(|i| i + base));
        self.ranges.len() - 1
    }

    fn add(&mut self, shape: Shape) -> usize {
        let (vs, ix) = shape.finish();
        self.parts(&vs, &ix)
    }
}

/// A match's mound as it stands now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoundDraw {
    pub at: V3,
    pub up: V3,
    pub dug: bool,
    pub outline: Option<Outline>,
}

/// A small thing a powerup puts in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Scent,
    Carrot,
    Fur,
    Blood,
}

/// A token, and where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenDraw {
    pub token: Token,
    pub place: Affine,
}

/// How far a slab's stones go below the ground, so a slope shows no daylight.
const SLAB_SUNK: Fx = cm(40);
/// How much longer than its share of the row a slab's stone is.
const SLAB_OVERLAP: Fx = cm(30);

/// One square of the map's stones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Square {
    pub center: P,
    pub radius: Fx,
    /// A row a shape, in the near meshes.
    pub near: Vec<u32>,
    /// One row: all the square's stones, in the far mesh.
    pub far: Vec<u32>,
}

/// A square's stones switch to the far mesh past here.
const NEAR_STONES: Fx = fx::from_int(84);
/// Stones are kept in squares this many bits of `Fx` wide: 64 m.
const SQUARE_SHIFT: u32 = 22;

/// What the world's meshes are, once built.
#[derive(Debug, Clone)]
pub struct Scape {
    pub field: Field,
    pub wind: Wind,
    pub bank: Bank,
    /// The kept instances: what the world source holds.
    pub instances: Vec<i32>,
    /// Rows drawn every frame.
    pub rows: Vec<u32>,
    pub squares: Vec<Square>,
    pub bare: Vec<(P, Fx)>,
    pub stones: usize,
    fresh: usize,
    dug: usize,
    arrow: usize,
    scent: usize,
    fur: usize,
    blood: usize,
    carrot: usize,
    meshes: usize,
    arrow_at: V3,
    /// Each mound's foot and the ground's normal there.
    mounds: Vec<(V3, V3)>,
}

fn ground_vertices(f: &Field) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(f.cols * f.rows);
    for r in 0..f.rows {
        for c in 0..f.cols {
            let i = r * f.cols + c;
            let [nx, ny, nz] = f.normals[i];
            let at = v3(f.x0 + ((c as Fx) << 17), f.heights[i], f.z0 + ((r as Fx) << 17));
            let normal = v3(nx, ny, nz);
            out.push(Vertex {
                pos: at,
                normal,
                hull: normal,
                color: f.colors[i],
                paint: OWN,
                bone: 0,
            });
        }
    }
    out
}

/// The dark heart of each wall, up under its cope, so the joints between
/// stones read as dark and no daylight shows through.
fn wall_cores(map: &Map, f: &Field) -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    s.brush(mix(pal::JOINT, pal::WHIN, cm(30)), OWN, 0);
    for w in &map.walls {
        let tier = wall::tiers(w.height);
        let len = fx::length(w.b.0 - w.a.0, w.b.1 - w.a.1).max(ONE);
        let (ax, az) = (fx::div(w.b.0 - w.a.0, len), fx::div(w.b.1 - w.a.1, len));
        let side = v3(-az, 0, ax);
        let steps = (len >> 18).max(1) + 1;
        let (foot, top) = (mul(w.half, cm(70)), mul(w.half, cm(28)));
        let rise = tier.lace_top - cm(8);
        let at = |i: i32| {
            let t = fx::ratio(i, steps);
            let (x, z) = (w.a.0 + mul(w.b.0 - w.a.0, t), w.a.1 + mul(w.b.1 - w.a.1, t));
            (x, f.height_at(x, z), z)
        };
        let corner = |i: i32, sign: i32, high: bool| {
            let (x, y, z) = at(i);
            let half = if high { top } else { foot } * sign;
            v3(x, y + if high { rise } else { -cm(30) }, z).add(side.scale(half))
        };
        for i in 0..steps {
            let quads = [
                [
                    corner(i, 1, false),
                    corner(i + 1, 1, false),
                    corner(i, 1, true),
                    corner(i + 1, 1, true),
                ],
                [
                    corner(i + 1, -1, false),
                    corner(i, -1, false),
                    corner(i + 1, -1, true),
                    corner(i, -1, true),
                ],
                [
                    corner(i, 1, true),
                    corner(i + 1, 1, true),
                    corner(i, -1, true),
                    corner(i + 1, -1, true),
                ],
            ];
            for [a, b, c, d] in quads {
                s.tri(a, b, c);
                s.tri(b, d, c);
            }
        }
    }
    s
}

fn wall_stone_place(st: &Stone) -> Affine {
    let (c, si) = (fx::cos(st.yaw), fx::sin(st.yaw));
    let (along, across) = (v3(c, 0, si), v3(-si, 0, c));
    let (cl, sl) = (fx::cos(st.lean), fx::sin(st.lean));
    let lean_along = along.scale(cl).add(v3(0, sl, 0));
    let lean_up = along.scale(-sl).add(v3(0, cl, 0));
    Affine {
        c0: lean_along.scale(st.size[0]),
        c1: lean_up.scale(st.size[1]),
        c2: across.scale(st.size[2]),
        t: v3(st.at[0], st.at[1], st.at[2]),
    }
}

/// A random number for one of a cairn's boulders.
fn roll(seed: u32, tier: usize, k: i32, n: u32) -> Fx {
    lattice(seed.wrapping_add(n * 37), tier as i32 * 91 + k, n as i32)
}

/// The boulders round each step of a cairn, tops level with the step.
fn cairn_stones(c: &Cairn, seed: u32) -> Vec<Stone> {
    let mut out = Vec::new();
    for (t, step) in c.tiers.iter().enumerate() {
        let ring = step.radius - cm(30);
        let count = (fx::mul(ring, cm(628)) / cm(66)).max(5);
        for k in 0..count {
            let r = |n: u32| roll(seed, t, k, n);
            let a = k * TURN / count + mul(r(1), TURN / 30);
            let tall = lerp(cm(56), cm(74), r(2));
            let tone = match r(3) >> 13 {
                0 | 1 => pal::GRANITE,
                2 | 3 => pal::GRANITE_DARK,
                4 => pal::WHIN_PALE,
                5 => pal::WHIN_MOSSY,
                _ => pal::WHIN,
            };
            out.push(Stone {
                at: [
                    c.x + mul(fx::cos(a), ring),
                    step.top - tall / 2 + cm(4),
                    c.z + mul(fx::sin(a), ring),
                ],
                size: [lerp(cm(74), cm(104), r(4)), tall, lerp(cm(58), cm(84), r(5))],
                yaw: a + TURN / 4,
                lean: 0,
                tone,
                course: Course::Loose,
                shape: (r(6) >> 14) as u8 % ROUND_SHAPES,
            });
        }
    }
    out
}

/// The stones of a slab, in a row from end to end: flat on top, the highest
/// level with the slab's top, and inside the outline that stops a body. The
/// end stones are narrower, to fit its round ends.
fn slab_stones(slab: &Slab, f: &Field, seed: u32) -> Vec<Stone> {
    let (dx, dz) = (slab.b.0 - slab.a.0, slab.b.1 - slab.a.1);
    let long = fx::length(dx, dz);
    let yaw = wall::angle_of(dx, dz);
    let way = (fx::div(dx, long), fx::div(dz, long));
    let count = if long > cm(380) { 4 } else { 3 };
    // the row runs the whole outline, past the ends of the line
    let tile = (long + 2 * slab.half) / count;
    (0..count)
        .map(|k| {
            let r = |n: u32| lattice(seed.wrapping_add(n), k, 3);
            let off = -slab.half + tile * k + tile / 2;
            let (x, z) = (slab.a.0 + mul(way.0, off), slab.a.1 + mul(way.1, off));
            let tall = mul(slab.top, lerp(cm(86), ONE, r(1)));
            let end = k == 0 || k == count - 1;
            let wide = mul(2 * slab.half, if end { cm(74) } else { cm(92) });
            Stone {
                at: [x, f.height_at(x, z) + (tall - SLAB_SUNK) / 2, z],
                size: [tile + SLAB_OVERLAP, tall + SLAB_SUNK, wide],
                yaw: yaw + mul(r(2), TURN / 40) - TURN / 80,
                lean: 0,
                tone: match r(3) >> 14 {
                    0 => pal::GRANITE,
                    1 => pal::WHIN_PALE,
                    _ => pal::WHIN,
                },
                course: Course::Loose,
                shape: (r(4) >> 14) as u8 % ROUND_SHAPES,
            }
        })
        .collect()
}

/// A place facing `dir` on the ground: the bank's front is its +z.
/// A tunnel's bank is only a little larger than a mound, a burrow, not a cave.
const MOUTH_SCALE: Fx = cm(40);

fn facing_place(t: V3, dir: P) -> Affine {
    Affine {
        c0: v3(dir.1, 0, -dir.0),
        c1: v3(0, ONE, 0),
        c2: v3(dir.0, 0, dir.1),
        t,
    }
}

fn unit(v: P) -> P {
    let l = fx::length(v.0, v.1).max(1);
    (fx::div(v.0, l), fx::div(v.1, l))
}

/// A gate leaf's place: hinged at `t`, running along `dir`.
fn leaf_place(t: V3, dir: P) -> Affine {
    Affine {
        c0: v3(dir.0, 0, dir.1),
        c1: v3(0, ONE, 0),
        c2: v3(-dir.1, 0, dir.0),
        t,
    }
}

fn kept(rows: &mut [u32], by: usize) {
    for row in rows.chunks_mut(DRAW_VALUES) {
        row[1] = WORLD_BANK;
        row[4] = WORLD_SOURCE;
        row[5] += (by * BYTES_PER_VALUE) as u32;
    }
}

impl Scape {
    pub fn build(map: &Map, seed: u32) -> Scape {
        let field = Field::build(map, seed);
        let wind = Wind::shipped();
        let mut bank = Bank::default();
        let ground = bank.parts(&ground_vertices(&field), &field.triangles());
        let hills = bank.add(hills::far_hills(&field, seed));
        let cores = bank.add(wall_cores(map, &field));
        let (bogs, wet) = dress::bogs(map, &field, seed);
        let mut bare = wet.clone();
        let bogs = bank.add(bogs);
        let post = bank.add(shapes::gate_post());
        let long = map.fold.gap.width / 2 - cm(10);
        let leaf = bank.add(shapes::gate_leaf(long));
        let bank_of_earth = bank.add(shapes::mouth_bank());
        let fresh = bank.add(shapes::mound_fresh());
        let dug = bank.add(shapes::mound_dug());
        let arrow = bank.add(shapes::arrow());
        let scent = bank.add(shapes::scent());
        let fur = bank.add(shapes::fur());
        let blood = bank.add(shapes::blood());
        let carrot = bank.add(shapes::carrot());

        let mut solid: Vec<(usize, Instance)> = vec![
            (ground, Instance::still(Affine::IDENTITY)),
            (hills, Instance::still(Affine::IDENTITY)),
            (cores, Instance::still(Affine::IDENTITY)),
            (bogs, Instance::still(Affine::IDENTITY)),
        ];

        // the Fold's gate: posts at the wall ends, leaves folded back inside
        let (a, b) = (map.fold.gap.a, map.fold.gap.b);
        let across = unit((b.0 - a.0, b.1 - a.1));
        let side = (-across.1, across.0);
        let to_center = (map.fold.center.0 - a.0, map.fold.center.1 - a.1);
        let dot = mul(to_center.0 >> 4, side.0) + mul(to_center.1 >> 4, side.1);
        let inward = if dot >= 0 { side } else { (-side.0, -side.1) };
        let open = |along: P| {
            unit((
                mul(along.0, cm(86)) + mul(inward.0, cm(50)),
                mul(along.1, cm(86)) + mul(inward.1, cm(50)),
            ))
        };
        for (end, away) in [(a, (-across.0, -across.1)), (b, across)] {
            let at = v3(end.0, field.height_at(end.0, end.1), end.1);
            solid.push((post, Instance::still(Affine::translate(at))));
            solid.push((leaf, Instance::still(leaf_place(at, open(away)))));
        }

        for (k, t) in map.trees.iter().enumerate() {
            let shape = shapes::tree(t, wind.dir, seed.wrapping_add(k as u32 * 97 + 1));
            let m = bank.add(shape);
            let at = v3(t.x, field.height_at(t.x, t.z), t.z);
            solid.push((m, Instance::still(Affine::translate(at))));
            bare.push(((t.x, t.z), t.trunk + cm(80)));
        }
        let mut stones = wall::stones(map, seed);
        for (k, c) in map.cairns.iter().enumerate() {
            let m = bank.add(shapes::cairn_core(c));
            solid.push((m, Instance::still(Affine::translate(v3(c.x, 0, c.z)))));
            stones.extend(cairn_stones(c, seed.wrapping_add(k as u32 * 19 + 40)));
            bare.push(((c.x, c.z), c.tiers[0].radius + cm(30)));
        }
        for (k, s) in map.slabs.iter().enumerate() {
            stones.extend(slab_stones(s, &field, seed.wrapping_add(k as u32 * 23 + 500)));
            bare.push((s.center(), s.reach() + cm(30)));
        }
        let middle = (map.length() / 2, map.width() / 2);
        for m in map.mouths() {
            let dir = unit((middle.0 - m.0, middle.1 - m.1));
            let back = mul(cm(110), MOUTH_SCALE);
            let t = v3(
                m.0 - mul(dir.0, back),
                field.height_at(m.0, m.1),
                m.1 - mul(dir.1, back),
            );
            let size = Affine::scale(v3(MOUTH_SCALE, MOUTH_SCALE, MOUTH_SCALE));
            solid.push((bank_of_earth, Instance::still(facing_place(t, dir).then(&size))));
            bare.push((m, mul(cm(360), MOUTH_SCALE)));
        }
        for m in map.mounds.iter().chain(map.spare_mounds.iter()) {
            bare.push(((m.x, m.z), cm(95)));
        }

        let first = bank.ranges.len();
        for k in 0..ROUND_SHAPES as u32 {
            bank.add(shapes::stone(k * 3 + seed, 0, cm(70)));
        }
        let cheek = bank.add(shapes::stone(seed + 77, 0, ONE));
        let far = bank.add(shapes::far_stone());
        let plants: Vec<usize> = Kind::ALL
            .iter()
            .map(|&k| bank.add(dress::prefab(k, seed)))
            .collect();
        let far_plant = bank.add(dress::far_prefab());

        let mut b = Batch::new(bank.ranges.len());
        for (m, i) in &solid {
            b.push(*m, i, None);
        }
        let (mut instances, mut rows, _) = b.finish(&bank.ranges);
        kept(&mut rows, 0);

        // the stones, by square
        let mut keyed: Vec<((i32, i32), &Stone)> = stones
            .iter()
            .map(|s| ((s.at[0] >> SQUARE_SHIFT, s.at[2] >> SQUARE_SHIFT), s))
            .collect();
        keyed.sort_by_key(|k| k.0);
        let shape_of = |s: &Stone| {
            if s.course == Course::Cheek {
                cheek
            } else {
                first + (s.shape % ROUND_SHAPES) as usize
            }
        };
        let mut squares = Vec::new();
        let mut at = 0;
        while at < keyed.len() {
            let key = keyed[at].0;
            let end = at + keyed[at..].iter().take_while(|k| k.0 == key).count();
            let mut b = Batch::new(bank.ranges.len());
            for (_, s) in &keyed[at..end] {
                let i = Instance {
                    coat: s.tone,
                    ..Instance::still(wall_stone_place(s))
                };
                b.push(shape_of(s), &i, None);
            }
            let (values, mut near, _) = b.finish(&bank.ranges);
            kept(&mut near, instances.len());
            let many = (end - at) as u32;
            let f = bank.ranges[far];
            let start = (instances.len() * BYTES_PER_VALUE) as u32;
            let far_row = vec![
                RIGID,
                WORLD_BANK,
                f.first * BYTES_PER_VALUE as u32,
                f.count,
                WORLD_SOURCE,
                start,
                many,
            ];
            instances.extend_from_slice(&values);
            let half = 1 << (SQUARE_SHIFT - 1);
            squares.push(Square {
                center: ((key.0 << SQUARE_SHIFT) + half, (key.1 << SQUARE_SHIFT) + half),
                radius: half * 3 / 2,
                near,
                far: far_row,
            });
            at = end;
        }

        // the dressing, by square: a far square leaves out what is thin
        let mut dressing = dress::scatter(map, seed, &wet);
        let square_of = |d: &dress::Dressing| (d.at.0 >> SQUARE_SHIFT, d.at.1 >> SQUARE_SHIFT);
        dressing.sort_by_key(|d| (square_of(d), d.kind));
        for group in dressing.chunk_by(|a, b| square_of(a) == square_of(b)) {
            let mut b = Batch::new(bank.ranges.len());
            for d in group {
                let y = field.height_at(d.at.0, d.at.1);
                b.push(plants[d.kind as usize], &d.instance(y), None);
            }
            let many = group.iter().filter(|d| !d.kind.thin()).count() as u32;
            let (values, mut near, _) = b.finish(&bank.ranges);
            kept(&mut near, instances.len());
            let f = bank.ranges[far_plant];
            let start = (instances.len() * BYTES_PER_VALUE) as u32;
            let far_row = vec![
                RIGID,
                WORLD_BANK,
                f.first * BYTES_PER_VALUE as u32,
                f.count,
                WORLD_SOURCE,
                start,
                many,
            ];
            instances.extend_from_slice(&values);
            let half = 1 << (SQUARE_SHIFT - 1);
            let key = square_of(&group[0]);
            squares.push(Square {
                center: ((key.0 << SQUARE_SHIFT) + half, (key.1 << SQUARE_SHIFT) + half),
                radius: half * 3 / 2,
                near,
                far: far_row,
            });
        }

        let top = map.fold.center;
        let high = field.height_at(top.0, top.1) + from_int(9);
        let mounds = map
            .mounds
            .iter()
            .chain(map.spare_mounds.iter())
            .map(|m| {
                let [nx, ny, nz] = field.normal_near(m.x, m.z);
                (v3(m.x, field.height_at(m.x, m.z), m.z), v3(nx, ny, nz))
            })
            .collect();
        Scape {
            meshes: bank.ranges.len(),
            stones: stones.len(),
            field,
            wind,
            bank,
            instances,
            rows,
            squares,
            bare,
            fresh,
            dug,
            arrow,
            scent,
            fur,
            blood,
            carrot,
            arrow_at: v3(top.0, high, top.1),
            mounds,
        }
    }

    /// The rows of kept things for this eye: everything that's always drawn
    /// and each square of stones in view, near or far.
    pub fn rows_for(&self, eye: V3, fwd: V3) -> Vec<u32> {
        let mut out = self.rows.clone();
        let flat = fx::length(fwd.x, fwd.z).max(1);
        let (fx_, fz) = (fx::div(fwd.x, flat), fx::div(fwd.z, flat));
        for s in &self.squares {
            let (dx, dz) = (s.center.0 - eye.x, s.center.1 - eye.z);
            let d = fx::length(dx, dz);
            let ahead = mul(dx >> 2, fx_) + mul(dz >> 2, fz);
            if ahead < -(s.radius + from_int(12)) / 4 {
                continue;
            }
            out.extend_from_slice(if d - s.radius < NEAR_STONES {
                &s.near
            } else {
                &s.far
            });
        }
        out
    }

    /// What changes in a match: its mounds as they stand, its tokens and the
    /// Fold's arrow. Values, draws and outlines; the outlines follow the
    /// draws' values, so a row's offset reads the same in either.
    pub fn moving_for(
        &self,
        mounds: &[MoundDraw],
        tokens: &[TokenDraw],
        tick: u32,
    ) -> (Vec<i32>, Vec<u32>, Vec<u32>) {
        let mut b = Batch::new(self.meshes);
        for m in mounds {
            let mesh = if m.dug { self.dug } else { self.fresh };
            b.push(mesh, &Instance::still(stand(m.at, 0, m.up, ONE)), m.outline);
        }
        for t in tokens {
            let mesh = match t.token {
                Token::Scent => self.scent,
                Token::Carrot => self.carrot,
                Token::Fur => self.fur,
                Token::Blood => self.blood,
            };
            b.push(mesh, &Instance::still(t.place), None);
        }
        b.push(self.arrow, &Instance::still(self.arrow_place(tick)), None);
        let (values, mut rows, mut lines) = b.finish(&self.bank.ranges);
        for row in rows.chunks_mut(DRAW_VALUES).chain(lines.chunks_mut(DRAW_VALUES)) {
            row[1] = WORLD_BANK;
            row[4] = FRAME_SOURCE;
        }
        (values, rows, lines)
    }

    fn arrow_place(&self, tick: u32) -> Affine {
        let beat = (tick % 120) as Fx * TURN / 120;
        let spin = (tick % 300) as Fx * TURN / 300;
        let tall = from_int(5);
        let bob = v3(0, mul(fx::sin(beat), cm(70)), 0);
        Affine::translate(self.arrow_at.add(bob))
            .then(&Affine::rot_y(spin))
            .then(&Affine::scale(v3(tall, tall, tall)))
    }

    /// What changes: the mounds, `dug` saying which are dug up, and the
    /// Fold's arrow, bobbing and turning from the tick. The arrow is a pure
    /// function of the tick, so rollback has nothing to keep.
    pub fn moving(&self, dug: &dyn Fn(usize) -> bool, tick: u32) -> (Vec<i32>, Vec<u32>) {
        let mut b = Batch::new(self.meshes);
        for (k, m) in self.mounds.iter().enumerate() {
            let mesh = if dug(k) { self.dug } else { self.fresh };
            b.push(mesh, &Instance::still(stand(m.0, 0, m.1, ONE)), None);
        }
        b.push(self.arrow, &Instance::still(self.arrow_place(tick)), None);
        let (values, mut rows, _) = b.finish(&self.bank.ranges);
        for row in rows.chunks_mut(DRAW_VALUES) {
            row[1] = WORLD_BANK;
            row[4] = FRAME_SOURCE;
        }
        (values, rows)
    }

    /// Bakes the grass, separately so its cost can be read on its own.
    pub fn bake_grass(&self, map: &Map, seed: u32) -> Bake {
        grass::bake(
            &Clear {
                map,
                field: &self.field,
                bare: &self.bare,
            },
            seed,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built(seed: u64) -> (Map, Scape) {
        let map = Map::generate(seed);
        let s = Scape::build(&map, seed as u32);
        (map, s)
    }

    #[test]
    fn every_row_stays_inside_the_bank_and_the_instances() {
        let (_, s) = built(1);
        let bytes = s.instances.len() * BYTES_PER_VALUE;
        for row in s
            .rows_for(v3(from_int(30), from_int(4), from_int(30)), v3(ONE, 0, 0))
            .chunks(DRAW_VALUES)
        {
            let index_end = row[2] as usize / BYTES_PER_VALUE + row[3] as usize;
            assert!(index_end <= s.bank.indices.len());
            let values = row[5] as usize + row[6] as usize * crate::render::INSTANCE_VALUES * BYTES_PER_VALUE;
            assert!(values <= bytes, "{values} {bytes}");
            assert_eq!(row[1], WORLD_BANK);
        }
    }

    #[test]
    fn a_far_square_draws_one_row_of_eight_sided_stones_and_a_near_one_a_row_a_shape() {
        let (_, s) = built(2);
        let sq = s.squares.iter().max_by_key(|q| q.near.len()).unwrap();
        assert!(sq.near.len() >= 3 * DRAW_VALUES);
        assert_eq!(sq.far.len(), DRAW_VALUES);
        let eye = |d: Fx| v3(sq.center.0 - d, from_int(3), sq.center.1);
        let hits = |e: V3, row: &[u32]| {
            s.rows_for(e, v3(ONE, 0, 0))
                .chunks(DRAW_VALUES)
                .any(|r| r == &row[..DRAW_VALUES])
        };
        assert!(hits(eye(from_int(20)), &sq.near));
        assert!(!hits(eye(from_int(20)), &sq.far));
        assert!(hits(eye(from_int(240)), &sq.far));
    }

    #[test]
    fn nothing_behind_the_eye_is_drawn() {
        let (_, s) = built(3);
        let sq = &s.squares[0];
        let eye = v3(sq.center.0 + from_int(180), from_int(3), sq.center.1);
        let rows = s.rows_for(eye, v3(ONE, 0, 0));
        assert!(rows
            .chunks(DRAW_VALUES)
            .all(|r| r != &sq.near[..DRAW_VALUES] && r != &sq.far[..DRAW_VALUES]));
    }

    #[test]
    fn a_slabs_stones_fill_the_outline_that_stops_a_body() {
        for seed in 1..20 {
            let map = Map::generate(seed);
            let field = Field::build(&map, seed as u32);
            assert!(!map.slabs.is_empty(), "seed {seed}");
            for s in &map.slabs {
                let mid = s.center();
                let (dx, dz) = (s.b.0 - s.a.0, s.b.1 - s.a.1);
                let long = fx::length(dx, dz);
                let way = (fx::div(dx, long), fx::div(dz, long));
                let (mut lo, mut hi) = (0, 0);
                for st in &slab_stones(s, &field, seed as u32) {
                    // the stone's two ends, along the slab from its middle
                    let (ox, oz) = (st.at[0] - mid.0, st.at[2] - mid.1);
                    let along = mul(ox, way.0) + mul(oz, way.1);
                    lo = lo.min(along - st.size[0] / 2);
                    hi = hi.max(along + st.size[0] / 2);
                    assert!(st.size[2] / 2 <= s.half, "{st:?}");
                    assert!(st.size[1] - SLAB_SUNK <= s.top + cm(1), "{st:?}");
                }
                // the row reaches each end, plus half the overlap
                let end = long / 2 + s.half + SLAB_OVERLAP / 2;
                assert!((hi - end).abs() <= cm(2), "{hi} against {end}");
                assert!((lo + end).abs() <= cm(2), "{lo} against {end}");
            }
        }
    }

    #[test]
    fn each_step_of_a_cairn_has_boulders_level_with_its_top() {
        let map = Map::generate(5);
        let c = &map.cairns[0];
        let stones = cairn_stones(c, 1);
        for step in &c.tiers {
            let on = stones
                .iter()
                .filter(|s| (s.at[1] + s.size[1] / 2 - step.top).abs() < cm(8))
                .count();
            assert!(on >= 5, "{on}");
        }
    }

    #[test]
    fn a_stone_of_a_wall_keeps_its_size_and_stands_upright() {
        let s = Stone {
            at: [from_int(10), ONE, from_int(20)],
            size: [cm(60), cm(30), cm(40)],
            yaw: TURN / 8,
            lean: 0,
            tone: pal::WHIN,
            course: Course::Flat,
            shape: 0,
        };
        let a = wall_stone_place(&s);
        assert!((a.c1.y - cm(30)).abs() < 4);
        assert!((fx::length(a.c0.x, a.c0.z) - cm(60)).abs() < cm(1));
        assert!(
            a.c0.cross(a.c1).dot(a.c2) > 0,
            "a mirrored stone would light backward"
        );
    }

    #[test]
    fn the_arrow_bobs_with_the_tick_and_rolling_back_gives_the_same_place() {
        let (_, s) = built(6);
        let at = |tick| s.moving(&|_| false, tick).0;
        assert_eq!(at(77), at(77));
        assert_ne!(at(0), at(30));
    }

    #[test]
    fn a_dug_mound_draws_its_own_mesh() {
        let (_, s) = built(6);
        let rows = |dug: bool| s.moving(&|_| dug, 0).1;
        assert_ne!(rows(true)[2], rows(false)[2]);
    }
}
