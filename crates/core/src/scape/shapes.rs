//! The hillside's things as meshes, each built from its reference photos
//! (`notes/hillside-look.md`): stones, trees, cairn steps, tunnel mouths in
//! their banks of earth, mounds, the Fold's gate and its arrow. Simple
//! polygons and soft forms, never a cube.

use crate::fx::{self, mul, Fx, ONE, TURN};
use crate::map::props::{Cairn, Tree};
use crate::mesh::{icosphere, ring, Rgb, Shape, COAT, OWN};
use crate::render::geom::{v3, Affine, V3};

use super::cm;
use super::noise::{band, lattice, lerp};
use super::palette::{self as pal, mix, pct, Tone};

pub fn flat_normal(a: V3, b: V3, c: V3) -> V3 {
    let k = 16;
    let (u, w) = (b.sub(a), c.sub(a));
    v3(u.x * k, u.y * k, u.z * k)
        .cross(v3(w.x * k, w.y * k, w.z * k))
        .norm()
}

/// `√|v|` with `v`'s sign: it pushes a ball's points out toward a cube's.
fn squared_up(v: Fx) -> Fx {
    let m = fx::sqrt(v.abs());
    if v < 0 {
        -m
    } else {
        m
    }
}

/// A stone about one unit across, soft-cornered and irregular. Its upper
/// faces catch lichen and its lower corners go dark, which is what makes
/// stacked stones read as stacked: each is bright on top and dark at the
/// joint. `corner` pushes it toward a block, for a wall's cheek.
pub fn stone(seed: u32, detail: u32, corner: Fx) -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    let (points, faces) = icosphere(detail);
    let jag = |i: usize, axis: u32| {
        let t = lattice(seed.wrapping_add(axis), i as i32, 3);
        lerp(cm(84), cm(112), t)
    };
    let shaped: Vec<V3> = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let q = v3(
                lerp(p.x, squared_up(p.x), corner),
                lerp(p.y, squared_up(p.y), corner),
                lerp(p.z, squared_up(p.z), corner),
            );
            v3(
                mul(q.x, jag(i, 1)) / 2,
                mul(q.y, jag(i, 2)) / 2,
                mul(q.z, jag(i, 3)) / 2,
            )
        })
        .collect();
    let shade = |p: V3| {
        // from the joint's dark at the bottom to full at the top
        let t = (p.y + ONE / 2).clamp(0, ONE);
        lerp(cm(52), cm(112), t)
    };
    for (i, f) in faces.iter().enumerate() {
        let p = [shaped[f[0]], shaped[f[1]], shaped[f[2]]];
        let n = flat_normal(p[0], p[1], p[2]);
        let fleck = lattice(seed.wrapping_add(9), i as i32, 7);
        if n.y > cm(25) && fleck < cm(14) {
            // a lichen patch: its own color, not the instance's tone
            s.brush([ONE; 3], OWN, 0);
            s.painted_tri(p, [n; 3], [pal::LICHEN; 3]);
        } else {
            s.brush([ONE; 3], COAT, 0);
            let c = |q: V3| {
                let v = shade(q);
                [v, v, v]
            };
            s.painted_tri(p, [n; 3], [c(p[0]), c(p[1]), c(p[2])]);
        }
    }
    s
}

/// A low-poly stone for far away: an octahedron, eight faces.
pub fn far_stone() -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    s.brush([ONE; 3], COAT, 0);
    let (h, top, bot) = (ONE / 2, v3(0, ONE / 2, 0), v3(0, -ONE / 2, 0));
    let ring4 = [v3(h, 0, 0), v3(0, 0, h), v3(-h, 0, 0), v3(0, 0, -h)];
    for i in 0..4 {
        let (a, b) = (ring4[i], ring4[(i + 1) % 4]);
        let n = flat_normal(top, b, a);
        s.painted_tri([top, b, a], [n; 3], [[ONE; 3]; 3]);
        let n = flat_normal(bot, a, b);
        s.painted_tri([bot, a, b], [n; 3], [[cm(60); 3]; 3]);
    }
    s
}

/// Which way a tree leans: with the wind, on the ground.
fn downwind(wind: [Fx; 2], reach: Fx) -> V3 {
    v3(mul(wind[0], reach), 0, mul(wind[1], reach))
}

