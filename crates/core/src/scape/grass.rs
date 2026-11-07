//! Grass, baked once per match (notes/grass.md, after Chumworld). Opaque,
//! vertex-colored three-sided shapes in chunks, no cards and no alpha. Each
//! one mixes a grass tone into the ground under it, so contrast stays low
//! and the grass doesn't shimmer. Blades carry the near view, as spikes of
//! three triangles because there are many of them; tufts carry the middle
//! distance, rush clumps stand proud here and there, and the ground's own
//! shading carries the rest.
//!
//! Vertices are packed into 16 bytes, so the whole map is a few megabytes
//! and needs no float: the page uploads the bytes as they are. Every prism
//! has the same index pattern, so all chunks share one index buffer.

use crate::fx::{self, from_int, Fx, ONE};
use crate::map::{table, Map, P};

use super::cm;
use super::field::{to_segment, Field};
use super::noise::{band, blotch, fbm, lattice, lerp};
use super::palette::{self as pal, mix, pct, Tone};

/// One prism: three corners at the foot, three at the top.
pub const PRISM_VERTICES: usize = 6;
/// Three sides of two triangles each, and a blunt top.
pub const PRISM_INDICES: usize = 21;
/// One spike: three corners at the foot and one tip.
pub const SPIKE_VERTICES: usize = 4;
/// Three sides of one triangle each.
pub const SPIKE_INDICES: usize = 9;
pub const VERTEX_BYTES: usize = 16;

/// Packed positions run from here, in steps of `POS_STEP_SHIFT`.
pub const ORIGIN: [Fx; 3] = [-table::TERRAIN_MARGIN, -cm(2000), -table::TERRAIN_MARGIN];
/// A packed x or z step is 1/128 m; a packed height step is 1/1024 m.
pub const STEP_SHIFT_XZ: u32 = 9;
pub const STEP_SHIFT_Y: u32 = 6;

/// How one tier of grass is placed and drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tier {
    pub cell: Fx,
    /// Percent of cells that hold a clump.
    pub fill: i32,
    pub chunk: Fx,
    /// Prisms in a clump, from this many to this many.
    pub prisms: (i32, i32),
    pub wide: Fx,
    pub tall: (Fx, Fx),
    pub lean: (Fx, Fx),
    /// Percent of a grass tone mixed into the ground under it.
    pub tint: i32,
    /// Fully in from `.1`, starting at `.0`; gone by `.3`, starting at `.2`.
    pub fade: [Fx; 4],
    /// Spikes, not prisms.
    pub spike: bool,
}

impl Tier {
    #[cfg(test)]
    fn vertices(&self) -> usize {
        if self.spike {
            SPIKE_VERTICES
        } else {
            PRISM_VERTICES
        }
    }

    fn indices(&self) -> usize {
        if self.spike {
            SPIKE_INDICES
        } else {
            PRISM_INDICES
        }
    }
}

pub const BLADES: Tier = Tier {
    cell: cm(45),
    fill: 88,
    chunk: cm(1800),
    prisms: (3, 3),
    wide: cm(9),
    tall: (cm(11), cm(22)),
    lean: (cm(4), cm(11)),
    tint: 70,
    fade: [-cm(100), 0, cm(1700), cm(2600)],
    spike: true,
};

pub const TUFTS: Tier = Tier {
    cell: cm(120),
    fill: 65,
    chunk: cm(3600),
    prisms: (1, 2),
    wide: cm(28),
    tall: (cm(12), cm(22)),
    lean: (cm(2), cm(5)),
    tint: 25,
    fade: [cm(1200), cm(2000), cm(6200), cm(7800)],
    spike: false,
};

/// Rush clumps: narrow, tall and straw-tipped.
pub const RUSHES: Tier = Tier {
    cell: cm(240),
    fill: 100,
    chunk: cm(3600),
    prisms: (7, 11),
    wide: cm(7),
    tall: (cm(28), cm(52)),
    lean: (cm(6), cm(22)),
    tint: 80,
    fade: [-cm(100), 0, cm(9000), cm(11000)],
    spike: false,
};

