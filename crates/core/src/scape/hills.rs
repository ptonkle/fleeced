//! The hills past the walls: ridge behind ridge, each with its own skyline,
//! heather-brown and olive close to, going blue with distance
//! (`wall-stane-b`, `tree-northmid`, `pine-broomy`). The nearest climbs from
//! the ground's edge; the rest stand behind it with their feet out of sight,
//! and mist pools at each foot so the crest in front stands out against it.
//!
//! The eye never gets above a crest, so each ridge is a front face only.

use crate::fx::{self, from_int, mul, Fx, ONE, TURN};
use crate::map::table;
use crate::mesh::{Shape, OWN};
use crate::render::geom::{v3, V3};

use super::cm;
use super::field::{moor, Field, LIT};
use super::noise::{band, fbm, lerp, value};
use super::palette::{self as pal, mix, pct, Tone};
use super::shapes::flat_normal;

/// Slices round the ring: about 35 pixels of skyline on a wide screen.
pub const SECTORS: i32 = 240;
/// How high the face is at each step up it, out of its crest's height.
const RISE: [Fx; 6] = [0, cm(12), cm(34), cm(62), cm(86), ONE];
/// How far along the way up each step is.
const ALONG: [Fx; 6] = [0, cm(30), cm(55), cm(76), cm(92), ONE];
/// A corner's normal is taken this many slices either way, so light picks out
/// whole slopes and not each long triangle.
const BROAD: i32 = 3;
/// How far each normal leans to the sky, for the same reason.
const HEAVEN: Fx = cm(20);
/// Below any ground, so no sky shows under a far ridge through a low col.
const FOOT: Fx = from_int(-40);

/// One ridge. Every distance is meters past the edge of the ground.
pub struct Ridge {
    /// Where its crest runs.
    pub out: i32,
    /// How far down its face runs toward the eye.
    depth: i32,
    /// The crest's lowest and highest.
    pub low: i32,
    pub high: i32,
    /// The ring its skyline is drawn from, in meters: smaller for wider hills.
    round: i32,
    /// How much of a summit is a sharp peak and not a dome.
    peaks: Fx,
    /// The color distance gives it, and how much of that it takes.
    far: Tone,
    share: Fx,
    /// How much mist lies at its foot.
    mist: Fx,
}

pub const RIDGES: [Ridge; 5] = [
    Ridge {
        out: 150,
        depth: 150,
        low: 10,
        high: 26,
        round: 400,
        peaks: 0,
        far: pal::HILL_OLIVE,
        share: 0,
        mist: 0,
    },
    Ridge {
        out: 320,
        depth: 170,
        low: 22,
        high: 58,
        round: 520,
        peaks: cm(5),
        far: pal::tone(92, 96, 92),
        share: cm(44),
        mist: cm(24),
    },
    Ridge {
        out: 540,
        depth: 220,
        low: 38,
        high: 100,
        round: 640,
        peaks: cm(10),
        far: pal::tone(88, 106, 116),
        share: cm(76),
        mist: cm(36),
    },
    Ridge {
        out: 820,
        depth: 280,
        low: 52,
        high: 150,
        round: 820,
        peaks: cm(15),
        far: pal::tone(92, 116, 136),
        share: cm(88),
        mist: cm(42),
    },
    Ridge {
        out: 1160,
        depth: 340,
        low: 70,
        high: 220,
        round: 1040,
        peaks: cm(20),
        far: pal::tone(112, 132, 152),
        share: ONE,
        mist: cm(46),
    },
];

/// The farthest a crest gets from the map's middle: the ground's corner, the
/// last ridge and half its depth of wander.
pub const REACH: Fx = from_int(1600);

/// `|2v - 1|`: rounded on top with a sharp V between, as the hills go.
fn billow(v: Fx) -> Fx {
    (2 * v - ONE).abs()
}

/// The direction of slice `i`.
fn heading(i: i32) -> (Fx, Fx) {
    let a = i.rem_euclid(SECTORS) * TURN / SECTORS;
    (fx::cos(a), fx::sin(a))
}