/// A soft lobe of leaves: a ball shaded smoothly from dark underneath to lit
/// on top. `at` and `radii` are in the tree's space.
fn lobe(s: &mut Shape, at: V3, radii: V3, lit: Tone, mid: Tone, dark: Tone, seed: u32) {
    let (points, faces) = icosphere(1);
    let bump = |i: usize| lerp(cm(90), cm(112), lattice(seed, i as i32, 11));
    let place = |i: usize, p: V3| {
        let k = bump(i);
        at.add(v3(
            mul(mul(p.x, radii.x), k),
            mul(mul(p.y, radii.y), k),
            mul(mul(p.z, radii.z), k),
        ))
    };
    let color = |p: V3, i: usize| -> Rgb {
        let t = (p.y + ONE) / 2;
        let base = if t < ONE / 2 {
            mix(dark, mid, t * 2)
        } else {
            mix(mid, lit, (t - ONE / 2) * 2)
        };
        // a little variation from clump to clump of leaves
        pct(base, 92 + (lattice(seed + 3, i as i32, 2) >> 12))
    };
    s.brush([ONE; 3], OWN, 0);
    for f in &faces {
        let (a, b, c) = (points[f[0]], points[f[1]], points[f[2]]);
        s.painted_tri(
            [place(f[0], a), place(f[1], b), place(f[2], c)],
            [a, b, c],
            [color(a, f[0]), color(b, f[1]), color(c, f[2])],
        );
    }
}

/// The kinds of tree on the hill, by the photos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Sycamore,
    Rowan,
    Pine,
}

/// A tree's kind comes from where it stands, so a seed keeps its trees.
pub fn kind_of(t: &Tree) -> Kind {
    match lattice(0x7EE, t.x >> 16, t.z >> 16) >> 13 {
        0..=3 => Kind::Sycamore,
        4 | 5 => Kind::Pine,
        _ => Kind::Rowan,
    }
}

/// A short massive trunk, thick limbs forking low, and a broad dome of
/// rounded lobes whose lowest ones droop well out to the sides
/// (`tree-sycamore-old`).
fn sycamore(s: &mut Shape, t: &Tree, wind: [Fx; 2], seed: u32) {
    let (h, c, r) = (t.height, t.crown, t.trunk);
    let d = |n: u32, lo: Fx, hi: Fx| lerp(lo, hi, lattice(seed.wrapping_add(n), 5, 9));
    let lean = |y: Fx| downwind(wind, mul(mul(c, cm(14)), mul(y, y)));
    let fork = mul(h, cm(30));
    s.brush(pal::BARK, OWN, 0);
    s.tube(
        &[
            ring(v3(0, -cm(30), 0), r + r / 4),
            ring(v3(0, mul(h, cm(4)), 0), r),
            ring(lean(cm(20)).add(v3(0, mul(h, cm(16)), 0)), r * 4 / 5),
            ring(lean(cm(30)).add(v3(0, fork, 0)), r * 3 / 4),
        ],
        8,
        v3(ONE, 0, 0),
    );
    // the limbs: a vase of four or five
    let limbs = 5;
    for i in 0..limbs {
        let a = i * TURN / limbs + d(30 + i as u32, -TURN / 16, TURN / 16);
        let (ca, sa) = (fx::cos(a), fx::sin(a));
        let out = |k: Fx| v3(mul(ca, mul(c, k)), 0, mul(sa, mul(c, k)));
        let base = lean(cm(30)).add(v3(0, fork - cm(30), 0));
        let mid = base.add(out(cm(30))).add(v3(0, mul(h, cm(20)), 0));
        let end = base.add(out(cm(62))).add(v3(0, mul(h, cm(34)), 0));
        s.brush(pal::BARK, OWN, 0);
        s.tube(
            &[ring(base, r * 2 / 5), ring(mid, r / 4), ring(end, r / 8)],
            6,
            v3(ONE, 0, 0),
        );
    }
    // the dome, ring by ring from the top down
    let rings: [(i32, Fx, Fx, Fx, Fx); 4] = [
        (1, 0, cm(84), cm(50), cm(34)),
        (5, cm(46), cm(74), cm(46), cm(34)),
        (7, cm(76), cm(57), cm(44), cm(30)),
        (9, cm(92), cm(38), cm(40), cm(22)),
    ];
    let mut n = 100;
    for (count, reach, high, wide, tall) in rings {
        for i in 0..count {
            n += 1;
            let a = i * TURN / count + d(n, -TURN / 24, TURN / 24) + (count * TURN / 40);
            let k = mul(c, reach + d(n + 300, -cm(7), cm(7)));
            let y = mul(h, high + d(n + 600, -cm(3), cm(3)));
            let at = v3(mul(fx::cos(a), k), y, mul(fx::sin(a), k)).add(lean(high));
            let w = mul(c, wide + d(n + 900, -cm(5), cm(7)));
            let radii = v3(w, mul(c, tall + d(n + 1200, -cm(3), cm(4))), w);
            lobe(
                s,
                at,
                radii,
                pal::LEAF_LIT,
                pal::LEAF,
                pal::LEAF_SHADE,
                seed.wrapping_add(n),
            );
        }
    }
}