pub const TIERS: [Tier; 3] = [BLADES, TUFTS, RUSHES];

/// The sward's mottling, in the ground's shading and baked into the blades:
/// patches 6 m across, clumps of 1.6 m and, on the ground alone, speckle of
/// 30 cm (cycles a meter), each settling to its middle value with distance.
pub const MOTTLE_SIZES: [Fx; 3] = [ONE / 6, ONE * 5 / 8, ONE * 10 / 3];
/// How fast a size settles: gone 70 of its wavelengths out, 21 m for the
/// speckle, where blades and tufts already cover it.
pub const MOTTLE_FAR: Fx = ONE / 70;
/// The three sizes' shares of the mottling.
pub const MOTTLE_MIX: [Fx; 3] = [ONE * 2 / 5, ONE * 2 / 5, ONE / 5];
/// Where the clumps' lattice sits against the patches', in cells.
pub const MOTTLE_SHIFT_XZ: P = (from_int(17), from_int(5));
/// The part of the mix that swings from the low end to the high end.
pub const MOTTLE_BAND: [Fx; 2] = [cm(32), cm(68)];
/// What the low and high ends multiply by: darker and bluer, paler and strawier.
pub const MOTTLE_LOW: Tone = [ONE * 80 / 100, ONE * 84 / 100, ONE * 78 / 100];
pub const MOTTLE_HIGH: Tone = [ONE * 118 / 100, ONE * 112 / 100, ONE * 106 / 100];

/// What the mottling multiplies by at (x, z): the shader's sum with the
/// speckle at its middle value.
fn mottle(x: Fx, z: Fx) -> Tone {
    let broad = blotch(fx::mul(x, MOTTLE_SIZES[0]), fx::mul(z, MOTTLE_SIZES[0]));
    let clump = blotch(
        fx::mul(x, MOTTLE_SIZES[1]) + MOTTLE_SHIFT_XZ.0,
        fx::mul(z, MOTTLE_SIZES[1]) + MOTTLE_SHIFT_XZ.1,
    );
    let sum = fx::mul(broad, MOTTLE_MIX[0]) + fx::mul(clump, MOTTLE_MIX[1]) + MOTTLE_MIX[2] / 2;
    let m = band(MOTTLE_BAND[0], MOTTLE_BAND[1], sum) * 2 - ONE;
    let end = if m < 0 { MOTTLE_LOW } else { MOTTLE_HIGH };
    [0, 1, 2].map(|i| lerp(ONE, end[i], m.abs()))
}

/// A tone times a multiplier, clipped to white.
fn times(c: Tone, by: Tone) -> Tone {
    [0, 1, 2].map(|i| fx::mul(c[i], by[i]).min(ONE))
}

/// One chunk's place in the baked bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    pub tier: usize,
    pub center: P,
    pub radius: Fx,
    pub first_byte: usize,
    pub prisms: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bake {
    pub vertices: Vec<u8>,
    pub chunks: Vec<Chunk>,
}

/// The same pattern for every prism, up to the largest chunk, and after them
/// the same for every spike.
pub fn shared_indices(prisms: usize, spikes: usize) -> Vec<u16> {
    let mut out = Vec::with_capacity(prisms * PRISM_INDICES + spikes * SPIKE_INDICES);
    for p in 0..prisms {
        let b = (p * PRISM_VERTICES) as u16;
        for side in 0..3u16 {
            let n = (side + 1) % 3;
            out.extend_from_slice(&[b + side, b + n, b + 3 + side]);
            out.extend_from_slice(&[b + n, b + 3 + n, b + 3 + side]);
        }
        out.extend_from_slice(&[b + 3, b + 4, b + 5]);
    }
    for s in 0..spikes {
        let b = (s * SPIKE_VERTICES) as u16;
        for side in 0..3u16 {
            out.extend_from_slice(&[b + side, b + (side + 1) % 3, b + 3]);
        }
    }
    out
}