/// Noise round a ring of radius `round` meters, so it closes up. Lattice
/// points are 256 m apart at `shift` 24.
fn around(seed: u32, i: i32, round: i32, shift: u32) -> Fx {
    let (c, s) = heading(i);
    let r = from_int(round);
    value(
        seed,
        mul(c, r) + from_int(4096),
        mul(s, r) + from_int(4096),
        shift,
    )
}

/// The skyline of ridge `n` at slice `i`.
pub fn crest(seed: u32, n: usize, i: i32) -> Fx {
    let r = &RIDGES[n];
    let at = |k: u32, shift: u32| around(seed + 900 + n as u32 * 16 + k, i, r.round, shift);
    let dome = billow(at(0, 24));
    let peak = ONE - billow(at(1, 24));
    let summit = lerp(dome, peak, r.peaks);
    let shoulder = at(2, 23);
    let rough = at(3, 22) - ONE / 2;
    let crag = at(4, 22) - ONE / 2;
    let shape = mul(summit, cm(62)) + mul(shoulder, cm(30)) + mul(rough, cm(20)) + mul(crag, cm(6));
    lerp(from_int(r.low), from_int(r.high), shape.clamp(0, ONE))
}

/// A point on ridge `n`'s face: slice `i`, `step` of the way up [`RISE`].
fn point(f: &Field, seed: u32, n: usize, i: i32, step: usize) -> V3 {
    let r = &RIDGES[n];
    let (c, s) = heading(i);
    let (cx, cz) = (f.length / 2, f.width / 2);
    let reach = |half: Fx, v: Fx| {
        if v.abs() < cm(1) {
            from_int(4000)
        } else {
            fx::div(half, v.abs())
        }
    };
    let m = table::TERRAIN_MARGIN;
    let edge = reach(cx + m, c).min(reach(cz + m, s));
    let at = |k: u32, shift: u32| around(seed + 980 + n as u32 * 16 + k, i, r.round, shift);
    // the crest wanders in and out, and spurs run down the face toward the eye
    let wander = mul(at(0, 23) - ONE / 2, from_int(r.depth));
    let spur = mul(at(1, 23) - ONE / 2, from_int(r.depth) / 6);
    let up = ALONG[step];
    let top = edge + from_int(r.out) + wander;
    let radius = if n == 0 && step == 0 {
        edge - cm(500)
    } else {
        let face = top - mul(ONE - up, from_int(r.depth)) - mul(ONE - up, mul(up * 4, spur));
        // the nearest ridge keeps to its own side of where it leaves the ground
        if n == 0 {
            face.max(edge + from_int(step as i32 * 6))
        } else {
            face
        }
    };
    let (x, z) = (cx + mul(c, radius), cz + mul(s, radius));
    // every ridge stands on the ground at this heading, so the hills climb
    // with the map's own slope and the high end never looks down on them
    let (ex, ez) = (cx + mul(c, edge), cz + mul(s, edge));
    let (lo_x, hi_x) = (f.x0 + cm(200), f.x0 + 2 * (cx + m) - cm(200));
    let (lo_z, hi_z) = (f.z0 + cm(200), f.z0 + 2 * (cz + m) - cm(200));
    let ground = f.height_at(ex.clamp(lo_x, hi_x), ez.clamp(lo_z, hi_z));
    let foot = if n == 0 { ground - cm(60) } else { FOOT };
    // gullies bite the middle of the slope and leave the crest alone
    let gully = mul(at(2, 22) - ONE / 2, cm(20));
    let height = ground + crest(seed, n, i) - foot;
    let lift = RISE[step] + mul(RISE[step], mul(ONE - RISE[step], gully));
    v3(x, foot + mul(height, lift), z)
}