/// A leaning rowan on a rocky foot: a short trunk under a dense lopsided dome
/// of clumps, wider than it is tall (`tree-rowan`).
fn rowan(s: &mut Shape, t: &Tree, wind: [Fx; 2], seed: u32) {
    let (h, c, r) = (mul(t.height, cm(76)), mul(t.crown, cm(84)), t.trunk * 3 / 5);
    let lean = |y: Fx| downwind(wind, mul(mul(c, cm(55)), y));
    s.brush(pal::BARK_PALE, OWN, 0);
    let fork = mul(h, cm(30));
    s.tube(
        &[
            ring(v3(0, -cm(30), 0), r + r / 3),
            ring(lean(cm(14)).add(v3(0, mul(h, cm(14)), 0)), r),
            ring(lean(cm(30)).add(v3(0, fork, 0)), r * 7 / 10),
        ],
        7,
        v3(ONE, 0, 0),
    );
    let heart = lean(cm(70)).add(v3(0, mul(h, cm(60)), 0));
    let fork_at = lean(cm(30)).add(v3(0, fork, 0));
    let wide = c;
    for (k, (at, radii)) in clumps(heart, wide, mul(h, cm(40)), 18, wind, seed)
        .iter()
        .enumerate()
    {
        let k = k as u32;
        if k.is_multiple_of(3) {
            s.brush(pal::BARK_PALE, OWN, 0);
            s.tube(&[ring(fork_at, r * 2 / 5), ring(*at, r / 6)], 5, v3(ONE, 0, 0));
        }
        let shade = 92 + (lattice(seed + k, 5, 9) >> 12);
        lobe(
            s,
            *at,
            *radii,
            pct(pal::ROWAN_LIT, shade),
            pct(pal::ROWAN, shade),
            pal::LEAF_SHADE,
            seed.wrapping_add(200 + k),
        );
    }
    // the rock it roots in
    s.brush(pal::GRANITE_DARK, OWN, 0);
    s.flat = true;
    s.ball(
        v3(-cm(60), cm(20), cm(30)),
        v3(cm(120), cm(60), cm(90)),
        &Affine::rot_y(TURN / 7),
        1,
    );
    s.ball(
        v3(cm(90), cm(10), -cm(50)),
        v3(cm(80), cm(44), cm(70)),
        &Affine::rot_y(TURN / 3),
        1,
    );
    s.flat = false;
}

/// Round clumps heaped into one crown about `heart`: a spiral out from the
/// top, so no two sit level, each lower and smaller the farther out it lies,
/// and the mass pushed downwind. Each is a place and its three half widths.
fn clumps(heart: V3, wide: Fx, tall: Fx, count: i32, wind: [Fx; 2], seed: u32) -> Vec<(V3, V3)> {
    let d = |n: i32, k: i32, lo: Fx, hi: Fx| lerp(lo, hi, lattice(seed.wrapping_add(n as u32), k, 8));
    let side = v3(-wind[1], 0, wind[0]);
    (0..count)
        .map(|k| {
            let out = fx::sqrt(fx::ratio(2 * k + 1, 2 * count));
            let a = k * (TURN * 382 / 1000) + d(1, k, -TURN / 16, TURN / 16);
            let along = mul(fx::cos(a), mul(out, mul(wide, cm(70))));
            let across = mul(fx::sin(a), mul(out, mul(wide, cm(62))));
            // longer downwind than up
            let along = if along > 0 { along * 5 / 4 } else { along * 3 / 4 };
            let drop = mul(mul(out, out), tall);
            let at = heart.add(downwind(wind, along)).add(side.scale(across)).add(v3(
                0,
                tall * 3 / 4 - drop + d(2, k, -tall / 9, tall / 9),
                0,
            ));
            let rx = mul(wide, lerp(cm(31), cm(22), out) + d(3, k, -cm(4), cm(4)));
            let ry = mul(rx, cm(74) + d(4, k, -cm(10), cm(10)));
            (at, v3(rx, ry, mul(rx, cm(92))))
        })
        .collect()
}

/// A Scots pine's green: the heaped crown, then a few lower clumps held out
/// on their own limbs with sky between them. Each clump comes with how far up
/// the trunk its limb starts.
fn pine_clumps(t: &Tree, wind: [Fx; 2], seed: u32) -> Vec<(V3, V3, Fx)> {
    let (h, c) = (t.height, t.crown);
    let d = |n: u32, lo: Fx, hi: Fx| lerp(lo, hi, lattice(seed.wrapping_add(n), 1, 4));
    let side = v3(-wind[1], 0, wind[0]);
    let heart = pine_line(t, wind, cm(74)).add(downwind(wind, mul(c, cm(10))));
    let mut all: Vec<(V3, V3, Fx)> = clumps(heart, mul(c, cm(76)), mul(h, cm(36)), 20, wind, seed)
        .iter()
        .enumerate()
        .map(|(k, (at, radii))| (*at, *radii, cm(60) + (k as Fx % 4) * cm(6)))
        .collect();
    // (height up the trunk, reach downwind, reach to one side, size)
    let low: [(Fx, Fx, Fx, Fx); 3] = [
        (cm(50), cm(40), cm(22), cm(20)),
        (cm(46), -cm(12), -cm(38), cm(17)),
        (cm(54), -cm(26), cm(32), cm(17)),
    ];
    for (k, (high, reach, across, size)) in low.iter().enumerate() {
        let k = k as u32;
        let from = pine_line(t, wind, *high);
        let rise = v3(0, mul(h, cm(6)) + d(30 + k, -cm(20), cm(30)), 0);
        let at = from
            .add(downwind(wind, mul(c, *reach)))
            .add(side.scale(mul(c, *across + d(40 + k, -cm(6), cm(6)))))
            .add(rise);
        let rx = mul(c, *size);
        all.push((at, v3(rx, mul(rx, cm(56)), mul(rx, cm(86))), *high));
    }
    all
}