fn pack_pos(p: [Fx; 3]) -> [u16; 3] {
    [
        ((p[0] - ORIGIN[0]) >> STEP_SHIFT_XZ).clamp(0, 65535) as u16,
        ((p[1] - ORIGIN[1]) >> STEP_SHIFT_Y).clamp(0, 65535) as u16,
        ((p[2] - ORIGIN[2]) >> STEP_SHIFT_XZ).clamp(0, 65535) as u16,
    ]
}

fn byte(v: Fx) -> u8 {
    ((v.clamp(0, ONE) as i64 * 255) >> 16) as u8
}

fn signed_byte(v: Fx) -> u8 {
    (((v.clamp(-ONE, ONE) as i64 * 127) >> 16) as i8) as u8
}

fn push_vertex(out: &mut Vec<u8>, p: [Fx; 3], color: Tone, bend: Fx, normal: [Fx; 3]) {
    for c in pack_pos(p) {
        out.extend_from_slice(&c.to_le_bytes());
    }
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&[byte(color[0]), byte(color[1]), byte(color[2]), byte(bend)]);
    out.extend_from_slice(&[
        signed_byte(normal[0]),
        signed_byte(normal[1]),
        signed_byte(normal[2]),
        0,
    ]);
}

/// Three corners round (0,0) at this radius, turned by `a`.
fn triangle(a: i32, r: Fx) -> [P; 3] {
    let third = fx::TURN / 3;
    [0, 1, 2].map(|i| {
        let b = a + i * third;
        (fx::mul(fx::cos(b), r), fx::mul(fx::sin(b), r))
    })
}

/// A grass prism with its foot at `foot`, leaning by `lean` on the ground.
#[allow(clippy::too_many_arguments)]
fn push_prism(
    out: &mut Vec<u8>,
    foot: [Fx; 3],
    tall: Fx,
    wide: Fx,
    lean: P,
    turn: i32,
    base: Tone,
    tip: Tone,
    normal: [Fx; 3],
) {
    let top_y = foot[1] + tall;
    // bend reaches its full strength at `BEND_FULL`, so a short blade bends
    // less than a tall one
    let bend = fx::div(tall.min(BEND_FULL), BEND_FULL);
    for (x, z) in triangle(turn, wide / 2) {
        push_vertex(out, [foot[0] + x, foot[1], foot[2] + z], base, 0, normal);
    }
    for (x, z) in triangle(turn, wide / 5) {
        push_vertex(
            out,
            [foot[0] + x + lean.0, top_y, foot[2] + z + lean.1],
            tip,
            bend,
            normal,
        );
    }
}

/// A blade with one tip, for a tier with so many that a prism's seven
/// triangles and six corners cost too much.
#[allow(clippy::too_many_arguments)]
fn push_spike(
    out: &mut Vec<u8>,
    foot: [Fx; 3],
    tall: Fx,
    wide: Fx,
    lean: P,
    turn: i32,
    base: Tone,
    tip: Tone,
    normal: [Fx; 3],
) {
    for (x, z) in triangle(turn, wide / 2) {
        push_vertex(out, [foot[0] + x, foot[1], foot[2] + z], base, 0, normal);
    }
    let at = [foot[0] + lean.0, foot[1] + tall, foot[2] + lean.1];
    let bend = fx::div(tall.min(BEND_FULL), BEND_FULL);
    push_vertex(out, at, tip, bend, normal);
}

/// Percent of a measured grass color a blade's tip is drawn in: the soft
/// light adds the rest back, as on the ground.
const TIP_LIT: i32 = 84;
/// Percent of the ground's color a blade's foot is drawn in, so the sward has
/// depth among its own blades.
const FOOT_SHADE: i32 = 84;
/// How tall a blade must be to take the wind's full bend.
pub const BEND_FULL: Fx = cm(20);
/// A foot is sunk this far, so a sloping cell shows no gap under it.
const FOOT_SINK: Fx = cm(2);

/// Random numbers for one cell of a tier: a fresh draw for each `n`.
fn draw(seed: u32, tier: usize, c: i32, r: i32, n: u32) -> Fx {
    lattice(seed.wrapping_add(n * 131 + tier as u32 * 977), c, r)
}