/// Ridge `n`'s color at a place, `up` of the way up its face: olive grass
/// low, heather and tawny bent grass in big patches, more heather the higher
/// it goes, then distance over it all and mist at the foot.
pub fn tone(seed: u32, n: usize, p: V3, up: Fx) -> Tone {
    let r = &RIDGES[n];
    let (x, z) = (p.x / 2, p.z / 2);
    let heather = band(cm(42), cm(66), fbm(seed + 1000, x, z, 23, 3) + up / 10);
    let tawny = band(cm(42), cm(62), fbm(seed + 1010, x, z, 23, 2));
    let own = mix(
        mix(pal::HILL_OLIVE, pal::HILL_TAWNY, tawny),
        pal::HILL_HEATHER,
        heather,
    );
    let far = mix(own, r.far, r.share);
    let foot = ONE - up;
    mix(far, pal::HILL_MIST, mul(r.mist, mul(foot, foot)))
}

/// Every ridge, one soft-shaded skin each: its hollows and spurs catch the
/// light, so it reads as a hill and not a cut-out.
pub fn far_hills(f: &Field, seed: u32) -> Shape {
    let mut s = Shape::new();
    s.brush([ONE; 3], OWN, 0);
    let steps = RISE.len();
    for n in 0..RIDGES.len() {
        let points: Vec<V3> = (0..SECTORS)
            .flat_map(|i| (0..steps).map(move |k| (i, k)))
            .map(|(i, k)| point(f, seed, n, i, k))
            .collect();
        let at = |i: i32, k: usize| points[i.rem_euclid(SECTORS) as usize * steps + k];
        let normal = |i: i32, k: usize| {
            let (lo, hi) = (k.saturating_sub(1), (k + 1).min(steps - 1));
            let (up, round) = (at(i, hi).sub(at(i, lo)), at(i + BROAD, k).sub(at(i - BROAD, k)));
            let m = flat_normal(V3::ZERO, up, round);
            let m = if m.y < 0 { m.neg() } else { m };
            m.lerp(V3::UP, HEAVEN).norm()
        };
        let paint = |i: i32, k: usize| {
            let t = tone(seed, n, at(i, k), RISE[k]);
            if n == 0 {
                // the skirt grows out of the moor at its foot
                let p = at(i, k);
                mix(pct(moor(seed, p.x, p.z), LIT), t, band(0, cm(34), RISE[k]))
            } else {
                t
            }
        };
        for i in 0..SECTORS {
            for k in 0..steps - 1 {
                let corner = [(i, k), (i, k + 1), (i + 1, k), (i + 1, k + 1)];
                let p = corner.map(|(a, r)| at(a, r));
                let m = corner.map(|(a, r)| normal(a, r));
                let c = corner.map(|(a, r)| paint(a, r));
                // wound to face up: a face seen from behind is lit from behind
                s.painted_tri([p[0], p[2], p[1]], [m[0], m[2], m[1]], [c[0], c[2], c[1]]);
                s.painted_tri([p[2], p[3], p[1]], [m[2], m[3], m[1]], [c[2], c[3], c[1]]);
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;

    fn field(seed: u32) -> Field {
        Field::build(&Map::generate(seed as u64), seed)
    }

    #[test]
    fn the_far_hills_climb_well_above_the_map() {
        let (vs, _) = far_hills(&field(1), 1).finish();
        let top = vs.iter().map(|v| v.pos.y).max().unwrap();
        assert!(top > from_int(90), "{top}");
    }

    #[test]
    fn every_skyline_has_real_summits_and_cols() {
        for (n, ridge) in RIDGES.iter().enumerate() {
            let line: Vec<Fx> = (0..SECTORS).map(|i| crest(3, n, i)).collect();
            let at = |i: i32| line[i.rem_euclid(SECTORS) as usize];
            let summits = (0..SECTORS)
                .filter(|&i| (1..=3).all(|d| at(i) > at(i - d) && at(i) >= at(i + d)))
                .count();
            let (lo, hi) = (*line.iter().min().unwrap(), *line.iter().max().unwrap());
            assert!(summits >= 8, "ridge {n}: {summits} summits");
            assert!(
                hi - lo > from_int(ridge.high - ridge.low) * 2 / 5,
                "ridge {n}: {lo} to {hi}"
            );
        }
    }

    #[test]
    fn the_ridges_go_bluer_and_paler_with_distance() {
        let spot = v3(from_int(700), 0, from_int(200));
        let lead = |n: usize| {
            let c = tone(2, n, spot, ONE);
            c[2] - c[0]
        };
        let bright = |n: usize| tone(2, n, spot, ONE).iter().sum::<i32>();
        assert!(lead(0) < 0, "the nearest is brown, not blue");
        for n in 1..RIDGES.len() {
            assert!(lead(n) > lead(n - 1), "ridge {n}");
        }
        assert!(bright(4) > bright(0));
    }

    #[test]
    fn mist_lies_at_a_far_ridges_foot_and_not_on_its_crest() {
        let spot = v3(from_int(700), 0, from_int(200));
        let sum = |c: Tone| c.iter().sum::<i32>();
        for n in 1..RIDGES.len() {
            assert!(sum(tone(2, n, spot, 0)) > sum(tone(2, n, spot, ONE)), "ridge {n}");
        }
    }

    #[test]
    fn every_face_of_a_ridge_faces_the_sky_so_it_takes_the_sun() {
        let (vs, ix) = far_hills(&field(2), 2).finish();
        for t in ix.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| vs[i as usize].pos);
            let (u, w) = (b.sub(a), c.sub(a));
            let up = u.z as i64 * w.x as i64 - u.x as i64 * w.z as i64;
            assert!(up > 0, "{a:?} {b:?} {c:?}");
        }
    }

    #[test]
    fn the_nearest_ridge_starts_on_the_ground() {
        let f = field(2);
        for i in [17, 60, 133] {
            let p = point(&f, 2, 0, i, 0);
            let x = p.x.clamp(f.x0 + cm(200), -f.x0 + table::LENGTH - cm(200));
            let z = p.z.clamp(f.z0 + cm(200), -f.z0 + table::WIDTH - cm(200));
            assert!((p.y - f.height_at(x, z)).abs() < from_int(2), "{}", p.y);
        }
    }

    #[test]
    fn every_ridge_stands_clear_of_the_map() {
        // the ground runs 40 m past the map, and the nearest ridge's foot
        // comes in 5 m from there, so 35 m at the closest: 34 allows for rounding
        let f = field(2);
        let clear = from_int(34);
        for n in 0..RIDGES.len() {
            for i in 0..SECTORS {
                for step in 0..RISE.len() {
                    let p = point(&f, 2, n, i, step);
                    let out = p.x < -clear
                        || p.x > table::LENGTH + clear
                        || p.z < -clear
                        || p.z > table::WIDTH + clear;
                    assert!(out, "ridge {n} slice {i} step {step}: {p:?}");
                }
            }
        }
    }

    #[test]
    fn the_ridges_ring_a_small_map_as_closely_as_a_big_one() {
        // the same 34 m clear of the map's edge as on Big, on Small's 195 by 105
        let f = Field::build(&Map::sized(2, crate::map::MapSize::Small), 2);
        let clear = from_int(34);
        for i in 0..SECTORS {
            let p = point(&f, 2, 0, i, 0);
            let out =
                p.x < -clear || p.x > from_int(195) + clear || p.z < -clear || p.z > from_int(105) + clear;
            assert!(out, "slice {i}: {p:?}");
        }
    }

    #[test]
    fn the_cameras_far_plane_takes_in_the_last_crest() {
        let f = field(2);
        let last = RIDGES.len() - 1;
        let far = (0..SECTORS)
            .map(|i| {
                let p = point(&f, 2, last, i, RISE.len() - 1);
                fx::length(p.x - table::LENGTH / 2, p.z - table::WIDTH / 2)
            })
            .max()
            .unwrap();
        assert!(far < REACH, "{far}");
    }
}