/// The middle of a pine's trunk `y` of the way up: a long lean with the wind
/// and one easy S in it.
fn pine_line(t: &Tree, wind: [Fx; 2], y: Fx) -> V3 {
    let wave = fx::sin(mul(y, TURN * 3 / 4));
    let lean = mul(t.crown, mul(y, cm(32)) + mul(wave, cm(5)));
    let side = v3(-wind[1], 0, wind[0]);
    downwind(wind, lean)
        .add(side.scale(mul(t.crown, mul(wave, cm(5)))))
        .add(v3(0, mul(t.height, y), 0))
}

/// A wind-bent Scots pine: a stout trunk going from gray plates to salmon as
/// it climbs and thins, under a heaped, irregular, rounded crown
/// (`pine-culloden`, `pine-affric`, `pine-broomy`).
fn pine(s: &mut Shape, t: &Tree, wind: [Fx; 2], seed: u32) {
    let r = t.trunk * 7 / 10;
    let steps = 8;
    let rings: Vec<_> = (0..=steps)
        .map(|k| {
            let y = fx::ratio(k, steps) * 9 / 10;
            let flare = if k == 0 { cm(130) } else { cm(100) };
            ring(
                pine_line(t, wind, y).sub(v3(0, cm(40) - mul(y, cm(40)), 0)),
                mul(mul(r, flare), cm(100) - mul(y, cm(84))),
            )
        })
        .collect();
    // the bark warms from gray plates to salmon as it climbs
    let first = s.drawn();
    s.brush(pal::BARK_PALE, OWN, 0);
    s.tube(&rings, 7, v3(ONE, 0, 0));
    let height = t.height;
    s.tint_from(first, |p| {
        mix(
            pal::BARK_PALE,
            pal::PINE_BARK,
            band(cm(14), cm(52), fx::div(p.y, height)),
        )
    });
    for (k, (at, radii, from)) in pine_clumps(t, wind, seed).iter().enumerate() {
        let k = k as u32;
        if k.is_multiple_of(2) || k >= 20 {
            s.brush(pal::PINE_BARK, OWN, 0);
            s.tube(
                &[ring(pine_line(t, wind, *from), r / 3), ring(*at, r / 8)],
                5,
                v3(0, ONE, 0),
            );
        }
        let shade = 90 + (lattice(seed + k, 3, 7) >> 11);
        lobe(
            s,
            *at,
            *radii,
            pct(pal::PINE_LIT, shade),
            pct(pal::PINE, shade),
            pal::PINE_SHADE,
            seed.wrapping_add(40 + k),
        );
    }
}

/// One tree, at the origin, standing on the ground at y = 0.
pub fn tree(t: &Tree, wind: [Fx; 2], seed: u32) -> Shape {
    let mut s = Shape::new();
    match kind_of(t) {
        Kind::Sycamore => sycamore(&mut s, t, wind, seed),
        Kind::Rowan => rowan(&mut s, t, wind, seed),
        Kind::Pine => pine(&mut s, t, wind, seed),
    }
    s
}

/// The solid heart of a cairn: a dark drum for each step, so feet that stand
/// on a step stand on something. The boulders round it are stones.
pub fn cairn_core(c: &Cairn) -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    let mut floor = c.tiers[0].top - cm(95);
    for (i, step) in c.tiers.iter().enumerate() {
        let (r, top) = (step.radius - cm(14), step.top);
        s.brush(
            if i % 2 == 0 {
                pal::GRANITE_DARK
            } else {
                pal::GRANITE
            },
            OWN,
            0,
        );
        s.tube(
            &[
                ring(v3(0, floor, 0), r + cm(10)),
                ring(v3(0, top - cm(16), 0), r),
                ring(v3(0, top - cm(3), 0), r - cm(10)),
            ],
            10,
            v3(ONE, 0, 0),
        );
        floor = top - cm(30);
    }
    s
}