fn between(lo: Fx, hi: Fx, t: Fx) -> Fx {
    lerp(lo, hi, t)
}

/// The tones a blade's foot and tip take over this ground.
fn tones(ground: Tone, tier: usize, t: Fx, moor: bool) -> (Tone, Tone) {
    if tier == 2 {
        let rush = mix(pal::RUSH_TIP, pal::MOOR_PALE, if moor { ONE } else { 0 });
        return (pal::RUSH_FOOT, mix(mix(pal::RUSH_FOOT, rush, t), ground, cm(15)));
    }
    let share = TIERS[tier].tint * ONE / 100;
    let grass = if moor {
        pal::MOOR_PALE
    } else if t < ONE / 2 {
        pal::TURF_LUSH
    } else {
        pal::TURF
    };
    let foot = mix(ground, pal::TURF_DEEP, if moor { 0 } else { cm(30) });
    let foot = if tier == 0 { pct(foot, FOOT_SHADE) } else { foot };
    let tip = mix(pct(ground, 110), pct(grass, TIP_LIT), share);
    (foot, tip)
}

/// What can stop grass growing at a point.
pub struct Clear<'a> {
    pub map: &'a Map,
    pub field: &'a Field,
    /// Round bare patches: a center and a radius.
    pub bare: &'a [(P, Fx)],
}

impl Clear<'_> {
    fn near_wall(&self, p: P, reach: Fx) -> bool {
        self.map.walls.iter().any(|w| to_segment(p, w.a, w.b) < reach)
    }

    /// Whether this tier grows at `p`. `t` is the cell's own chance draw.
    fn grows(&self, tier: usize, p: P, t: Fx) -> bool {
        if self
            .bare
            .iter()
            .any(|&(c, r)| fx::length(p.0 - c.0, p.1 - c.1) < r)
        {
            return false;
        }
        // a wall stands about half a meter wide each way, and a clump's
        // prisms spread up to 30 cm from its middle
        if self.near_wall(p, cm(90)) {
            return false;
        }
        let worn = self.field.worn_near(p.0, p.1);
        if t < worn {
            return false;
        }
        if tier == 2 {
            return !self.map.in_fold(p.0, p.1);
        }
        true
    }
}

/// Where rushes grow: loose wet lines, and along a wall's foot.
fn rushes_want(seed: u32, clear: &Clear, p: P, t: Fx) -> bool {
    let wet = band(cm(60), cm(72), fbm(seed + 301, p.0, p.1, 21, 3));
    let by_wall = if clear.near_wall(p, cm(320)) { ONE / 8 } else { 0 };
    t < wet / 4 + by_wall
}

/// Bakes the whole map's grass. Walls, worn ground and `bare` keep it off.
pub fn bake(clear: &Clear, seed: u32) -> Bake {
    let mut out = Bake {
        vertices: Vec::new(),
        chunks: Vec::new(),
    };
    for (t, tier) in TIERS.iter().enumerate() {
        let across = |len: Fx| (len + 2 * table::TERRAIN_MARGIN) / tier.chunk + 1;
        for cz in 0..across(clear.map.width()) {
            for cx in 0..across(clear.map.length()) {
                let start = out.vertices.len();
                let lo = (
                    -table::TERRAIN_MARGIN + cx * tier.chunk,
                    -table::TERRAIN_MARGIN + cz * tier.chunk,
                );
                let cells = tier.chunk / tier.cell;
                let mut prisms = 0;
                for r in 0..cells {
                    for c in 0..cells {
                        let (gc, gr) = (cx * cells + c, cz * cells + r);
                        prisms += clump(t, tier, clear, seed, lo, (c, r), (gc, gr), &mut out.vertices);
                    }
                }
                if prisms > 0 {
                    let half = tier.chunk / 2;
                    out.chunks.push(Chunk {
                        tier: t,
                        center: (lo.0 + half, lo.1 + half),
                        radius: half * 3 / 2,
                        first_byte: start,
                        prisms: prisms as usize,
                    });
                }
            }
        }
    }
    out
}