/// A bank of earth with a dark tunnel mouth in its front: grass over the top,
/// bare soil round the opening, stones at the rim. It faces +z.
pub fn mouth_bank() -> Shape {
    let mut s = Shape::new();
    // the bank: a soft heap, green on top, bare at the front
    let (points, faces) = icosphere(2);
    let radii = v3(cm(270), cm(140), cm(200));
    let center = v3(0, -cm(10), -cm(70));
    let color = |p: V3| -> Rgb {
        let front = band(cm(5), cm(70), p.z);
        let high = band(cm(10), cm(60), p.y);
        let turf = mix(pal::TURF_DEEP, pal::TURF, high);
        let soil = mix(pal::SOIL_DARK, pal::SOIL, band(-cm(20), cm(40), p.y));
        mix(turf, soil, mul(front, ONE - mul(high, cm(55))))
    };
    let place = |p: V3| {
        // a lower bump, so the top is not a perfect ball
        let wobble = cm(100) + (lattice(5, p.x >> 12, p.z >> 12) >> 13) * cm(2);
        center.add(v3(
            mul(p.x, radii.x),
            mul(mul(p.y, radii.y), wobble),
            mul(p.z, radii.z),
        ))
    };
    s.brush([ONE; 3], OWN, 0);
    for f in &faces {
        let (a, b, c) = (points[f[0]], points[f[1]], points[f[2]]);
        if place(a).y < -cm(40) && place(b).y < -cm(40) && place(c).y < -cm(40) {
            continue;
        }
        s.painted_tri(
            [place(a), place(b), place(c)],
            [a, b, c],
            [color(a), color(b), color(c)],
        );
    }
    // the opening: an arched dark throat going back into the bank
    let sides = 12;
    let arch = |k: i32, r: Fx, z: Fx| -> V3 {
        let a = k * TURN / sides;
        v3(
            mul(fx::cos(a), mul(r, cm(118))),
            cm(40) + mul(fx::sin(a), mul(r, cm(82))),
            z,
        )
    };
    let (front, deep, r) = (cm(136), cm(126), cm(66));
    let n = v3(0, 0, ONE);
    for k in 0..sides {
        let (a0, a1) = (arch(k, r, front), arch(k + 1, r, front));
        let (b0, b1) = (arch(k, r * 4 / 5, deep), arch(k + 1, r * 4 / 5, deep));
        let (rim, dark) = (pal::SOIL_DARK, pct(pal::SOIL_DARK, 12));
        s.painted_tri([a0, b0, a1], [n; 3], [rim, dark, rim]);
        s.painted_tri([a1, b0, b1], [n; 3], [rim, dark, dark]);
        s.painted_tri([b0, arch(k + 1, 0, deep), b1], [n; 3], [dark; 3]);
    }
    // stones at the rim, the biggest as a lintel
    s.flat = true;
    let stones: [(Fx, Fx, Fx, Fx, Tone); 7] = [
        (0, cm(112), cm(70), cm(26), pal::WHIN),
        (-cm(82), cm(62), cm(34), cm(24), pal::WHIN_DARK),
        (cm(84), cm(66), cm(34), cm(26), pal::WHIN_WARM),
        (-cm(82), cm(6), cm(40), cm(26), pal::WHIN),
        (cm(82), cm(2), cm(44), cm(24), pal::WHIN_DARK),
        (-cm(46), cm(104), cm(30), cm(18), pal::WHIN_WARM),
        (cm(52), cm(100), cm(30), cm(18), pal::WHIN),
    ];
    for (x, y, wide, tall, tone) in stones {
        s.brush(tone, OWN, 0);
        s.ball(
            v3(x, y, cm(138)),
            v3(wide, tall, cm(30)),
            &Affine::rot_y(x / 3),
            1,
        );
    }
    s
}

/// A fresh mound: a soft low heap of dark earth, lumpy, with a few clods.
pub fn mound_fresh() -> Shape {
    let mut s = Shape::new();
    let (points, faces) = icosphere(2);
    let lump = |p: V3| lerp(cm(84), cm(116), lattice(21, p.x >> 13, p.z >> 13));
    let place = |p: V3| {
        let k = lump(p);
        v3(
            mul(mul(p.x, cm(62)), k),
            mul(mul(p.y, cm(34)), k) - cm(4),
            mul(mul(p.z, cm(56)), k),
        )
    };
    let color = |p: V3| -> Rgb {
        let t = lattice(22, p.x >> 12, (p.z + p.y) >> 12);
        mix(pal::SOIL, pal::SOIL_PALE, t / 2)
    };
    s.brush([ONE; 3], OWN, 0);
    for f in &faces {
        let (a, b, c) = (points[f[0]], points[f[1]], points[f[2]]);
        if a.y < -cm(40) && b.y < -cm(40) && c.y < -cm(40) {
            continue;
        }
        s.painted_tri(
            [place(a), place(b), place(c)],
            [a, b, c],
            [color(a), color(b), color(c)],
        );
    }
    s.flat = true;
    let clods: [(Fx, Fx, Fx); 5] = [
        (cm(58), cm(30), cm(9)),
        (-cm(50), cm(40), cm(7)),
        (cm(20), -cm(60), cm(8)),
        (-cm(34), -cm(46), cm(6)),
        (cm(72), -cm(20), cm(5)),
    ];
    for (x, z, r) in clods {
        s.brush(pal::SOIL_DARK, OWN, 0);
        s.ball(v3(x, r / 2, z), v3(r * 3 / 2, r, r), &Affine::rot_y(x), 0);
    }
    s
}

/// A dug mound: a dark hollow with thrown earth round it and a spoil heap to
/// one side, so it can't be taken for a fresh one from any angle.
pub fn mound_dug() -> Shape {
    let mut s = Shape::new();
    // the hollow, a dark dish just above the turf
    let sides = 12;
    let at = |k: i32, r: Fx, y: Fx| {
        let a = k * TURN / sides;
        v3(mul(fx::cos(a), r), y, mul(fx::sin(a), mul(r, cm(86))))
    };
    let up = V3::UP;
    for k in 0..sides {
        let (a, b) = (at(k, cm(60), cm(7)), at(k + 1, cm(60), cm(7)));
        let (c, d) = (at(k, cm(26), cm(4)), at(k + 1, cm(26), cm(4)));
        let (rim, deep) = (pal::SOIL, pct(pal::SOIL_DARK, 40));
        s.brush([ONE; 3], OWN, 0);
        s.painted_tri([a, b, c], [up; 3], [rim, rim, deep]);
        s.painted_tri([b, d, c], [up; 3], [rim, deep, deep]);
        s.painted_tri([c, d, v3(0, cm(4), 0)], [up; 3], [deep; 3]);
    }
    // the rim of thrown earth
    s.flat = true;
    for k in 0..11 {
        let a = k * TURN / 11 + TURN / 40;
        let r = cm(64) + (lattice(30, k, 0) >> 11) * cm(1);
        let size = cm(9) + (lattice(31, k, 0) >> 12) * cm(1);
        let tone = if k % 3 == 0 { pal::SOIL_PALE } else { pal::SOIL };
        s.brush(tone, OWN, 0);
        s.ball(
            v3(mul(fx::cos(a), r), size / 2, mul(fx::sin(a), mul(r, cm(86)))),
            v3(size * 3 / 2, size, size * 5 / 4),
            &Affine::rot_y(a),
            0,
        );
    }
    // the spoil heap
    s.flat = false;
    s.brush(pal::SOIL_PALE, OWN, 0);
    s.ball(
        v3(cm(92), cm(4), cm(30)),
        v3(cm(42), cm(24), cm(34)),
        &Affine::rot_y(TURN / 9),
        1,
    );
    s
}

/// A wooden gate leaf, hinged at the origin and running along +x: five bars
/// and a brace, weathered gray.
pub fn gate_leaf(long: Fx) -> Shape {
    let mut s = Shape::new();
    let wood = pal::tone(138, 124, 104);
    s.brush(wood, OWN, 0);
    let bars = 5;
    let (low, high) = (cm(22), cm(112));
    let side = v3(0, ONE, 0);
    for k in 0..bars {
        let y = low + (high - low) * k / (bars - 1);
        s.tube(&[ring(v3(0, y, 0), cm(4)), ring(v3(long, y, 0), cm(4))], 6, side);
    }
    for x in [cm(8), long / 2, long - cm(6)] {
        s.tube(
            &[
                ring(v3(x, low - cm(8), 0), cm(5)),
                ring(v3(x, high + cm(8), 0), cm(5)),
            ],
            6,
            v3(ONE, 0, 0),
        );
    }
    s.tube(
        &[ring(v3(cm(8), high, 0), cm(4)), ring(v3(long / 2, low, 0), cm(4))],
        6,
        v3(0, 0, ONE),
    );
    s.tube(
        &[ring(v3(long / 2, high, 0), cm(4)), ring(v3(long, low, 0), cm(4))],
        6,
        v3(0, 0, ONE),
    );
    s
}

/// A stout gate post, standing at the origin.
pub fn gate_post() -> Shape {
    let mut s = Shape::new();
    s.brush(pal::tone(112, 98, 80), OWN, 0);
    s.tube(
        &[
            ring(v3(0, -cm(30), 0), cm(13)),
            ring(v3(0, cm(138), 0), cm(11)),
            ring(v3(0, cm(146), 0), cm(6)),
        ],
        8,
        v3(ONE, 0, 0),
    );
    s
}