/// One cell's clump of grass, if it has one. Returns how many prisms it laid.
#[allow(clippy::too_many_arguments)]
fn clump(
    t: usize,
    tier: &Tier,
    clear: &Clear,
    seed: u32,
    lo: P,
    local: (Fx, Fx),
    global: (Fx, Fx),
    out: &mut Vec<u8>,
) -> i32 {
    let (gc, gr) = global;
    let d = |n: u32| draw(seed, t, gc, gr, n);
    if d(0) >= tier.fill * ONE / 100 {
        return 0;
    }
    let p = (
        lo.0 + local.0 * tier.cell + fx::mul(d(1), tier.cell),
        lo.1 + local.1 * tier.cell + fx::mul(d(2), tier.cell),
    );
    let moor = !clear.map.inside(p.0, p.1);
    if moor {
        // blades stop at the wall; past it only tufts and rushes, thinly
        if t == 0 || d(7) < ONE / 2 {
            return 0;
        }
    }
    if !clear.grows(t, p, d(3)) || (t == 2 && !rushes_want(seed, clear, p, d(4))) {
        return 0;
    }
    let ground = times(clear.field.color_near(p.0, p.1), mottle(p.0, p.1));
    let normal = clear.field.normal_near(p.0, p.1);
    let count = between(tier.prisms.0 * ONE, (tier.prisms.1 + 1) * ONE, d(5)) >> 16;
    for i in 0..count {
        let n = 10 + 8 * i as u32;
        let e = |k: u32| d(n + k);
        // a clump's prisms stand in a small ring round its middle
        let spread = match t {
            0 => tier.cell,
            1 => 0,
            _ => tier.cell / 4,
        };
        let (ox, oz) = (fx::mul(e(0) - ONE / 2, spread), fx::mul(e(1) - ONE / 2, spread));
        let (x, z) = (p.0 + ox, p.1 + oz);
        let foot = [x, clear.field.height_at(x, z) - FOOT_SINK, z];
        let tall = between(tier.tall.0, tier.tall.1, e(2));
        let lean_r = between(tier.lean.0, tier.lean.1, e(3));
        // rushes splay outward; grass leans any way
        let way = if t == 2 {
            super::wall::angle_of(ox.max(1), oz)
        } else {
            e(4)
        };
        let lean = (fx::mul(fx::cos(way), lean_r), fx::mul(fx::sin(way), lean_r));
        let (base, tip) = tones(ground, t, e(5), moor);
        if tier.spike {
            push_spike(out, foot, tall, tier.wide, lean, e(6), base, tip, normal);
        } else {
            push_prism(out, foot, tall, tier.wide, lean, e(6), base, tip, normal);
        }
    }
    count
}

/// How much of the bake a frame may draw: every chunk near the eye and not
/// behind it. Values per draw: tier, first byte, index count, and the byte in
/// the shared indices its pattern starts at.
pub const DRAW_VALUES: usize = 4;

pub fn visible(bake: &Bake, eye: [Fx; 3], fwd: [Fx; 3]) -> Vec<u32> {
    let flat = fx::length(fwd[0], fwd[2]).max(1);
    let (fx_, fz) = (fx::div(fwd[0], flat), fx::div(fwd[2], flat));
    let mut out = Vec::new();
    let spikes_from = (most_prisms(bake) * PRISM_INDICES * 2) as u32;
    for c in &bake.chunks {
        let (dx, dz) = (c.center.0 - eye[0], c.center.1 - eye[2]);
        let d = fx::length(dx, dz);
        if d - c.radius > TIERS[c.tier].fade[3] {
            continue;
        }
        // behind the camera by more than a chunk's reach, or off to the side:
        // the view is 41 degrees each way, inside this 45-degree wedge
        let ahead = fx::mul(dx, fx_) + fx::mul(dz, fz);
        let aside = fx::mul(dx, fz) - fx::mul(dz, fx_);
        if ahead < -c.radius - cm(600) || aside.abs() > ahead.max(0) + c.radius + cm(600) {
            continue;
        }
        let tier = &TIERS[c.tier];
        out.extend_from_slice(&[
            c.tier as u32,
            c.first_byte as u32,
            (c.prisms * tier.indices()) as u32,
            if tier.spike { spikes_from } else { 0 },
        ]);
    }
    out
}