/// The Fold's arrow: thick shaft, wide head, pointing down, in brass. It
/// stands with its tip at the origin and is a meter tall, so its instance
/// sets its size.
pub fn arrow() -> Shape {
    let mut s = Shape::new();
    let (brass, shade) = (pal::tone(236, 190, 84), pal::tone(186, 134, 50));
    let sides = 8;
    let side = v3(ONE, 0, 0);
    s.brush(brass, OWN, 0);
    s.tube(
        &[ring(v3(0, cm(40), 0), cm(13)), ring(v3(0, cm(100), 0), cm(15))],
        sides,
        side,
    );
    s.brush(shade, OWN, 0);
    s.tube(
        &[ring(v3(0, cm(36), 0), cm(34)), ring(v3(0, cm(46), 0), cm(30))],
        sides,
        side,
    );
    s.brush(brass, OWN, 0);
    s.spike(v3(0, cm(40), 0), v3(0, 0, 0), cm(34), sides, side);
    s
}

/// One puff of Sniff Snoff's scent, about half a meter across at full size.
pub fn scent() -> Shape {
    let mut s = Shape::new();
    s.brush(pal::tone(206, 222, 150), OWN, 0);
    s.ball(v3(0, 0, 0), v3(cm(26), cm(20), cm(26)), &Affine::IDENTITY, 1);
    s
}

/// A tuft of a Wolf's fur, torn off by a Lunge.
pub fn fur() -> Shape {
    let mut s = Shape::new();
    s.brush(pal::tone(226, 214, 188), OWN, 0);
    s.ball(v3(0, 0, 0), v3(cm(16), cm(9), cm(16)), &Affine::IDENTITY, 1);
    s
}

/// A drop of a Wolf's blood.
pub fn blood() -> Shape {
    let mut s = Shape::new();
    s.brush(pal::tone(150, 22, 24), OWN, 0);
    s.ball(v3(0, 0, 0), v3(cm(9), cm(9), cm(9)), &Affine::IDENTITY, 1);
    s
}

/// A carrot on a stick, from the holder's mouth at the origin out along +z:
/// the stick, its string, and the carrot hanging point down.
pub fn carrot() -> Shape {
    let mut s = Shape::new();
    let (tip, top) = (v3(0, cm(40), cm(140)), v3(0, cm(4), cm(140)));
    let side = v3(ONE, 0, 0);
    s.brush(pal::tone(112, 78, 44), OWN, 0);
    s.tube(&[ring(v3(0, 0, 0), cm(2)), ring(tip, cm(2))], 5, side);
    s.brush(pal::tone(232, 224, 200), OWN, 0);
    s.tube(&[ring(tip, cm(1)), ring(top, cm(1))], 4, side);
    s.brush(pal::tone(226, 122, 44), OWN, 0);
    s.spike(top, v3(0, -cm(30), cm(140)), cm(8), 6, side);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;
    use crate::mesh::Vertex;

    fn bounds(vs: &[Vertex]) -> (V3, V3) {
        let mut lo = v3(i32::MAX, i32::MAX, i32::MAX);
        let mut hi = v3(i32::MIN, i32::MIN, i32::MIN);
        for v in vs {
            lo = v3(lo.x.min(v.pos.x), lo.y.min(v.pos.y), lo.z.min(v.pos.z));
            hi = v3(hi.x.max(v.pos.x), hi.y.max(v.pos.y), hi.z.max(v.pos.z));
        }
        (lo, hi)
    }

    #[test]
    fn a_stone_is_about_a_unit_across_and_not_a_cube() {
        let (vs, ix) = stone(1, 0, cm(70)).finish();
        let (lo, hi) = bounds(&vs);
        for (a, b) in [(lo.x, hi.x), (lo.y, hi.y), (lo.z, hi.z)] {
            let across = b - a;
            assert!((cm(70)..=cm(125)).contains(&across), "{across}");
        }
        assert_eq!(ix.len(), 60, "twenty faces, never a cube's twelve");
    }

    #[test]
    fn a_stone_is_dark_at_the_joint_and_bright_on_top() {
        let (vs, _) = stone(2, 1, cm(70)).finish();
        let coat: Vec<&Vertex> = vs.iter().filter(|v| v.paint == COAT).collect();
        let low = coat
            .iter()
            .filter(|v| v.pos.y < -cm(30))
            .map(|v| v.color[0])
            .max()
            .unwrap();
        let high = coat
            .iter()
            .filter(|v| v.pos.y > cm(30))
            .map(|v| v.color[0])
            .min()
            .unwrap();
        assert!(low < high, "{low} {high}");
    }

    #[test]
    fn some_stones_carry_lichen_on_their_upper_faces_only() {
        let lichen = pal::LICHEN;
        let mut found = 0;
        for seed in 0..8 {
            let (vs, _) = stone(seed, 1, cm(70)).finish();
            for v in vs.iter().filter(|v| v.color == lichen) {
                assert!(v.normal.y > 0, "lichen on a lower face");
                found += 1;
            }
        }
        assert!(found > 0);
    }

    fn the_tree(kind: Kind) -> Tree {
        let map = Map::generate(1);
        for seed in 1..40 {
            let m = Map::generate(seed);
            if let Some(t) = m.trees.iter().find(|t| kind_of(t) == kind) {
                return *t;
            }
        }
        map.trees[0]
    }

    #[test]
    fn every_kind_of_tree_turns_up_on_the_hill() {
        for kind in [Kind::Sycamore, Kind::Rowan, Kind::Pine] {
            assert_eq!(kind_of(&the_tree(kind)), kind);
        }
    }

    #[test]
    fn a_sycamore_is_as_tall_and_wide_as_the_map_says() {
        let t = the_tree(Kind::Sycamore);
        let (vs, _) = tree(&t, [ONE, 0], 3).finish();
        let (lo, hi) = bounds(&vs);
        assert!(
            hi.y > t.height * 4 / 5 && hi.y < t.height * 13 / 10,
            "{} {}",
            hi.y,
            t.height
        );
        assert!(hi.x - lo.x > t.crown * 8 / 5, "{} {}", hi.x - lo.x, t.crown);
    }

    #[test]
    fn a_pine_leans_with_the_wind_and_a_change_of_wind_turns_it() {
        let t = the_tree(Kind::Pine);
        let lean = |wind: [Fx; 2]| {
            let (vs, _) = tree(&t, wind, 4).finish();
            let n = vs.len() as i64;
            vs.iter().map(|v| v.pos.x as i64).sum::<i64>() / n
        };
        assert!(lean([ONE, 0]) > ONE as i64, "{}", lean([ONE, 0]));
        assert!(lean([-ONE, 0]) < -(ONE as i64), "{}", lean([-ONE, 0]));
    }

    #[test]
    fn a_pine_crown_is_round_clumps_not_stacked_disks() {
        let t = the_tree(Kind::Pine);
        let all = pine_clumps(&t, [ONE, 0], 4);
        assert!(all.len() >= 10);
        for (_, radii, _) in &all {
            assert!(radii.y * 2 > radii.x, "{radii:?}");
        }
        // no two sit level: tiers would put clumps on one height
        let mut heights: Vec<Fx> = all.iter().map(|(at, _, _)| at.y).collect();
        heights.sort();
        assert!(heights.windows(2).all(|w| w[1] > w[0]), "{heights:?}");
    }

    #[test]
    fn a_pine_trunk_is_stout_at_the_foot() {
        let t = the_tree(Kind::Pine);
        let (vs, _) = tree(&t, [ONE, 0], 4).finish();
        let foot = vs.iter().filter(|v| v.pos.y < 0).map(|v| v.pos.x.abs()).max();
        assert!(foot.unwrap() > mul(t.trunk, cm(60)), "{foot:?}");
    }

    #[test]
    fn a_cairns_core_rises_to_each_step_height() {
        let map = Map::generate(2);
        let c = &map.cairns[0];
        let (vs, _) = cairn_core(c).finish();
        let (_, hi) = bounds(&vs);
        let top = c.tiers[c.tiers.len() - 1].top;
        assert!((hi.y - top).abs() < cm(10), "{} {}", hi.y, top);
    }

    #[test]
    fn a_dug_mound_stays_low_and_a_fresh_one_stands_taller() {
        let top = |s: Shape| bounds(&s.finish().0).1.y;
        assert!(top(mound_fresh()) > cm(25));
        assert!(top(mound_dug()) < top(mound_fresh()));
    }

    #[test]
    fn a_mouths_throat_goes_dark() {
        let (vs, _) = mouth_bank().finish();
        let darkest = vs
            .iter()
            .map(|v| v.color[0] + v.color[1] + v.color[2])
            .min()
            .unwrap();
        assert!(darkest < cm(10), "{darkest}");
    }

    #[test]
    fn the_arrow_points_down_with_its_tip_at_the_origin() {
        let (vs, _) = arrow().finish();
        let (lo, hi) = bounds(&vs);
        assert_eq!(lo.y, 0);
        assert_eq!(hi.y, ONE);
        // wider at the head than the shaft's top
        let at = |y: Fx| {
            vs.iter()
                .filter(|v| (v.pos.y - y).abs() < cm(4))
                .map(|v| v.pos.x)
                .max()
        };
        assert!(at(cm(40)).unwrap() > at(cm(100)).unwrap());
    }

    #[test]
    fn a_gate_leaf_is_as_long_as_asked() {
        let (vs, _) = gate_leaf(cm(350)).finish();
        let (lo, hi) = bounds(&vs);
        assert!(
            lo.x <= cm(4) && (hi.x - cm(350)).abs() <= cm(6),
            "{} {}",
            lo.x,
            hi.x
        );
    }
}