fn most_of(bake: &Bake, spike: bool) -> usize {
    let found = bake.chunks.iter().filter(|c| TIERS[c.tier].spike == spike);
    found.map(|c| c.prisms).max().unwrap_or(0)
}

/// The most prisms in a chunk, which sizes the shared index buffer.
pub fn most_prisms(bake: &Bake) -> usize {
    most_of(bake, false)
}

/// The most spikes in a chunk, which sizes the rest of it.
pub fn most_spikes(bake: &Bake) -> usize {
    most_of(bake, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::from_int;
    use crate::map::MapSize;

    fn baked(seed: u64) -> (Map, Field, Bake) {
        let map = Map::generate(seed);
        let field = Field::build(&map, seed as u32);
        let b = bake(
            &Clear {
                map: &map,
                field: &field,
                bare: &[],
            },
            seed as u32,
        );
        (map, field, b)
    }

    fn feet(bake: &Bake, chunk: &Chunk) -> Vec<(Fx, Fx, Fx)> {
        let each = TIERS[chunk.tier].vertices() * VERTEX_BYTES;
        let bytes = &bake.vertices[chunk.first_byte..chunk.first_byte + chunk.prisms * each];
        bytes
            .chunks(each)
            .map(|p| {
                let u = |i: usize| u16::from_le_bytes([p[i], p[i + 1]]) as Fx;
                (
                    (u(0) << STEP_SHIFT_XZ) + ORIGIN[0],
                    (u(2) << STEP_SHIFT_Y) + ORIGIN[1],
                    (u(4) << STEP_SHIFT_XZ) + ORIGIN[2],
                )
            })
            .collect()
    }

    #[test]
    fn a_prism_has_seven_triangles_and_shares_one_index_pattern() {
        let idx = shared_indices(2, 0);
        assert_eq!(idx.len(), 42);
        assert_eq!(*idx.iter().max().unwrap(), 11);
        assert_eq!(idx[21] - idx[0], 6, "the second prism starts six vertices on");
    }

    #[test]
    fn a_spike_has_three_sides_meeting_at_its_tip_and_follows_the_prisms() {
        let idx = shared_indices(1, 2);
        assert_eq!(idx.len(), 21 + 18);
        // the spikes' pattern counts from the first corner again, as each
        // draw binds its own chunk: 0 1 3, 1 2 3, 2 0 3, then 4 on
        assert_eq!(&idx[21..30], &[0, 1, 3, 1, 2, 3, 2, 0, 3]);
        assert_eq!(&idx[30..33], &[4, 5, 7]);
    }

    #[test]
    fn a_blade_on_a_dark_patch_is_darker_than_one_on_a_pale_patch() {
        // scan the mottle for its two ends: they must be the shader's ends
        let spots = (0..400).flat_map(|i| (0..40).map(move |k| (i * ONE / 3, k * ONE)));
        let tones: Vec<Tone> = spots.map(|(x, z)| mottle(x, z)).collect();
        let lowest = tones.iter().map(|t| t[1]).min().unwrap();
        let highest = tones.iter().map(|t| t[1]).max().unwrap();
        assert!(lowest < ONE * 9 / 10, "{lowest}");
        assert!(highest > ONE * 108 / 100, "{highest}");
        assert_eq!(
            times([ONE, ONE / 2, 0], MOTTLE_HIGH),
            [ONE, MOTTLE_HIGH[1] / 2, 0]
        );
    }

    #[test]
    fn a_frame_leaves_out_a_chunk_well_off_to_the_side() {
        let (_, _, b) = baked(5);
        let (ex, ez) = (from_int(130), from_int(70));
        let list = visible(&b, [ex, from_int(14), ez], [ONE, 0, 0]);
        for d in list.chunks(DRAW_VALUES) {
            let c = b.chunks.iter().find(|c| c.first_byte as u32 == d[1]).unwrap();
            let (ahead, aside) = ((c.center.0 - ex).max(0), (c.center.1 - ez).abs());
            assert!(aside <= ahead + c.radius + from_int(7), "{:?}", c.center);
        }
    }

    #[test]
    fn a_blade_draw_starts_at_the_spike_pattern() {
        let (_, _, b) = baked(5);
        let list = visible(&b, [from_int(130), from_int(14), from_int(70)], [ONE, 0, 0]);
        let first = (most_prisms(&b) * PRISM_INDICES * 2) as u32;
        let (mut blades, mut tufts) = (0, 0);
        for d in list.chunks(DRAW_VALUES) {
            if d[0] == 0 {
                blades += 1;
                assert_eq!(d[3], first);
                assert_eq!(d[2] % 9, 0, "whole spikes");
            } else {
                tufts += 1;
                assert_eq!(d[3], 0);
                assert_eq!(d[2] % 21, 0, "whole prisms");
            }
        }
        assert!(blades > 0 && tufts > 0, "{blades} {tufts}");
    }

    /// The contract's sizes: a lamb's shoulder is 38 cm, a crouched Wolf's 60.
    #[test]
    fn grass_stays_ankle_high_and_nothing_tops_a_crouched_wolf() {
        for tier in [BLADES, TUFTS] {
            assert!(tier.tall.1 <= cm(25), "{}", tier.tall.1);
        }
        assert!(RUSHES.tall.1 < cm(55), "{}", RUSHES.tall.1);
    }

    /// Short turf from the play camera: 3 blades in nearly every 45 cm cell
    /// is about 13 a square meter, and walls and worn ground take few of them.
    #[test]
    fn the_sward_is_thick_in_the_open_field() {
        let (map, _, b) = baked(3);
        let (mid, half) = ((from_int(130), from_int(70)), from_int(9));
        let mut near = 0;
        for chunk in b.chunks.iter().filter(|c| c.tier == 0) {
            let inside = |p: &&(Fx, Fx, Fx)| (p.0 - mid.0).abs() < half && (p.2 - mid.1).abs() < half;
            near += feet(&b, chunk).iter().filter(inside).count();
        }
        assert!(map.inside(mid.0, mid.1));
        assert!(near > 10 * 18 * 18, "{near} blades on 324 square meters");
    }

    #[test]
    fn a_vertex_packs_into_sixteen_bytes_and_keeps_its_place() {
        let mut out = Vec::new();
        let p = [from_int(100), from_int(12), from_int(70)];
        push_vertex(&mut out, p, [ONE, 0, ONE / 2], ONE, [0, ONE, -ONE]);
        assert_eq!(out.len(), VERTEX_BYTES);
        let u = |i: usize| u16::from_le_bytes([out[i], out[i + 1]]) as Fx;
        assert!(((u(0) << STEP_SHIFT_XZ) + ORIGIN[0] - p[0]).abs() < 1 << STEP_SHIFT_XZ);
        assert!(((u(2) << STEP_SHIFT_Y) + ORIGIN[1] - p[1]).abs() < 1 << STEP_SHIFT_Y);
        assert_eq!(&out[8..12], &[255, 0, 127, 255]);
        assert_eq!(out[13] as i8, 127);
        assert_eq!(out[14] as i8, -127);
    }

    #[test]
    fn a_smaller_map_bakes_less_grass_and_every_size_keeps_the_budget() {
        // the same cells over less hill: Small is about 36% of Big's map,
        // but the 40 m margin round it doesn't shrink
        let bytes = |size: MapSize| {
            let map = Map::sized(1, size);
            let field = Field::build(&map, 1);
            let clear = Clear {
                map: &map,
                field: &field,
                bare: &[],
            };
            bake(&clear, 1).vertices.len()
        };
        let (small, medium, big) = (bytes(MapSize::Small), bytes(MapSize::Medium), bytes(MapSize::Big));
        assert!(
            small * 10 < big * 8 && small < medium && medium < big,
            "{small} {medium} {big}"
        );
        assert!(big < 56 << 20, "{big} bytes");
    }

    #[test]
    fn the_whole_map_bakes_to_the_size_the_notes_planned() {
        let (_, _, b) = baked(1);
        let shapes: usize = b.chunks.iter().map(|c| c.prisms).sum();
        let bytes: usize = b
            .chunks
            .iter()
            .map(|c| c.prisms * TIERS[c.tier].vertices() * VERTEX_BYTES)
            .sum();
        assert_eq!(b.vertices.len(), bytes);
        // about 680k blades over 57,000 square meters at 64 bytes each, about the
        // notes' 30 MB
        assert!((550_000..1_000_000).contains(&shapes), "{shapes}");
        assert!(b.vertices.len() < 56 << 20, "{} bytes", b.vertices.len());
        assert!(
            most_prisms(&b) * PRISM_VERTICES < 65536,
            "a chunk fits a u16 index"
        );
        assert!(
            most_spikes(&b) * SPIKE_VERTICES < 65536,
            "a chunk fits a u16 index"
        );
    }

    #[test]
    fn no_grass_grows_through_a_wall() {
        let (map, _, b) = baked(2);
        for chunk in &b.chunks {
            for (x, _, z) in feet(&b, chunk) {
                for w in &map.walls {
                    let d = to_segment((x, z), w.a, w.b);
                    assert!(d > cm(50), "grass {d} from a wall at ({x}, {z})");
                }
            }
        }
    }

    #[test]
    fn blades_stop_at_the_outer_wall_and_a_foot_stands_on_the_drawn_ground() {
        let (map, field, b) = baked(3);
        for chunk in b.chunks.iter().filter(|c| c.tier == 0) {
            for (x, y, z) in feet(&b, chunk) {
                let spread = cm(60);
                assert!(
                    map.inside(x, z)
                        || map.inside(x + spread, z)
                        || map.inside(x - spread, z)
                        || map.inside(x, z + spread)
                        || map.inside(x, z - spread),
                    "a blade past the wall at ({x}, {z})"
                );
                let gap = y - (field.height_at(x, z) - FOOT_SINK);
                assert!(gap.abs() < cm(4), "{gap}");
            }
        }
    }

    #[test]
    fn a_bare_patch_keeps_grass_off() {
        let map = Map::generate(4);
        let field = Field::build(&map, 4);
        let center = (from_int(130), from_int(70));
        let radius = from_int(6);
        let bare = [(center, radius)];
        let b = bake(
            &Clear {
                map: &map,
                field: &field,
                bare: &bare,
            },
            4,
        );
        for chunk in &b.chunks {
            for (x, _, z) in feet(&b, chunk) {
                let d = fx::length(x - center.0, z - center.1);
                // a clump's prisms stand up to a third of a cell from its middle
                assert!(d > radius - cm(70), "{d}");
            }
        }
    }

    #[test]
    fn a_frame_draws_chunks_by_the_eye_and_none_behind_it() {
        let (_, _, b) = baked(5);
        let eye = [from_int(130), from_int(14), from_int(70)];
        let list = visible(&b, eye, [ONE, -ONE / 3, 0]);
        assert_eq!(list.len() % DRAW_VALUES, 0);
        let draws: Vec<&[u32]> = list.chunks(DRAW_VALUES).collect();
        let blades = draws.iter().filter(|d| d[0] == 0).count();
        assert!((3..=40).contains(&blades), "{blades}");
        assert!(
            draws.len() * 3 < b.chunks.len(),
            "{} of {}",
            draws.len(),
            b.chunks.len()
        );
        // far behind the eye nothing is drawn
        for d in &draws {
            let chunk = b.chunks.iter().find(|c| c.first_byte as u32 == d[1]).unwrap();
            assert!(chunk.center.0 > eye[0] - from_int(50), "{:?}", chunk.center);
        }
    }

    #[test]
    fn the_bake_comes_from_the_seed_alone() {
        let (map, field, first) = baked(6);
        let again = |seed| {
            let bare = [];
            let clear = Clear {
                map: &map,
                field: &field,
                bare: &bare,
            };
            bake(&clear, seed)
        };
        assert_eq!(first, again(6));
        assert_ne!(first.vertices, again(7).vertices, "on the same ground");
    }
}
